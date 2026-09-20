---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 04
type: execute
wave: 3
depends_on: ["125-01", "125-02", "125-03"]
files_modified:
  - src/server/skills.rs
  - src/server/builder.rs
  - tests/skills_integration.rs
  - examples/s44_server_skills.rs
  - examples/c10_client_skills.rs
  - pmcp-book/src/ch12-8-skills.md
  - pmcp-course/src/part8-advanced/ch23-skills.md
  - pmcp-course/src/part8-advanced/ch23-exercises.md
  - pmcp-course/src/quizzes/ch23-skills.toml
autonomous: true
requirements: [D-03, D-08]
user_setup: []

estimate:
  tokens: 40000
  raw_tokens: 80000
  tasks: 3
  confidence: high

must_haves:
  truths:
    - "`resources/list` on a skills-backed handler returns exactly one entry per registered SKILL.md and nothing else — no synthesized discovery-index entry, and still no supporting-file entries (D-08, ROADMAP SC#2 and SC#4)."
    - "`resources/read` on the retired discovery-index URI returns the handler's ordinary unknown-URI error rather than a synthesized JSON document (D-08)."
    - "SKILL.md and every supporting file remain byte-identical through `resources/read`, and the dual-surface prompt-fallback byte-equality still holds on both LF and CRLF fixtures (ROADMAP SC#4)."
    - "`cargo run --example s44_server_skills` and `cargo run --example c10_client_skills` both exit 0, with c10 demonstrating `skills/list` in place of the retired index read (D-03, D-08, ROADMAP SC#4)."
    - "Every user-facing snippet — book chapter, course chapter, course exercises, course quiz — describes the method-based discovery surface, and every snippet skill body carries real frontmatter so it produces a conforming entry (D-03)."
    - "The `src/server/skills.rs` module doctest and the closing doctest of `pmcp-book/src/ch12-8-skills.md` remain byte-equal mirrors of each other after both are updated (the rule stated at `src/server/skills.rs:19`)."
  artifacts:
    - src/server/skills.rs
    - src/server/builder.rs
    - tests/skills_integration.rs
    - examples/s44_server_skills.rs
    - examples/c10_client_skills.rs
    - pmcp-book/src/ch12-8-skills.md
    - pmcp-course/src/part8-advanced/ch23-skills.md
    - pmcp-course/src/part8-advanced/ch23-exercises.md
    - pmcp-course/src/quizzes/ch23-skills.toml
  key_links:
    - "`SkillsHandler::new` list-push -> `resources/list` output -> 12 tracked assertion sites across 4 Rust files. Removing the push without updating all of them turns a correct change into a red suite."
    - "`src/server/skills.rs:19` -> `pmcp-book/src/ch12-8-skills.md` closing doctest: a byte-equality rule between a source file and a book chapter. They move together or the stated rule becomes false."
    - "`examples/c10_client_skills.rs:109-114` ASSERTS on the index read — unlike s44, which only prints. c10 panics rather than printing wrong output, so it is the load-bearing example edit."
---

<assumption_delta_decision>
**Primary noun:** skill discovery.
**Decision:** `promote`.
**Rationale:** Discovery moves from a singular synthesized index resource
(`skill://index.json`, agentskills.io discovery schema 0.2.0) to the method-based
`skills/list` the SEP-2640 working group chose. The method becomes the primary and
only discovery surface; the index resource is removed rather than kept alongside,
because it also violates the draft's URI structure rule (the index name is not a
skill name, and its `/SKILL.md` sibling does not exist). D-08 authorizes removal by
default, with a legacy gate only if a plan-time blast-radius check showed a consumer
needing it — the check was performed (12 tracked in-repo sites, all owned by this
repo, no external consumer known) and no legacy gate is required.
</assumption_delta_decision>

<objective>
Retire `skill://index.json` and bring every user-facing surface onto the method-based
discovery story, with real frontmatter everywhere a snippet registers a skill.

Purpose: with `skills/list` answering, the synthesized index is both redundant and
nonstandard. Leaving it served means a pmcp server advertises two discovery
surfaces, one of which the draft does not define — and one of which the draft's URI
rules forbid. The cleanup is not cosmetic: `examples/c10_client_skills.rs` ASSERTS on
the index read and will panic, and four documentation surfaces teach the retired
shape.

