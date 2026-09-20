---
phase: 69
plan: 02
subsystem: documentation / proposal-seed
tags: [rmcp-parity, proposals, ergonomics, seed-phase]
requires: [69-01]
provides: [69-PROPOSALS.md, PARITY-MACRO-01, PARITY-HANDLER-01, PARITY-CLIENT-01]
affects: [.planning/phases/69-.../69-PROPOSALS.md]
tech_added: []
patterns: [one-REQ-ID-per-proposal, Row-ID-bijection, pmcp-native-fix-framing]
key_files_created:
  - .planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-PROPOSALS.md
  - .planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-02-SUMMARY.md
key_files_modified: []
decisions:
  - HANDLER-02 and HANDLER-05 bundled into a single PARITY-HANDLER-01 proposal (shared edit site on RequestHandlerExtra, shared design review)
  - Slotted PARITY-HANDLER-01 → v2.2 (new runtime surface), PARITY-CLIENT-01 → late v2.1 (additive API), PARITY-MACRO-01 → late v2.1 (pmcp-macros polish)
metrics:
  duration_minutes: ~25
  completed_date: 2026-04-16
  proposal_count: 3
  total_estimated_plan_count: 10
  high_rows_addressed: 4
  high_rows_total_in_research: 4
---

# Phase 69 Plan 02: Draft follow-on phase proposals from High-severity rmcp parity gaps Summary

**One-liner:** Translated the 4 High-severity rows from Plan 01's `69-RESEARCH.md` into 3 plannable follow-on phase proposals (HANDLER-02 + HANDLER-05 bundled) with single PARITY-<SURFACE>-01 requirement IDs, ready for Plan 03's quality gate and `/gsd-add-phase` promotion.

## Deliverables

- `.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-PROPOSALS.md` — 180+ lines, 3 proposals, summary table, per-proposal 6-subsection template, validation footer.

## Proposals Shipped

| # | Title | REQ-ID | Row IDs addressed | Target slot | Plans |
|---|-------|--------|-------------------|-------------|-------|
| 1 | Enrich `RequestHandlerExtra` with typemap extensions and peer back-channel | PARITY-HANDLER-01 | HANDLER-02, HANDLER-05 | v2.2 | 4 |
| 2 | Typed client call helpers and auto-paginating list-all convenience methods | PARITY-CLIENT-01 | CLIENT-02 | late v2.1 | 3 |
| 3 | Rustdoc fallback for `#[mcp_tool]` tool descriptions | PARITY-MACRO-01 | MACRO-02 | late v2.1 | 3 |

**Total estimated plan count across all proposals:** 10 plans (4 + 3 + 3).

## Row-ID Bijection Verification

**High Row IDs in 69-RESEARCH.md (source of truth):** `{CLIENT-02, HANDLER-02, HANDLER-05, MACRO-02}` — exactly 4 rows, matching Plan 01's Executive Summary.

**Row IDs cited in proposal `Derived from:` lines:** `{CLIENT-02, HANDLER-02, HANDLER-05, MACRO-02}` — 4/4.

**Row IDs cited in proposal `### Rationale / Evidence` subsections:** `{CLIENT-02, HANDLER-02, HANDLER-05, MACRO-02}` — 4/4.

**Bijection result:** Surjective coverage achieved. Every High Row ID is cited in at least one proposal's Derived-from line AND at least one proposal's Rationale subsection. No proposal cites a fabricated or non-existent Row ID. Non-High Row IDs (e.g., CLIENT-03, CLIENT-04, MACRO-04) appear only in `Out of scope:` bullets as explicit exclusions referencing the Medium-severity future-work classification per D-16 — this is proper traceability, not a bijection violation.

## Validation Results (Task 2 sweep)

| Check | Result |
|-------|--------|
| Proposal count vs header | 3 == 3 ✓ |
| All 6 template subsections per proposal | 3/3 ✓ |
| Goal sentence-1 starts with functional verb | Extend / Add / Enable ✓ |
| Single unique REQ-ID per proposal | 3 unique IDs, each appearing 2× (table + body) ✓ |
| Plan count ∈ {3,4,5} | 4, 3, 3 ✓ |
| Success Criteria bullets ∈ [3,5] | 5, 5, 5 ✓ |
| Row-ID bijection (High rows ⊆ cited) | 4/4 ✓ |
| No "adopt rmcp" / "copy rmcp" phrasing | ✓ |
| Validated footer present | ✓ |

