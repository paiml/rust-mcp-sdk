---
phase: 119-documentation-three-shapes-v2-migration
plan: 06
subsystem: documentation
tags: [mdbook, pmcp-course, agents, teams, pmcp-agent, pmcp-team-servers, cargo-pmcp, quiz]

# Dependency graph
requires:
  - phase: 119-02
    provides: "pmcp-book/src/ch12-15-agents-as-mcp-clients.md (the book chapter this course chapter complements), the first green leg of tests/docs04_examples_run.rs, and the MEASURED create-missing asymmetry + mdbook-exercises theme side effect this plan depends on"
  - phase: 119-04
    provides: "tests/docs04_examples_run.rs complete at three legs — s50_standalone_vs_sampled, s49_sampling_host, doc_review_team — so the exercises' pass predicates could be cross-checked against asserted strings rather than guessed, plus the `--features runtime` canonical-invocation decision"
  - phase: 110
    provides: "The cargo pmcp agent new|dev and team dev verbs this chapter leads with"
provides:
  - "pmcp-course/src/part8-advanced/ch24-agents-and-teams.md — the Part VIII Agents & Teams chapter (462 lines, ch23-skills depth)"
  - "pmcp-course/src/part8-advanced/ch24-exercises.md — three tiered exercises with predicates sourced from measured output (211 lines)"
  - "pmcp-course/src/quizzes/ch24-agents-and-teams.toml — a 7-question quiz behind a live {{#quiz}} include"
  - "A Part VIII SUMMARY group in the course's own nav conventions"
  - "CONFIRMED BY EXECUTION: the course build renders a new chapter and leaves no unexpected file under pmcp-course/src"
affects: [119-10]

actuals:
  tokens: 10025   # chars/4 over the realized diff (~40,100 chars across 4 files)
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Course chapter shape: H1 thesis → Learning Objectives → Why-it-matters → ONE load-bearing design property with its own ## section → Tier 1/2/3 → Cross-SDK → Chapter Contents → Knowledge Check → quiz include → rule + ./-prefixed forward pointer"
    - "Exercise pass predicates are sourced from the strings an automated test already asserts, so a doc and its test cannot disagree"
    - "Course reachability is asserted with test -f, never with mdbook build"

key-files:
  created:
    - pmcp-course/src/part8-advanced/ch24-agents-and-teams.md
    - pmcp-course/src/part8-advanced/ch24-exercises.md
    - pmcp-course/src/quizzes/ch24-agents-and-teams.toml
  modified:
    - pmcp-course/src/SUMMARY.md

key-decisions:
  - "QUIZ DECISION: ship BOTH the {{#quiz}} include and the TOML. The asymmetry decides it — a missing TOML behind a live include is a build risk under the CI-pinned mdbook-quiz 0.4.0, while an omitted include is not. Shipping both matches ch23, this chapter's structural sibling. Verified by observing the quiz render into the built HTML, not merely by the build exiting 0."
  - "requirements-completed is EMPTY. DOCS-04 requires book chapters AND runnable examples AND README AND course; this plan is the course quarter only, and 119-07 (book) and 119-09 (README) were still unmerged when this ran. Booking DOCS-04 here would credit three other plans' work."
  - "The load-bearing design property named for this chapter is the SOURCE-AGNOSTIC LOOP — the same AgentEngine over the same ResolvedAgentConfig runs against any CompletionSource with no loop change. It is what s50_standalone_vs_sampled demonstrates and what the closing banner asserts."
  - "The doc_review_team example is cited ONLY as `--features runtime` (119-04's canonical form). The literal string `doc_review_team --all-features` appears nowhere in either new file, so the docs cannot disagree with the tests."
  - "No v2 migration content was written for the course (D-09): DOCS-04 names README/course, DOCS-05 does not mention the course, and that distinction was honoured literally."

patterns-established:
  - "A course exercise whose subject is 'both halves ran' must verify BOTH halves, not just the summary banner — the banner is the claim, the section headers are the evidence"
  - "Cross-book references from the course use GitHub blob URLs rather than ../../pmcp-book/ relative paths, so a course build can never be induced to reach outside its own src tree"

requirements-completed: []

