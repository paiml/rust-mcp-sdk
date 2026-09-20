---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 03
type: execute
wave: 2
depends_on: ["125-01"]
files_modified:
  - src/server/skills.rs
  - tests/skills_integration.rs
autonomous: true
requirements: [D-02, D-05]
user_setup: []

estimate:
  tokens: 45000
  raw_tokens: 90000
  tasks: 3
  confidence: high

must_haves:
  truths:
    - "A skill's entry `resources` manifest lists the skill's own SKILL.md URI first followed by every registered reference URI — the manifest is complete, and every digest covers exactly the bytes `resources/read` returns for that URI (D-05)."
    - "Every emitted digest matches `^sha256:[0-9a-f]{64}$` and every emitted `size` equals the byte length of the served content, for arbitrary generated skills (property, not example)."
    - "Emitted `frontmatter` is verbatim: a nested `metadata:` object, a list-valued field and a non-required scalar field all survive the YAML-to-JSON round trip with their authored shapes, and the emitted object is never reconstructed from `resolved_description()` (which `with_description` can legitimately override)."
    - "LF-authored and CRLF-authored frontmatter blocks produce identical `frontmatter` JSON — the existing CRLF lock is preserved."
    - "A skill whose body carries no frontmatter block is EXCLUDED from `skills/list` entries, is still readable byte-identically via `resources/read`, and produces a build-time warning naming it — never a silently synthesized `{name, description}` (D-02)."
    - "`Skills` build-time validation REJECTS a skill whose final URI segment does not equal its frontmatter `name`, and only when a frontmatter `name` is present (ROADMAP SC#3, gap #4c)."
    - "A skill exceeding 512 resource entries or 16,777,216 total bytes produces a build-time warning; it is not rejected (ROADMAP SC#3, gap #5)."
    - "All 40+ existing frontmatter-less `Skill::new(...)` call sites — including the in-module proptest strategies and the duplicate-URI tests — keep compiling and keep passing (RESEARCH Pitfall 3)."
  artifacts:
    - src/server/skills.rs
    - tests/skills_integration.rs
  key_links:
    - "`Skills::entries()` -> `parse_frontmatter_value` -> `serde_yaml`: the single YAML seam. A second parse path anywhere would make the emitted frontmatter and the served SKILL.md capable of disagreeing, which the draft makes a hard host-side load failure."
    - "`SkillResourceRef.digest` / `.size` -> the exact bytes `SkillsHandler::read` returns for the same URI. Computing them from anything else makes the manifest and the served content disagree by construction."
    - "`Skills::entries()` and `Skills::into_handler()` must share one validation function, or a registry can produce entries it refuses to serve (or vice versa)."
---

<objective>
Make every entry the tracer proved on the wire actually complete and actually
validated: full `resources` manifests with per-file digests, verbatim frontmatter
including nested and list-valued fields, the D-02 warn-and-exclude path for
frontmatter-less skills, frontmatter name-identity rejection, and the SEP limits
warning.

Purpose: the tracer emitted a one-element manifest for a single skill. A conforming
host fetches every file in the manifest and compares the entry field-by-field
against what it reads; an incomplete manifest or a reconstructed frontmatter is a
guaranteed host-side rejection, not a graceful degradation.

Output: `Skills::entries()` produces entries a conforming host will accept, and
refuses to produce ones it would not.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-CONTEXT.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-RESEARCH.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-PATTERNS.md
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md
</context>

## Artifacts this phase produces

Created in **this plan** (125-03). See 125-01-PLAN.md for the phase-wide table.

