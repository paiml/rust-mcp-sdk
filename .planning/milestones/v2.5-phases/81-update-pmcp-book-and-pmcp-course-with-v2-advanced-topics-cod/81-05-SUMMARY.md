---
phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
plan: 05
subsystem: docs
tags: [docs, course, code-mode, derive-macro, exercises, quiz, mdbook]

# Dependency graph
requires:
  - phase: 80-sep-2640-skills-support
    provides: pmcp-code-mode + pmcp-code-mode-derive crates (D-02 derive-macro-first framing)
provides:
  - Rewritten pmcp-course/src/part8-advanced/ch22-code-mode.md (~419 lines, was 223)
  - New pmcp-course/src/quizzes/ch22-code-mode.toml (8-question .ai.toml quiz)
  - New pmcp-course/src/part8-advanced/ch22-exercises.md (3-exercise page with falsifiable verify blocks)
affects:
  - 81-04 (owns SUMMARY.md insertion for ch22-exercises.md — coordination, not modification)
  - 81-07 (cross-property consistency audit + mdBook build verification)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Course chapter shape: Learning Objectives → Why X Matters for Enterprise → How It Works (pipeline diagram) → Adding via #[derive(CodeMode)] (5 mechanical steps) → Worked Example (success + rejection paths) → config.toml → Policy Evaluation → Security Properties Reference → Hands-On → Knowledge Check."
    - "Derive-macro-first framing: first occurrence of #[derive(CodeMode)] precedes any 'manual handler' / 'without the derive' / 'manually register' phrasing (revision R-2 Audit F check 3)."
    - "Quiz format: .ai.toml matching ch20-mcp-apps.toml structure; verified via Python 3.11+ tomllib (revision R-3)."
    - "Exercise format: three difficulty tiers (Introductory/Intermediate/Advanced) with falsifiable '### Verify your solution' H3 sections (revision R-9 count parity)."
    - "Cross-link style for excerpt+full-example: `Full example: [path](GitHub permalink)` (D-07/D-08)."

key-files:
  created:
    - pmcp-course/src/quizzes/ch22-code-mode.toml
    - pmcp-course/src/part8-advanced/ch22-exercises.md
  modified:
    - pmcp-course/src/part8-advanced/ch22-code-mode.md

key-decisions:
  - "Used pmcp 2.7.0, pmcp-code-mode 0.5.1, pmcp-code-mode-derive 0.2.0 (sourced from Cargo.toml at the time of this plan)."
  - "Default GraphQL language attribute reaffirmed as the no-flag-required happy path; the other three languages opt-in via feature flags."
  - "Single-file chapter retained (no sub-chapters) — matches the original 223-line course structure and ch20-mcp-apps.md."
  - "SUMMARY.md insertion for ch22-exercises.md is owned by plan 81-04 Task 4; this plan creates the FILE only (revision B-1)."

patterns-established:
  - "Worked-example mirror pattern: the course chapter and the book chapter (plan 81-02) both restructure around examples/s41_code_mode_graphql.rs without depending on each other. They will end up structurally similar because they share the source-of-truth example file."
  - "Quiz TOML structural guard: parse-cleanly + ≥6 [[questions]] blocks together catch both syntax errors and parse-valid-but-structurally-wrong TOML."
  - "Exercise verify-section pattern: every '## Exercise' heading is paired with exactly one '### Verify your solution' H3; the verify block asserts count parity."

requirements-completed: []

# Metrics
duration: ~25min
completed: 2026-05-15
---

# Phase 81 Plan 05: Code Mode Course Chapter v2 Rewrite Summary

**Full rewrite of ch22-code-mode.md (~419 lines, was 223) against current `pmcp-code-mode` 0.5.1 + `pmcp-code-mode-derive` 0.2.0; added a new 8-question .ai.toml comprehension quiz and a 3-exercise hands-on page with falsifiable verify blocks.**

## Performance

- **Duration:** ~25 minutes
- **Started:** 2026-05-15 (Wave 1 parallel executor)
- **Completed:** 2026-05-15
- **Tasks:** 4 (1 inspection-only + 3 implementation)
- **Files modified:** 1 (chapter rewrite)
- **Files created:** 2 (quiz + exercises)

## Accomplishments

