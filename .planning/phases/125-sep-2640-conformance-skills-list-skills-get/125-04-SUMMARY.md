---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 04
subsystem: api
tags: [sep-2640, skills, discovery, behavior-break, docs, examples, doctest-mirror, mcp-protocol]

# Dependency graph
requires:
  - phase: 125-01
    provides: "`Skills::entries()` / `SkillEntry` / `SkillResourceRef` — the projection both examples and the book chapter now demonstrate in place of the retired index read."
  - phase: 125-02
    provides: "`skills/get` plus the MEASURED two-level `resources/read` unknown-URI error shape (-32603 on the wire carrying -32601 inside the message), which this plan's replacement tests pin at the correct level."
  - phase: 125-03
    provides: "Complete `resources` manifests, verbatim frontmatter, and `validate_names` — the behavior the four documentation surfaces now describe, and the hard reject the book chapter's new frontmatter block had to satisfy."
  - phase: 80-skills
    provides: "`SkillsHandler` and the synthesized discovery index this plan removes."
provides:
  - "ONE discovery surface: `skills/list` / `skills/get`. The synthesized `skill://index.json` resource is gone from `resources/list` and from `resources/read`."
  - "The retired URI takes the ORDINARY unknown-URI path, pinned by two replacement tests (unit + integration) rather than deleted assertions."
  - "Two absence assertions (`src/server/builder.rs`, `tests/skills_integration.rs`) that fail if the index is ever reintroduced."
  - "Two runnable examples that demonstrate the entry PROJECTION honestly and disclaim the RPC they cannot reach, each naming `tests/skills_routing.rs` as the wire proof."
  - "Four documentation surfaces describing the shipped behavior, with the book/source doctest mirror byte-equal and its `rust,no_run` assertions now EXECUTED by a real unit test."
affects: [125-05 make test-skills and deferrals]

actuals:
  tokens: 17718   # chars/4 over the realized diff (70,874 chars, 3a33f17b~1..HEAD)
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Retirement-with-a-note: a removed surface leaves a rustdoc block at the deletion site stating what was removed, why, and that the fall-through path is deliberate — so the next reader does not 'restore' it."
    - "Replace, never delete, a test whose subject was removed: the index-read tests became retired-URI-errors tests, so a reintroduced short-circuit still fails something."
    - "Invert rather than delete an assertion whose subject was removed (`assert!(contains)` -> `assert!(!contains)`), keeping the regression detectable from the surface a user observes."
    - "Honest-projection example: when the demonstrated API is unreachable from the example's shape, demonstrate the value it would carry, SAY that it is not an RPC, and name the test that is."
    - "Execute a `no_run` doctest's assertions from a real unit test, so the canonical copied-by-readers snippet cannot silently become false."

key-files:
  created: []
  modified:
    - src/server/skills.rs
    - src/server/builder.rs
    - tests/skills_integration.rs
    - examples/s44_server_skills.rs
    - examples/c10_client_skills.rs
    - pmcp-book/src/ch12-8-skills.md
    - pmcp-course/src/part8-advanced/ch23-skills.md
    - pmcp-course/src/part8-advanced/ch23-exercises.md
    - pmcp-course/src/quizzes/ch23-skills.toml