coverage:
  - id: D1
    description: "A Part VIII course chapter for Agents & Teams exists at ch23-skills depth, with Learning Objectives, a three-tier progression, a named load-bearing design property, and a Knowledge Check"
    requirement: "DOCS-04"
    verification:
      - kind: other
        ref: "test -f pmcp-course/src/part8-advanced/ch24-agents-and-teams.md; wc -l = 462 (>= 400); grep -c '^## Learning Objectives' = 1; '^## Tier 1'/'^## Tier 2'/'^## Tier 3' = 1 each; '^## Knowledge Check' = 1"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every example the course cites is named by its FULL runnable cargo invocation, and only the three real cargo pmcp verbs appear"
    requirement: "DOCS-04"
    verification:
      - kind: other
        ref: "grep -ohE 'cargo pmcp [a-z-]+ [a-z-]+' | sort -u → exactly {agent new, agent dev, team dev}; s50 full invocation count 2; doc_review_team --features runtime count 2; 'doc_review_team --all-features' count 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "Three tiered exercises ship with escalating difficulty and a pass predicate each, every predicate naming a string the corresponding example genuinely prints"
    requirement: "DOCS-04"
    verification:
      - kind: other
        ref: "3x '^## Exercise ', 3x '^### Verify your solution', 3x '**Difficulty:**'; Ex2 predicate carries the exact S50_BANNER and Ex3 the exact DOC_REVIEW_TEAM_BANNER from tests/docs04_examples_run.rs"
        status: pass
    human_judgment: false
  - id: D4
    description: "The Part VIII nav carries the ch24 group in the course's own conventions and both new chapters are provably reachable"
    requirement: "DOCS-04"
    verification:
      - kind: other
        ref: "SUMMARY lines 182/183 with ./part8-advanced/ prefixes, positioned after ch23-exercises (180) and before '# Appendices' (187); test -f on both chapter paths"
        status: pass
    human_judgment: false
  - id: D5
    description: "The course build creates nothing unexpected under pmcp-course/src, and the mdbook-exercises theme side effect is not committed"
    verification:
      - kind: other
        ref: "git status --porcelain --untracked-files=all pmcp-course/src after build lists only SUMMARY.md + the quiz TOML (the two chapters already committed); git diff --quiet -- pmcp-course/src/theme/ succeeds"
        status: pass
    human_judgment: false
  - id: D6
    description: "Both books build and the full repo gate passes"
    verification:
      - kind: other
        ref: "mdbook build pmcp-course exit 0; mdbook build pmcp-book exit 0; make quality-gate exit 0"
        status: pass
    human_judgment: false

# Metrics
duration: 40min
completed: 2026-08-19
status: complete
---

# Phase 119 Plan 06: pmcp-course Agents & Teams Chapter Summary

**The third shape of DOCS-04's three-shapes rule delivered: a 462-line Part VIII course chapter teaching Agents & Teams at `ch23-skills` depth, cargo-pmcp-first, plus 211 lines of exercises whose pass predicates are the same strings `tests/docs04_examples_run.rs` asserts — so a doc and its test cannot drift apart.**

## Performance

- **Duration:** ~40 min including two `make quality-gate` attempts
- **Tasks:** 3
- **Files:** 4 (3 created, 1 modified), 809 insertions

## Task Commits

1. **Task 1: the course chapter at ch23-skills depth** — `c55ac7c0` (docs)
2. **Task 2: exercises with measured pass predicates** — `2515d759` (docs)
3. **Task 3: Part VIII nav, quiz decision, reachability** — `edd539f6` (docs)

## Accomplishments

- **`ch24-agents-and-teams.md` (462 lines)** follows `ch23-skills.md`'s spine exactly: H1 thesis closing with pointers to the examples, the exercises file and the quiz; `## Learning Objectives` (7 bullets, all opening "By the end of this chapter, you will be able to"); `## Why Agents & Teams Matter for Enterprise MCP` with an ASCII comparison table; ONE load-bearing design property in its own `##` section; Tier 1 / Tier 2 / Tier 3; `## Cross-SDK Compatibility`; `## Chapter Contents`; `## Knowledge Check`; the quiz include; and a `---` rule plus a `./`-prefixed forward pointer.
- **The load-bearing property is `## The Source-Agnostic Loop`** — the same `AgentEngine`, driven by the same `ResolvedAgentConfig`, runs against any `CompletionSource` with no loop change. The section states the failure mode the seam prevents (per-provider branches silently drifting on retry policy, iteration accounting and termination rules) and closes on the banner `s50_standalone_vs_sampled` prints, noting that `tests/docs04_examples_run.rs` asserts it.
- **The tiers run mechanical → real-world → composition**, each naming its example by full runnable invocation:
  - Tier 1 is CLI-only, no crate API: `cargo pmcp agent new` then `cargo pmcp agent dev --source fixed`, with a `--source` comparison table and the real success line `✓ agent run (fixed) finished: Completed`.
  - Tier 2 walks `cargo run -p pmcp-agent --example s50_standalone_vs_sampled`, quotes both section headers and the closing banner, explains why the agent is a server and a client at once, and points at the book's `ch17-04-sampling-hosting.md` for the sampling-direction disambiguation rather than re-deriving it. `cargo run --example s49_sampling_host` is cited for the inverse direction.
  - Tier 3 walks `cargo pmcp team dev` first, then `cargo run -p pmcp-team-servers --example doc_review_team --features runtime`, with a table mapping each of the four reference servers to what it owns.
