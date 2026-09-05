---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 05
subsystem: infra
tags: [sep-2640, skills, quality-gate, makefile, fuzzing, libfuzzer, proptest, rustdoc, deferrals, ci]

# Dependency graph
requires:
  - phase: 125-01
    provides: "`Skills::entries()` / `SkillEntry` / `SkillResourceRef` and `sha256_digest_hex` — the entry-synthesis path the fuzz target and the stable proptest both drive."
  - phase: 125-02
    provides: "`skills/get`, the `ServerCore` method-reach boundary this plan's rustdoc records (R-38), and `build_skills_list_response` — whose owned-`Vec` parameter turned out to be the pre-existing `make lint` failure."
  - phase: 125-03
    provides: "`entries_with_diagnostics`, the four-way `FrontmatterParse`, `SkillDiagnostic`, `validate_names` and `exceeds_skill_limits` — the surface both new tests assert against, and the rustdoc whose five private intra-doc links `doc-check` caught the moment it could see the module."
  - phase: 125-04
    provides: "the measured `make book-test` baseline in `deferred-items.md` (appended to, not replaced) and the docs04 stale-binary remedy this plan needed twice."
provides:
  - "`make test-skills`, chained into `quality-gate`, running FOUR separately-captured selectors each with its OWN zero-count guard — the first gate leg in the repo that compiles and RUNS `src/server/skills.rs`."
  - "`skills` in `doc-check`'s feature list, so the module's rustdoc is warning-checked locally under `RUSTDOCFLAGS=\"-D warnings\"`."
  - "`fuzz_skill_entry`: a libFuzzer target for entry synthesis, registered in `fuzz/Cargo.toml`, SCHEDULED in `.github/workflows/fuzz.yml`, and proven registered by a nightly-free source-scan test over all three artifacts."
  - "The same no-panic invariant on stable: a proptest over three framings plus 12 named malformed frontmatter shapes, both with anti-vacuity assertions."
  - "A complete deferral record in `src/server/skills.rs` rustdoc — transport reach, the `ServerCore` boundary, five owned deferrals, the name-bearing non-change — with zero SATD markers."
  - "A GREEN `make quality-gate`, which required fixing two pre-existing red legs (`make lint`, `make lint-plans`) that four earlier plans never ran."
affects: [phase 125 complete; next skills phase of v2.7 inherits the deferral record]

actuals:
  tokens: 16320   # chars/4 over the realized diff (65,281 chars, 82ef71ac~1..HEAD)
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Per-selector gate guards, never a summed total: each selector is captured on its own and guarded on its own count, because one healthy selector keeps an aggregate nonzero while a sibling goes dark."
    - "Prove a gate fails on zero by MAKING it fail: a negative control that emptied one selector while the others reported 106 tests, so the summed-guard alternative is measured to be inadequate rather than argued to be."
    - "Nightly-free fuzz registration proof: a source-text `#[test]` reading the target file, the `[[bin]]` stanza and the CI matrix row, because the fuzz crate is workspace-excluded and `make test-fuzz` swallows every failure with `|| echo`."
    - "Whole-line equality, never `contains`, for a source-scan gate over a list: `contains(\"- x\")` is satisfied by `- xX`."
    - "Anti-vacuity for a property whose interesting branch is rare: add a generated framing that ALWAYS reaches it (`valued`), and assert it does."
    - "Deferrals as a rustdoc table with an owner per row — documentation a `check-todos` scanner cannot flag, at the place a reader of the module will find it."

key-files:
  created:
    - fuzz/fuzz_targets/fuzz_skill_entry.rs
  modified:
    - Makefile
    - fuzz/Cargo.toml
    - .github/workflows/fuzz.yml
    - tests/skills_routing.rs
    - src/server/skills.rs
    - src/server/core.rs
    - src/server/mod.rs
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-01-PLAN.md
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-02-PLAN.md
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-03-PLAN.md
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-04-PLAN.md
    - .planning/phases/125-sep-2640-conformance-skills-list-skills-get/125-05-PLAN.md

key-decisions:
  - "The zero-test guard is PER SELECTOR and the adequacy of that choice is MEASURED, not asserted: a negative control emptied selector 3 while selectors 1 and 2 reported 106 tests between them, so a summed total would have stayed comfortably nonzero and reported green over exactly the blind spot the leg exists to close (R-32)."
  - "`SKILLS_FEATURES := skills,streamable-http,http-client,testing` — an explicit list, not `--all-features` (R-33). `testing` is required and was NOT in the plan's text: `tests/common/duplex.rs` gates its whole body on it, and routing tests 17/18 need the duplex `ServerCore` harness."
  - "`skills` added to the SHARED `[dependencies.pmcp]` feature list in `fuzz/Cargo.toml`, so every fuzz binary builds with it (R-35 accepted explicitly, with the reasoning written into the file beside the four features already under that arrangement)."
  - "The matrix row was ADDED rather than deferred (R-34), and the registration test asserts the row as well as the stanza — registration proves a file exists, the requirement is recurring execution."
  - "Both pre-existing red gate legs were FIXED rather than deferred. They are phase 125's own debt, they block the exact command this plan is chartered to make meaningful, and leaving them red leaves CI red."
  - "The 12 `lint-plans` violations were rewritten in place across all five `125-*-PLAN.md` files. Only the SPELLING of the verify commands changed (`bash -o pipefail -c \"...\"`); no verification intent was altered."

