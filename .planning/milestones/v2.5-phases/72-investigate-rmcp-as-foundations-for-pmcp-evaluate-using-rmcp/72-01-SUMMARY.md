---
phase: 72
plan: 01
subsystem: research/decision
tags:
  - research
  - decision
  - rmcp
  - foundations
  - inventory
  - strategy-matrix
  - context-locks
  - reviews-mode
requires:
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-RESEARCH.md
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-REVIEWS.md
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-VALIDATION.md
provides:
  - 72-CONTEXT.md (T6/T7 locks — both UNKNOWN with explicit Resolution paths)
  - 72-INVENTORY.md (29 rows on 9-column evidence schema, real file:line defining items)
  - 72-STRATEGY-MATRIX.md (5 scored option rows {A,B,C1,C2,D}, 25 cells, E as footnote)
  - REQUIREMENTS.md rows RMCP-EVAL-01..05 + 5 Traceability rows
affects:
  - .planning/REQUIREMENTS.md (5 new REQ-IDs, Coverage footer bumped 23→28)
tech-stack:
  added: []
  patterns:
    - "9-column inversion inventory evidence schema (family | pmcp file:line | rmcp evidence | symbols | API surface | impls/macros | serde-compat | feature flags | downstream crates)"
    - "{A,B,C1,C2,D} + E-as-footnote strategy matrix option set"
    - "CONTEXT.md explicit-UNKNOWN-with-Resolution-path convention for rubric inputs that cannot be derived from repo state"
key-files:
  created:
    - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-CONTEXT.md
    - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-INVENTORY.md
    - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-STRATEGY-MATRIX.md
    - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-01-SUMMARY.md
  modified:
    - .planning/REQUIREMENTS.md
decisions:
  - "T6 and T7 locked as UNKNOWN with explicit Resolution paths rather than guessed — both resolve via user input at /gsd-verify-work for Plan 03"
  - "Inventory rows use bare-text file:line (no backticks) in the primary evidence cell so the VALIDATION grep gate `| path.rs:NNN |` matches"
  - "Per-task atomic commits were squash-combined into the single plan-Task-4 commit so the plan's Task 4 verify block (`git log -1 --name-only | grep 72-CONTEXT.md ...`) passes"
metrics:
  duration: "single session, ~35m wall"
  completed: "2026-04-19"
---

# Phase 72 Plan 01: Seed Inventory + Strategy Matrix + CONTEXT Locks Summary

Seed Phase 72's anchor deliverables — CONTEXT locks, inversion inventory (29 rows on 9-col schema with real file:line evidence), 5-option strategy matrix (A/B/C1/C2/D with E demoted to footnote), and 5 RMCP-EVAL-01..05 REQ-IDs in REQUIREMENTS.md — all in reviews-mode revised form addressing 72-REVIEWS.md HIGH-3, replan #3, and replan #7.

## Reviews-Mode Revision Summary

| Reviews finding | How addressed in this plan |
|---|---|
| **HIGH-3** (Codex) Inventory evidence standard too weak (5-col schema) | Row schema upgraded to **9 columns**: (1) family, (2) pmcp file:line defining item, (3) rmcp evidence, (4) exact symbols, (5) public API surface, (6) owned impls/macros, (7) serde-compat risk, (8) feature flags, (9) downstream crates. 29 rows produced; all `file:line` values point at real defining items (struct/enum/trait/const), not `:1` placeholders. |
| **Replan action #3** (Gemini) T6/T7 risk of permanent UNRESOLVED | Created `72-CONTEXT.md` with explicit `breaking_change_window:` and `production_user_tolerance:` fields — both marked UNKNOWN, each with an explicit `Resolution path:` naming what input closes them (user question at /gsd-verify-work). Prevents T6/T7 from silently defaulting Plan 03 to DEFER. |
| **Replan action #7** (Codex) Strategy option set {A,B,C,D,E} ill-defined | Split C into **C1 (types-only)** and **C2 (transports-only)** — materially different migration profiles. Demoted E (Fork) from scored row to `## Contingency (not scored): E. Fork` footnote. Matrix now has exactly 5 scored option rows × 5 criteria = 25 scored cells, no TBD. Valid Plan 03 recommendation set is `{A, B, C1, C2, D, DEFER}`; E is NOT a valid pick. |

## Inventory Row Counts

**Total rows:** 29 (plan required ≥15)

**Overlap rating breakdown:**
| Overlap | Count |
|---|---|
| EXACT | 14 |
| Partial | 6 |
| pmcp-exclusive | 8 |
| pmcp-superset | 1 |
| UNVERIFIED | 6 |

**Serde-compat rating breakdown (cell occurrences — some rows have multiple ratings):**
| Serde-compat | Count |
|---|---|
| EXACT | 18 |
| compatible-via-adapter | 1 |
| INCOMPATIBLE | 1 |
| N/A | 14 |
| UNVERIFIED | 8 |

**docs.rs/rmcp/1.5.0 links cited:** 30 (plan required ≥10)

**UNVERIFIED rows carried into Plan 02 spike:** 6 rows — row 3 (Completion/Logging capabilities), 11 (Auth), 13 (Completable), 25 (Session), 26 (Batch), 29 (Logging). Plan 02 Slice 1 spike resolves T3/T4 with real compile data and closes these UNVERIFIED rows where possible.

