---
phase: 119-documentation-three-shapes-v2-migration
plan: 07
subsystem: docs
tags: [pmcp-book, agents, teams, pmcp-team-servers, mdbook, DOCS-04]
status: complete

requires:
  - "pmcp-book/src/ch12-15-agents-as-mcp-clients.md (plan 119-02) — the sibling chapter this one is written against"
  - "pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md (plan 119-05) — the migration chapter this one links instead of restating"
  - "crates/pmcp-team-servers/examples/doc_review_team.rs + its tests/docs04_examples_run.rs leg (plan 119-04) — the canonical --features runtime invocation"
provides:
  - "pmcp-book/src/ch12-16-agent-teams.md — the last of Phase 111's three named chapters"
  - "the completed DOCS-04 BOOK shape: all three Phase-111 chapters reachable from Part III in one green create-missing=false build"
affects:
  - "pmcp-book/src/SUMMARY.md Part III nav (one added line, nothing moved)"

tech-stack:
  added: []
  patterns:
    - "book chapter spine: H1 → era-disambiguation blockquote callout → three-paragraph intro ending in an 'After this chapter you should be able to' sentence → ## The Problem → ## Step N → ## What You Built (objectives folded into the intro, NOT a ## Learning Objectives heading — that is the course convention)"
    - "examples cited by full runnable invocation including -p and --features, never a bare example number"

key-files:
  created:
    - pmcp-book/src/ch12-16-agent-teams.md
  modified:
    - pmcp-book/src/SUMMARY.md

decisions:
  - "Cited `cargo run -p pmcp-team-servers --example doc_review_team --features runtime`, NOT the `--all-features` form the example's own module doc (line 24) shows. D-12 forbids editing example source, so the discrepancy stands in the source; the chapter cites the form `tests/docs04_examples_run.rs` actually asserts."
  - "Every factual claim about the example's output was taken from an actual run, not from reading the source: the '4 hosting task(s) torn down cleanly' count and the `team_approval__ask_reviewer` tool name are both observed values."
  - "requirements-completed is deliberately EMPTY. DOCS-04 spans book + course + README + runnable examples; this plan is the book-chapter quarter only, and 119-06 (course) and 119-09 (README) had not merged when this plan ran."

metrics:
  duration: ~35 min
  completed: 2026-08-19
  tasks: 2
  commits: 2

actuals:
  tokens: 4200
  tasks: 2
  commits: 2

requirements-completed: []
---

# Phase 119 Plan 07: Agent Teams Book Chapter Summary

Shipped `ch12-16-agent-teams.md` — the second of Phase 111's two to-write book chapters — and
wired it into Part III, completing DOCS-04's book shape: all three Phase-111 chapters are now
reachable in one green `create-missing = false` build.

## What Was Built

**Task 1 — `pmcp-book/src/ch12-16-agent-teams.md` (267 lines)** · commit `680860ba`

Follows the measured Part III spine established by `ch12-13` and matched by its sibling `ch12-15`:

- H1, then the D-05 era-disambiguation callout in the book's only callout form (a blockquote with
  a bolded lead) — protocol eras `v1`/`v2`, crate versions always name-attached.
- A three-paragraph intro closing with an "After this chapter you should be able to …" sentence.
  No `## Learning Objectives` heading — objectives are folded into the intro, which is the book's
  convention (the course's is the opposite).
- `## The Problem (Why a Team, Not a Bigger Agent)` — three reasons composition beats one agent
  with a longer tool list, ending on the load-bearing observation that a team member is itself
  just an MCP client, so nothing in Chapter 12.15 stops being true.
- `## The Four Reference Servers` — a table naming team-fs, mem-mcp, approval-mcp and team-mcp
  with their tool prefixes, read from `crates/pmcp-team-servers/src/lib.rs` rather than inferred
  from the example's banner. Includes the `derive_attachment` rule (team-mcp iff ≥2 agents,
  approval-mcp iff ≥1 human role, team-fs/mem-mcp opt-in only), read from `compose/derive.rs`.
- `## Step 1: Run a team` — `cargo pmcp team dev` is the chapter's FIRST runnable command
  (cargo-pmcp-first, D-11), with `--package`/`--data-dir`, `--serve`/`--port` and
  `--llm`/`--model` shown. All six flags verified against `cargo-pmcp/src/commands/team/dev.rs`.
- `## Step 2: Walk the doc-review flow` — the `doc_review_team` walkthrough, all seven steps, the
  offline-by-construction story (in-memory `DuplexTransport`, `FixedSourceFactory`, no sockets),
  and the discovered-not-hardcoded ask tool.
- `## Step 3: From the example to your own team` — `TeamRuntimeBuilder`, the `Option`-returning
  accessors that encode the derivation rule in the type, and countable teardown via
  `shutdown() -> usize`. Cross-links 12.15 rather than re-deriving the single-agent loop.
- `## Protocol eras and teams` — short; teams inherit the dual-version story from their members,
  link to 12.17 for the detail.
- `## What You Built` — five capability bullets, then cross-links as bare relative filenames.

**Task 2 — `pmcp-book/src/SUMMARY.md`** · commit `7e85c7f1`