patterns-established:
  - "A gate leg that reaches new code will find defects the moment it exists. `doc-check` gaining `skills` immediately produced five rustdoc errors, and `make lint` — reached only through `quality-gate` — had been red since 125-02. Adding reach IS the work; the fixes are its first output."
  - "`cargo clippy --all-targets --all-features -- -D warnings` is strictly WEAKER than `make lint` (which adds `-W clippy::pedantic -W clippy::nursery`). CLAUDE.md says so; four consecutive plans of this phase ran the weaker command anyway and shipped a pedantic violation."

requirements-completed: [D-01, D-09, D-10, D-11]

coverage:
  - id: D1
    description: "`make quality-gate` compiles AND runs the skills module's tests: a dedicated `make test-skills` leg is chained into the gate, and it FAILS rather than passing when it observes zero tests (D-09, RESEARCH Pitfall 2)."
    requirement: "D-09"
    verification:
      - kind: other
        ref: "make test-skills -> exit 0, exactly 4 `test result:` lines, selectors reporting 99 / 9 / 13 / 19 passed"
        status: pass
      - kind: other
        ref: "NEGATIVE CONTROL: selector 3's filter replaced with a name matching nothing -> make test-skills exit 2, message names selector 3. Selectors 1+2 had already reported 106 tests, so a summed guard would have passed"
        status: pass
      - kind: other
        ref: "make quality-gate -> exit 0, and its output carries the four `✓ selector N ... ran (N passed)` lines"
        status: pass
      - kind: other
        ref: "sed -n '/^test-skills:/,/^$/p' Makefile | grep -c -- '-eq 0' -> 4 (one guard per selector); grep -c -- '--all-features' -> 0"
        status: pass
    human_judgment: false
  - id: D2
    description: "`skills` is added to neither the `full` nor the `full-v2` enumerated feature list, and `tests/v1_severability_tripwire.rs` still passes unchanged (D-09)."
    requirement: "D-09"
    verification:
      - kind: other
        ref: "grep -n '^full' / '^full-v2' Cargo.toml -> neither line contains the token `skills`"
        status: pass
      - kind: integration
        ref: "cargo test --all-features --test v1_severability_tripwire -- --test-threads=1 -> 18 passed, 0 failed"
        status: pass
    human_judgment: false
  - id: D3
    description: "`make doc-check` reaches the skills module, so the phase's new rustdoc is warning-checked locally rather than only in CI."
    verification:
      - kind: other
        ref: "Makefile:1442 feature list now contains `skills`; `make doc-check` -> exit 0, zero `warning:` lines"
        status: pass
      - kind: other
        ref: "First run after the change FAILED with 5 `private-intra-doc-links` errors in 125-03's rustdoc (exit 101) — the leg's first output, fixed to plain code spans"
        status: pass
    human_judgment: false
  - id: D4
    description: "A fuzz target exercising entry synthesis on arbitrary bytes is registered in `fuzz/Cargo.toml`, its source file exists, and a source-scan test fails if either is removed — registration is verified without requiring a nightly toolchain (CLAUDE.md ALWAYS-fuzz requirement)."
    verification:
      - kind: unit
        ref: "tests/skills_routing.rs#fuzz_skill_entry_is_registered_and_scheduled"
        status: pass
      - kind: other
        ref: "NEGATIVE CONTROL 2: `name = \"fuzz_skill_entryX\"` in fuzz/Cargo.toml -> test FAILED naming the missing stanza. NEGATIVE CONTROL 3: source file moved aside -> test FAILED naming the missing file. Both restored byte-identical"
        status: pass
      - kind: other
        ref: "cargo +nightly fuzz build fuzz_skill_entry -> exit 0 (extra evidence; the test needs no nightly)"
        status: pass
    human_judgment: false
  - id: D5
    description: "`fuzz_skill_entry` is a row in `.github/workflows/fuzz.yml`'s target matrix, so the target is EXECUTED on a schedule rather than merely registered (R-34)."
    verification:
      - kind: unit
        ref: "tests/skills_routing.rs#fuzz_skill_entry_is_registered_and_scheduled (whole-line matrix-row equality plus two established-member anti-vacuity rows)"
        status: pass
      - kind: other
        ref: "NEGATIVE CONTROL 1: matrix row renamed to `- fuzz_skill_entryX` -> test FAILED. The FIRST spelling of this assertion used `contains(\"- fuzz_skill_entry\")` and PASSED against that same control; the prefix-substring hole was measured and fixed"
        status: pass
    human_judgment: false
  - id: D6
    description: "The `ServerCore` method-reach boundary established in 125-02 appears in the module's deferral record with the same standing as the stdio deferral: what is deferred, why the current type architecture forbids it, and what would have to change first (R-38)."
    verification:
      - kind: manual
        ref: "src/server/skills.rs module header, section 'Reach boundary: a `ServerCore` serves skill RESOURCES, not the skill METHODS' — names the typed `ProtocolHandler::handle_request` ingress as cause, an internal-request ingress as the precondition, and `server_core_declares_no_skills_field_or_skills_method` as the enforcing guard"
        status: pass
      - kind: other
        ref: "make doc-check -> exit 0, so the section is rustdoc-valid and warning-free"
        status: pass
    human_judgment: true
  - id: D7
    description: "Arbitrary bytes as a SKILL.md body never panic entry synthesis, asserted on stable by a property test as well as by the fuzz target."
    verification:
      - kind: unit
        ref: "src/server/skills.rs#prop_entry_synthesis_never_panics_on_arbitrary_bodies (three framings per case; `valued` framing is prop_assert'ed to always yield an entry, so the digest/size branch is provably reached)"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#entry_synthesis_survives_the_named_malformed_frontmatter_shapes (12 named shapes; asserts >= 3 of them produce an entry)"
        status: pass
      - kind: other
        ref: "cargo +nightly fuzz run fuzz_skill_entry -max_total_time=60 -> 520,501 executions, 3,859 new corpus units, ZERO crashes"
        status: pass
    human_judgment: false
  - id: D8
    description: "Every deferral this phase makes is recorded in rustdoc prose and in the plan record, and NONE of them is a code TODO/FIXME/HACK/XXX marker (D-01, ROADMAP SC#5, CLAUDE.md zero-SATD)."
    requirement: "D-01"
    verification:
      - kind: other
        ref: "grep -cE 'TODO|FIXME|HACK|XXX' src/server/skills.rs -> 0"
        status: pass
      - kind: other
        ref: "make check-todos -> exit 0, 'No technical debt comments'"
        status: pass
      - kind: manual
        ref: "Five-row deferral table in the module header, each row naming WHAT is deferred, WHY it is legal, and its OWNER; three compatibility-only items moved to this SUMMARY per R-36"
        status: pass
    human_judgment: true
  - id: D9
    description: "`set_skills_capabilities` keeps auto-declaring the extension with an empty object, and its rustdoc states both that the empty object means `directoryRead: false` and that the two mandatory methods are answered over streamable HTTP only (D-10, D-01, R-40)."
    requirement: "D-10"
    verification:
      - kind: other
        ref: "sed -n '/fn set_skills_capabilities/,/^}/p' src/server/skills.rs | grep -c 'json!({})' -> 1 (body unchanged)"
        status: pass
      - kind: manual
        ref: "Rustdoc sections: 'What the EMPTY declaration object means', 'What declaring the extension COMMITS the server to', and 'The uncomfortable half, said at the site where it matters' — the last states the build-time-vs-transport race, what a stdio operator should expect, and the two closing options"
        status: pass
    human_judgment: true
  - id: D10
    description: "The stdio-reach deferral names its owner and the measured hazard: over stdio the frame fails at `parse_message` and the server actor breaks its receive loop (D-01)."
    requirement: "D-01"
    verification:
      - kind: manual
        ref: "src/server/skills.rs module header, 'Transport reach' section — names the measured failure chain (parse_method_message -> TransportError::InvalidMessage -> the actor's Err arm breaks), the owner (next skills phase, v2.7), and why widening is bigger than the skills work (the `(RequestId, Request)` channel type)"
        status: pass
      - kind: integration
        ref: "tests/skills_routing.rs#stdio_ingress_rejects_a_skills_list_frame (the measurement the prose reports, with a resources/list control)"
        status: pass
    human_judgment: true
  - id: D11
    description: "`skills/list` returns a single page with no `nextCursor`; cursor pagination is recorded as a deferral rather than silently dropped (D-11)."
    requirement: "D-11"
    verification:
      - kind: manual
        ref: "Row 5 of the module header's deferral table: what is deferred, why a single page is conformant (an absent cursor means the listing is complete), and the revisit trigger"
        status: pass
      - kind: integration
        ref: "tests/skills_routing.rs#skills_list_returns_a_conforming_entry_on_v2 (asserts no `nextCursor` on the wire — 125-01's proof, unchanged)"
        status: pass
    human_judgment: true