| Symbol / artifact | Kind | Location | Visibility |
|---|---|---|---|
| `SkillDiagnostic` | enum: frontmatter-missing / limit-exceeded / name-mismatch | `src/server/skills.rs` | `pub(crate)` |
| `Skills::entries_with_diagnostics` | `pub(crate) fn (&self) -> Result<(Vec<SkillEntry>, Vec<SkillDiagnostic>)>` | `src/server/skills.rs` | `pub(crate)` |
| `Skills::validate_names` | `pub(crate) fn (&self) -> Result<()>` — the shared gap-#4c check | `src/server/skills.rs` | `pub(crate)` |
| `MAX_SKILL_RESOURCES` = 512, `MAX_SKILL_TOTAL_BYTES` = 16_777_216 | consts | `src/server/skills.rs` | private |

## Plan-time finding that supersedes RESEARCH Pitfall 3

**RESEARCH Pitfall 3 states that gap 4a — "final URI segment must equal
`Skill::name()`", checked unconditionally — "breaks nothing existing". That is
measurably false.** `grep -rn 'with_path(' src tests examples` at plan time found
six in-`src` call sites, of which **three would fail** under an unconditional 4a
reject:

- `src/server/skills.rs:846-853` — `Skill::new("a", "").with_path("p")` and
  `Skill::new("b", "").with_path("p")`, registered deliberately to prove
  duplicate-URI detection. Both violate 4a, so the test would receive a
  name-identity error instead of the duplicate-URI error it asserts.
- `src/server/skills.rs:864-871` — `Skill::new("b", "").with_path("a")` in the
  cross-skill reference-collision test. Final segment `a` != name `b`.
- `src/server/skills.rs:1144-1152` — `skills_strategy_with_refs`, which rewrites
  every generated skill's path to `p{i}` while keeping its generated name.
  Essentially **every** proptest case violates 4a.

Beyond the repo, `pmcp-book/src/ch12-8-skills.md:392` teaches
`.with_path("team/topic")` as an exercise on a differently-named skill, so 4a as a
reject is a breaking change to documented, taught usage.

**Resolution, and why it is not a scope reduction:** ROADMAP success criterion #3
scopes the reject precisely — "rejects a skill whose final URI segment does not
equal **its frontmatter `name`**". That is gap **4c**, which is conditional on
frontmatter being present, and none of the three breaking sites has frontmatter.
4c therefore ships as a hard reject, in full, exactly as the criterion states.
Gap 4a ships too — as a `tracing::warn!` with its own test — because no CONTEXT
decision authorizes turning it into a reject, SC#3 does not ask for it, and the
measurement above shows the reject would break unrelated existing behavior.
Promoting 4a to a reject belongs with the already-recorded "strict frontmatter
mode" deferral in `125-CONTEXT.md` `## Deferred Ideas`.

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Complete resources manifests and verbatim frontmatter</name>

  <files>src/server/skills.rs, tests/skills_integration.rs</files>

  <read_first>
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md — the exact shapes of `SkillEntry`, `SkillResourceRef`, `parse_frontmatter_value` and `sha256_digest_hex` as the tracer landed them.
    - src/server/skills.rs — `Skill` struct and accessors (:155-320), `reference_uri` (:288-300), `resolved_description` (:276) and `with_description` (:184-200) whose override semantics forbid reconstructing frontmatter from it, `Skills::into_handler` (:437-471) and its `IndexMap` ordering contract, `parse_frontmatter_description` (:656-680) for the BOM/CRLF handling to preserve, and `SkillsHandler::read` (:545-580) which defines the exact bytes each URI serves.
    - src/server/skills.rs:1116-1160 — the existing `skill_strategy` and `skills_strategy_with_refs` proptest strategies; the new digest property test should reuse them rather than minting a third generator.
    - tests/skills_integration.rs:41-70 — `build_widget_skill_lf` and `build_widget_skill_crlf`, the LF/CRLF fixture pair that locks the existing CRLF behavior (RESEARCH A4).
    - 125-RESEARCH.md `### Pattern 2: Entry synthesis at into_handler()` — the digest/size definition and assumption A6 (the digest covers the same bytes `resources/read` returns).
    - 125-RESEARCH.md `## Code Examples` — the SEP wire shape showing `resources` carrying the skill's own SKILL.md entry first, then supporting files.
  </read_first>

  <behavior>
    - A skill with two references produces an entry whose `resources` has exactly 3 elements: the SKILL.md URI first, then the two reference URIs in registration order.
    - Each `resources[i].digest` equals `"sha256:" + lowercase_hex(sha256(bytes_served_for_that_uri))` and each `resources[i].size` equals that byte slice's length.
    - A frontmatter block carrying `name`, `description`, a scalar `license`, a list-valued field and a nested `metadata:` mapping produces a `frontmatter` JSON object with all five keys, the list as a JSON array and the nested mapping as a JSON object.
    - A skill constructed with `Skill::new("x", body_whose_frontmatter_description_is_A).with_description("B")` emits `frontmatter.description == "A"` — the authored value, not the override.
    - The LF fixture and its CRLF twin produce `frontmatter` JSON that compares equal after only the skill-name difference is accounted for.
    - Entry order across a multi-skill registry equals registration order (the `IndexMap` contract), with no response-time sort.
  </behavior>

  <action>