key-decisions:
  - "The retirement is recorded as an INTENTIONAL, AUTHORIZED behavior break (D-08, ROADMAP SC#2) — not as a safety proof. The blast-radius grep enumerated 12 in-repo sites; it establishes nothing about consumers outside this repository, and the authorization never depended on it (R-30)."
  - "`INDEX_JSON_MIME` was removed too: a grep confirmed the index was its only consumer."
  - "Both index-asserting tests were REPLACED rather than deleted. The unit replacement compares the retired URI's error against a control stranger URI's error for SHAPE equality, so it proves the ordinary path is taken rather than merely that something errored."
  - "`examples/c10_client_skills.rs` demonstrates `Skills::entries()` and states in module doc AND printed output that it is not an RPC. R-27 is correct that no `skills/get` equivalent is reachable: `sep_2640_flow` takes `&dyn ResourceHandler`, `skills/get` is not a `ResourceHandler` method, and the file holds no transport."
  - "`examples/s44_server_skills.rs` builds a SEPARATE projection registry rather than reusing the builder's, with a comment naming the two constraints that force it (the duplicate-URI merge, and `SkillPromptHandler` being `pub(crate)`)."
  - "Added `the_module_doctest_assertions_actually_hold` (a deviation, not in the plan). Both doctest mirrors are `rust,no_run`, so NEITHER `cargo test --doc` nor `mdbook test` executes their `assert!`s — the plan's instruction to 'change the assertion to something the new body satisfies' was otherwise unverifiable by any harness."
  - "`make book-test` is left RED. Measured against HEAD: identical failure counts before and after this plan's edits. Pre-existing, out of scope, logged to deferred-items.md."

patterns-established:
  - "A plan `<verify>` gate that a pre-existing repo-wide failure makes unsatisfiable is discharged by MEASURING the baseline (restore HEAD version, re-run, restore working copy, diff to prove restoration) rather than by asserting 'it was already broken'."
  - "The docs04 stale-binary guard can FALSE-POSITIVE on `crates/*/examples/` binaries: it compares mtime against root `src/` even when that file is not in the example's build graph. `cargo build -p <pkg> --example <name>` will NOT clear it (cargo reports fresh); touching the example's own source does."

requirements-completed: [D-03, D-08]