# Metrics
duration: 51 min
completed: 2026-09-02
status: complete
---

# Phase 125 Plan 05: Close the loop — gate reach, fuzz coverage, deferral record Summary

**`make quality-gate` now compiles, rustdoc-lints and RUNS `src/server/skills.rs` through a four-selector leg that fails loudly — naming the empty selector — rather than green on zero tests; entry synthesis is fuzzed on 520,501 arbitrary-byte inputs and pinned on stable by a property test with anti-vacuity guards; and every deferral this phase makes lives in the module's rustdoc with an owner and zero SATD markers. Making the gate reach this code immediately produced its own first output: five rustdoc errors and two entirely red gate legs that four earlier plans had never run.**

## Performance

- **Duration:** 51 min
- **Started:** 2026-09-02T08:05:00Z
- **Completed:** 2026-09-02T08:56:00Z
- **Tasks:** 3
- **Files created:** 1
- **Files modified:** 12 (7 source/build, 5 plan documents)

## Accomplishments

- **The gate reaches this module, and the guard that makes that meaningful is measured rather than argued.** `make test-skills` runs four selectors as four separately-captured commands, each with its own zero-count guard and its own failure message naming what goes dark. The plan asserts a summed total would be inadequate; this run **proved** it: a negative control that emptied selector 3 was caught by name, and at the moment it fired selectors 1 and 2 had already reported 106 tests between them — a summed guard would have been comfortably nonzero and green. `full` and `full-v2` are untouched and the severability tripwire still passes 18/18.

- **`--features` is explicit, and the list needed a member the plan did not name.** `skills,streamable-http,http-client,testing`. The first three are in the plan; `testing` is not, and without it `tests/common/duplex.rs` compiles to nothing and routing tests 17 and 18 — the `ServerCore` boundary proofs — silently vanish. Found by running the leg rather than by reading the `#![cfg]` headers, which is the point of building the leg first.

- **Adding reach produced defects on contact — three times.** `doc-check` gaining `skills` failed on its first run with five `private-intra-doc-links` errors in 125-03's rustdoc. `make lint` turned out to have been RED at HEAD since 125-02 (`build_skills_list_response` taking an owned `Vec<Value>`, tripping pedantic `needless_pass_by_value`). `make lint-plans` had 12 D-19 violations across all five of this phase's PLAN.md files. None of the three was visible to the commands 125-01..04 ran. All three are fixed.

