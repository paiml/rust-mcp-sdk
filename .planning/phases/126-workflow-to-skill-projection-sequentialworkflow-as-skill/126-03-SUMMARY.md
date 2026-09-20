---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
plan: 03
subsystem: api
tags: [sep-2640, skills, workflow, projection, fuzzing, wire-tests, sc-4, always-fuzz]

# Dependency graph
requires:
  - phase: 126-01
    provides: "`SequentialWorkflow::as_skill()`, the `yaml_double_quoted` frontmatter encoder, and SC-4's IN-PROCESS half"
  - phase: 125-sep-2640-conformance-skills-list-skills-get
    provides: "the `skills/list` route, `skill_resource_manifest`, `validate_names`, and `tests/skills_routing.rs`'s loopback `StreamableHttpServer` fixtures (`skills_fixture_server`, `post_v2_skills_list`, `spawn_default_config`)"
provides:
  - "SC-4 on the WIRE: a projected skill's `skills/list` entry conformance and `resources/read` byte-identity, asserted off the JSON a remote caller receives"
  - "wire-level verification of REVIEWS finding 1 / T-126-21, mutation-proven red"
  - "`fuzz/fuzz_targets/fuzz_workflow_projection.rs` — the CLAUDE.md ALWAYS/FUZZ target for `as_skill()`, with the finding-3 byte-chunk-first harness shape"
  - "`fuzz_workflow_projection_is_registered_and_scheduled` — a registration tripwire proven red on all three of its legs"
  - "the `[[bin]]` stanza and the `.github/workflows/fuzz.yml` matrix row that make the target actually run"
affects:
  - 126-04
  - 126-05
  - 126-06
  - 126-07

# Actuals (#2632) — estimateTokens scale (chars/4 over the realized diff), NOT a harness token count.
actuals:
  tokens: 8836
  tasks: 2
  commits: 2

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Mutation-proof over assertion-count: every new gate in this plan was verified by BREAKING the thing it guards and observing red, not by observing green. The adversarial wire test was checked against a `render_frontmatter` reverted to raw concatenation; the fuzz tripwire was checked against three independent breakages (source hidden, `[[bin]]` name renamed, matrix row suffixed)."
    - "Byte-chunk-first fuzz harness: split the ORIGINAL `&[u8]` with `split_at`, then `from_utf8_lossy` each chunk independently. Decoding once and range-slicing the result panics on a non-char-boundary index and the crash is misattributed to the code under test."
    - "Anti-vacuity BEFORE structure: the wire helper asserts a >200-byte body and a one-element `skills` array before any conformance clause runs, so an empty or error response cannot satisfy the clauses by having nothing to check. This is what actually caught the mutation — the raw-concat frontmatter made the entry disappear from `skills/list` entirely rather than ship malformed."

key-files:
  created:
    - fuzz/fuzz_targets/fuzz_workflow_projection.rs
  modified:
    - tests/skills_routing.rs
    - fuzz/Cargo.toml
    - .github/workflows/fuzz.yml

key-decisions:
  - "SC-4 wire clause 3 compares the frontmatter `name` against the segment BEFORE the trailing `/SKILL.md`, not the literal final URI segment — the plan's wording would have compared `refund-flow` against `SKILL.md` and failed on a correct implementation."
  - "The fuzz target chunks the ORIGINAL byte slice with `split_at` and lossy-decodes each chunk independently; it never range-slices a decoded `String` (REVIEWS finding 3 / T-126-22)."
  - "`make test-fuzz` is REJECTED as fuzz evidence for this phase. The registration tripwire plus a hand-run under `cargo +nightly` is the evidence, and the tripwire was proven red on all three legs."
  - "The fuzz target's header DESCRIBES the forbidden slicing expression instead of writing it out, because an acceptance gate greps this file for the literal syntax (same collision 126-01 hit with `with_path`)."

patterns-established:
  - "A wire test's failure message names the SC clause it covers, so a red run reports which part of the contract broke rather than which line."
  - "A registration tripwire's third leg (the CI matrix row) uses SCOPED whole-line equality, not `contains` — a prefix substring passes `contains` against a typo'd row while nothing runs. Verified here: a `- fuzz_workflow_projectionX` row goes red."

requirements-completed: [SC-4]