coverage:
  - id: D1
    description: "`resources/list` on a skills-backed handler returns exactly one entry per registered SKILL.md and nothing else — no synthesized discovery entry, and still no supporting-file entries (D-08, ROADMAP SC#2 and SC#4)."
    requirement: "D-08"
    verification:
      - kind: unit
        ref: "src/server/skills.rs#test_1_7_skills_into_handler_happy_path (len == 2 for a 2-skill registry)"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#test_1_7a_skills_into_handler_preserves_registration_order (len == 3 for 3 skills, registration order, 10x)"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#test_1_9_skills_handler_list_excludes_references (retired-entry count == 0 AND len == 1)"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#resources_list_returns_skill_md_only (len == 2 + explicit absence assertion)"
        status: pass
      - kind: unit
        ref: "src/server/builder.rs#test_2_4_skills_compose_with_existing_resources (ABSENCE assertion on the composed handler)"
        status: pass
    human_judgment: false
  - id: D2
    description: "`resources/read` on the retired discovery-index URI returns the handler's ordinary unknown-URI error rather than a synthesized JSON document (D-08). NOTE: at the HANDLER level this is METHOD_NOT_FOUND (-32601); on the WIRE the dispatch tail re-wraps it to -32603 carrying -32601 in the message (125-02's correction to D-06's single-level phrasing)."
    requirement: "D-08"
    verification:
      - kind: unit
        ref: "src/server/skills.rs#test_1_12_skills_handler_read_retired_index_uri_is_unknown (error SHAPE compared against a control stranger URI, plus a positive control)"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#resources_read_retired_index_uri_is_unknown (code + message + positive control)"
        status: pass
    human_judgment: false
  - id: D3
    description: "SKILL.md and every supporting file remain byte-identical through `resources/read`, and the dual-surface prompt-fallback byte-equality still holds on both LF and CRLF fixtures (ROADMAP SC#4)."
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#dual_surface_byte_equal_construction_level"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#dual_surface_byte_equal_wire_level_via_get_prompt"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#dual_surface_byte_equal_crlf_and_mixed_line_endings"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#proptest_byte_equality_under_arbitrary_skill_content"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#prop_manifest_rows_are_the_bytes_the_handler_serves (125-03's manifest-vs-served-bytes property, unaffected)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Both examples exit 0, with c10 asserting on the `Skills::entries()` PROJECTION in place of the retired index read, and stating in its output and module doc that it demonstrates the projection rather than an RPC round trip (D-03, D-08, ROADMAP SC#4)."
    requirement: "D-03"
    verification:
      - kind: other
        ref: "cargo run --example s44_server_skills --features skills,full -> exit 0, no 'index.json', no 'panicked'; prints 3 conforming entries with URIs, names, file counts and sha256 digests"
        status: pass
      - kind: other
        ref: "cargo run --example c10_client_skills --features skills,full -> exit 0, no 'index.json', no 'panicked'/'assertion'; stdout carries sha256:-prefixed values"
        status: pass
      - kind: other
        ref: "make test-examples -> exit 0 (87 examples across 3 covered trees, 0 failures)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Neither example, nor any documentation surface this plan touches, claims to have exercised `skills/list` or `skills/get` over MCP; each names `tests/skills_routing.rs` as the authoritative end-to-end proof (R-29)."
    verification:
      - kind: other
        ref: "grep -c 'skills_routing' examples/c10_client_skills.rs examples/s44_server_skills.rs -> 2 and 2 (both nonzero)"
        status: pass
      - kind: manual
        ref: "Both examples print an explicit 'NOT an RPC / this is Skills::entries(), read IN PROCESS' block; the book chapter states the same beside its snippet; the course chapter frames discovery as the method pair."
        status: pass
    human_judgment: true
  - id: D6
    description: "Every user-facing snippet — book chapter, course chapter, course exercises, course quiz — describes the method-based discovery surface, and every snippet skill body carries real frontmatter so it produces a conforming entry (D-03)."
    requirement: "D-03"
    verification:
      - kind: other
        ref: "grep -rc 'index.json' pmcp-book/src/ pmcp-course/src/ -> 0 for every file"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#the_module_doctest_assertions_actually_hold (proves the canonical snippet yields exactly ONE conforming entry with uri and frontmatter name as documented)"
        status: pass
      - kind: other
        ref: "cargo test --doc --all-features -- --test-threads=1 -> 479 passed, 0 failed"
        status: pass
    human_judgment: false
  - id: D7
    description: "The `src/server/skills.rs` module doctest and the closing doctest of `pmcp-book/src/ch12-8-skills.md` remain byte-equal mirrors of each other after both are updated (the rule stated at `src/server/skills.rs:18`)."
    verification:
      - kind: other
        ref: "Both blocks extracted (sed strip of the '//! ' prefix; awk on the single ```rust,no_run fence) and diffed: 25 lines each (non-empty), EMPTY diff. Re-verified AFTER rustfmt."
        status: pass
    human_judgment: false
  - id: D8
    description: "The synthesized discovery index is absent from the IMPLEMENTATION region, checked structurally rather than by line number and with comment lines stripped (R-31)."
    verification:
      - kind: other
        ref: "sed -n '1,/^#[cfg(test)]/p' src/server/skills.rs | grep -v '^\\s*//' | grep -c 'index.json' -> 0, over a 1432-line non-empty region"
        status: pass
      - kind: other
        ref: "grep -v '^\\s*//' src/server/skills.rs | grep -c 'build_discovery_index_json' -> 0; same for 'index_json' -> 0; SKILL_INDEX_URI/INDEX_JSON_MIME -> 0 file-wide"
        status: pass
    human_judgment: false

# Metrics
duration: 42 min
completed: 2026-09-02
status: complete
---

# Phase 125 Plan 04: Retire the synthesized discovery index Summary

**pmcp now advertises ONE skill-discovery surface — the SEP-2640 `skills/list` / `skills/get` method pair — with the synthesized `skill://index.json` resource removed as an intentional published break, all 12 tracked assertion sites re-pointed (two of them now asserting its absence), both runnable examples demonstrating the entry projection honestly rather than implying an RPC they hold no transport to make, and the book/source doctest mirror byte-equal with its previously-unexecuted assertions now pinned by a real test.**

