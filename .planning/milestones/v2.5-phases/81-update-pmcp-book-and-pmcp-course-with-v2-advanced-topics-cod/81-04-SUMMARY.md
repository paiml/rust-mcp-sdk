---
phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
plan: 04
subsystem: pmcp-course
tags: [docs, course, skills, sep-2640, exercises, quiz]
requires:
  - .planning/phases/80-sep-2640-skills-support/80-CONTEXT.md
  - examples/s44_server_skills.rs
  - examples/c10_client_skills.rs
  - examples/skills/hello-world/SKILL.md
  - examples/skills/refunds/SKILL.md
  - examples/skills/code-mode/SKILL.md
  - tests/skills_integration.rs
  - src/server/skills.rs
provides:
  - course-chapter: pmcp-course/src/part8-advanced/ch23-skills.md
  - course-exercises: pmcp-course/src/part8-advanced/ch23-exercises.md
  - course-quiz: pmcp-course/src/quizzes/ch23-skills.toml
  - summary-index-entries: ch22-exercises.md + ch23-skills.md + ch23-exercises.md
affects:
  - pmcp-course/src/SUMMARY.md
tech-stack:
  added: []
  patterns:
    - "Three-tier walkthrough (hello-world / refunds / code-mode) — same shape as book Skills chapter (D-06)"
    - "Inline excerpts + 'Full example:' GitHub permalink cross-link style (D-07/D-08)"
    - "Course-style framing additions: Learning Objectives + Why X Matters + Knowledge Check + ../quizzes/*.toml quiz embed"
    - "Falsifiable per-exercise '### Verify your solution' H3 heading (revision R-9)"
    - ".ai.toml quiz format matching ch20-mcp-apps.toml structure (revision R-3)"
key-files:
  created:
    - pmcp-course/src/part8-advanced/ch23-skills.md
    - pmcp-course/src/part8-advanced/ch23-exercises.md
    - pmcp-course/src/quizzes/ch23-skills.toml
  modified:
    - pmcp-course/src/SUMMARY.md
decisions:
  - "Mirrored book Skills chapter section ordering for D-06 (Tier 1/2/3 + Cross-SDK Compatibility + Future Work)"
  - "Added course-style framing on top (Learning Objectives + Why X Matters + Knowledge Check) per D-09"
  - "Embedded quiz via `{{#quiz ../quizzes/ch23-skills.toml}}` at end of Knowledge Check section to match ch20 pattern"
  - "Absorbed ch22-exercises.md SUMMARY slot here per revision B-1 — plan 81-05 creates the file but does not touch SUMMARY.md"
  - "All Rust fences use `rust,ignore` (no `rust,no_run`) — doctest mandate D-10 is book-only"
metrics:
  duration: "single session (Wave 1 parallel executor)"
  tasks_completed: 4
  files_created: 3
  files_modified: 1
  total_lines_added: 796
  completed_date: "2026-05-15"
---

# Phase 81 Plan 04: Skills Course Chapter Summary

Authored the new Skills course chapter (`ch23-skills.md`), its exercises
page (`ch23-exercises.md`), its `.ai.toml` quiz (`ch23-skills.toml`), and
the SUMMARY.md index entries that slot all three new pages plus the
ch22-exercises.md slot created by plan 81-05.

## Files Created/Modified

| Path                                                    | Status   | Lines | Notes                                       |
| ------------------------------------------------------- | -------- | ----: | ------------------------------------------- |
| `pmcp-course/src/part8-advanced/ch23-skills.md`          | Created  |   443 | Course chapter; six H2 sections mirror book |
| `pmcp-course/src/part8-advanced/ch23-exercises.md`       | Created  |   222 | 3 exercises; 3 `### Verify your solution`   |
| `pmcp-course/src/quizzes/ch23-skills.toml`               | Created  |   131 | 7 questions; parses under `tomllib`         |
| `pmcp-course/src/SUMMARY.md`                             | Modified |    +4 | Three new index entries inserted            |

Total: 4 files touched, 796 lines added (3 new, 1 in-place).

## Commits