- Rewrote `pmcp-course/src/part8-advanced/ch22-code-mode.md` against the current API surface, leading with `#[derive(CodeMode)]` as the canonical entry point (D-02). The chapter now mirrors the worked-example structure that plan 81-02 introduces in the book (D-08 cross-property consistency).
- Added `pmcp-course/src/quizzes/ch22-code-mode.toml` — a brand-new 8-question .ai.toml comprehension quiz covering HMAC binding, derive-macro field-name convention, the rejection path, TokenSecret minimum length, the three-layer policy model, executor adapter selection, and the NoopPolicyEvaluator caveat. Verified parse-clean under Python 3.11+ `tomllib` (revision R-3) and ≥6 `[[questions]]` blocks (structurally valid).
- Added `pmcp-course/src/part8-advanced/ch22-exercises.md` — three-tier hands-on exercises (Introductory wiring → Intermediate rejection-path → Advanced custom PolicyEvaluator). Each exercise has a falsifiable `### Verify your solution` H3 (revision R-9; count parity 3 = 3).
- Confirmed mdBook build passes for `pmcp-course/` with all three artifacts in place.

## Task Commits

Each task was committed atomically:

1. **Task 1: Confirm exercises-page non-existence and quiz status** — no commit (read-only inspection). Confirmed ch22-code-mode.md exists, ch22-exercises.md does NOT exist, ch22-code-mode.toml does NOT exist, s41_code_mode_graphql.rs exists. Plan proceeded in three-artifact mode (rewrite chapter + create quiz + create exercises page).
2. **Task 2: Full rewrite of ch22-code-mode.md against current API (course-style)** — `a1eca45f` (docs).
3. **Task 3: Author ch22-code-mode.toml quiz (TOML-parse-verified)** — `1c57031d` (docs).
4. **Task 4: Author ch22-exercises.md (Code Mode course exercises)** — `99af1ed9` (docs).

## Files Created/Modified

- `pmcp-course/src/part8-advanced/ch22-code-mode.md` — Full rewrite (419 lines, delta +196 lines over the previous 223). Restructured around `examples/s41_code_mode_graphql.rs` worked example with explicit success and rejection paths. Sections in order: Learning Objectives → Why Code Mode Matters for Enterprise MCP → How Code Mode Works (pipeline diagram) → Adding Code Mode with `#[derive(CodeMode)]` (Steps 1-5) → Worked Example (success + rejection paths) → Configuration in `config.toml` (Platform-Level Policy) → Policy Evaluation (Cedar / AVP / Custom) → Security Properties Reference → Hands-On (Run the Example) → Knowledge Check. Cross-links to `ch22-exercises.md` from Hands-On + Knowledge Check sections.
- `pmcp-course/src/quizzes/ch22-code-mode.toml` — 8-question quiz, 147 lines. Format matches `ch20-mcp-apps.toml`. Questions: (1) HMAC binding contents MC, (2) `language` attribute SA, (3) required four field names MC, (4) default rejection-path behavior MC, (5) TokenSecret minimum 16-byte SA, (6) three-layer policy ordering MC, (7) JsCodeExecutor scenario MC, (8) NoopPolicyEvaluator caveat MC.
- `pmcp-course/src/part8-advanced/ch22-exercises.md` — 173 lines. Three exercises (Wire derive macro / Trigger rejection path / Swap NoopPolicyEvaluator), each with `### Verify your solution` and 2 follow-up Questions to answer.

## Inline Excerpt Catalog

Sources cited inline in the rewritten chapter (each anchored by grep landmarks rather than hardcoded line numbers, per revision R-6):

| Section in chapter                       | Source file                          | Grep anchor used                          |
| ---------------------------------------- | ------------------------------------ | ----------------------------------------- |
| Step 2 (Derive and Configure)            | `examples/s41_code_mode_graphql.rs`  | `#[derive(CodeMode)]`                     |
| Step 4 (GraphQLExecutor direct impl)     | `examples/s41_code_mode_graphql.rs`  | `impl CodeExecutor for GraphQLExecutor`   |
| Step 5 (register + main)                 | `examples/s41_code_mode_graphql.rs`  | `register_code_mode_tools`, `fn main`     |
| Worked Example — Success path            | `examples/s41_code_mode_graphql.rs`  | `--- SUCCESS PATH ---`                    |
| Worked Example — Rejection path          | `examples/s41_code_mode_graphql.rs`  | `--- REJECTION PATH ---`                  |
| Pipeline diagram                         | `pmcp-book/src/ch12-9-code-mode.md`  | `LLM generates code`, `validate_code`     |
| config.toml examples (OpenAPI/GraphQL/SQL) | current `ch22-code-mode.md` (pre-rewrite REUSE) | `[server]`, `[code_mode]`                |
| Categorization rules                     | current `ch22-code-mode.md` (pre-rewrite REUSE) | category-table content                   |