Output: one discovery surface, twelve updated assertion sites, two runnable examples,
and four documentation surfaces that describe what the code now does.
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
@.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-03-SUMMARY.md
</context>

## Artifacts this phase produces

This plan **removes** rather than creates. See 125-01-PLAN.md for the phase-wide
creation table.

| Symbol / artifact | Kind | Location | Action |
|---|---|---|---|
| `SKILL_INDEX_URI` | const | `src/server/skills.rs:58` | removed |
| `INDEX_JSON_MIME` | const | `src/server/skills.rs:60` | removed if it has no other consumer |
| `build_discovery_index_json` | private fn | `src/server/skills.rs:514-530` | removed |
| `SkillsHandler.index_json` | struct field | `src/server/skills.rs` | removed |
| the `list_resources.push(...)` index entry | statement | `src/server/skills.rs:499-503` | removed |
| the `if uri == SKILL_INDEX_URI` short-circuit | statement | `src/server/skills.rs:549` | removed |

## The measured blast radius — 12 tracked sites

Enumerated at plan time with `grep -rn 'skill://index.json\|SKILL_INDEX_URI\|build_discovery_index_json'`
over tracked source. **This list corrects 125-RESEARCH.md Pattern 3 in two
directions**, so use this table, not that one.

| File | Lines | Kind |
|---|---|---|
| `src/server/skills.rs` | 58, 60, 500, 504, 514, 549 | implementation |
| `src/server/skills.rs` | 804, 826, 913, 979, 987, 1111, 1240, 1392 | 7 unit-test assertions + 1 proptest URI list |
| `src/server/builder.rs` | 2302 | **1 unit-test assertion — ABSENT from RESEARCH Pattern 3** |
| `tests/skills_integration.rs` | 182, 228, 232, 366 | 3 tests, one of them a proptest |
| `examples/s44_server_skills.rs` | 19, 99 | doc header + `println!` |
| `examples/c10_client_skills.rs` | 109, 113 | a `read` call and **two `assert_eq!` that will PANIC** |
| `pmcp-book/src/ch12-8-skills.md` | 124, 181, 266, 384 | prose + snippet |
| `pmcp-course/src/part8-advanced/ch23-skills.md` | 171, 220 | prose |
| `pmcp-course/src/part8-advanced/ch23-exercises.md` | 35, 40, 101 | exercise instructions |
| `pmcp-course/src/quizzes/ch23-skills.toml` | 40 | **quiz answer text — ABSENT from RESEARCH Pattern 3** |

**Do NOT edit `pmcp-book/book/**` or `pmcp-course/book/**`.** They match the grep but
are untracked mdBook build output — `git ls-files` returns nothing for them. They
regenerate from `src/`.

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Retire the synthesized discovery index and update all 12 Rust assertion sites</name>

  <files>src/server/skills.rs, src/server/builder.rs, tests/skills_integration.rs</files>

  <read_first>
    - src/server/skills.rs:54-62 — the constant block holding the index URI and the two MIME constants; check whether `INDEX_JSON_MIME` has any consumer besides the index before removing it.
    - src/server/skills.rs:490-532 — `SkillsHandler::new` (the `list_resources.push` of the index entry and the `index_json` field assignment) and `build_discovery_index_json`.
    - src/server/skills.rs:534-580 — `SkillsHandler::list` and `read`, including the `if uri == SKILL_INDEX_URI` short-circuit at :549 and the unknown-URI error path at :556-575, which must remain the response for the retired URI.
    - src/server/skills.rs:800-830, 905-995, 1105-1115, 1235-1245, 1385-1395 — every unit-test assertion that names the index, including the length assertions (`resources.len()`), the positional assertions (`resources[2]`, `resources[3]`, `resources[1]`) and the proptest URI list at :1240.
    - src/server/builder.rs:2295-2310 — the `assert!(uris.contains(&"skill://index.json"))` assertion RESEARCH Pattern 3 omitted.
    - tests/skills_integration.rs:165-190 — `resources_list_returns_skill_md_and_index_only`, whose name and its `assert_eq!(result.resources.len(), 3, "2 SKILL.md + 1 index = 3")` both encode the retired shape.
    - tests/skills_integration.rs:220-240 — `resources_read_index_returns_resource_with_text_application_json`, which becomes meaningless and must be replaced rather than deleted silently.
    - tests/skills_integration.rs:345-380 — the proptest whose URI loop reads the index URI.
    - 125-CONTEXT.md D-08 — removal by default; a legacy gate only if a blast-radius check showed a consumer needing it. The check is in this plan's blast-radius table; no consumer needs it.
  </read_first>

  <behavior>
    - `resources/list` on a handler built from 2 skills returns exactly 2 resources, both SKILL.md URIs, in registration order.
    - `resources/list` still contains no supporting-file URI, for any registry (the existing "readable but not listable" property is unchanged).
    - `resources/read` on the retired discovery-index URI returns the handler's ordinary unknown-URI error, the same one any other unregistered URI gets.
    - `resources/read` on every registered SKILL.md and every supporting file returns byte-identical content to before this change.
    - The dual-surface byte-equality property still holds for both the LF and the CRLF fixtures.
  </behavior>

  <action>