Expand `Skills::entries()` from the tracer's one-element manifest to the complete,
conforming manifest.

1. `src/server/skills.rs`: for each skill, build `resources` as the skill's own
   `skill_md_uri()` first with digest and size over `skill.body().as_bytes()`,
   followed by one `SkillResourceRef` per registered reference in registration
   order, each keyed on `reference_uri(relative_path)` with digest and size over
   that reference's `body().as_bytes()`. These are exactly the byte slices
   `SkillsHandler::read` returns for the same URIs, so the manifest and the served
   content cannot disagree by construction — state that invariant in the rustdoc.
   Preserve `IndexMap` insertion order for entries; do not sort at response time,
   which the module header already documents as the deterministic-ordering
   contract.

2. Emit `frontmatter` from `parse_frontmatter_value` only. Never reconstruct it
   from `Skill::name()` or `Skill::resolved_description()`: `with_description` is
   an explicit override, so `resolved_description()` can legitimately differ from
   the SKILL.md's authored `description:` line, and the draft requires the emitted
   object to be identical to the file's. Add the reasoning as rustdoc on the
   emission site.

3. Add unit tests in the in-module test block covering every `<behavior>` row —
   name them so a `--lib skills` filter selects them. Add the multi-field verbatim
   test with a fixture carrying `name`, `description`, `license`, a list-valued
   field and a nested `metadata:` mapping. Add the LF/CRLF frontmatter equality
   test.

4. Add a proptest over the existing `skills_strategy_with_refs` asserting, for every
   generated registry that yields entries: every `digest` matches
   `^sha256:[0-9a-f]{64}$`, every `size` equals the length of the bytes the
   corresponding `resources/read` returns for that URI, and the manifest length
   equals `1 + references().count()`. Assert against the handler's actual read
   output rather than re-deriving the bytes in the test, so the property cannot pass
   by both sides making the same mistake.