## Performance

- **Duration:** 42 min
- **Started:** 2026-09-02T07:14:00Z
- **Completed:** 2026-09-02T07:56:43Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- **One discovery surface, and the removal is honest about what it is.** The index URI constant, `INDEX_JSON_MIME`, `build_discovery_index_json`, the `index_json` field, the `list_resources.push(...)` and the `read` short-circuit are all gone. A retirement note at the deletion site states what was removed, the two independent reasons (the methods supersede it; its URI violated the draft's own structure rule — its name segment is not a skill name and it has no `/SKILL.md` sibling), and that the fall-through to the unknown-URI path is deliberate. That note is what stops the next reader "fixing" the absence.

- **Both index-asserting tests were REPLACED, not deleted, and the replacements are stronger than an `is_err()`.** The unit replacement reads the retired URI AND a control stranger URI, then asserts the two errors are the same SHAPE — same code, same message modulo the echoed URI — so it proves the *ordinary path* is taken rather than merely that something failed. It also carries a positive control (a registered SKILL.md still reads) so it cannot pass against a handler that refuses everything.

- **Two assertions now guard the absence.** `src/server/builder.rs`'s composed-handler test and `tests/skills_integration.rs`'s listing test both assert the retired URI is NOT present, rather than dropping the line. A reintroduction fails from the surface a server author actually observes.

- **c10 tells the truth about what it is.** R-27 was right: there is no `skills/get` equivalent reachable from this file — `sep_2640_flow` takes `&dyn ResourceHandler`, `skills/get` is not a `ResourceHandler` method, and the example holds no client, no server and no transport. It now reads `entries()` before `into_handler()` consumes the registry (no clone needed, because `entries` borrows), asserts on the entry's URI, verbatim frontmatter name, four-element manifest and digest shape, and prints an explicit "NOT an RPC ... the end-to-end wire proof is `tests/skills_routing.rs`" block. A reader who copies it expecting a working `skills/get` client is now told plainly that it is not one.

- **s44's apparent duplication is documented as forced.** It builds a separate projection registry because merging one into `.skills(...)` alongside `bootstrap_skill_and_prompt` would present a duplicate `skill://code-mode/SKILL.md` at build time, and because `SkillPromptHandler` is `pub(crate)` so a direct `.prompt(...)` is unreachable from `examples/`. Both constraints are named in a comment at the site, so the shape reads as forced rather than careless.

- **The book chapter now shows the wire shape AND labels the local read as local.** A `skills/list` response body is rendered as JSON prose so a reader learns what a host receives; the runnable Rust beside it is explicitly a `Skills::entries()` projection, not a request. The §9 "readable but not listable" section gained the half it was missing: references are not *listed*, but they are fully described by the entry's `resources` manifest — which is where a host learns their URIs at all.

- **The doctest mirror moved on both sides, and its assertions are now actually checked.** D-03's frontmatter block makes `starts_with("# Hello")` false, so it became `starts_with("---\nname: hello-world\n")` + `contains("# Hello")`, identically in `src/server/skills.rs` and the book chapter (verified byte-equal by extraction and diff, re-verified after rustfmt). Both blocks are `rust,no_run` — compiled but **never executed** by `cargo test --doc` or `mdbook test` — so a new unit test runs those assertions for real and additionally proves the snippet yields exactly one conforming entry.

## Task Commits

1. **Task 1: retire the synthesized skill discovery index** — `3a33f17b` (feat)
2. **Task 2: move both skills examples onto the method-based discovery surface** — `448037d0` (docs)
3. **Task 3: teach method-based skill discovery across all four doc surfaces** — `7db05286` (docs)

_Task 1 carries `tdd="true"`. No separate RED commit: the pre-commit hook runs the Toyota Way quality gate on every commit and `--no-verify` is prohibited for this run, so a deliberately-failing commit cannot land. This is the same constraint 125-03 recorded. Every task's `<verify>` block ran green before its commit._

## Files Created/Modified

- `src/server/skills.rs` — removed `SKILL_INDEX_URI`, `INDEX_JSON_MIME`, `build_discovery_index_json`, the `index_json` field and both serving sites, replaced by a retirement note; 8 unit assertion sites re-pointed (`test_1_7`, `test_1_7a`, `test_1_9`, `test_1_12` renamed + rewritten, `test_1_16`, `collect_all_uris`, `test_1_21`); module doctest updated with D-03 frontmatter; **1 new test** (`the_module_doctest_assertions_actually_hold`).
- `src/server/builder.rs` — `test_2_4`'s index assertion inverted to an absence assertion with a comment saying why.
- `tests/skills_integration.rs` — `resources_list_returns_skill_md_and_index_only` renamed to `..._skill_md_only` (3 -> 2 plus an absence assertion); `resources_read_index_returns_resource_with_text_application_json` replaced by `resources_read_retired_index_uri_is_unknown`; proptest read loop drops the index URI; module doc corrected.
- `examples/s44_server_skills.rs` — doc header renumbered and re-scoped; projection registry + entry printout; index `println!` removed.
- `examples/c10_client_skills.rs` — index read and its two `assert_eq!` replaced by `skills_entry_projection`; registry built once with `entries()` before `into_handler()`; module doc and printed output disclaim the RPC.
- `pmcp-book/src/ch12-8-skills.md` — 6 sites rewritten (list bullet + index body, §9 paragraph, client-flow snippet, example-output paragraph, the `Skill` bullet, the SDK-comparison paragraph), new "Discovery is `skills/list` and `skills/get`" section with the wire-shape JSON, 3 exercises updated, closing doctest given frontmatter.
- `pmcp-course/src/part8-advanced/ch23-skills.md` — the two index paragraphs plus the `SkillsHandler::list()` sentence rewritten.
- `pmcp-course/src/part8-advanced/ch23-exercises.md` — exercises 1 and 2 rewritten; step counts and difficulty preserved, only what they assert changed.
- `pmcp-course/src/quizzes/ch23-skills.toml` — answer text and context corrected so a reader answering against the current code is graded correct.

## Decisions Made

See `key-decisions` in the frontmatter. The three most consequential:

1. **The removal is recorded as an authorized break, not a proven-safe one (R-30).** The blast-radius grep found 12 in-repo sites and no external consumer — but a repository grep has no visibility outside the repository, so "no external consumer known" is an absence of observation, not an observation of absence. D-08 and ROADMAP SC#2 authorize the removal by default; the enumeration exists so no in-repo consumer is left broken, not to prove the break is harmless.

2. **c10 demonstrates the projection and says so.** The alternative — quietly substituting a different call and letting the prose imply an RPC — would teach a client API that does not exist. Both examples and both chapters now state the boundary and name `tests/skills_routing.rs`.

3. **`make book-test` is left red, having measured that it was already red.** See deviation 2.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] The doctest mirror's assertions are never executed by any harness; added a unit test that executes them**