| Task | Commit    | Subject                                                              |
| ---- | --------- | -------------------------------------------------------------------- |
| 1    | a9fd9672  | docs(81-04): add ch23-skills.md Skills course chapter                |
| 2    | 75b9684b  | docs(81-04): add ch23-exercises.md Skills course exercises           |
| 3    | 4e973a18  | docs(81-04): add ch23-skills.toml Skills course quiz                 |
| 4    | 55c612d6  | docs(81-04): index ch22-exercises + ch23-skills + ch23-exercises in SUMMARY.md |

## SUMMARY.md Insertions (grep-anchored description per revision R-6)

Three new SUMMARY.md lines were inserted immediately after the
`- [Code Mode: Validated LLM Code Execution](./part8-advanced/ch22-code-mode.md)`
entry in the "Part VIII: Advanced Patterns" block:

```text
  - [Chapter 22 Exercises](./part8-advanced/ch22-exercises.md)

- [Skills: Agent Workflow Instructions](./part8-advanced/ch23-skills.md)
  - [Chapter 23 Exercises](./part8-advanced/ch23-exercises.md)
```

Indentation rules respected:
- `ch22-exercises.md` is 2-space-indented (child of `ch22-code-mode.md`,
  mirrors the ch20/ch21 exercises pattern).
- `ch23-skills.md` is at file root (top-level bullet, sibling of
  `ch22-code-mode.md`).
- `ch23-exercises.md` is 2-space-indented (child of `ch23-skills.md`).

No existing entries were renumbered (CONTEXT.md D-05).

**Coordination note (revision B-1):** Plan 81-05 owns the CREATION of
`pmcp-course/src/part8-advanced/ch22-exercises.md` but does NOT edit
SUMMARY.md. That SUMMARY entry was absorbed into this plan's Task 4 so a
single SUMMARY edit owns all three new Phase-81 course-side index lines.
The placeholder `ch22-exercises.md` (a single `# Chapter 22 Exercises`
H1) exists pre-81-05 and lets mdBook resolve the link cleanly until plan
81-05 fills in the body.

## Verification Results

### Chapter verify (Task 1)