5. `tests/skills_integration.rs`: add an integration-level assertion that the
   manifest for `build_widget_skill_lf` names all three URIs and that reading each
   one through the handler returns content whose length equals the manifest's
   `size` for it. Keep every existing test in this file passing unchanged.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or the summary line reads "0 passed" / "running 0 tests"</fails_when>
    <automated>cargo test --all-features --test skills_integration -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or fewer than 11 tests passed (the file's existing 9 tests + 2 proptests must all still run)</fails_when>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit — the 125-01 wire assertions on digest shape and size must still hold with the expanded manifest</fails_when>
  </verify>

  <acceptance_criteria>
    - A `--lib skills` run reports a test asserting a 3-element manifest for a 2-reference skill, with the SKILL.md URI at index 0.
    - A proptest exists asserting the digest regex and the size-equals-served-bytes property, and it reads the served bytes back through the `ResourceHandler` rather than recomputing them from the `Skill`.
    - A test asserts `frontmatter.description` equals the authored frontmatter value on a skill that also called `with_description` with a different value.
    - A test asserts the LF and CRLF fixtures produce equal `frontmatter` JSON for every key other than the deliberately-differing name.
    - `grep -c 'resolved_description' src/server/skills.rs` shows no new occurrence inside the entry-synthesis function: `sed -n '/fn entries_with_diagnostics/,/^    }/p' src/server/skills.rs | grep -c resolved_description` returns 0.
    - `cargo test --all-features --test skills_integration -- --test-threads=1` exits 0 with every pre-existing test still present in the reported names.
  </acceptance_criteria>

  <done>
Every entry carries a complete `resources` manifest whose digests and sizes are
provably the served bytes, and verbatim frontmatter including nested and
list-valued fields. Committed.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Warn and exclude frontmatter-less skills (D-02)</name>

  <files>src/server/skills.rs, tests/skills_integration.rs</files>

  <read_first>
    - src/server/skills.rs — the entry-synthesis function as Task 1 left it, plus the in-module test block's `Skill::new("x", "body")` constructions at :216, :244, :248, :306 and the unit tests using `Skill::new("a", "")`, `Skill::new("foo", "body")`, `Skill::new("zeta", "")`.
    - src/server/skills.rs:1116-1140 — `skill_strategy`, which generates `name` from `[a-z]{1,8}` and `body` from `[a-zA-Z]{0,20}`: arbitrary bodies that will essentially never contain valid frontmatter, and therefore the natural coverage for this path.
    - tests/skills_integration.rs:319-350 — the `Skill::new("propskill", body)` proptest, same situation.
    - src/server/core.rs:2217 and :1134 — the in-repo `tracing::warn!` idiom to follow.
    - 125-CONTEXT.md D-02 and 125-RESEARCH.md Pitfall 4 — why synthesizing `{name, description}` is a guaranteed host-side rejection rather than a graceful default (SEP §Integrity: the host MUST NOT load the skill on a field mismatch).
    - Makefile:1799-1802 — `check-todos` greps `src/` for TODO/FIXME/HACK/XXX. Deferrals go in rustdoc prose, never as a marker comment.
  </read_first>

  <behavior>
    - `Skills::entries()` on a registry of one frontmatter-bearing and one frontmatter-less skill returns exactly one entry, for the frontmatter-bearing skill.
    - The excluded skill's SKILL.md is still returned byte-identically by `resources/read` on its URI.
    - The excluded skill still appears in `resources/list`.
    - The exclusion produces a `SkillDiagnostic` naming the excluded skill's SKILL.md URI, and `Skills::entries()` emits a `tracing::warn!` carrying that URI.
    - A registry of only frontmatter-less skills yields an empty entry vector and no error — an empty listing is SEP-legal ("MAY return an empty or partial listing").
    - No entry is ever emitted whose `frontmatter` was synthesized from `Skill::name()` or `resolved_description()`.
  </behavior>

  <action>
Implement the D-02 warn-and-exclude semantics with an observable diagnostic, so the
behavior is unit-testable without installing a tracing subscriber.

1. `src/server/skills.rs`: add a `pub(crate) enum SkillDiagnostic` with a variant
   for a frontmatter-less skill carrying its SKILL.md URI. Add
   `pub(crate) fn entries_with_diagnostics(&self) -> Result<(Vec<SkillEntry>, Vec<SkillDiagnostic>)>`
   holding the real logic, and make the public `pub fn entries(&self)` a thin
   wrapper that calls it, emits one `tracing::warn!` per diagnostic naming the URI
   and the reason, and returns only the entries. Keeping the diagnostics
   crate-private adds no public API surface while making every warn path directly
   assertable from the in-module test block.

2. A skill whose `parse_frontmatter_value` returns `None` is skipped: no entry, one
   diagnostic. Do NOT hard-error — 40+ existing `Skill::new(...)` call sites, the
   in-module doctests, and both proptest strategies construct frontmatter-less
   skills, and D-02 chose partial listing over breaking them. Do NOT synthesize a
   `{name, description}` object: the draft makes a field-by-field mismatch a
   mandatory host-side refusal, so a synthesized entry ships a server that looks
   conformant and is unusable.

3. Record the strict-mode option in the `entries()` rustdoc as prose: a future
   fallible or strict variant may reject frontmatter-less skills once canonical
   surfaces are cleaned up. Write it as documentation, not as a TODO/FIXME/HACK/XXX
   marker — `make check-todos` greps `src/` for exactly those tokens and CLAUDE.md
   forbids self-admitted technical debt in code.

4. Add in-module unit tests for every `<behavior>` row, driving
   `entries_with_diagnostics` directly for the diagnostic assertions and
   `into_handler()` plus `read`/`list` for the still-served assertions.

5. `tests/skills_integration.rs`: add an integration test registering one
   frontmatter-bearing and one frontmatter-less skill, asserting `resources/list`
   still enumerates both SKILL.md URIs and `resources/read` on the excluded one
   returns its body byte-identically — the exclusion is from the skills listing
   only, never from the resource surface.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" / "running 0 tests" in the summary line</fails_when>
    <automated>cargo test --all-features --test skills_integration -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or any pre-existing test name missing from the reported set</fails_when>
    <automated>make check-todos</automated>
    <fails_when>non-zero exit, or the output contains "Found technical debt comments"</fails_when>
  </verify>

  <acceptance_criteria>
    - A `--lib skills` test asserts a mixed registry yields exactly 1 entry and exactly 1 diagnostic.
    - A `--lib skills` test asserts a registry of only frontmatter-less skills yields 0 entries and `Ok`, not `Err`.
    - An integration test asserts the excluded skill is still in `resources/list` and still readable byte-identically.
    - `grep -c 'tracing::warn!' src/server/skills.rs` returns at least 1.
    - `make check-todos` exits 0 — no SATD marker was introduced for the strict-mode deferral.
    - Every pre-existing test in `src/server/skills.rs` and `tests/skills_integration.rs` still passes without edits to its assertions.
  </acceptance_criteria>

  <done>
A frontmatter-less skill is excluded from `skills/list` with a named warning, stays
fully readable through `resources/read`, and no entry anywhere carries a synthesized
frontmatter object. Committed.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 3: Frontmatter name-identity reject and the SEP limits warning</name>

  <files>src/server/skills.rs</files>

  <read_first>
    - src/server/skills.rs:321-357 — `validate_reference_path`, the in-file validation-fn shape to copy: one `if` per rule, each returning `Error::validation` with the offending value interpolated.
    - src/server/skills.rs:437-471 — `Skills::into_handler`, which collects ALL duplicates before erroring rather than failing on the first. Match that aggregation style.
    - src/server/skills.rs:280-292 — `resolved_path` and `skill_md_uri`, which define what "final URI segment" means.
    - src/server/skills.rs:846-871 and :1144-1152 — the three call sites the "Plan-time finding" section above measured as breaking under an unconditional gap-4a reject. Read them before writing any name rule.
    - .planning/ROADMAP.md, the `### Phase 125:` success-criteria list, item 3 — the reject is scoped to the FRONTMATTER name.
    - 125-RESEARCH.md `## Code Examples` §Limits — 512 resource entries counted with SKILL.md included, and 16,777,216 bytes summed over the manifest's `size` values.
  </read_first>

  <behavior>
    - A skill whose body frontmatter carries `name: refunds` but whose resolved path's final segment is `billing` causes `Skills::entries()` and `Skills::into_handler()` to both return `Err(Error::Validation)` whose message names both the URI and the frontmatter name.
    - A skill with no frontmatter, or with frontmatter carrying no `name` key, is never rejected by the name rule regardless of its path.
    - Multiple mismatching skills in one registry produce a single error listing every offender, not just the first.
    - A skill whose resolved path's final segment differs from `Skill::name()` but whose frontmatter name matches (or is absent) is accepted, and produces a diagnostic and a `tracing::warn!` rather than an error.
    - A skill with 513 total resource entries produces a limit diagnostic naming its URI and the count; it is still emitted as an entry.
    - A skill whose manifest sizes sum above 16,777,216 produces a limit diagnostic naming its URI and the byte total; it is still emitted as an entry.
    - A skill with exactly 512 entries and exactly 16,777,216 bytes produces no limit diagnostic — the bounds are inclusive.
  </behavior>

  <action>
Add the two validation rules at the build-time choke point, with the reject scoped
exactly as ROADMAP success criterion 3 states.

1. `src/server/skills.rs`: add `pub(crate) fn validate_names(&self) -> Result<()>`
   implementing gap 4c only — for each skill whose `parse_frontmatter_value` yields
   an object with a string `name` key, compare that value against the final `/`
   segment of `resolved_path()`; on mismatch, record the offender. Aggregate all
   offenders and return one `Error::validation` naming every URI and its expected
   and actual name, matching `into_handler`'s existing collect-then-error style and
   `validate_reference_path`'s interpolation style. Call `validate_names` from BOTH
   `entries_with_diagnostics` and `into_handler`, so a registry can never produce
   entries it would refuse to serve.

2. Implement gap 4a as a DIAGNOSTIC, not a reject: when the final segment of
   `resolved_path()` differs from `Skill::name()`, add a name-mismatch
   `SkillDiagnostic` variant carrying both values, warned by `entries()` alongside
   the other diagnostics. The reject form is out of scope: ROADMAP criterion 3
   scopes the reject to the frontmatter name, three in-repo constructions
   (`src/server/skills.rs:846-853`, `:864-871`, `:1144-1152`) deliberately use a
   path whose final segment differs from the constructor name for reasons unrelated
   to naming, and `pmcp-book/src/ch12-8-skills.md:392` teaches that construction.
   State that scoping in the function's rustdoc as prose, and name the
   already-recorded strict-mode deferral as where promotion belongs — not as a
   TODO/FIXME/HACK/XXX marker.

3. Add private consts `MAX_SKILL_RESOURCES` = 512 and `MAX_SKILL_TOTAL_BYTES` =
   16_777_216 with a rustdoc citation to the SEP Limits section, and a
   limit-exceeded `SkillDiagnostic` variant. Count `resources` entries with the
   SKILL.md included and sum the manifest's `size` values. Exceeding either bound is
   a warning, never a rejection — ROADMAP criterion 3 says "warns". Both bounds are
   inclusive: 512 and 16,777,216 exactly are within limits.

4. Add in-module unit tests for every `<behavior>` row. For the limit tests,
   construct the oversized skill programmatically rather than embedding large
   literals, and keep the byte-total test under a size that would slow the suite —
   a handful of references whose bodies sum just past the bound is sufficient, since
   the guard sums declared sizes.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" / "running 0 tests" in the summary line</fails_when>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1 2>&amp;1 | grep -c 'test result: ok'</automated>
    <fails_when>output is 0 — no "test result: ok" line means the filtered run produced no passing summary at all</fails_when>
    <automated>cargo test --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or any line beginning with "failures:" in the output — this is the full CI-matching run and must stay clean after the new rejects land</fails_when>
  </verify>

  <acceptance_criteria>
    - A test asserts a frontmatter `name: refunds` on a skill resolving to a path ending `billing` yields `Err(Error::Validation)` from BOTH `entries()` and `into_handler()`.
    - A test asserts a frontmatter-less skill with any path is accepted by the name rule.
    - A test asserts two mismatching skills produce one error message naming both.
    - A test asserts the 512-entry and 16,777,216-byte bounds are inclusive (no diagnostic at exactly the bound, a diagnostic one past it).
    - `src/server/skills.rs:846-853`, `:864-871` and the `skills_strategy_with_refs` proptest all still pass with their assertions unedited — confirm by running the full `--lib skills` filter and checking `test_1_8`, `test_1_8a` and `prop_1_17_no_reference_ever_listed` appear in the passing set.
    - `make check-todos` exits 0.
    - `cargo test --all-features -- --test-threads=1` exits 0.
  </acceptance_criteria>

  <reversibility rating="costly">
    Making frontmatter name-identity a hard `Err` changes `Skills::into_handler`'s
    accept/reject behavior for any downstream registry whose frontmatter name
    disagrees with its URI. It is required by ROADMAP success criterion 3 and is
    conditional on frontmatter being present, so no frontmatter-less construction is
    affected. Reversal is a one-function change; it is flagged, not gated.
  </reversibility>

  <done>
A frontmatter name that disagrees with its URI's final segment is rejected at build
time with an aggregated message; a constructor-name mismatch and an over-limit skill
each warn; every pre-existing skills test still passes unedited. Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Server author's SKILL.md bytes -> `parse_frontmatter_value` | Arbitrary text reaching a YAML parser at build time. Not attacker-controlled in the usual sense, but arbitrary — a registry may be assembled from files on disk. |
| Registry size -> response size | The number and total byte size of skills determines the `skills/list` response size. |
| Emitted `frontmatter` -> every `skills/list` caller | Author-supplied fields cross to the wire verbatim. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-11 | Denial of service | `parse_frontmatter_value` on arbitrary bytes | medium | mitigate | The parse returns `Option` and never panics or unwraps on malformed input; a non-object or unparseable block yields `None`, taking the D-02 exclusion path. Asserted by the proptest over `skill_strategy`'s arbitrary bodies, and by the dedicated fuzz target 125-05 registers. |
| T-125-12 | Denial of service | Unbounded registry inflating `skills/list` | medium | mitigate | The SEP 512-entry / 16 MiB per-skill guard warns at build time so an operator sees the growth before a host does. Catalog-level pagination stays a recorded deferral (D-11) rather than an unstated gap. |
| T-125-13 | Information disclosure | Verbatim frontmatter carrying author secrets | medium | accept | Verbatim emission is required by the draft; mitigated by the rustdoc warning added in 125-01 and reinforced here on the nested-field path, where a `metadata:` block is the likeliest place a secret hides. |
| T-125-14 | Tampering | Manifest disagreeing with served bytes | high | mitigate | Digest and size are computed from the same `&str` bodies `SkillsHandler::read` returns for the same URIs, and the proptest reads the bytes back through the handler rather than recomputing them, so a divergence cannot pass by both sides making the same mistake. |
| T-125-15 | Spoofing | Digest read as an integrity guarantee | low | accept | SEP line 267: digests are unsigned and supplied by the same server that supplies the content; hosts MUST NOT treat a match as a security boundary. Documented in rustdoc; pmcp makes no integrity claim. |
</threat_model>

<verification>
- `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` exits 0 with a nonzero passed count.
- `cargo test --all-features -- --test-threads=1` exits 0 (matches CI).
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0, including the cognitive-complexity budget: `entries_with_diagnostics` must stay under 25, which is why validation and diagnostics are separate functions rather than one nested loop.
- `make check-todos` exits 0.
</verification>

<success_criteria>
- Entries carry complete manifests whose digests and sizes are the served bytes.
- Frontmatter is verbatim, including nested and list-valued fields, on LF and CRLF.
- Frontmatter-less skills are excluded with a named warning and never synthesized.
- Frontmatter name-identity is rejected; constructor-name mismatch and SEP limit overruns warn.
- Every pre-existing skills test passes with unedited assertions.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-03-SUMMARY.md` when done.
</output>