- **Found during:** Task 3 (verifying the assertion change)
- **Issue:** The plan says the existing `assert!(prompt_text.starts_with("# Hello"))` "will no longer hold once the body opens with a frontmatter delimiter — change the assertion to something the new body satisfies". Both mirrors are ` ```rust,no_run `, which **compiles but does not run**. Measured: `cargo test --doc` lists the merged module doctest as `src/server/mod.rs - server::skills (line 213) - compile`, and `mdbook test` reports its twin as `- compile` too. So no harness would have caught a wrong replacement assertion — the plan's instruction was unverifiable as written, and the claim would have shipped on reasoning alone. Readers who copy the snippet DO run it.
- **Fix:** Added `the_module_doctest_assertions_actually_hold` to `src/server/skills.rs`'s test module. It executes both new assertions against exactly the doctest's body, additionally asserts the OLD assertion is now false (so a future reverter sees why it changed), and — per D-03 — proves the snippet produces exactly one conforming entry with the documented `uri()` and frontmatter `name`.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `cargo test -p pmcp --all-features --lib the_module_doctest -- --test-threads=1` -> 1 passed. Byte-equality of the mirrors re-verified after the addition and after rustfmt (25 lines each, empty diff).
- **Committed in:** `7db05286`

**2. [Rule 1 - Plan gate unsatisfiable due to pre-existing state] `make book-test` cannot exit 0; measured as pre-existing and left red**

- **Found during:** Task 3 (`<verify>` gate)
- **Issue:** Task 3's `<verify>` requires `make book-test` to exit 0. It exits 2 with **27** `test result: FAILED` lines and **324** `error[E0433]: cannot find module or crate 'pmcp'` errors, across **26** chapter files — including `introduction.md`, `ch01`–`ch15`, and chapters this plan never touched. `mdbook test` is not linking the `pmcp` rlib.
- **Fix:** Measured the baseline rather than asserting it. The HEAD version of `pmcp-book/src/ch12-8-skills.md` was restored, `make book-test` re-run, and the working copy restored (byte-identity confirmed by `diff`). **Baseline and post-change runs are identical**: exit 2, 27 FAILED, 324 unlinked-crate errors, the same two ch12-8 doctest failures (the Try-It-Yourself block moved 360 -> 412 only because Task 3 inserted prose above it). This plan neither added nor removed a failure. Per the SCOPE BOUNDARY rule the pre-existing harness breakage is logged to `deferred-items.md`, not fixed — it is a build-tooling problem no plan in phase 125 owns, and `make book-test` is deliberately not chained into `quality-gate`, so it gates neither the pre-commit hook nor CI.
- **Files modified:** none (plus `.planning/.../deferred-items.md`)
- **Verification:** the baseline/post-change table above; `cargo test --doc --all-features` (which DOES link `pmcp`) -> 479 passed, 0 failed, covering the chapter's mirrored doctest through its `src/server/skills.rs` twin.
- **Committed in:** n/a (a measurement and a deferral, not a code change)

**3. [Rule 3 - Blocking] The docs04 stale-binary guard false-positived and would not clear via the documented rebuild**

- **Found during:** Task 3 (full-suite run)
- **Issue:** `tests/docs04_examples_run.rs` failed for `doc_review_team` and `s50_standalone_vs_sampled` with its STALE assertion, aborting the suite before the doctests. The documented remedies did not clear it: `cargo build -p pmcp-team-servers --all-features --example doc_review_team` reported *Finished in 0.16s* (cargo considers the binary fresh), and deleting the binary and rebuilding restored it from cache **with its original mtime**. Root cause: those crates' `pmcp` dependency does not enable the `skills` feature, so `src/server/skills.rs` is not in their build graph and cargo correctly sees nothing to redo — while the guard compares raw mtime against root `src/` regardless. `docs04`'s own header records this limitation for `crates/*/examples/` binaries.
- **Fix:** `touch`ed the two examples' OWN source files, which forces a genuine recompile-and-relink with fresh mtimes. No file content changed (`git status --porcelain` on both -> empty), so this is a rebuild, not a workaround that hides a real staleness.
- **Files modified:** none
- **Verification:** `cargo test --all-features -- --test-threads=1` -> **exit 0**, 139 `test result: ok`, 0 `FAILED`, 0 `failures:` lines, doctests reached (479 passed) so the run completed rather than aborting early.
- **Committed in:** n/a (a build action, not a code change)

---

**Total deviations:** 3 (1 x Rule 2 missing-critical, 1 x Rule 1 unsatisfiable plan gate, 1 x Rule 3 blocking).
**Impact on plan:** No scope creep; no file touched outside `files_modified`. The one substantive addition (deviation 1) converts an unexecuted claim in the most-copied snippet in the chapter into a checked one. The two others are measurement work: one proving a red gate was already red, one clearing a guard that was firing on a file its target does not compile.

## Issues Encountered

**Three measurement hazards, all documented in advance by earlier plans, all handled; no code defect.**

1. **The stale-binary guard fired and resisted the documented fix** — see deviation 3. New detail worth carrying forward: for a `crates/*/examples/` binary whose crate does not enable the feature that changed, `cargo build -p <pkg> --example <name>` will NOT clear the guard, and neither will deleting the binary (cargo re-hardlinks it from cache with the old mtime). Touching the example's own source is what works.

2. **`cargo test` aborts after the FIRST failing target.** The run that hit the stale-binary guard reported 12 `test result: ok` lines; the clean run reported 139. Every intermediate figure in this session was treated as a lower bound.

3. **Every grep, `sed`, `awk` and `cargo` invocation was run through an absolute binary path** (`/usr/bin/grep`, `/usr/bin/sed`, `/usr/bin/awk`, `/Users/guy/.cargo/bin/cargo`), per the `rtk`-proxy hazard 125-01, 125-02 and 125-03 each recorded. This mattered most for the doctest-mirror diff, which is a byte-equality claim over two extracted text blocks. No corruption was observed, which is the expected result of avoiding the proxy rather than evidence it is fixed.

## Known Stubs

**None introduced.** This plan removes rather than creates.

One item is deferred and is a pre-existing failure rather than a stub this plan authored: **`make book-test` is red repo-wide** (26 chapters, `mdbook test` not linking `pmcp`). Measured identical before and after this plan's edits; recorded in `.planning/phases/125-sep-2640-conformance-skills-list-skills-get/deferred-items.md` with the baseline table and the suggested owner.

One boundary worth naming as a boundary: **both doctest mirrors remain `rust,no_run`**, so neither `cargo test --doc` nor `mdbook test` executes their assertions. That is unchanged pre-existing shape, and it is now compensated rather than merely noted — `the_module_doctest_assertions_actually_hold` executes them.

## Threat Flags

None. Every trust boundary this plan crosses is in the plan's own `<threat_model>`, and each disposition landed as written:

- **T-125-16** (removed index still reachable through a stale short-circuit) — *mitigate*, landed. The `read` short-circuit is DELETED, not emptied, so the retired URI falls through to the pre-existing unknown-URI path with no special case and no change to that path's error code. Asserted by two replacement tests (which compare the error's SHAPE against a control stranger URI, not merely its existence) and by the structural absence greps.
- **T-125-17** (documentation teaching a non-conforming construction) — *mitigate*, landed. The canonical book/source snippet now carries real frontmatter whose `name` equals its resolved URI's final segment, so a reader who copies it gets a skill that appears in `skills/list` rather than one 125-03's D-02 path silently excludes. Enforced by `the_module_doctest_assertions_actually_hold` (which asserts the snippet yields exactly one conforming entry) rather than by `make book-test`, which is red for unrelated reasons.
- **T-125-18** (silent coverage loss from deleting a test) — *mitigate*, landed. Both index-asserting tests are REPLACED with error/absence assertions, and two further sites (`builder.rs`, `skills_integration.rs`) invert rather than drop their assertions, so a regression that reintroduces the index fails in four places.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Ready for 125-05 (the final plan of this phase).**

- **The retirement is complete and self-guarding.** Zero `index.json` occurrences remain in implementation code, in either example, or in any of the four documentation surfaces. The eight that remain are all inside absence assertions or error assertions across four files — the intended end state, and exactly what the plan's `<verification>` specifies.
- **Carry into 125-05's deferral list:** `make book-test` is red repo-wide and is NOT this phase's to fix — see `deferred-items.md` for the measured baseline. If 125-05 adds a `make test-skills` leg to the quality gate, do not chain `book-test` into it.
- **New hazard for 125-05's own full-suite runs:** the docs04 stale-binary guard's `crates/*/examples/` false positive (deviation 3). The remedy is `touch crates/<pkg>/examples/<name>.rs` then rebuild — NOT `cargo build -p <pkg> --example <name>`, which reports fresh, and NOT deleting the binary, which cargo re-hardlinks with the old mtime.
- **`make quality-gate` still does not reach this module** — every leg pins `--features "full"`, which excludes `skills`. Unchanged from 125-01/02/03; this plan was verified with `--all-features` throughout. The dedicated `make test-skills` leg (D-09) remains 125-05's.
- **`requirements mark-complete D-03 D-08` is expected to be a no-op** — D-03/D-08 are 125-CONTEXT decision IDs, not `REQUIREMENTS.md` requirement IDs (the v2.6 requirement set is PKG-01..PKGR-01). They are recorded in this SUMMARY's `requirements-completed` exactly as 125-01/02/03 recorded theirs.

## Self-Check: PASSED

- All 9 modified files exist on disk and carry the changes (569 insertions / 207 deletions across the three commits).
- `git log --oneline --all | grep 3a33f17b` -> found (Task 1, feat).
- `git log --oneline --all | grep 448037d0` -> found (Task 2, docs).
- `git log --oneline --all | grep 7db05286` -> found (Task 3, docs).
- Plan `<verification>` re-run at close: `cargo test --all-features -- --test-threads=1` -> **exit 0**, 139 `test result: ok`, 0 `FAILED`, 0 `failures:`, doctests reached (479 passed); `cargo test --doc --all-features -- --test-threads=1` -> 479 passed, 0 failed; `make test-examples` -> exit 0 (87 examples, 0 failures), both skills examples run to completion; `cargo clippy --all-targets --all-features -- -D warnings` -> clean; `cargo fmt --all -- --check` -> clean. `make book-test` -> exit 2, MEASURED identical to its HEAD baseline (deviation 2).
- Plan `<verification>` grep: `grep -rn 'index.json' src/ tests/ examples/ pmcp-book/src/ pmcp-course/src/` -> 8 occurrences, ALL inside absence assertions or error assertions (`src/server/skills.rs` x4, `src/server/builder.rs` x1, `tests/skills_integration.rs` x3). Zero in implementation, examples or docs.
- Task 1 acceptance greps: implementation-region `index.json` -> **0** over a **1432-line** (non-empty) region anchored on `^#[cfg(test)]`; `build_discovery_index_json` -> 0; `index_json` -> 0; `SKILL_INDEX_URI` / `INDEX_JSON_MIME` -> 0; `builder.rs` `index.json` -> 1 (>= 1 floor), inside the absence assertion.
- Task 2 acceptance greps: `index.json` in each example -> 0; `skills_routing` -> 2 in each example (both nonzero); c10 assert count -> 10 (>= 3); `git status --short examples/skills/` -> **0 lines** (no fixture edited; all three already conform).
- Task 3 acceptance: doctest mirror extracted from both sides -> **25 lines each, EMPTY diff**, re-verified after rustfmt; `git ls-files -- pmcp-book/book pmcp-course/book` -> 0 paths; `git status --porcelain` on those dirs -> 0 lines; `grep -rc 'index.json' pmcp-book/src/ pmcp-course/src/` -> 0 for every file.
- Plan `<success_criteria>`: all five met — one discovery surface ✅; the retired URI errors through the ordinary path and that is tested at unit and integration level ✅; all 12 tracked assertion sites updated, two asserting absence ✅; both examples run clean and c10 still self-checks (10 assertions) ✅; four documentation surfaces describe the shipped behavior and the doctest mirror is byte-equal ✅.

---
*Phase: 125-sep-2640-conformance-skills-list-skills-get*
*Completed: 2026-09-02*