## Deviations from Plan

### Rule 3 — Blocking issue identified and documented, not silently swallowed

**1. [Rule 3 - Blocking tool bug] Regex in Task 2's `<verify>` Python block does not match RESEARCH.md pipe-table rows**

- **Found during:** Task 2 automated verification run
- **Issue:** The plan's verify script uses `re.match(r'^\| ((?:...)-\d{2}) \|.*\| (High|Medium|Low) *$', line)` to extract severity per row. RESEARCH.md pipe-table rows actually end with `| High |` (trailing pipe + optional whitespace), so the regex matches zero rows and produces a spurious `S_research = {}` set — causing the bijection check to falsely report "proposals cite non-High Row IDs" for every cited ID.
- **Fix:** Did NOT modify 69-PROPOSALS.md structure (the proposals are already correct). Instead: (a) ran a manually corrected version of the check locally (`\| (High|Medium|Low) \|?\s*$`) and confirmed bijection passes — all 4 High Row IDs cited, no fabricated IDs; (b) documented the regex bug inline in 69-PROPOSALS.md's `**Validated:**` footer so Plan 03 can use the corrected pattern; (c) documented here so the next planner can patch the plan template if the same `<verify>` block is reused for future "proposals-from-research" plans.
- **Files modified:** `.planning/phases/69-.../69-PROPOSALS.md` (expanded validation footer only)
- **Commit:** Task 2 commit (34ca6de0)

No architectural changes. No content changes to proposals. Zero Rule 4 items.

## Key Design Decisions

1. **HANDLER-02 + HANDLER-05 bundled into one proposal** — both High rows edit the exact same struct (`RequestHandlerExtra`), benefit from a single coordinated review, and can be shipped in the same phase without serialization cost. Splitting would double-review the same struct.
2. **PARITY-HANDLER-01 → v2.2, others → late v2.1** — the handler enrichment adds new runtime surface area (a typemap, a peer trait), which is the v2.2 boundary. The client helpers and macro rustdoc fallback are additive DX polish fitting v2.1's "close DX gaps" charter.
3. **No CLIENT-03/CLIENT-04 (Medium) proposals** — D-16 is explicit that only High-severity gaps become proposals. CLIENT-03 (typed notification handler trait) and CLIENT-04 (client-side ProgressDispatcher) are referenced in Proposal 2's Out-of-scope section as future work, keeping a traceable link without inflating Phase 69's scope.
4. **Suggested phase numbers:** Proposal 2 → Phase 70 (concrete), Proposal 3 → Phase 71 (concrete), Proposal 1 → TBD (v2.2 slotting is not sequenced with Phase 69 in current ROADMAP). Final numbers assigned by `/gsd-add-phase`.

## Known Stubs

None. This is a documentation-only plan; every output is content-complete.

## Authentication / Auth Gates

None encountered.

## Ready for Plan 03

Plan 03 can proceed with:
- **Quality gate:** re-run the bijection check (with the corrected regex noted above) and the 6-subsection template check.
- **REQUIREMENTS.md landing:** add PARITY-HANDLER-01, PARITY-CLIENT-01, PARITY-MACRO-01 to `.planning/REQUIREMENTS.md` under a new "v2.1/v2.2 rmcp Parity Follow-ons" sub-section, with checkboxes unmarked (future work), each linked to its proposal title in 69-PROPOSALS.md.
- **STATE.md update:** advance Current Plan from 2→3, record 4 High-severity rows closed by 3 proposals as a decision.
- **ROADMAP.md update:** `[x]` the Plan 02 checkbox under Phase 69.

## Self-Check: PASSED

- `.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-PROPOSALS.md` — FOUND
- Commit `37f450e1` (Task 1 draft) — FOUND in git log
- Commit `34ca6de0` (Task 2 validation) — FOUND in git log
- Header `**Proposal count:** 3` — FOUND in file
- All 4 High Row IDs cited in Derived-from lines — VERIFIED via grep
- Validated footer with Plan-03 regex note — VERIFIED via grep