coverage:
  - id: D1
    description: "SC-4 (wire half) — a projected skill served by a real loopback `StreamableHttpServer` answers `skills/list` with a single `skill://refund-flow/SKILL.md` entry carrying EXACTLY two frontmatter keys with verbatim values, a `sha256:` + 64-lowercase-hex digest checked character by character, and a `size` equal to the served body's byte length."
    requirement: "SC-4"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#projected_workflow_skill_is_conforming_on_the_wire"
        status: pass
    human_judgment: false
  - id: D2
    description: "SC-4 (wire byte-identity) — `resources/read` of `skill://refund-flow/SKILL.md` over the same loopback server returns text byte-equal to `skill.body()`, the exact bytes the published digest covers (D-05's one-body invariant, stated where a consumer can see it)."
    requirement: "SC-4"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#projected_workflow_skill_is_conforming_on_the_wire (clause 5)"
        status: pass
      - kind: integration
        ref: "tests/skills_routing.rs#projected_workflow_skill_survives_an_adversarial_description_on_the_wire (clause 5)"
        status: pass
    human_judgment: false
  - id: D3
    description: "REVIEWS finding 1 / T-126-21 observed from OUTSIDE the SDK — a workflow description carrying `: `, `#` AND an embedded newline still yields a two-key frontmatter whose `description` equals the original verbatim (newline included) and whose `name` is present and matches the entry URI's skill segment. MUTATION-PROVEN: reverting `render_frontmatter` to raw concatenation turns this test red."
    requirement: "SC-4"
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#projected_workflow_skill_survives_an_adversarial_description_on_the_wire"
        status: pass
      - kind: other
        ref: "mutation check: `render_frontmatter` with `slug`/`description` in place of `yaml_double_quoted(...)` -> FAILED (`skills` array empty), plain-description test still ok"
        status: pass
    human_judgment: false
  - id: D4
    description: "CLAUDE.md ALWAYS/FUZZ (registration) — the fuzz target source exists, declares `fuzz_target!`, is registered by a `[[bin]]` stanza whose `path` canonicalizes to that file, and is a whole-line row in `.github/workflows/fuzz.yml`'s matrix. Asserted by a normal `#[test]` because `make test-fuzz` exits 0 having run nothing on stable."
    verification:
      - kind: integration
        ref: "tests/skills_routing.rs#fuzz_workflow_projection_is_registered_and_scheduled"
        status: pass
      - kind: other
        ref: "red-proof leg 1: source moved aside -> FAILED (`the fuzz target source is missing at ...`)"
        status: pass
      - kind: other
        ref: "red-proof leg 2: `[[bin]]` name renamed -> FAILED (`no [[bin]] stanza with name = \"fuzz_workflow_projection\"`)"
        status: pass
      - kind: other
        ref: "red-proof leg 3: matrix row suffixed `X` -> FAILED (`is NOT a row in .github/workflows/fuzz.yml's target matrix`)"
        status: pass
    human_judgment: false
  - id: D5
    description: "CLAUDE.md ALWAYS/FUZZ (execution) — `as_skill()` does not panic on arbitrary bytes across four author-text positions, and the four other invariants (slug legality, one trailing newline, byte-equal determinism, intact four-line frontmatter) hold over a 200,000-run campaign."
    verification:
      - kind: other
        ref: "cargo +nightly fuzz build fuzz_workflow_projection -> Finished `release` profile, exit 0"
        status: pass
      - kind: other
        ref: "cargo +nightly fuzz run fuzz_workflow_projection -- -runs=200000 -max_total_time=30 -> `Done 200000 runs in 19 second(s)`, exit 0, zero crash artifacts"
        status: pass
    human_judgment: false
  - id: D6
    description: "Project quality gates unchanged — PMAT complexity at the zero-violation baseline, `make lint-skills` warning-free under pedantic+nursery (which lints `--lib --tests`, so it reaches the new integration tests), workspace `cargo fmt --check` clean, zero SATD, and all four `make test-skills` selectors green with non-zero counts."
    verification:
      - kind: other
        ref: "pmat quality-gate --fail-on-violation --checks complexity -> Total violations: 0"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make lint-skills -> exit 0, zero warnings"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-skills -> exit 0; selectors ran 152 / 9 / 15 / 23"
        status: pass
      - kind: other
        ref: "cargo fmt --all -- --check -> exit 0; make check-todos -> No technical debt comments"
        status: pass
    human_judgment: false

# Metrics
duration: 27 min
completed: 2026-09-04
status: complete
---

# Phase 126 Plan 03: SC-4 on the wire and the ALWAYS/FUZZ tripwire Summary

