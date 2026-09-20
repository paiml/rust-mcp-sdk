---
phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
plan: 09
subsystem: docs
tags: [docs, gap-closure, audit-relaxation, audit-a, spec-amendment, revision-pass-2, R-1A, R-9]

# Dependency graph
requires:
  - phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
    provides: "81-07-PLAN.md (Wave 2 audit plan), 81-07-AUDIT.md (Wave 2 audit report) — both pre-existing; 81-09-PLAN.md (this plan's source)"
provides:
  - "Amended Audit A spec in 81-07-PLAN.md with revision R-9 = R-1A clauses a/b/c/d/e"
  - "Revision Notes section appended to 81-07-PLAN.md with dated R-9 entry"
  - "WARN-reclassifiable W-7 floor (no longer a hard FAIL gate in Task 3 verify block)"
  - "Source-match guard pattern for two-line Full-example anchors (R-1A.c)"
  - "Source-side cosmetic omission WARN classification (R-1A.d) for non-load-bearing line elision"
affects:
  - "81-10 (re-audit plan that executes the relaxed Audit A spec)"
  - "Phase 81 shippability gate (FAIL findings under strict Audit A → reclassified to PASS/WARN under R-9)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Spec amendment via dated revision note (R-N) — established convention extended"
    - "WARN-vs-FAIL severity classification for audit findings — refined granularity"
    - "Symmetric normalization (block AND source) for substring relations"
    - "Source-match guard pattern (language-fence consistency + filename/signature appearance) for anchor-followed-by-block scope"

key-files:
  created:
    - .planning/phases/81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod/81-09-SUMMARY.md
  modified:
    - .planning/phases/81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod/81-07-PLAN.md

key-decisions:
  - "Applied user-locked R-1A decision: relax audit, do not modify Wave 1 chapter content"
  - "Symmetric de-indent normalization (clause a) preserves substring relations — applied to BOTH block and source"
  - "Source-match guard (clause c) required for two-line anchors — prevents silent mis-comparison when a markdown block follows a Rust anchor"
  - "Non-load-bearing definition for clause d narrowed to: no struct field / control-flow / API shape changes — semantic drift still produces FAIL"
  - "W-7 floor (clause e) softened to WARN when N<10 — preserves N>=10 as a target, keeps phase shippable, documents anchor-density gap honestly"
  - "Commit message uses 'Predicts ...' language (per plan WARNING 6) — predictions are NOT guarantees; plan 81-10 records actuals"

patterns-established:
  - "Audit spec evolution via revision notes: B-N (build fix), R-N (relaxation/rewrite), W-N (warning-class) — R-9 fits the convention"
  - "Symmetric normalization rule for contiguous-substring comparisons"

requirements-completed: []

# Metrics
duration: 6min
completed: 2026-05-15
---

# Phase 81 Plan 09: Audit A Relaxation per R-1A (revision R-9) Summary

**Amended `81-07-PLAN.md` Audit A spec with revision R-9 (R-1A clauses a/b/c/d/e): de-indentation tolerance, `// ...` placeholder WARN classification, two-line anchor with source-match guard, source-side cosmetic omission WARN classification, and W-7 floor WARN-reclassification when N<10 — without modifying any Wave 1 chapter content.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-05-15T23:35:58Z
- **Completed:** 2026-05-15T23:42:00Z (approximate)
- **Tasks:** 1
- **Files modified:** 1 (81-07-PLAN.md)
- **Files created:** 1 (81-09-SUMMARY.md, this file)

## Accomplishments

- Eight surgical edits applied to `81-07-PLAN.md`:
  1. Frontmatter `must_haves.truths` Audit A truth amended to enumerate R-9 + clauses a/b/c/d/e.
  2. `<objective>` bullet 3 (Audit A description) amended to reference R-9 relaxations.
  3. Task 3 `<action>` algorithm section gained an "Revision R-9 (R-1A) relaxations" paragraph with all five clauses (a–e) detailed BEFORE the pseudo-shell algorithm block.
  4. The pseudo-shell algorithm itself was updated inline: normalize step gains de-indent stripping (R-9.a); anchor-scanner step gains two-line acceptance + source-match guard (R-9.c); FAIL branch split into three sub-branches (R-9.b WARN, R-9.d WARN, else FAIL); W-7 reporting step added (R-9.e PASS/WARN classification).
  5. Task 3 `<verify>` block: the hardcoded `EXCERPT_COUNT >= 10` FAIL gate REMOVED; replaced with `grep -qE 'W-7.*(PASS|WARN)'` and a `Total excerpts checked:` presence check. W-7 is now WARN-reclassifiable.
  6. Task 3 `<done>` paragraph: appended sentence enumerating R-9 clauses a/b/c/d/e.
  7. `<threat_model>` row T-81-07-03 (Audit A false-negative threat): mitigation cell extended with five sub-mitigations (i–v) explaining how R-9 clauses retain safety while relaxing.
  8. `<verification>` section bullet about Audit A contiguous-block matching: amended to reference R-9 relaxations.
  9. Appended new `## Revision Notes` H2 section at end of file with dated R-9 (2026-05-15) entry covering all five clauses, closure predictions, and the explicit "what this revision does NOT change" list.

- Pre-existing audit findings PREDICTED disposition under R-9 (per WARNING 6 — these are predictions, not guarantees; actual values are recorded by plan 81-10):
  - Findings 1, 2, 3 (book ch12-9 main-fn de-indent) → predicted PASS under R-1A.a.
  - Finding 4 (course ch22 L196 `// ...` placeholder) → predicted WARN under R-1A.b.
  - Finding 5 (course ch22 L228 source-side commentary/`println!` omission without `// ...`) → predicted WARN under R-1A.d.
  - W-7 (coverage floor) → predicted WARN under R-1A.e if relaxed-scan N<10.