One line inserted at column 0 between the Chapter 12.15 block (entry + its indented
`Sampling & Hosting` child) and Chapter 12.17. `git diff --numstat` confirms exactly **1 added,
0 removed** — nothing was moved and the re-parented `Sampling & Hosting` child was not touched.

## Verification Evidence

| Check | Result |
|---|---|
| `wc -l ch12-16-agent-teams.md` | **267** (≥ 220 required) |
| `head -1` | `# Chapter 12.16: Agent Teams` |
| `grep -c 'cargo pmcp team dev'` | 4 (≥ 1) |
| `grep -c '…doc_review_team --features runtime'` | 1 (≥ 1) |
| `grep -c 'doc_review_team --all-features'` | **0** |
| `grep -ohE 'cargo pmcp [a-z-]+ [a-z-]+' \| sort -u` | exactly one line: `cargo pmcp team dev` (T-119-33 mitigated) |
| `grep -c '^## Learning Objectives'` | 0 |
| `grep -c '^## What You Built'` | 1 |
| Cross-links present | ch12-15 ×3, ch12-17 ×2, ch17-04 ×1 |
| `cargo build -p pmcp-team-servers --example doc_review_team --features runtime` | **exit 0** (T-119-36) |
| Example actually RUN | exit 0, printed `✅ doc-review flow complete — 4 hosting task(s) torn down cleanly.` |
| `git status --porcelain crates/pmcp-team-servers/examples/` | clean (D-12 held) |
| `grep -c 'ch12-16-agent-teams.md' SUMMARY.md` | 1 |
| `grep -c 'ch17-04-sampling-hosting.md' SUMMARY.md` | **1** (T-119-34 mitigated) |
| Ordering in SUMMARY.md | 12.15 @43 → Sampling & Hosting @44 → **12.16 @45** → 12.17 @46 |
| `git diff --numstat pmcp-book/src/SUMMARY.md` | `1  0` (T-119-34 mitigated) |
| `cd pmcp-book && mdbook build` | **exit 0** — the reachability proof for all three Phase-111 chapters (T-119-35) |
| `cd pmcp-course && mdbook build` | exit 0 |
| `git diff --quiet -- pmcp-course/src/theme/` | **exit 0** after restore (T-119-37 mitigated) |
| `git status --porcelain -uall pmcp-course/src` | clean — no file mdbook created |
| `cargo test --test docs04_examples_run` | **3 passed** |
| `make quality-gate` | **exit 0** |

The chapter's factual claims about the example were taken from an observed run rather than from
reading source. Two specifics worth recording: the teardown line prints **4** hosting tasks (one
per attached server), and the dynamic approval tool resolves to **`team_approval__ask_reviewer`**
for this team's single `reviewer` human role.

## Deviations from Plan

None — the plan executed exactly as written. No deviation rule fired.

## Notes / Out-of-Scope Observations

**`make quality-gate` exits 0 while its fuzz leg builds nothing.** The gate log carries ~80
`error:` lines of the form `the option 'Z' is only accepted on the nightly compiler` /
`failed to build fuzz script`, once per fuzz target, because `cargo fuzz` needs nightly and the
local toolchain is stable. The target swallows those failures, so the aggregate gate still exits
0. This is **pre-existing, not caused by this plan** (this plan changed only markdown), and it
matches the already-recorded project finding that `make validate-always` fuzzes nothing on
stable. Recorded here rather than filed as a new deferred item, because `deferred-items.md` is
owned by another plan in this phase and three sibling agents were writing concurrently.

**Staleness-guard prerequisite.** `cargo test --test docs04_examples_run` fails loudly (by
design, not by skip) unless each example binary is built first. Two of the three legs
(`s50_standalone_vs_sampled`, `s49_sampling_host`) belong to plans 119-02/119-04; building them
was a read-only prerequisite for running the suite — no source was touched.

## Requirements Ledger

`requirements-completed: []` — **deliberately empty, and this is the correct answer.**

The plan frontmatter lists `requirements: [DOCS-04]`, but DOCS-04 requires book chapters AND
runnable examples AND README AND course coverage. This plan delivered the **book-chapter quarter
only**. Plans 119-06 (course) and 119-09 (README) were executing in parallel in sibling worktrees
and neither had merged when this plan ran, so booking DOCS-04 complete here would be a false
green in the ledger — the same defect plan 119-01 hit and reverted. Whichever plan closes the
last DOCS-04 surface should book it.

Per the orchestrator's instruction, `STATE.md` and `ROADMAP.md` were **not** modified by this
agent.

## Known Stubs

None. The chapter contains no placeholder text, no TODO/FIXME, and every command it shows was
executed or built during verification.

## Threat Flags

None. This plan writes markdown and adds one nav line; it introduces no endpoint, auth path,
file-access pattern or schema change.

## Self-Check: PASSED

- `pmcp-book/src/ch12-16-agent-teams.md` — FOUND
- `pmcp-book/src/SUMMARY.md` — FOUND (modified, 1 line added)
- commit `680860ba` — FOUND in `git log`
- commit `7e85c7f1` — FOUND in `git log`