**A projected workflow skill now proves SEP-2640 conformance on a loopback socket — two-key verbatim frontmatter, a character-by-character-validated `sha256:`+64-hex manifest, and a `resources/read` payload byte-equal to `skill.body()` — and `as_skill()` gains a fuzz target that is registered, CI-scheduled, hand-executed for 200,000 runs, and pinned by a tripwire proven red on every one of its three legs.**

## Performance

- **Duration:** 27 min
- **Started:** 2026-09-04T16:26:00Z
- **Completed:** 2026-09-04T16:53:00Z
- **Tasks:** 2 executed
- **Files modified:** 4 (1 created, 3 modified)

## Accomplishments

- **SC-4's wire half holds.** Two `#[tokio::test]`s stand up the existing `skills_fixture_server` loopback `StreamableHttpServer` with a registry containing a `SequentialWorkflow::as_skill()` result and assert five clauses off the WIRE JSON: entry identity (`skill://refund-flow/SKILL.md`), a frontmatter object with **exactly two** keys and both values verbatim (D-13), the frontmatter `name` equal to the entry URI's skill segment (SC-1 seen from outside the SDK), a complete manifest whose digest is `sha256:` plus 64 characters each drawn from `0123456789abcdef` and whose `size` equals the served body's byte length, and a `resources/read` payload byte-equal to `skill.body()`. `tests/skills_routing.rs` moved 20 → 23 tests.

- **REVIEWS finding 1 is now falsifiable from outside the SDK, and was proven so.** The adversarial test's description carries all three YAML hazards at once — `: `, `#`, and an embedded newline followed by `metadata: injected`. To confirm the test is not decorative, `render_frontmatter` was temporarily reverted to the raw `format!("...name: {}\ndescription: {}...", slug, description)` shape the original plan had specified. The adversarial test went **red** and the plain-description test stayed **green**. The failure mode observed is worth recording because it is *not* the obvious one: the malformed frontmatter did not ship a third key, it made the entry **disappear from `skills/list` entirely** (`"skills":[]`), caught by the anti-vacuity clause rather than by any of the conformance clauses. That is `build_artifact_inner`'s diagnostic-downgrade path in action, and it is exactly why the anti-vacuity assertions run first.