Remove the synthesized discovery surface and re-point every assertion that measured
it.

1. `src/server/skills.rs`: delete the discovery-index URI constant, the
   `build_discovery_index_json` function, the `index_json` field on `SkillsHandler`,
   the `list_resources.push(...)` statement that appended the index entry in
   `SkillsHandler::new`, and the `read` short-circuit that served it. Delete
   `INDEX_JSON_MIME` too, unless a grep shows another consumer. After removal, a
   read of the retired URI must fall through to the existing unknown-URI error path
   unchanged — do NOT add a special case for it, and do NOT change that path's error
   code (the -32601 divergence is D-06's recorded out-of-scope observation).

2. `src/server/skills.rs` unit tests: update the eight assertion sites. Length
   assertions drop by one; positional assertions shift down by one index; the read
   test for the index becomes a test asserting the retired URI now yields the
   unknown-URI error; the proptest URI seed list at :1240 drops the index entry.
   Rename any test whose NAME encodes the retired shape so the name still describes
   what it asserts.

3. `src/server/builder.rs`: update the assertion at :2302 to assert the index URI is
   ABSENT from the built server's URI set, rather than present. Asserting absence
   rather than deleting the assertion keeps a regression detectable.

4. `tests/skills_integration.rs`: rename
   `resources_list_returns_skill_md_and_index_only` to describe SKILL.md-only
   listing, change its length assertion from 3 to 2 and update its message string,
   and add an explicit assertion that the retired index URI is absent from the list.
   Replace `resources_read_index_returns_resource_with_text_application_json` with a
   test asserting a read of the retired URI errors — a deleted test is a silent
   coverage loss, a replaced one is a pinned decision. Drop the index URI from the
   proptest's read loop at :366.

5. Do NOT introduce a legacy feature gate. The blast-radius check in this plan's
   table found 12 in-repo sites and no external consumer, which is the condition
   D-08 set for removal by default.
  </action>

  <verify>
    <automated>cargo test -p pmcp --all-features --lib skills -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" / "running 0 tests" in the summary line</fails_when>
    <automated>cargo test --all-features --test skills_integration -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "running 0 tests", or fewer than 11 tests passed</fails_when>
    <automated>cargo test -p pmcp --all-features --lib server::builder -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the summary line</fails_when>
    <automated>cargo test --all-features --test skills_routing -- --test-threads=1</automated>
    <fails_when>non-zero exit — the method-based discovery surface must be unaffected by removing the resource-based one</fails_when>
  </verify>

  <acceptance_criteria>
    - `grep -rc 'index.json' src/server/skills.rs src/server/builder.rs` shows occurrences only inside test assertions that assert ABSENCE or an error, and zero occurrences in the implementation region: `sed -n '1,600p' src/server/skills.rs | grep -c 'index.json'` returns 0.
    - `grep -c 'build_discovery_index_json' src/server/skills.rs` returns 0.
    - `grep -c 'index_json' src/server/skills.rs` returns 0.
    - A test asserts a `resources/list` on a 2-skill registry returns exactly 2 resources.
    - A test asserts reading the retired discovery-index URI returns an error rather than content.
    - The `src/server/builder.rs` assertion asserts absence, not deletion — `grep -c 'index.json' src/server/builder.rs` returns exactly 1.
    - `cargo test --all-features -- --test-threads=1` exits 0.
  </acceptance_criteria>

  <reversibility rating="costly">
    Removing a served resource is an observable behavior change for any downstream
    host that reads it. Authorized by D-08 and by ROADMAP success criterion 2, and
    the blast-radius check found no consumer outside this repo. Restoring it is a
    revert of one commit; flagged, not gated.
  </reversibility>

  <done>
The synthesized discovery index is gone from the implementation, all 12 tracked Rust
assertion sites measure the new shape (including two that assert its absence), and
every pre-existing byte-identity and not-listable property still passes. Committed.
  </done>
</task>

<task type="auto">
  <name>Task 2: Bring the two runnable examples onto the method-based discovery surface</name>

  <files>examples/s44_server_skills.rs, examples/c10_client_skills.rs</files>

  <read_first>
    - examples/s44_server_skills.rs — the whole file. Its doc header numbered list (:17-31) names the index as item 2, and its output block (:93-101) prints it. Note the three registered skills at :83-85.
    - examples/skills/hello-world/SKILL.md, examples/skills/refunds/SKILL.md, examples/skills/code-mode/SKILL.md — all three ALREADY carry `---`-delimited frontmatter with `name` and `description`, and all three names already match their resolved URIs' final segments (`refunds` under `.with_path("acme/billing/refunds")` resolves to final segment `refunds`). D-03's frontmatter requirement is therefore already satisfied for these three; verify this before assuming edits are needed.
    - examples/c10_client_skills.rs — the whole file, in particular the index read and its two `assert_eq!` at :107-114. This example ASSERTS, so it panics rather than printing wrong output.
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-SUMMARY.md and 125-02-SUMMARY.md — the exact entry shape and the `skills/list` / `skills/get` call surface available from an example.
    - Makefile:799-830 — `test-examples`, which BUILDS every example and is chained into `quality-gate` through `test-all`; and the run tests it feeds (`tests/docs04_examples_run.rs` and siblings) that execute example binaries.
  </read_first>

  <action>
Make both examples demonstrate the surface the code now serves.

1. `examples/s44_server_skills.rs`: remove the discovery-index line from the doc
   header's numbered list and renumber the remaining items, and remove the
   `println!` naming the auto-synthesized index. Replace both with the method-based
   story: state that the server answers `skills/list` and `skills/get`, and print
   the count of conforming entries the registry produced along with the first
   entry's URI and digest, so the example demonstrates the new surface rather than
   merely omitting the old one. Confirm the three embedded SKILL.md files already
   carry frontmatter with names matching their resolved URIs — per the read_first
   note they do, so no fixture edit should be needed; if a mismatch is found,
   correct the frontmatter, never the registration.

2. `examples/c10_client_skills.rs`: replace the index read and its two `assert_eq!`
   with the `skills/get` equivalent — fetch the entry for a registered SKILL.md URI
   and assert on the entry's `uri`, on its `frontmatter.name`, and on its
   `resources[0].digest` matching the `sha256:` + 64-lowercase-hex shape. Keep the
   example ASSERTING rather than only printing: an example that panics on
   regression is the strongest of the three doc surfaces, and that property is why
   this file is the load-bearing edit. Update the surrounding comment that describes
   `resources/list` as returning "SKILL.md + index ONLY".

3. Run both examples end to end and confirm the printed output describes the current
   behavior — the manual verification row in `125-VALIDATION.md` exists because the
   example asserts are partial and the printed narrative is not machine-checked.
  </action>

  <verify>
    <automated>cargo build -p pmcp --all-features --examples 2>&amp;1 | tail -5</automated>
    <fails_when>non-zero exit, or any line containing "error[" or "error:"</fails_when>
    <automated>cargo run --example s44_server_skills --features skills,full</automated>
    <fails_when>non-zero exit, or the output still contains the string "index.json", or the output contains "panicked"</fails_when>
    <automated>cargo run --example c10_client_skills --features skills,full</automated>
    <fails_when>non-zero exit, or the output contains "panicked" or "assertion", or the output still contains the string "index.json"</fails_when>
    <automated>make test-examples</automated>
    <fails_when>non-zero exit, or the output names an example that failed to compile</fails_when>
  </verify>

  <acceptance_criteria>
    - `grep -c 'index.json' examples/s44_server_skills.rs` returns 0.
    - `grep -c 'index.json' examples/c10_client_skills.rs` returns 0.
    - `cargo run --example c10_client_skills --features skills,full` exits 0 and its stdout contains a `sha256:` prefixed value.
    - `examples/c10_client_skills.rs` still contains at least three `assert_eq!` or `assert!` calls — the example remains self-checking.
    - `make test-examples` exits 0.
    - No file under `examples/skills/` was modified unless a frontmatter name genuinely mismatched its resolved URI; if one was, the SUMMARY names it and why.
  </acceptance_criteria>

  <done>
Both examples run clean, demonstrate `skills/list` and `skills/get`, name no retired
URI, and c10 still asserts on real wire shape. Committed.
  </done>
</task>

<task type="auto">
  <name>Task 3: Update the four documentation surfaces and keep the doctest mirror byte-equal</name>

  <files>pmcp-book/src/ch12-8-skills.md, pmcp-course/src/part8-advanced/ch23-skills.md, pmcp-course/src/part8-advanced/ch23-exercises.md, pmcp-course/src/quizzes/ch23-skills.toml, src/server/skills.rs</files>

  <read_first>
    - src/server/skills.rs:1-40 — the module header, including line 19's rule that the module doctest is a byte-equal mirror of the closing doctest in `pmcp-book/src/ch12-8-skills.md`, and the doctest body itself with its `assert!(prompt_text.starts_with("# Hello"))`.
    - pmcp-book/src/ch12-8-skills.md — lines 124 (the resources/list bullet naming the index), 181 (the "readable but not listable" paragraph), 266 (the index read snippet), 384 (the example-output paragraph), and the closing doctest at ~:360-375 whose `Skill::new("hello-world", "# Hello\nThis is a minimal skill.\n")` body carries NO frontmatter.
    - pmcp-course/src/part8-advanced/ch23-skills.md:165-230 — the two paragraphs describing the auto-synthesized index.
    - pmcp-course/src/part8-advanced/ch23-exercises.md:30-45 and :95-110 — three exercise instructions that tell the reader to assert on the index.
    - pmcp-course/src/quizzes/ch23-skills.toml:35-45 — the quiz answer text describing the expected `resources/list` contents.
    - Makefile:1348-1360 — the `book-test` target, which runs `mdbook test` over the book. Note it is NOT chained into `quality-gate`, so it must be run explicitly.
  </read_first>

  <action>
Bring all four tracked documentation surfaces onto the method-based discovery story,
and honour the mirror rule.

1. `pmcp-book/src/ch12-8-skills.md`: rewrite the four index-naming sites to describe
   `skills/list` and `skills/get` as the discovery surface, and `resources/list` as
   returning one entry per registered SKILL.md with supporting files readable but
   not listable. Replace the index-read snippet at ~:266 with a `skills/get`
   snippet whose shape matches what `examples/c10_client_skills.rs` now does, so
   the chapter and the example teach the same call.

2. Per D-03, give the chapter's closing doctest skill a real frontmatter block so
   the canonical book snippet produces a CONFORMING entry rather than one the D-02
   path excludes. The frontmatter `name` must equal the skill's resolved URI final
   segment or the 125-03 name-identity rule rejects it. The existing
   `assert!(prompt_text.starts_with("# Hello"))` will no longer hold once the body
   opens with a frontmatter delimiter — change the assertion to something the new
   body satisfies, and make the SAME change in the `src/server/skills.rs` module
   doctest so the byte-equal mirror stated at `src/server/skills.rs:19` remains
   true. Both sides move in this commit or the stated rule becomes false.

3. `pmcp-course/src/part8-advanced/ch23-skills.md`: rewrite the two index
   paragraphs the same way.

4. `pmcp-course/src/part8-advanced/ch23-exercises.md`: rewrite the three exercise
   instructions so the reader asserts on the method-based listing. Keep the
   exercises' difficulty and structure; change what they assert, not how many steps
   they have.

5. `pmcp-course/src/quizzes/ch23-quizzes` — specifically
   `pmcp-course/src/quizzes/ch23-skills.toml`: update the answer text so a reader
   who answers correctly against the current code is graded correct. A quiz whose
   right answer is the retired behavior is worse than no quiz.

6. Do NOT touch `pmcp-book/book/**` or `pmcp-course/book/**`. They match a grep for
   the retired URI but are untracked mdBook build output and regenerate from `src/`.
  </action>

  <verify>
    <automated>cargo test --doc --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or "0 passed" in the doctest summary — the updated module doctest must actually run</fails_when>
    <automated>make book-test</automated>
    <fails_when>non-zero exit, or the output contains "test result: FAILED"</fails_when>
    <automated>git ls-files -- pmcp-book/book pmcp-course/book</automated>
    <fails_when>the command prints any path — that would mean build output became tracked and this task's do-not-edit rule was violated in the wrong direction</fails_when>
    <automated>cargo test --all-features -- --test-threads=1</automated>
    <fails_when>non-zero exit, or any line beginning with "failures:"</fails_when>
  </verify>

  <acceptance_criteria>
    - `grep -rc 'index.json' pmcp-book/src/ pmcp-course/src/` returns 0 for every listed file.
    - `git status --porcelain pmcp-book/book pmcp-course/book` shows no staged or committed changes under those directories.
    - The `src/server/skills.rs` module doctest body and the closing doctest of `pmcp-book/src/ch12-8-skills.md` are byte-equal — verify by extracting both and diffing them; the diff must be empty.
    - The book chapter's closing doctest skill body opens with a frontmatter delimiter and its frontmatter `name` equals the skill's resolved URI final segment.
    - `cargo test --doc --all-features -- --test-threads=1` exits 0.
    - `make book-test` exits 0.
    - The course quiz's correct answer describes the method-based listing.
  </acceptance_criteria>

  <done>
Book chapter, course chapter, course exercises and course quiz all describe the
method-based discovery surface; the canonical book snippet carries conforming
frontmatter; and the source-to-book doctest mirror is byte-equal and passing.
Committed.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Retired resource URI -> `resources/read` | A caller may still request the removed URI. It must reach the ordinary unknown-URI path, not a special case. |
| Documentation -> server author | Snippets are copied into production servers. A snippet teaching a non-conforming construction propagates non-conformance. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-125-16 | Information disclosure | Removed index still reachable through a stale short-circuit | medium | mitigate | The `read` short-circuit is deleted, not merely emptied, so the retired URI falls through to the existing unknown-URI error path. Asserted by a replacement test and by the absence grep in the acceptance criteria. |
| T-125-17 | Tampering | Documentation teaching a non-conforming construction | medium | mitigate | D-03 gives every canonical snippet real frontmatter, so a reader who copies a snippet gets a skill that appears in `skills/list` rather than one silently excluded by the D-02 path. Enforced by `make book-test` and by the name-identity rule from 125-03 rejecting a mismatched snippet. |
| T-125-18 | Repudiation | Silent coverage loss when a test is deleted rather than replaced | low | mitigate | Both index-asserting tests are REPLACED with absence/error assertions rather than deleted, so a regression that reintroduces the index is still detectable. |
</threat_model>

<verification>
- `cargo test --all-features -- --test-threads=1` exits 0 (matches CI).
- `cargo test --doc --all-features -- --test-threads=1` exits 0.
- `make test-examples` exits 0 and both examples run to completion.
- `make book-test` exits 0.
- `grep -rc 'index.json' src/ tests/ examples/ pmcp-book/src/ pmcp-course/src/` shows occurrences only in the two absence/error assertions.
</verification>

<success_criteria>
- One discovery surface: the method-based one.
- Reading the retired URI errors through the ordinary path, and that is tested.
- All 12 tracked assertion sites updated; two of them assert absence.
- Both examples run clean and c10 still self-checks.
- Four documentation surfaces describe the shipped behavior, and the doctest mirror is byte-equal.
</success_criteria>

<output>
Create `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-04-SUMMARY.md` when done.
</output>