Plan-defined automated verify block executed successfully:
- `## Learning Objectives` heading present.
- `## Why Skills Matter for Enterprise MCP` heading present.
- `## The Dual-Surface Invariant` heading present.
- `## Tier 1`, `## Tier 2`, `## Tier 3` headings present.
- `## Cross-SDK Compatibility` heading present.
- `## Future Work` heading present.
- `## Knowledge Check` heading present.
- Canonical phrase containing `byte-equal` present (revision R-2 cross-property
  anchor: "...the bootstrap helper makes drift between the two surfaces
  structurally impossible — the skill text and the prompt text are
  byte-equal by construction.").
- Citation of `tests/skills_integration.rs` present (revision R-2 anchor).
- Zero `rust,no_run` blocks (course chapter is `rust,ignore` only per D-10).
- File length: 443 lines (≥ 350 floor).

### Exercises verify (Task 2)

- 3 `## Exercise N` headings (Introductory / Intermediate / Advanced).
- 3 `### Verify your solution` H3 headings.
- `grep -c '^### Verify your solution'` (3) equals `grep -c '^## Exercise'` (3)
  → revision R-9 falsifiable per-exercise criterion satisfied.
- File length: 222 lines (≥ 80 floor).

### Quiz verify (Task 3)

- `python3 -c 'import tomllib; tomllib.loads(...)'` exited 0 → TOML parses
  cleanly under Python 3.13 (revision R-3).
- 7 `[[questions]]` blocks (5 MultipleChoice + 2 ShortAnswer).
- `id = "ch23-skills"` and `lesson_id = "ch23"` present.
- File length: 131 lines (≥ 40 floor).
- Structural inspection via `python3 tomllib`: each question has the
  required keys (`type`, `prompt.prompt`, plus `prompt.distractors` +
  `answer.answer` + `answer.position` for MC; `answer.answer` +
  `answer.alternatives` for SA; `context` and `id` on all).

### SUMMARY verify (Task 4)

- `ch22-exercises.md`, `ch23-skills.md`, `ch23-exercises.md` all present.
- `awk` ordering check confirms `ch22-code-mode.md` < `ch22-exercises.md`
  < `ch23-skills.md` < `ch23-exercises.md` in line order.

### mdBook build (success criterion)

`cd pmcp-course && mdbook build` exited 0. Rendered HTML output present
at `pmcp-course/book/part8-advanced/ch23-skills.html` (43.0K) and
`pmcp-course/book/part8-advanced/ch23-exercises.html` (22.7K). No errors
or warnings in build log.

## Cross-Reference Inventory (Full Example Anchors)

Every `Full example:` link in the chapter resolves to an actual repo path:

| Link Text                                    | Target                                                                                                                          | Resolves? |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | --------- |
| Server skills example                        | https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs                                                  | Yes       |
| Client skills example                        | https://github.com/paiml/rust-mcp-sdk/blob/main/examples/c10_client_skills.rs                                                  | Yes       |

Inline excerpts grep-anchored on whole functions / whole SKILL.md
sections so a future drift check can compare against the live example
(D-07 grep-stable boundary requirement). Excerpts pulled from:
`examples/skills/hello-world/SKILL.md`, `examples/skills/refunds/SKILL.md`,
`examples/skills/code-mode/SKILL.md`, the `build_code_mode_skill()` function
in `s44_server_skills.rs`, the registration block in the same file, the
`as_prompt_text()` implementation in `src/server/skills.rs`, and the
byte-equality `assert_eq!` in `c10_client_skills.rs`.

Exercises (`ch23-exercises.md`) cross-link back to both example files at
each tier; quiz `context` blocks cite the chapter, 80-CONTEXT.md, and
spike-findings so all answers are answerable from chapter materials.

## Deviations from Plan

None — plan executed exactly as written.

The plan accounted for both the per-task verify blocks and the
phase-level mdBook-build success criterion; all blocks passed
first-try with the content as written. No Rule 1/2/3 auto-fixes
required.

The book Skills chapter (`pmcp-book/src/ch12-8-skills.md`) is owned by
plan 81-01 (parallel Wave 1) and does not exist in this worktree.
Where the plan said "mirror the book Skills chapter," the chapter
mirrors the book chapter's *specification* (plan 81-01 task description)
rather than a concrete file — same H2 ordering, same inline excerpts
grep-anchored against the same source files. When 81-01 lands and 81-07
runs Audit F, both chapters should reference the same source files via
the same grep anchors.

## Threat Flags

None — documentation-only change. No new authentication, authorization,
input validation, or trust-boundary surface introduced. Threat model in
the plan (T-81-04-01 through T-81-04-06) covered: inline excerpt drift
(mitigated via grep-anchored boundaries; 81-07 audits), security framing
(chapter explicitly states Skills are pure context-loading), quiz answer
correctness (each `context` block cites source), cross-plan SUMMARY
ownership (single-edit ownership of SUMMARY.md per revision B-1), TOML
structural correctness (`tomllib` parse asserted), and exercise verify
ambiguity (per-exercise H3 count parity asserted).

## Self-Check: PASSED

- `pmcp-course/src/part8-advanced/ch23-skills.md` — FOUND (443 lines)
- `pmcp-course/src/part8-advanced/ch23-exercises.md` — FOUND (222 lines)
- `pmcp-course/src/quizzes/ch23-skills.toml` — FOUND (131 lines)
- `pmcp-course/src/SUMMARY.md` — FOUND (modified; three new entries present)
- Commit `a9fd9672` (Task 1) — FOUND in git log
- Commit `75b9684b` (Task 2) — FOUND in git log
- Commit `4e973a18` (Task 3) — FOUND in git log
- Commit `55c612d6` (Task 4) — FOUND in git log
- `mdbook build` for `pmcp-course/` — PASSED (exit 0, both ch23 HTML files rendered)
- Plan-level success criteria met: ch23 slotted after ch22-code-mode.md without
  renumbering existing chapters, three-tier walkthrough mirrors book Skills
  spec, no STATE.md / ROADMAP.md modifications.