- **The ALWAYS/FUZZ requirement is discharged with an artefact that can be red.** `fuzz/fuzz_targets/fuzz_workflow_projection.rs` drives four author-text positions (workflow name, description, step name, step guidance) and asserts five invariants as real assertions: no panic; a legal `[a-z0-9-]` slug that is non-empty, at most 64 characters and unhyphenated at both ends; exactly one trailing newline (SC-5's precondition); **byte-equal determinism** across two independent renders (the invariant a unit test is least likely to catch, since the nondeterministic accessor varies per process); and an **intact four-line frontmatter block** (finding 1 as a fuzzable property — an unescaped newline splits the `description:` line and the fourth line stops being `---`).

- **The finding-3 harness correction is implemented and pinned in prose.** The target splits the ORIGINAL `&[u8]` into four chunks with `split_at` and calls `String::from_utf8_lossy` on each chunk independently. It never range-slices a decoded `String`. `split_at` was chosen over range indexing specifically so no computed integer indexes anything text-shaped at all, and the module header records *why* a later editor must not collapse the four decodes into one.

- **Registration, scheduling and execution are all separately proven.** `fuzz_workflow_projection_is_registered_and_scheduled` keeps all five checks of its Phase-125 sibling and was verified red on each leg independently:

  | Leg broken | Result |
  |---|---|
  | source file moved aside | FAILED — `the fuzz target source is missing at …` |
  | `[[bin]]` `name` renamed to `…_typo` | FAILED — `no [[bin]] stanza with name = "fuzz_workflow_projection"` |
  | matrix row changed to `- fuzz_workflow_projectionX` | FAILED — `is NOT a row in .github/workflows/fuzz.yml's target matrix` |

  The third leg also demonstrates the whole-line-equality choice paying off: a `contains("- fuzz_workflow_projection")` check would have passed against that `X`-suffixed row.

- **The fuzzer was actually run, under nightly, by hand.** Verbatim stdout tail:

  ```
  INFO: Running with entropic power schedule (0xFF, 100).
  INFO: Seed: 133199897
  INFO: Loaded 1 modules   (1023334 inline 8-bit counters)
  INFO:      291 files found in .../fuzz/corpus/fuzz_workflow_projection
  INFO: seed corpus: files: 291 min: 1b max: 443b total: 16934b rss: 78Mb
  #292      INITED cov: 697 ft: 1346 corp: 242/14920b exec/s: 0 rss: 91Mb
  #200000   DONE   cov: 697 ft: 1367 corp: 251/16Kb lim: 1968 exec/s: 10526 rss: 537Mb
  Done 200000 runs in 19 second(s)
  ```

  Exit 0, zero crash artifacts in `fuzz/artifacts/fuzz_workflow_projection/`. The corpus directory it grew is covered by `fuzz/.gitignore:7` (`corpus/*`), confirmed with `git check-ignore -v`.

## Task Commits

1. **Task 1: SC-4 on the wire** — `32feadf2` (test)
2. **Task 2: the fuzz target, its registration, and the tripwire** — `8dd1d6ec` (test)

## Files Created/Modified

- `fuzz/fuzz_targets/fuzz_workflow_projection.rs` — **created**, 247 lines. Header carries the `cargo +nightly fuzz run` command with the reason the prefix is mandatory, a `# Why THIS boundary` section, the four-chunk ordering rationale, the numbered `# Invariants` list, and seed-corpus suggestions. Body is `four_lossy_fields` → `workflow_from` → `check_projection`, with the `fuzz_target!` driving two shapes (natural positions, then the same chunks rotated one slot so a short name-slot chunk also reaches the description encoder).
- `tests/skills_routing.rs` — **+438 lines**: the two SC-4 wire tests plus the shared `assert_sc4_holds_on_the_wire` helper, `projected_workflow`, `is_lowercase_hex_64`, `skill_name_segment_of`, and the `fuzz_workflow_projection_is_registered_and_scheduled` tripwire. The file's own property tables were extended so the new tests are discoverable from the header.
- `fuzz/Cargo.toml` — **+22 lines**, entirely one `[[bin]]` stanza and its justification comment. `git diff -U0` confirms zero removals and zero `[dependencies]` changes; `:60` already enabled `skills` on the `pmcp` dep.
- `.github/workflows/fuzz.yml` — **+6 lines**: the `- fuzz_workflow_projection` matrix row and its comment.

## Decisions Made

- **Clause 3 measures the right segment.** See the deviation below; this is the one place the plan's literal wording could not be implemented as written.
- **`split_at` over range indexing in the fuzz harness.** Functionally equivalent to four explicit byte ranges, but it removes every `[a..b]` form from the file, which makes the "no computed indexing of text" property greppable rather than a matter of reading.
- **`make test-fuzz` is not evidence, and the target's own header says so.** The header states the stable-toolchain failure (`the option 'Z' is only accepted on the nightly compiler`) and the `|| echo` that swallows it, so the next operator does not reach for the Makefile target and conclude the campaign ran.
- **Tests live in `skills_routing.rs`, not a new file.** A new integration target matches none of `make test-skills`' four selectors and would need a fifth guarded selector; the plan's prohibition was honoured and verified (`tests/skills_projection*.rs` — no matches).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The plan's SC-4 clause 3 named the wrong URI segment**

- **Found during:** Task 1
- **Issue:** The plan specified "The final path segment of `entry.uri()` equals the frontmatter `name`." The entry URI is `skill://refund-flow/SKILL.md`, whose literal final segment is `SKILL.md`. The identity `validate_names` actually enforces is between the frontmatter `name` and `final_path_segment(skill.resolved_path())` (`src/server/skills/mod.rs:1243,1459`) — the segment *before* the trailing `/SKILL.md`. Implemented literally, the assertion would have compared `refund-flow` against `SKILL.md` and failed against a correct implementation.
- **Fix:** A named `skill_name_segment_of` helper that strips the `/SKILL.md` suffix (panicking with a clear message if the suffix is absent, which is itself a conformance statement about the entry URI shape) and then takes the last segment. Its rustdoc records why `rsplit('/').next()` alone is wrong, so the next reader does not "simplify" it back.
- **Files modified:** `tests/skills_routing.rs`
- **Verification:** `projected_workflow_skill_is_conforming_on_the_wire` and its adversarial twin both pass; the helper's panic message would fire on any entry URI not ending in `/SKILL.md`.
- **Committed in:** `32feadf2`

**2. [Rule 1 - Bug] The fuzz target's header defeated its own acceptance gate**

- **Found during:** Task 2 (acceptance-criteria verification loop)
- **Issue:** Two acceptance criteria collide. One requires the header to state *why* the byte-chunk-first order is mandatory; the other requires that the file "contains no `&s[..` / `&s[a..b]` indexing of a `String` or `&str` by a computed integer". Writing the panicking expression out verbatim in the explanation satisfied the first and made a naive grep for the second return 1. This is the same class as plan 126-01's deviation 2, where a comment containing `with_path` broke a `grep -c 'with_path' == 0` gate.
- **Fix:** The header now *describes* the offending expression ("range-slicing a decoded `String` at its own `len() / 4`") and states parenthetically that it is described rather than quoted precisely so the gate reads honestly. The mechanism — three-byte `U+FFFD`, non-char-boundary index, `byte index N is not a char boundary` — is fully preserved.
- **Files modified:** `fuzz/fuzz_targets/fuzz_workflow_projection.rs`
- **Verification:** `grep -nE '&[A-Za-z_]+\[' fuzz/fuzz_targets/fuzz_workflow_projection.rs` returns zero lines (rc=1); `grep -c 'char boundary'` returns 2. Rebuilt and re-ran the 200,000-run campaign after the edit.
- **Committed in:** `8dd1d6ec`

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** None on scope. Both are inside files the plan already assigned, and neither weakens a gate — deviation 1 makes an assertion *correct* where the plan's wording was unimplementable, and deviation 2 preserves the required explanation while letting the required grep read honestly.

## Authentication Gates

None — this plan touched no authenticated service.

## Known Stubs

None. No placeholder values, no `TODO`/`FIXME` markers (`make check-todos`: *No technical debt comments*), and no skipped or ignored tests were added.

## Issues Encountered

- **`rtk` corrupted command output twice, both times in ways that would have produced a false reading.** `grep` over `/tmp/test-skills.log` returned no matches for `test result:` in a file that plainly contained them, and the log itself was truncated to 4,703 bytes where the real output was 18,356. Re-running through `/usr/bin/make` and `/usr/bin/grep` gave the true four-selector counts (152 / 9 / 15 / 23). This is the recorded project hazard; every number in this SUMMARY was read through an absolute binary path.
- **No git hooks are installed** in this checkout (`.git/hooks/` holds only samples), so CLAUDE.md's pre-commit quality gate does not fire. Every gate was run manually before each commit: `cargo fmt --all --check`, `make lint-skills`, `make check-todos`, `make test-skills`, and `pmat quality-gate --checks complexity`. Same finding as plan 126-01 — still worth the phase owner's attention, since a contributor relying on the hook gets no enforcement here.
- **The fuzz crate has a pre-existing `cargo fmt` diff** in `fuzz/fuzz_targets/content_tolerant_reader.rs`. Out of scope (the `fuzz/` crate is in the root workspace's `exclude` array and is not fmt-gated by any make target); the new target itself is `rustfmt --check` clean. Not fixed, per the scope boundary.

## Next Phase Readiness

- **SC-4 is now proven on both halves** — in-process (126-01, `tests/skills_integration.rs`) and on the wire (here). Plan **126-06** can record the D-14 golden against bytes whose digest is already demonstrated to be the digest a remote caller receives.
- **Plan 126-04** inherits an unchanged `projection::project_with_notices` seam; this plan touched no `src/` file at all, so the `ProjectionOutput` work starts from exactly the tree 126-02 left.
- **Plan 126-06/07** should note in the CHANGELOG that the fuzz target is CI-scheduled, not merely registered.
- **No blockers.** Both HIGH threats in this plan's register are closed: T-126-02 by the executed campaign plus the tripwire, T-126-21 by the mutation-proven adversarial wire test.

## Self-Check: PASSED

- `fuzz/fuzz_targets/fuzz_workflow_projection.rs` present on disk; `tests/skills_routing.rs`, `fuzz/Cargo.toml`, `.github/workflows/fuzz.yml` all modified and committed.
- Both commit hashes resolve in `git log --oneline --all`: `32feadf2`, `8dd1d6ec`. Neither commit deleted a tracked file (`git diff --diff-filter=D HEAD~1 HEAD` empty for both).
- All plan-level `<verification>` commands re-run green after the final task commit: `--test skills_routing` **23 passed** (baseline 20, +3, strictly greater as required); `make test-skills` exit 0 with all four selector guards reporting non-zero (152 / 9 / 15 / 23); `cargo +nightly fuzz build fuzz_workflow_projection` exit 0; `cargo +nightly fuzz run … -runs=200000 -max_total_time=30` → `Done 200000 runs in 19 second(s)`, exit 0.
- All four prohibitions hold: no `tests/skills_projection*.rs` exists; `make test-fuzz` was never cited as evidence; `grep -c nextest tests/skills_routing.rs` returns 0; `git diff -U0 fuzz/Cargo.toml` shows 22 insertions, 0 deletions, and no `[dependencies]` change.