Synthetic snippets (adapted for pedagogy, not verbatim excerpts) are marked with `<!-- synthetic -->` HTML comments so plan 81-07 Audit A skips them: the three standard-adapter snippets in Step 4 and the three policy-evaluator snippets in the Policy Evaluation section.

## Resolved Version Pins

Sourced from `Cargo.toml` at the time of this plan (revision W-4 / Audit E requires byte-equal pins between this chapter and `pmcp-book/src/ch12-9-code-mode.md`; plan 81-02 independently sources from the same Cargo.toml, so the values should match):

- `pmcp = { version = "2.7.0", features = ["full"] }`
- `pmcp-code-mode = "0.5.1"`
- `pmcp-code-mode-derive = "0.2.0"`

## Decisions Made

- Followed the plan exactly. Three-artifact mode (rewrite + create quiz + create exercises) was the planned mode per revision B-1.
- For the executor selection in the chapter, kept both the direct-impl option (with full GraphQLExecutor verbatim from the example) and the three-adapter shorthand (marked synthetic). This mirrors the planned structure for plan 81-02's book rewrite.
- Quiz question 8 answer position chosen as `3` (matching ch20's convention of varied positions across MultipleChoice questions).

## Deviations from Plan

**None — plan executed exactly as written.**

The original Write of `ch22-code-mode.md` placed both `#[derive(CodeMode)]` AND the phrase "manual handler registration" on the same lead-paragraph line, which caused the Audit F check 3 grep-line-precedence assertion to fail (both at line 3). Removed the inline "manual handler registration exists" clause from the lead paragraph; the equivalent statement now appears later at line 60 in the "Adding Code Mode with `#[derive(CodeMode)]`" section. This was a same-line-precedence ambiguity resolution, not a plan deviation — the plan's intent is preserved.

## Issues Encountered

- Initial Write placed `#[derive(CodeMode)]` and "manual handler registration" on the same line, causing the line-precedence audit to fail. Resolved by removing the inline clause from the lead paragraph; the phrase still appears later in the canonical-path framing for Step 1-5 (line 60).

## User Setup Required

None — documentation-only change.

## Threat Flags

None — documentation-only change. No new authentication, authorization, or trust-boundary surface introduced.

## Self-Check

**Files claimed to exist:**

- `pmcp-course/src/part8-advanced/ch22-code-mode.md` — FOUND (419 lines).
- `pmcp-course/src/quizzes/ch22-code-mode.toml` — FOUND (147 lines, 8 `[[questions]]` blocks, `tomllib.loads` exited 0).
- `pmcp-course/src/part8-advanced/ch22-exercises.md` — FOUND (173 lines, 3 `## Exercise` headings, 3 `### Verify your solution` headings — count parity satisfied).

**Commits claimed:**

- `a1eca45f` — FOUND in `git log`.
- `1c57031d` — FOUND in `git log`.
- `99af1ed9` — FOUND in `git log`.

**mdBook build:** `mdbook build` for `pmcp-course/` exited 0 with no warnings (preprocessors `mdbook-exercises` v0.1.4 and `mdbook-quiz` ran successfully).

**Confirmation that SUMMARY.md was NOT edited:** `pmcp-course/src/SUMMARY.md` has not been touched by this plan. Plan 81-04 Task 4 owns the insertion for `ch22-exercises.md`. The existing `ch22-code-mode.md` entry in `SUMMARY.md` (line 168) is unchanged.

## Self-Check: PASSED

## Next Phase Readiness

- Plan 81-04 can proceed to add the `ch22-exercises.md` entry to `pmcp-course/src/SUMMARY.md` (single-edit ownership confirmed).
- Plan 81-07 can run the cross-property consistency audit comparing this chapter against `pmcp-book/src/ch12-9-code-mode.md` (Audit E version-pin byte-equality, Audit F derive-first precedence, Audit A inline-excerpt verbatim-against-source).
- The mdBook build for `pmcp-course/` is green with the new artifacts in place.

---
*Phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod*
*Plan: 05*
*Completed: 2026-05-15*
