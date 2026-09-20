# Phase 81: Update pmcp-book and pmcp-course with v2 advanced topics (code-mode, tasks, skills) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in 81-CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-15
**Phase:** 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
**Areas discussed:** Depth-per-topic, Skills chapter structure, Examples integration style, Course exercises & quizzes

---

## Depth — Skills coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Full chapter, both properties | Book ch12-8 (~500–700 lines) + Course ch23 with sub-chapters. Mirrors Tasks structure. | ✓ |
| Book full, course minimal | Book ch12-8 full; course gets one compact chapter without sub-chapters. | |
| Compact in both | One short chapter each (~200 lines) pointing at the three-tier example. | |

**User's choice:** Full chapter, both properties (Recommended).
**Notes:** Skills is brand-new and warrants first-class treatment. Chapter
shape ends up as a single chapter (no sub-chapters per tier) — see
"Skills chapter structure" below.

---

## Depth — Code Mode handling

| Option | Description | Selected |
|--------|-------------|----------|
| Full rewrite of both chapters | Book ch12-9 (146 lines) and course ch22 (223 lines) both rewritten for v2 derive macro + split crates. | ✓ |
| Additive update only | Keep structure, add sections for derive macro. | |
| Book rewrite, course light | Book full rewrite, course additive only. | |

**User's choice:** Full rewrite of both chapters (Recommended).
**Notes:** Existing chapters predate the v2 derive macro, the split-out
`pmcp-code-mode` crate, and CMSUP-01..06 requirements. Additive updates
would leave outdated framing intact.

---

## Depth — Tasks handling

| Option | Description | Selected |
|--------|-------------|----------|
| Targeted refresh only | Audit for v2 drift, patch affected sections, no rewrite. | ✓ |
| Add v2 features section | Append "What's new in v2" section, keep rest intact. | |
| Full rewrite | Rewrite both to align with latest Tasks API. | |

**User's choice:** Targeted refresh only (Recommended).
**Notes:** Tasks chapters (book ch12-7 587 lines; course ch21 + 3 sub-chapters,
~35K total) are already solid. Refresh is mechanical drift correction
(protocol version, current API surface), not pedagogical re-think.

---

## Skills chapter structure — Book placement

| Option | Description | Selected |
|--------|-------------|----------|
| ch12-8 between Tasks and Code Mode | Part III: Advanced Features, Tasks (12.7) → Skills (12.8) → Code Mode (12.9). | ✓ |
| ch24-extensions (Protocol Extensions) | Frame Skills as a protocol extension in the existing ch24. | |
| New ch12-6 before Tasks | Pedagogical ordering simplest → most complex. | |

**User's choice:** ch12-8 between Tasks and Code Mode (Recommended).
**Notes:** Treats Skills as a co-equal v2 advanced feature alongside
Tasks and Code Mode rather than framing it as "just another protocol
extension".

---

## Skills chapter structure — Layout

| Option | Description | Selected |
|--------|-------------|----------|
| Single chapter, mirrors three-tier example | One file, sections per tier (hello-world / refunds / code-mode), dual-surface invariant up front. | ✓ |
| Parent + sub-chapters per tier | ch12-8 parent + ch12-8-01/02/03 sub-chapters per tier. | |
| Concept chapter + reference example | Concept-focused chapter + appendix walkthrough. | |

**User's choice:** Single chapter, mirrors three-tier example (Recommended).
**Notes:** Three-tier example from Phase 80 (hello-world / refunds / code-mode)
maps cleanly to three in-chapter sections without needing sub-chapter
scaffolding. Same shape in book ch12-8 and course ch23.

---

## Examples integration — Reference style

| Option | Description | Selected |
|--------|-------------|----------|
| Inline excerpts + cross-link | Quote 10–40 line snippets inline + "Full example: …" cross-link. | ✓ |
| Full embedded examples | Walk entire example top-to-bottom inline. | |
| Pointers only, no inline code | Concept-only chapters with external links. | |

**User's choice:** Inline excerpts + cross-link (Recommended).
**Notes:** Matches the existing ch12-7 Tasks style. Reader-friendly without
forcing the chapter to mirror the live example line-for-line.

---

## Examples integration — Book vs course symmetry

| Option | Description | Selected |
|--------|-------------|----------|
| Same style in both | Both properties use inline excerpts + cross-links; differentiation lives in exercises/quizzes. | ✓ |
| Book lighter, course heavier | Book cookbook-style; course deeper inline. | |
| Book heavier, course lighter | Book embeds full examples; course leans on exercises. | |

**User's choice:** Same style in both (Recommended).
**Notes:** Easier maintenance, consistent reader experience. The course's
distinguishing voice stays in its exercises and quizzes, not in code
presentation.

---

## Course exercises & quizzes — Topic coverage

| Option | Description | Selected |
|--------|-------------|----------|
| All three topics | Skills: full new set + quiz. Code Mode: refresh ch22 set. Tasks: light quiz refresh only. | ✓ |
| Skills only, defer rest | Skills gets new set; Code Mode and Tasks stay as-is. | |
| Defer all exercises to follow-on phase | Land chapters now, exercises later. | |

**User's choice:** All three topics (Recommended).
**Notes:** Course style mandates exercises and quizzes for new chapters,
and the Code Mode rewrite would leave existing exercises mismatched if
they're not refreshed together.

---

## Book doctests — Skills chapter

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — doctest at end of Skills chapter | `rust,no_run` doctest covering .skill(...) + .bootstrap_skill_and_prompt(...). Verifiable via cargo test --doc. | ✓ |
| No — cross-link to examples only | Keep `rust,ignore` blocks per existing convention. | |

**User's choice:** Yes — doctest at end of Skills chapter (Recommended).
**Notes:** Follows the Phase 66 pmcp-macros README pattern. Workspace-wide
migration of other chapters is explicitly out of scope.

---

## Course slot for Skills

| Option | Description | Selected |
|--------|-------------|----------|
| ch23 Skills (append, accept asymmetry) | Append to Part VIII end; book ordering Tasks→Skills→Code Mode differs from course Tasks→Code Mode→Skills. | ✓ |
| Insert as ch22 Skills, shift Code Mode to ch23 | Mirror book ordering; renumbers existing ch22. | |

**User's choice:** ch23 Skills (append, accept asymmetry) (Recommended).
**Notes:** Renumbering would break every existing cross-reference to
ch22 Code Mode. Asymmetric ordering between book and course is
acceptable given the avoided breakage.

---

## Claude's Discretion

Items left explicitly to the researcher/planner per CONTEXT.md "Claude's
Discretion" subsection:

- Exact chapter lengths within the rough budgets stated.
- Which specific Tasks-chapter sections drift relative to the current API.
- Whether the Code Mode rewrite uses a single end-to-end example or
  multiple smaller snippets.
- Exact exercise count and quiz length per chapter.
- Whether each chapter ends with a single "Future Work" section or
  surfaces deferred items inline.

## Deferred Ideas

- `#[pmcp::skill]` proc macro coverage — gets a chapter sub-section
  when the macro itself lands (post-Phase 80 follow-on).
- SEP-2640 §4 archive distribution — gated on Phase 80 GAP #2.
- Course chapter renumbering to align with book ordering — needs its
  own dedicated TOC-refresh phase.
- Workspace-wide migration of book chapters from `rust,ignore` to
  `rust,no_run` doctests — needs `mdbook test` CI plumbing.
- `mdbook test` integration in CI — gates the migration above.