## Task Commits

Each task was committed atomically:

1. **Task 1: Amend Audit A spec in 81-07-PLAN.md per R-1A (clauses a/b/c/d/e)** — `bf7ee3c5` (docs)

**Plan metadata commit:** (this SUMMARY.md commit follows — recorded separately)

## Files Created/Modified

- `.planning/phases/81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod/81-07-PLAN.md` — amended (frontmatter truth, objective bullet 3, Task 3 action, Task 3 verify, Task 3 done, threat model T-81-07-03, verification bullet, new Revision Notes section). Grew from 457 → 512 lines.
- `.planning/phases/81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod/81-09-SUMMARY.md` — created (this file).

NO chapter content modified — verified by `git diff --name-only HEAD~1 HEAD -- pmcp-book/src/ pmcp-course/src/` returning empty.

## R-1A Clauses (Five Relaxations Now in Spec)

| Clause | Relaxation | Severity Change |
|--------|------------|-----------------|
| R-1A.a | Leading whitespace stripped to common minimum-indent per block (symmetric: block AND source) before contiguous-substring match | de-indent drift → PASS (was FAIL) |
| R-1A.b | `// ...` placeholder lines in chapter blocks accepted when contiguous match succeeds on elision | placeholder drift → WARN (was FAIL) |
| R-1A.c | Two-line `Full example:\n[`path`](url)` anchor matched across line break, GATED by source-match guard (language-fence consistency OR filename/signature/struct-name appearance) | unmatched Skills-chapter anchors → IN-SCOPE (was unscanned) |
| R-1A.d | Source-side elision of non-load-bearing lines (commentary, `println!`, doc-comments) without a `// ...` marker accepted when contiguous match succeeds on elision; "non-load-bearing" = no struct-field/control-flow/API-shape changes | source-side cosmetic omission → WARN (was FAIL) |
| R-1A.e | W-7 coverage floor (>=10 in-scope excerpts) reclassified to WARN when relaxed scan yields N<10 | W-7 floor → WARN if N<10 (was hard FAIL gate) |

## Decisions Made

- **Symmetric normalization is non-negotiable for R-1A.a.** Stripping only the block (asymmetric) would break the substring relation and create false negatives. The spec mandates the same minimum-indent stripping be applied to the source file's text. This is the explicit mitigation T-81-07-03 sub-mitigation (i).
- **R-1A.c source-match guard requires at least ONE of three conditions** (language-fence consistency, basename substring appearance, or signature/struct-name appearance). This avoids false-broadening: a markdown block following a Rust anchor (e.g., course Skills L175/176 cites `s44_server_skills.rs` but the next block excerpts a different file's SKILL.md) is OUT OF SCOPE, not a FAIL.
- **R-1A.d "non-load-bearing" is defined narrowly** to prevent the relaxation from hiding semantic drift. Struct field changes, control-flow changes (added/removed `if`/`match`/`for`/`while`/early-return), and API shape changes (function signature, trait-impl-set) still produce FAIL even if the chapter block silently elides them.
- **R-1A.e softens W-7 to WARN rather than removing it.** The N>=10 ambition is preserved as a target; the WARN documents the cap honestly. The phase remains shippable under PASS WITH WARNINGS.
- **Commit message phrased in PREDICTIVE language** ("Predicts reclassification of ...") per WARNING 6: actual outcomes are recorded by plan 81-10's empirical re-audit, not by this spec amendment.

## Deviations from Plan

None — plan 81-09's single task (Task 1) executed exactly as specified. All eight edits applied per the plan's templated text. Commit message used verbatim from the plan's `<action>` template.

## Issues Encountered

- PreToolUse Read-before-edit hook fired a reminder before each Edit invocation in this session. The file had been read once at session start, and the runtime accepted every subsequent edit; the hook output was advisory, not a hard block. All 8 edits succeeded.

## Self-Check

- File `.planning/phases/81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod/81-07-PLAN.md` — FOUND (512 lines)
- Commit `bf7ee3c5` — FOUND in `git log --oneline -3`
- Audit A spec contains all five R-1A clauses (verified by 13 separate grep checks: `revision R-9`, `R-1A`, `de-indentation tolerant`, `WARN-severity`, `two-line anchor`, `source-match guard`, `R-1A.d`/`R-9.d`, `R-1A.e`/`R-9.e`, `source-side omission`, `non-load-bearing`, `W-7 floor reclassified`, `## Revision Notes`, `R-9 (2026-05-15)`) — all 13 PASSED.
- No chapter content modified — `git diff --name-only HEAD~1 HEAD -- pmcp-book/src/ pmcp-course/src/` returns empty (count = 0).
- Only file modified: `81-07-PLAN.md`.

**Self-Check: PASSED**

## Verdict

`Audit A spec relaxed per R-1A clauses a/b/c/d/e; ready for plan 81-10 empirical re-audit.`

## Next Phase Readiness

- Plan 81-10 (Wave 2 re-audit) can now run the relaxed Audit A spec and produce `81-10-AUDIT.md` with empirical PASS/WARN/FAIL classifications for each of the 7 findings in `81-07-AUDIT.md`.
- No blockers from this plan.

---
*Phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod*
*Plan: 09*
*Completed: 2026-05-15*