- **`ch24-exercises.md` (211 lines)** ships three exercises at Introductory (10 min) / Intermediate (25 min) / Advanced (40 min), each with five numbered steps and a `### Verify your solution` pass predicate. Exercise 2's predicate requires **all three** of the two source-section headers and the closing banner — the "both, not just the last" framing that makes the tier's claim non-vacuous. `## Prerequisites` records that `cargo test` does not build example targets, which is the one thing that trips people up.
- **Quiz shipped** — 7 questions (5 MultipleChoice, 2 ShortAnswer) mirroring `ch23-skills.toml`'s format, behind a live `{{#quiz}}` include. Verified by observing 3 `quiz` occurrences in the rendered HTML, not merely by the build exiting 0.
- **`make quality-gate` exits 0**, and both books build.

## Evidence

**Reachability was asserted with `test -f`, never with the build — and this is the whole point.**
`pmcp-course/book.toml:11` sets `create-missing = true`, so `mdbook build` **writes** a missing
chapter into `pmcp-course/src/` and exits 0. The build is structurally incapable of detecting a
broken nav entry here, and because the file it creates is *untracked*, `git diff` cannot see it
either. Both new chapter paths were therefore asserted with explicit `test -f`, and after the build
`git status --porcelain --untracked-files=all pmcp-course/src` listed **only** this plan's own
deliberate paths (`SUMMARY.md` modified, the quiz TOML untracked; the two chapters were already
committed). No third path appeared, so no SUMMARY entry points at a file that does not exist.

**Predicate ↔ test cross-check (the reason this plan ran in wave 4, after 119-04):**

| Exercise | Predicate string | Asserted in `tests/docs04_examples_run.rs` as |
|---|---|---|
| 2 | `Done — the same AgentEngine ran standalone and hosted-sampled.` | `S50_BANNER` |
| 3 | `doc-review flow complete` | `DOC_REVIEW_TEAM_BANNER` |
| 1 | `✓ agent run (fixed) finished: Completed` | not a test string — read from `cargo-pmcp/src/commands/agent/dev.rs` `report_success` |

Exercise 1's predicate has no test counterpart because the CLI verbs are not example binaries; it
was derived from the CLI source (`new.rs` emits four files; `dev.rs` `report_success` prints the
source tag and terminal outcome) rather than invented.

**Verb-set enforcement (T-119-28):** `grep -ohE 'cargo pmcp [a-z-]+ [a-z-]+' | sort -u` over the
chapter prints exactly three lines — `cargo pmcp agent dev`, `cargo pmcp agent new`,
`cargo pmcp team dev` — matching `AgentCommand`'s two variants and `TeamCommand`'s one. No
`cargo pmcp agent --help` form was written, which would have matched the same extraction regex.

**Gate results:** `test -f` ×3 pass · `mdbook build pmcp-course` exit 0 · `mdbook build pmcp-book`
exit 0 · `git diff --quiet -- pmcp-course/src/theme/` pass · `make quality-gate` exit 0.

## Decisions Made