- **The fuzz target is registered, scheduled, AND run.** 520,501 executions in 61 seconds under libFuzzer, 3,859 new corpus units, zero crashes. Each input is driven through the path twice: once as the whole SKILL.md body (the delimiter scan) and once as the CONTENTS of a well-formed block, so `serde_yaml` — the only third-party parser this module reaches from author-supplied bytes — is exercised without the fuzzer first having to guess a `---` prefix. Six invariants, with sizes and digest shape re-derived inside the target so it is an independent statement rather than production agreeing with itself.

- **The registration test's first spelling was wrong, and the negative control caught it.** `workflow.contains("- fuzz_skill_entry")` PASSED against a matrix whose only row read `- fuzz_skill_entryX` — a prefix substring satisfies `contains`, so a typo'd row would have reported green while nothing ran. Rewritten to whole-line equality over the parsed rows; all three negative controls (renamed matrix row, renamed `[[bin]]` stanza, deleted source file) now bite.

- **The stable property would have been near-vacuous for half its claim, and that was caught before it shipped.** Arbitrary text framed as a YAML document is almost always a scalar, which takes the `NotAMapping` exclusion path — so the digest, size and object-shape assertions would have run on virtually no generated input. A third framing (`valued`) puts the arbitrary text where a FIELD VALUE goes, so the frontmatter is always a mapping and those assertions are reached on every case, with a `prop_assert!` that refuses the alternative. Its first spelling used Rust's `{:?}` escape, which is **not** valid YAML (`\u{1e000}` where YAML wants `\U0001E000`) — the property failed on its first run, and the framing was wrong, not the parser. `serde_json::to_string` is the correct encoder because YAML 1.2 is a superset of JSON.

- **Every deferral is documentation with an owner, and nothing is a marker.** The module header gained a transport-reach section with the MEASURED stdio failure chain, the `ServerCore` boundary with its precondition for widening, a five-row deferral table, and the name-bearing non-change with its stated cost. `set_skills_capabilities` says the uncomfortable half out loud at the declaration site. `grep -cE 'TODO|FIXME|HACK|XXX' src/server/skills.rs` returns 0 and `make check-todos` exits 0.

## Task Commits

1. **Task 1: the `make test-skills` gate leg with a zero-test-count guard** — `82ef71ac` (feat)
2. **Task 2: fuzz target for entry synthesis, with a nightly-free registration proof** — `37145e71` (feat)
3. **Task 3: record every deferral in rustdoc — and nowhere else** — `dbd9d199` (docs)

_No separate RED commits. There is no pre-commit hook installed in this checkout, so a deliberately-failing commit could have landed — but each task's change is one behavioural unit whose tests and implementation cannot be split without leaving the tree uncompilable (the `&[Value]` signature change and its two call sites; the fuzz target and its registration stanza). Every task's `<verify>` block ran green before its commit._

## Files Created/Modified

- **`fuzz/fuzz_targets/fuzz_skill_entry.rs` (new)** — 6 invariants, two framings per input, an independent `digest_is_well_formed`, and a module doc naming the boundary, the two framings' distinct purposes and 10 corpus cases worth seeding.
- **`Makefile`** — `SKILLS_FEATURES` variable; the `test-skills` target (4 selectors, 4 count guards, 3 named-binary extractor checks, 1 `Doc-tests pmcp` presence check) with a 35-line header recording the measured hole and why a summed total is rejected; `test-skills` chained into `quality-gate` after `test-all`; `skills` added to `doc-check`'s feature list with a note on why that list is safe to extend; a help line.
- **`fuzz/Cargo.toml`** — `skills` added to the shared `[dependencies.pmcp]` feature list with the R-35 acceptance written in; the `[[bin]]` stanza with a header naming the boundary and pointing at the enforcing test.
- **`.github/workflows/fuzz.yml`** — matrix row 5, with a comment recording that enrolment is not automatic.
- **`tests/skills_routing.rs`** — test 19, `fuzz_skill_entry_is_registered_and_scheduled`: three artifacts, two anti-vacuity checks, whole-line matrix matching.
- **`src/server/skills.rs`** — module header gains ~75 lines of deferral record; `set_skills_capabilities` gains ~45 lines of rustdoc (body unchanged); five private intra-doc links demoted to code spans; **2 new tests** (`entry_synthesis_survives_the_named_malformed_frontmatter_shapes`, `prop_entry_synthesis_never_panics_on_arbitrary_bodies`) plus the shared `assert_entry_synthesis_is_total` helper.
- **`src/server/core.rs`, `src/server/mod.rs`** — `build_skills_list_response` takes `&[Value]`; two call sites updated. Fixes the pre-existing `make lint` failure.
- **Five `125-*-PLAN.md` files** — 12 `<automated>` lines rewritten to `bash -o pipefail -c "..."`. Spelling only; no verification intent changed.

## Decisions Made

See `key-decisions` in the frontmatter. The three most consequential:

1. **The per-selector guard's necessity was measured, not asserted.** The plan says a summed total is inadequate; a negative control demonstrated it on this exact recipe. That reading (106 tests present while one selector was empty) is what makes the four-guard shape defensible to a future reader who finds it verbose.