## T6/T7 Lock Status (72-CONTEXT.md)

| Threshold | Locked value | Resolution path |
|---|---|---|
| **T6** — pmcp v2.x breaking-change window | UNKNOWN | Ask user directly at /gsd-verify-work for Plan 03: "Is the pmcp v2.x breaking-change window still open? If closed, provide the close date." Binary question, no code analysis required. |
| **T7** — Production-user tolerance for v3.0 | UNKNOWN (floor: ≥ 2 from repo evidence — pmcp-run + Lambda deployment) | Ask user at /gsd-verify-work, or run lightweight pmcp-user survey (forum post + crates.io dependents listing). |

Both values are explicit UNKNOWN with documented resolution paths — not silently undefined. Plan 03's decision tree will count these into the "resolved thresholds" tally when the user input arrives.

## Matrix Scoring Highlights (Directional)

| Criterion | Option that scored highest (directional) | Reasoning |
|---|---|---|
| Maintenance reduction | A. Full adopt (HIGH, ~7,600 LOC) | Deletes rows 1–10 + 15 per inventory totals |
| Migration cost (low = better) | D (ZERO) and C2 (LOW) | No or narrow scope; transports-only borrow minimizes ripple |
| Breaking-change surface (low = better) | D (ZERO) and C2 (LOW) | Types unchanged under D and C2 |
| Enterprise feature preservation | B, C1, C2, D all FULL by construction | Only A is CONDITIONAL, subject to Slice 2 verification against T9 |
| Spec-upgrade agility | A (HIGH) | rmcp's ~6-week spec cadence beats pmcp's manual tracking |

**No single option dominates** — A maximises maintenance reduction but creates the biggest migration + breaking-change bill; D zeroes migration but also zeroes maintenance reduction. The decision rubric (RMCP-EVAL-04, seeded in Plan 02) is the tie-breaker. Plan 03 applies the decision tree with T6/T7 inputs from CONTEXT.md.

## Cross-Doc Consistency Check

- **INVENTORY pmcp-exclusive rows ↔ matrix enterprise-feature-preservation cells:** PASS. All 8 pmcp-exclusive rows (14, 16, 17, 19, 20, 22, 23, 28) are retained under B/C1/C2/D; the matrix cells for these options all show "FULL" preservation. A is flagged CONDITIONAL pending Slice 2 T9 checklist.
- **Strategy matrix LOC cells ↔ INVENTORY Totals:** PASS. A's "~6,400 LOC + ~1,200" in the matrix matches inventory Totals row 1 and row 3. C1's "~4,500" matches Totals row 2. C2's "~1,200" matches Totals row 3.
- **CONTEXT.md T6/T7 ↔ matrix Risk Annotation §2-§3:** PASS. Risk Annotation §2 ("pmcp v2.x breaking-change window") references 72-CONTEXT.md T6 directly; §3 references T7 directly. No contradictions.
- **REQUIREMENTS.md RMCP-EVAL-02 text ↔ matrix option set:** PASS. REQ-ID 02 names "A. Full adopt / B. Hybrid wrapper / C1. Selective borrow — types only / C2. Selective borrow — transports only / D. Status quo + upstream PRs" exactly; matrix rows match.
- **REQUIREMENTS.md RMCP-EVAL-05 text ↔ matrix E-footnote rule:** PASS. REQ-ID 05 says "E (Fork) is NOT a valid recommendation"; matrix footnote §Rule for Plan 03 says the same.

## Deviations from Plan

### Task 4 commit shape — atomic-per-task squash-combined into aggregate

- **What:** Plan Task 4 prescribed a single commit covering all 4 deliverables. GSD executor protocol says "commit after each task". I committed each task atomically (4 commits: b12a1414, 0495f418, bc92d677, fce5ba9c), then `git reset --soft HEAD~4` and re-committed as a single aggregate commit (20e12154) so the Task 4 verify block passes (`git log -1 --name-only` lists all four files; subject contains `reviews-mode`).
- **Why:** Satisfies both protocols. The soft-reset operated only on commits I created in this session, with no risk of losing others' work.
- **Classification:** Rule 3 (blocking issue for Task 4 verify) — auto-fixed.

### No framing changes from 72-RESEARCH.md

The inventory uses 72-RESEARCH.md's seeded row content as the starting point; reviews-mode revisions added columns (not content overrides). Option-set split C→C1/C2 and E-footnote demotion came from 72-REVIEWS.md replan #7 (already authored before this plan) and were faithfully mirrored. No fresh framing decisions introduced by this plan.

## Self-Check

**Files:**
- `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-CONTEXT.md` — FOUND
- `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-INVENTORY.md` — FOUND
- `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-STRATEGY-MATRIX.md` — FOUND
- `.planning/REQUIREMENTS.md` — MODIFIED (+5 RMCP-EVAL rows, +5 Traceability rows, Coverage 23→28)

**Commits:**
- `20e12154` — aggregate Plan 01 commit (`docs(72): seed RMCP-EVAL REQ-IDs, CONTEXT locks, inversion inventory (9-col schema), strategy matrix (A/B/C1/C2/D, E-as-footnote) [reviews-mode]`) — FOUND on HEAD with all 4 files.

## Self-Check: PASSED