1. **Ship both the quiz include and the TOML** (Task 3's explicit either/or). Recorded above with its
   reasoning: a missing TOML behind a live include is a build risk under the CI-pinned
   `mdbook-quiz` 0.4.0; an omitted include is not. Consistency with ch23 broke the tie toward
   shipping both.
2. **`requirements-completed: []`** — see the DOCS-04 note under Notes for the orchestrator.
3. **Cross-book links use GitHub blob URLs**, not the `../../pmcp-book/src/...` relative form that
   `ch20-mcp-apps.md:160` uses. Under `create-missing = true` a course build that can be induced to
   reach outside its own `src` tree is a hazard worth not introducing; the GitHub form is also the
   convention `ch23-skills.md` already uses for example links.
4. **`--features runtime` only.** The chapter mentions that the example's own header names a broader
   `--all-features` superset, but never writes the two tokens adjacently, so the literal
   `doc_review_team --all-features` count is 0 and the docs cannot disagree with the tests.

## Deviations from Plan

None. All three tasks executed as written, and every acceptance criterion was executed rather than
reasoned about.

## Issues Encountered

**The machine hit disk exhaustion mid-run — environmental, not a code defect, and exactly the
residual risk 119-04 flagged to the orchestrator.**

The first `make quality-gate` invocation failed with exit 1 after ~10 minutes, and the failure
destroyed its own diagnostics: the harness could not write the captured output (`ENOSPC`), so the
output file was 0 bytes. `df -h /` had reported **70 GiB free** ten minutes earlier and **0** at the
failure — four parallel wave-4 worktrees each building a full `target/` tree drained it.

Resolved by deleting **only** regenerable build cache inside this worktree:
`rm -rf target/debug/incremental` (11 GB), taking free space to 9.8 GiB. Nothing tracked, no sibling
worktree and no example binary was touched. The re-run exited **0**.

Note for the orchestrator: the volume still reports `926Gi size / 12Gi used / 9.8Gi avail`, so most
of it remains held by APFS snapshots or purgeable space that a sub-agent cannot reclaim. This is a
machine-level condition needing operator action, not a phase defect, and it is the second wave in a
row to hit it.

## Known Stubs

None. No stub, no skipped test, no unrun `<verify>`. Both new files are complete prose with no
placeholder text, and every verification in the plan's `<verification>` block was executed.

Nothing was appended to `.planning/WINDOWS.md` because there is no defect of any tracked kind to
record.

## Threat Flags

None. This plan writes markdown and TOML only — no code, no new dependency, no network surface, no
key material (T-119-SC holds: zero packages installed).

Threat register outcomes: **T-119-28** (a nonexistent `cargo pmcp` verb) closed by the verb-set
extraction above. **T-119-29** (a silently empty published chapter) closed by `test -f` plus the
untracked-file sweep. **T-119-30** (a dangling `{{#quiz}}` include) closed by shipping the TOML and
observing the quiz render. **T-119-31** (predicates no example satisfies) closed by the
predicate ↔ test cross-check table. **T-119-32** (committing the theme diff) closed by
`git diff --quiet -- pmcp-course/src/theme/`.

## Notes for the orchestrator

- **STATE.md, ROADMAP.md and REQUIREMENTS.md were NOT modified** (worktree mode, per instruction).
  The GSD state-update step was deliberately **not run**, which also avoids the requirements-ledger
  defect 119-01 hit and reverted.
- **DOCS-04 must NOT be booked from this plan.** It requires book chapters **and** runnable examples
  **and** README **and** course. This plan is the **course quarter** only; 119-07 (book chapter) and
  119-09 (README) were landing in parallel and unmerged when this ran, and 119-02/119-04 already
  declined to book it for the same reason. `requirements-completed: []` is the correct answer here.
- **File scope was respected exactly.** Nothing under `pmcp-book/` was touched — in particular
  `pmcp-book/src/SUMMARY.md`, which plan 119-07 owns, is untouched (`git status --porcelain
  pmcp-book/` prints nothing). `README.md` and `CHANGELOG.md` (119-09's) are untouched.
- `pmcp-course/src/theme/` was restored after the **last** course build, and `.pmat/*` cache churn
  from `make quality-gate` was restored with `git checkout -- .pmat/`. The worktree is clean.

## Next Phase Readiness

- DOCS-04's course shape is complete. Once 119-07 and 119-09 merge, all four of DOCS-04's shapes
  (book chapters, runnable examples, README, course) exist and the requirement can be booked by
  whichever plan closes the phase.
- 119-10's closing gate has one fewer unknown: `mdbook build` for both books and `make quality-gate`
  were all observed green with these changes in the tree.
- The `create-missing = true` blindness is now documented in a second place (this SUMMARY) beyond
  119-VALIDATION's Manual-Only table, with the untracked-file sweep recorded as the concrete
  detector any future course-touching plan should copy.

## Self-Check: PASSED

- Files: `pmcp-course/src/part8-advanced/ch24-agents-and-teams.md`,
  `pmcp-course/src/part8-advanced/ch24-exercises.md`,
  `pmcp-course/src/quizzes/ch24-agents-and-teams.toml`, `pmcp-course/src/SUMMARY.md` — all present.
- Commits: `c55ac7c0`, `2515d759`, `edd539f6` — all resolve to commit objects.
- Working tree clean apart from this plan's own committed work.

---
*Phase: 119-documentation-three-shapes-v2-migration*
*Completed: 2026-08-19*