2. **Both pre-existing red gate legs were fixed rather than deferred.** The SCOPE BOUNDARY rule says not to fix unrelated pre-existing failures — but these are neither unrelated nor someone else's: they are phase 125's own artifacts, they block the exact command this plan exists to make meaningful, and both were introduced by earlier plans of this phase running commands narrower than their own gate. Deferring them would have shipped a plan whose whole subject is gate reach while leaving the gate red.

3. **The `fuzz-coverage` and `fuzz-24h` jobs' hardcoded lists were NOT extended.** R-34's requirement is scheduled execution, which the `fuzz` job's matrix delivers. `fuzz-24h` is manual-only and its 6-hours-per-target budget is a CI-cost decision this plan does not own. The consequence — no coverage report for this target — is stated in `deferred-items.md` rather than hidden.

## Deferrals recorded in this SUMMARY rather than in the module header (R-36)

Three compatibility and roadmap facts that do not change how the module behaves for someone using it, and whose presence in the header would make the header stop being read:

1. **Client wrappers `list_skills()` / `get_skill()` / `read_skill_uri()`** (spike gap #7). Additive public API on `Client`, which compiles to wasm32 — a different file entirely, and `Client`-side ownership. Deferred to a later v2.7 phase. Nothing in this module changes if they land; a host today calls the two methods directly over streamable HTTP.

2. **Promoting the constructor-name mismatch (gap 4a) from a warning to a rejection.** 125-03 shipped it as `SkillDiagnostic::NameMismatch`, a warning, because three in-repo constructions and `pmcp-book`'s taught `.with_path("team/topic")` exercise deliberately give a skill a path whose final segment differs from its constructor name. Promotion is a behaviour break that belongs after those surfaces are reconciled — and the hard reject that ROADMAP SC#3 actually requires (gap 4c, the FRONTMATTER name) already ships.

3. **The `resources/read` error-code divergence (D-06 observation).** `SkillsHandler::read` answers an unknown URI with `METHOD_NOT_FOUND` (-32601) at the handler, which the dispatch tail re-wraps so a caller on the wire sees **-32603 carrying -32601 inside the message string** — 125-02's correction to D-06's single-level phrasing. The draft's convention for an unknown resource is an invalid-params code. Changing it is observable behaviour with its own pinning test (`resources_read_unknown_uri_still_returns_method_not_found`), so it belongs to a phase that owns that error contract. `skills/get`'s -32602 is deliberately NOT aligned to it; the two codes reading differently is a decision, and both are pinned.

## Contract-first check (re-confirmed, per Task 3 step 6)

The plan-time finding holds. `../provable-contracts/contracts/` has no `pmcp/` subdirectory (the directory itself does not resolve from this checkout). `grep -rl skills contracts/` returns **zero files**. `make comply` exits 0 with zero `BINDING DRIFT` lines — its fail-closed half, `comply-bindings-check`, is scoped to `contracts/team-servers/binding.yaml` and `crates/pmcp-team-servers/src`, and all four bindings resolve. **No skills contract exists and none is required by any enforced gate.**

The adjacent note is likewise unchanged and deliberately not actioned: `contracts/mcp-protocol-sdk-v1.yaml`'s v2 name-bearing method table still omits both skills methods, which is now recorded in the module header as a decision with its stated cost rather than left implicit.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The `test-skills` feature list needed `testing`, which the plan did not name**

- **Found during:** Task 1
- **Issue:** The plan says to use "`skills` plus the transport features `tests/skills_routing.rs`'s `#![cfg]` header requires", i.e. `streamable-http` and `http-client`. That file also declares `#[path = "common/duplex.rs"] mod duplex;`, and `duplex.rs` gates its entire body on `feature = "testing"`. Without it the harness compiles to nothing and routing tests 17 and 18 — the `ServerCore` boundary proofs this very plan documents in Task 3 — disappear from the run.
- **Fix:** `SKILLS_FEATURES := skills,streamable-http,http-client,testing`, with the three companions and their reasons enumerated in the recipe's header comment so the next reader does not have to rediscover the mapping.
- **Files modified:** `Makefile`
- **Verification:** selector 4 reports 19 tests including `server_core_builder_still_serves_skills_as_resources` and `server_core_declares_no_skills_field_or_skills_method`.
- **Committed in:** `82ef71ac`

**2. [Rule 1 - Bug] Five `private-intra-doc-links` rustdoc errors, surfaced the instant `doc-check` could see the module**

- **Found during:** Task 1 (`make doc-check` verify)
- **Issue:** `make doc-check` exited 101 on its first run after `skills` joined its feature list. 125-03's rustdoc on `Skills::entries` and `Skills::into_handler` links to `[`skill_resource_manifest`]`, `[`validate_names`]` (twice) and `[`exceeds_skill_limits`]` — all crate-private, and `-D warnings` promotes `rustdoc::private_intra_doc_links` to an error. This is the leg's first output and exactly what it exists to catch.
- **Fix:** All five demoted from intra-doc links to plain code spans. A code span is the honest rendering for a private item in public documentation — the link would resolve only under `--document-private-items`, which no published build uses.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `make doc-check` -> exit 0, zero `warning:` lines.
- **Committed in:** `82ef71ac`

**3. [Rule 1 - Bug] The registration test's matrix assertion was satisfied by a prefix substring**

- **Found during:** Task 2 (negative control)
- **Issue:** The first spelling used `workflow.contains("- fuzz_skill_entry")`. Negative control 1 renamed the matrix row to `- fuzz_skill_entryX` and the test **passed** — a prefix substring satisfies `contains`, so a typo'd, renamed or partially-deleted row would have reported green while nothing ran. The test would have been decorative for the exact failure it was written to catch.
- **Fix:** Parse the file's `- `-prefixed rows, trim, and compare for whole-line equality. The failure message prints the rows it saw.
- **Files modified:** `tests/skills_routing.rs`
- **Verification:** all three negative controls now FAIL the test — renamed matrix row, renamed `[[bin]]` stanza, deleted source file — and the positive case passes. Both edited files restored byte-identical (`diff -q`).
- **Committed in:** `37145e71`

**4. [Rule 1 - Bug] The proptest's `valued` framing used Rust's Debug escape, which is not valid YAML**

- **Found during:** Task 2 (first run of the new proptest)
- **Issue:** The framing was `format!("---\nname: fuzzed\nfree: {raw:?}\n---\n")` on the assumption that a Rust Debug string literal is a valid YAML double-quoted scalar. It is not: Rust renders a non-ASCII char as `\u{1e000}` while YAML wants `\U0001E000`. The property failed on `raw = "\u{1e000}"`, and the framing was wrong rather than the parser.
- **Fix:** `serde_json::to_string(&raw)`. YAML 1.2 is a superset of JSON, so a serde_json string literal is always a valid YAML double-quoted scalar. The reasoning and the measured input are recorded at the site so the framing is not "simplified" back.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `prop_entry_synthesis_never_panics_on_arbitrary_bodies` passes with the `prop_assert!` on the framing live.
- **Committed in:** `37145e71`

**5. [Rule 2 - Missing Critical] Both new stable tests would have been vacuous for their most valuable assertions**

- **Found during:** Task 2 (step 4)
- **Issue:** The plan asks the proptest to assert "no panic AND a well-formed digest or a frontmatter diagnostic". Arbitrary text framed as a YAML document is essentially always a scalar, so `NotAMapping` is the outcome and the digest/size/object-shape assertions would run on almost no generated input. The named-shape test has the same hazard: if every named shape happened to take an exclusion path, it would assert only that nothing panicked. This is the defect class 125-03 recorded twice.
- **Fix:** A third generated framing (`valued`) that always yields an entry, `prop_assert!`ed to do so; and a counter across the 12 named shapes with `assert!(produced_entries >= 3)`. The helper returns whether an entry was produced precisely so both anti-vacuity checks are possible.
- **Files modified:** `src/server/skills.rs`
- **Verification:** both tests pass with the anti-vacuity assertions live; the lib selector went 97 -> 99.
- **Committed in:** `37145e71`

**6. [Rule 1 - Bug] `make lint` was RED at HEAD — pedantic `needless_pass_by_value` on 125-02's `build_skills_list_response`**

- **Found during:** Task 3 (`make quality-gate` verify)
- **Issue:** `make quality-gate` failed at its `lint` leg: `build_skills_list_response(id, skills: Vec<Value>, ..)` in `src/server/core.rs` takes an owned `Vec` it only serializes, and `make lint` adds `-W clippy::pedantic`, which makes `needless_pass_by_value` an error under this repo's policy. The function is UNGATED, so `--features "full"` compiles it. 125-01..04 each verified with `cargo clippy --all-targets --all-features -- -D warnings`, which is strictly weaker — CLAUDE.md says so in as many words — and none of them ran `make quality-gate`.
- **Measured as pre-existing rather than assumed:** `src/server/skills.rs` was restored to HEAD (working copy saved and afterwards confirmed byte-identical by `diff -q`) and `make lint` re-run: **exit 2, the identical error at `src/server/core.rs:2495`.** `src/server/skills.rs` is `#[cfg(feature = "skills")]`-gated at `src/server/mod.rs:194` and so is not even compiled by `make lint`, and no commit of this plan touched `core.rs` before this fix.
- **Fix:** Signature changed to `&[Value]`, matching its `build_skills_get_response` sibling, plus the two call sites (`src/server/mod.rs:1736`, and the in-module test). A comment at the parameter records why it changed and why nothing caught it.
- **Files modified:** `src/server/core.rs`, `src/server/mod.rs`
- **Verification:** `make lint` -> exit 0. Full suite unaffected: 139 `test result: ok`, 0 failures.
- **Committed in:** `dbd9d199`

**7. [Rule 3 - Blocking] `make lint-plans` was RED at HEAD — 12 D-19 violations across all five of this phase's plans**

- **Found during:** Task 3 (`make quality-gate` verify)
- **Issue:** `quality-gate`'s FIRST leg failed with 12 RULE-1 violations: `<automated>` verify commands that pipe a build/test invocation into `grep`/`tail` with no `pipefail`, so the pipeline reports the last stage's status and **a failing build reports PASS**. Four in 125-01, five in 125-02, one each in 125-03, 125-04 and 125-05. The lint runs only inside `make quality-gate`; `scripts/lint-plan-verify-commands.sh` is opt-OUT above phase 118, so phase 125 was covered from the moment its plans were authored and nothing ran it.
- **Measured as pre-existing:** `git diff --name-only c8125913..HEAD -- '.planning/phases/125*/125-*-PLAN.md'` returns **zero files**, so no commit since plan creation touched a PLAN.md; the failing state is exactly the authored one.
- **Fix:** Each of the 12 wrapped as `bash -o pipefail -c "..."`, the form the linter's own remediation message prescribes. Only the spelling changed — every command still runs the same thing and still feeds the same `<fails_when>`; what changed is that a failing build can no longer be reported as a pass.
- **Files modified:** all five `125-*-PLAN.md`
- **Verification:** `make lint-plans` -> exit 0, "no verification command masks the exit status of the thing it verifies" over 37 plan files and 1,478 verification lines.
- **Committed in:** `dbd9d199`

**8. [Rule 3 - Blocking] The docs04 stale-binary guard fired again; cleared by the 125-04 remedy**

- **Found during:** Task 3 (full-suite run)
- **Issue:** `cargo test --all-features` aborted at `tests/docs04_examples_run.rs` with the STALE assertion for `doc_review_team` and `s50_standalone_vs_sampled` — the exact hazard 125-04 recorded, triggered by this plan's edits to `src/server/skills.rs`, a file neither example's crate compiles.
- **Fix:** `touch`ed the two examples' own sources and rebuilt with `-p <owning-package>`. `git status --porcelain` on both -> empty, so this is a rebuild and not a change that hides real staleness.
- **Files modified:** none
- **Verification:** `cargo test --all-features -- --test-threads=1` -> exit 0, **139** `test result: ok`, 0 FAILED, 0 `failures:` lines, doctests reached (479 passed).
- **Committed in:** n/a (a build action)

---

**Total deviations:** 8 auto-fixed (4 x Rule 1 bug, 3 x Rule 3 blocking, 1 x Rule 2 missing-critical).

**Impact on plan:** No scope creep in the sense that matters — every file touched outside `files_modified` (`src/server/core.rs`, `src/server/mod.rs`, four sibling PLAN.md files) was touched to make `make quality-gate` exit 0, which is Task 3's own stated verification. Five of the eight deviations are the plan's own gate finding real defects the moment it could see them, which is the plan working rather than the plan being wrong. The three that are about this plan's own new code (deviations 3, 4, 5) all make the result stronger than the literal text: a source-scan gate that cannot be fooled by a prefix, a property whose framing actually reaches the parser, and two tests that refuse to pass while measuring nothing.

## Issues Encountered

**Four measurement hazards, all handled; no unresolved code defect.**

1. **A gate that has never run is not a gate that passes.** Two of `quality-gate`'s legs were red before this plan, both from phase 125's own work, and the phase had shipped four plans without noticing. Recorded in `deferred-items.md` as a process finding, because the lesson generalises past this phase: verifying with a narrower command than the gate uses is how a gate goes stale, and it is the same shape one level up from the `--features "full"` hole this plan closed.

2. **`cargo test` aborts after the FIRST failing target,** so every intermediate count in this session was treated as a lower bound. The stale-binary run reported 12 `test result: ok` lines; the clean run reported 139.

3. **The docs04 stale-binary guard fired again** for the same two `crates/*/examples/` binaries and the same reason (mtime compared against root `src/` even though those crates' `pmcp` dep does not enable `skills`). The 125-04 remedy — touch the example's OWN source, then `cargo build -p <owning-package>` — worked unchanged.

4. **Every `grep`, `sed`, `awk`, `make`, `cat` and `cargo` invocation was run through an absolute binary path** (`/usr/bin/grep`, `/usr/bin/sed`, `/usr/bin/awk`, `/usr/bin/make`, `/bin/cat`, `/Users/guy/.cargo/bin/cargo`), per the `rtk`-proxy hazard 125-01..04 each recorded. This mattered most for the negative controls, which are exit-code and message claims. No corruption was observed, which is the expected outcome of avoiding the proxy rather than evidence it is fixed.

## Known Stubs

**None introduced.**

Two boundaries worth naming as boundaries, both recorded in `deferred-items.md` and in `.planning/WINDOWS.md`:

1. **`fuzz_skill_entry` is scheduled but not coverage-measured.** The `fuzz` job's matrix runs it; the `fuzz-coverage` and `fuzz-24h` jobs carry their own hardcoded four-name loops that were deliberately not extended. See the deferral note for why, and for the suggested fix (make both loops read the matrix).

2. **`make book-test` remains RED repo-wide** — 26 chapters, `mdbook` not linking the `pmcp` rlib. Pre-existing, measured identical to its HEAD baseline by 125-04, and deliberately NOT chained into `quality-gate` by this plan, per that plan's explicit hand-off.

**Residual on the fuzz evidence, stated plainly:** the matrix row means the target runs on the daily schedule and on PRs touching `src/**` or `fuzz/**`, so the first CI campaign result arrives with the next such run. This plan's own evidence at merge time is the registration test, the stable proptest, and a **local** 61-second campaign of 520,501 executions with zero crashes. Claiming the target is "fuzzed in CI" as of this commit would overstate what has been observed.

## Threat Flags

None. Every trust boundary this plan crosses is in the plan's own `<threat_model>`, and each disposition landed as written:

- **T-125-19** (hostile bytes -> `serde_yaml`) — *mitigate*, landed and MEASURED. `fuzz_skill_entry` drives arbitrary bytes with no unwrap on any input-derived `Result`, and the stable proptest plus the 12 named shapes cover the malformed cases a generator will not reach. 520,501 executions, zero crashes. No new dependency: `serde_yaml` and its transitive `unsafe-libyaml` were already in the graph, and `cargo audit` (run inside `quality-gate`) names neither.
- **T-125-20** (a gate leg reporting green having run zero tests) — *mitigate*, landed, and the mitigation's ADEQUACY is measured: the per-selector guard was proven necessary by a negative control in which the summed alternative would have passed.
- **T-125-21** (capability declared but not honoured on a transport) — *mitigate*, landed as documentation, which is the only lever available. The declaration is computed before a transport is chosen, so it cannot be made conditional here; `set_skills_capabilities`'s rustdoc states the reach, what a stdio operator should expect, and the two changes that would close it.
- **T-125-22** (a deferral recorded as a code marker) — *mitigate*, landed. `grep -cE 'TODO|FIXME|HACK|XXX' src/server/skills.rs` -> 0; `make check-todos` -> exit 0.
- **T-125-SC** — *accept*, unchanged. This plan installs no package.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Phase 125 is complete: 5 plans, 5 summaries.**

- **The gate now sees this module and is GREEN.** `make quality-gate` -> exit 0, with `test-skills` reporting 99 / 9 / 13 / 19 across its four selectors. `cargo test --all-features -- --test-threads=1` -> exit 0, 139 targets ok. This is the first time in the phase that the repo's own gate has passed over the skills work.
- **What the next skills phase (v2.7) inherits, all recorded with owners in `src/server/skills.rs`'s header:** stdio (and every non-HTTP) transport reach; the `ServerCore` method reach, gated on the same `ProtocolHandler` ingress change; `resources/directory/read`; a strict frontmatter mode; cursor pagination. Plus the three compatibility items in this SUMMARY: client wrappers, the gap-4a promotion, and the `resources/read` error-code divergence.
- **The two changes that would widen the reach are named, not just wished for:** either give `ProtocolHandler` an ingress that accepts an internally-routed request (non-semver-breaking, and it closes the stdio and `ServerCore` deferrals together), or defer capability resolution until a transport is known. A widener starts at `ProtocolHandler::handle_request`'s signature, and `server_core_declares_no_skills_field_or_skills_method` will tell them if they add delegates first.
- **A standing warning for the next phase:** verify with `make quality-gate`, not with `cargo clippy --all-targets --all-features -- -D warnings`. The latter is strictly weaker and this phase shipped a pedantic violation past it four times.
- **`requirements mark-complete D-01 D-09 D-10 D-11` is expected to be a no-op** — those are 125-CONTEXT decision IDs, not `REQUIREMENTS.md` requirement IDs (the v2.6 set is PKG-01..PKGR-01). They are recorded in this SUMMARY's `requirements-completed` exactly as 125-01..04 recorded theirs.

## Self-Check: PASSED

- All 13 files exist on disk and carry the changes: 882 insertions / 23 deletions across the three commits.
- `git log --oneline --all | grep 82ef71ac` -> found (Task 1, feat).
- `git log --oneline --all | grep 37145e71` -> found (Task 2, feat).
- `git log --oneline --all | grep dbd9d199` -> found (Task 3, docs).
- Plan `<verification>` re-run at close: `make quality-gate` -> **exit 0**, and its output carries the four `✓ selector` lines with nonzero counts; `cargo test --all-features -- --test-threads=1` -> **exit 0**, 139 `test result: ok`, 0 FAILED, 0 `failures:`; `cargo clippy --all-targets --all-features -- -D warnings` -> clean; `make lint` -> exit 0; `make doc-check` -> exit 0 with `skills` in its list, 0 `warning:` lines; `make check-todos` -> exit 0; `make comply` -> exit 0, 0 `BINDING DRIFT`; `cargo fmt --all -- --check` -> clean.
- Task 1 acceptance: `grep -c '^test-skills:' Makefile` -> **1**; `grep -c 'test-skills' Makefile` -> **5** (>= 4); recipe `-eq 0` count -> **4**; recipe `--all-features` count -> **0**; `--test-threads=1` count -> **4**; `test-unit` / `test-integration` / `test(` counts in the recipe -> **0 / 0 / 0**; `doc --no-deps -A 2` shows `skills` in the feature list; `^full` and `^full-v2` in `Cargo.toml` carry no `skills` token; `v1_severability_tripwire` -> 18 passed.
- Task 1 negative control: `make test-skills` with selector 3 emptied -> **exit 2**, message names selector 3; Makefile restored byte-identical.
- Task 2 acceptance: `fuzz/fuzz_targets/fuzz_skill_entry.rs` exists with 1 `fuzz_target!`; `grep -c 'fuzz_skill_entry' fuzz/Cargo.toml` -> **4** (>= 2); `.github/workflows/fuzz.yml` -> **1**; `cargo +nightly fuzz build fuzz_skill_entry` -> exit 0; `cargo +nightly fuzz run ... -max_total_time=60` -> 520,501 runs, 0 crashes; three negative controls all FAIL the registration test, all files restored byte-identical.
- Task 3 acceptance: module header carries the transport-reach section (with the measured stdio loop-break), the `ServerCore` boundary (naming the typed ingress and the precondition), a **five-row** deferral table, and the name-bearing non-change; `sed -n '/fn set_skills_capabilities/,/^}/p' src/server/skills.rs | grep -c 'json!({})'` -> **1**; `grep -cE 'TODO|FIXME|HACK|XXX' src/server/skills.rs` -> **0**.
- Doctest mirror rule (from `src/server/skills.rs:18`) re-verified after all edits: both blocks extracted and diffed -> **23 lines each, EMPTY diff**.
- Plan `<success_criteria>`: all four met — the gate compiles, rustdoc-lints and runs this module's tests and fails on zero tests ✅; `full` / `full-v2` untouched and the tripwire passes ✅; a fuzz target exists, is registered, is scheduled, and its registration is proven without nightly ✅; every deferral is in rustdoc with an owner and no SATD marker exists ✅.

---
*Phase: 125-sep-2640-conformance-skills-list-skills-get*
*Completed: 2026-09-02*
