---
phase: 81
reviewers: [codex, gemini]
reviewed_at: 2026-05-15T18:39:01Z
plans_reviewed:
  - 81-01-PLAN.md
  - 81-02-PLAN.md
  - 81-03-PLAN.md
  - 81-04-PLAN.md
  - 81-05-PLAN.md
  - 81-06-PLAN.md
  - 81-07-PLAN.md
---

# Cross-AI Plan Review — Phase 81

## Codex Review

## Summary

The plan suite is strong on intent and coverage: it maps Phase 81 cleanly into book Skills, book Code Mode, book Tasks refresh, course Skills, course Code Mode, course Tasks refresh, and a final audit. The load-bearing decisions from Phase 80 are repeatedly carried through, especially the Skills dual-surface invariant and Code Mode derive-macro-first framing. The main risks are execution complexity and verification quality: several verify blocks are grep-based, the inline-excerpt audit claims byte-for-byte validation but only checks stable substrings, and Wave 1 parallelism creates consistency risks between book/course chapters authored independently.

## Strengths

- Clear phase boundaries: Skills and Code Mode get full treatment; Tasks stays explicitly limited to drift correction.
- Good preservation of Phase 80 decisions, especially no `#[pmcp::skill]` macro claims and no SEP-2640 archive-mode overpromising.
- Strong emphasis on reader-visible source-of-truth examples: `s44_server_skills.rs`, `c10_client_skills.rs`, and `s41_code_mode_graphql.rs`.
- Good course/book distinction: same technical examples, but course adds Learning Objectives, exercises, and quizzes.
- Good coordination fix in 81-04/81-05: one plan owns `SUMMARY.md`, preventing likely merge conflicts.
- 81-06 baseline approach is a solid improvement over hardcoded thresholds for structure-preserving docs refreshes.
- 81-07 correctly verifies mdBook by exit code rather than grepping for `"error"`.

## Concerns

- **HIGH: 81-07 inline excerpt audit is weaker than advertised.** It says excerpts match byte-for-byte, but the prescribed method only greps one stable substring. A stale or partially edited excerpt could pass as long as one line still exists in the source.

- **HIGH: Wave 1 book/course duplication can drift.** 81-01 and 81-04 independently author Skills chapters; 81-02 and 81-05 independently author Code Mode chapters. The final audit checks some shared artifacts, but not full consistency of the dual-surface explanation, language table, security claims, or example excerpts across both properties.

- **HIGH: 81-01 doctest may be fragile because it touches production source for documentation verification.** Adding a large book-specific doctest to `src/server/skills.rs` creates codebase churn for a docs phase. It is justified, but the exact snippet may fail if `Server::builder().build()?` error types or feature gates differ from the assumed API.

- **MEDIUM: Several plans rely on guessed or pre-read line numbers.** Examples like "lines 45-62", "lines 63-71", and "lines 108-143" will rot quickly. The plans also demand "grep-stable" excerpts, so line-number references should be secondary, not directive.

- **MEDIUM: `make quality-gate` in 81-01 may be too heavy for the scope.** A docs chapter plus one doctest does not necessarily need the whole project quality gate, especially if that target is slow or affected by unrelated repo state. The plan already has focused doctest and build checks.

- **MEDIUM: 81-07 is audit-only even for blocking failures.** If mdBook or doctests fail, the plan records failure but does not remediate. That is acceptable for a pure audit plan, but it means "7/7 complete" can still leave the phase not shippable.

- **MEDIUM: Quiz TOML validity is not actually verified.** Grep and line counts do not prove TOML parses or matches the expected quiz schema.

- **MEDIUM: Tasks refresh audits may miss non-snippet prose drift.** 81-03 and 81-06 focus heavily on names, signatures, imports, JSON, and protocol strings. That is good, but stale behavioral prose can remain if it does not contain obvious identifiers.

- **LOW: Exact line-count targets may bias authors toward padding.** Minimums like 400 or 500-700 lines are useful budgeting hints, but verify blocks using hard floors can reward verbosity over accuracy.

- **LOW: "Use em dash" conflicts mildly with tooling preferences.** Existing docs may use em dashes, so this is probably fine, but it creates exact-character fragility in grep and summary matching.

## Suggestions

- **81-07 Task 3:** Replace substring checks with exact contiguous excerpt matching. Extract each fenced block marked as an excerpt, normalize trailing whitespace, and verify the full block appears in the cited source file. For synthetic snippets, require an explicit marker like `<!-- synthetic -->` or skip with a counted reason.

- **81-07 Task 3:** Add cross-property consistency checks beyond version pins:
  - Skills book/course both contain the same core sentence defining the dual-surface invariant.
  - Both Skills chapters cite `tests/skills_integration.rs`.
  - Both Code Mode chapters contain the same language-to-feature table.
  - Both Code Mode chapters state `#[derive(CodeMode)]` before any manual registration discussion.

- **81-01 Task 3:** Before prescribing the doctest body as fixed, add a small discovery step to compile a scratch snippet or inspect existing builder examples. Keep the exact final snippet flexible if `build()?` or `Skill::as_prompt_text()` visibility differs.

- **81-01 Task 3 / 81-07 Task 2:** Prefer `cargo test --doc -p pmcp --features skills,full -- src/server/skills.rs` or an equivalent filtered invocation if supported, then run broader doctests in 81-07. This makes failures easier to attribute.

- **81-04 and 81-05 quiz tasks:** Add a TOML parse verification, for example with `taplo`, `python -c 'import tomllib...'`, or an existing repo quiz validator if available.

- **81-03 and 81-06:** Add a small "prose drift" search list: `SSE`, `serverless`, `owner binding`, `experimental.tasks`, `TaskSupport::Required`, `TaskSupport::Optional`, `tasks/result`, `tasks/cancel`. This catches stale behavioral claims not expressed as type names.

- **81-02 and 81-05:** Verify adapter names and feature flags directly from crate exports, not just current chapter content. If `JsCodeExecutor`, `SdkCodeExecutor`, or `McpCodeExecutor` are behind different modules/features, the chapter should avoid over-specific dependency advice.

- **81-07:** Treat mdBook or doctest failure as `FAIL` and recommend immediate remediation before marking Phase 81 complete. Audit-only is fine, but the success criteria should not imply completion if blocking checks fail.

- **All chapter-writing plans:** Replace hard line-count minimums with content checks where possible. For example, count mandatory sections, required example links, required source citations, and required concept terms instead of only `wc -l`.

## Risk Assessment

**Overall risk: MEDIUM.**

The scope is documentation-only and the plans are unusually explicit, which lowers implementation risk. The higher risk comes from over-specified prose generation, duplicated book/course work in parallel, and verification that is partly theatrical: grep checks and stable-substring audits can miss exactly the kind of documentation drift this phase is meant to prevent. Strengthening 81-07's excerpt and cross-property consistency audits would move the suite closer to LOW risk.

---

## Gemini Review

# Phase 81 Plan Review: Update pmcp-book and pmcp-course with v2 Advanced Topics

The proposed 7-plan suite for Phase 81 is a comprehensive and well-structured approach to closing the documentation debt for PMCP's v2 advanced features. By splitting the work into parallelized content delivery (Wave 1) and a dedicated cross-property consistency audit (Wave 2), the phase ensures high throughput while maintaining a rigorous safety net against the most common documentation failure: drift between prose and the live codebase.

## Strengths

- **Coherent Pedagogy:** The plans strictly enforce the "dual-surface invariant" as the lead framing for Skills and the "derive-macro-first" approach for Code Mode across both the book and the course, ensuring a consistent mental model for learners.
- **Drift Prevention by Design:** The use of a compile-verified `rust,no_run` doctest in the Skills book chapter (Plan 81-01), mirrored in the source code, provides a "living" example that won't rot.
- **Robust Verification:** Plan 81-06's strategy of recording pre-edit baselines to verify structural preservation during mechanical refreshes is an excellent engineering practice for documentation updates.
- **Comprehensive Audit (Plan 81-07):** The Wave 2 audit is the highlight of the suite. It goes beyond simple build checks to include heuristic excerpt-drift detection, cross-link validation, and version-pin consistency checks between properties.
- **Coordination & Isolation:** Plan 81-04's ownership of the course `SUMMARY.md` edits for both itself and Plan 81-05 is a smart move to prevent merge conflicts in the table of contents during parallel execution.

## Concerns

- **Heuristic Excerpt Drift Audit (HIGH-1, Plan 81-07 Task 3):** The heuristic for Audit A ("nearest preceding Full example link") may be fragile if a long section contains multiple excerpts from different files but only one link at the H2 level.
- **Cross-Property Dependency (LOW-1, Plan 81-05 Task 2):** The plan notes that 81-02 and 81-05 run in parallel and thus cannot depend on each other's output. While they both derive from the same `s41_code_mode_graphql.rs` source, there is a minor risk of stylistic divergence in the prose that the audit might not fully capture if it only checks excerpts.
- **Doctest Feature Requirements (LOW-2, Plan 81-01 Task 3):** The plan correctly identifies that `skills,full` features are needed to compile the doctest. However, if a developer runs a bare `cargo test --doc`, the test will be skipped. While documented, this may lead to "silent passes" in local environments if the developer isn't following the exact CLI instructions in the plan.

## Suggestions

- **Refine Excerpt Heuristic (Plan 81-07):** Explicitly instruct the audit agent to look for the specific `Full example: [`filename`](URL)` markdown pattern as the anchor for Audit A to increase the precision of the file-to-excerpt mapping.
- **Course Exercise Verification (Plan 81-04/05):** Since the exercises pages (`ch22-exercises.md`, `ch23-exercises.md`) are new, the "Verify your solution" sections should be reviewed by the agent for clarity to ensure the user-facing success criteria are falsifiable and clear.
- **Audit Findings Remediation:** The audit plan (81-07) explicitly avoids auto-fixing. Consider adding a small task to the end of 81-07 that generates a draft "Gaps Resolution" plan as a block of text, making it easier for the operator to trigger the follow-on cleanup.

## Risk Assessment: LOW

The overall risk is **LOW**. Documentation-only phases have minimal impact on system integrity, and the inclusion of Plan 81-07 as a Wave 2 gate provides a strong defense against shipping misleading or broken documentation. The only code modification (`src/server/skills.rs`) is confined to doc comments and is feature-gated.

---
**Verdict:** The 7 plans are ready for execution. The coordination of Wave 1 parallel tasks and the Wave 2 audit provides an ideal balance of speed and accuracy.

---

## Consensus Summary

Both reviewers agree the 7-plan suite is well-structured, faithful to Phase 80's load-bearing decisions, and reader-pedagogy is sound. They diverge on how much verification weight the Wave 2 audit (81-07) is actually carrying — Gemini reads it as a strong safety net, Codex reads it as partly theatrical.

### Agreed Strengths

- **Phase 80 fidelity carried through.** Dual-surface invariant (Skills) and derive-macro-first framing (Code Mode) are consistently lead in both properties.
- **Coordination move.** 81-04 owning the course `SUMMARY.md` for both 81-04 and 81-05 prevents merge conflicts under Wave 1 parallelism.
- **81-06 baseline approach.** Recording pre-edit `wc -l` / H2 count / exercise count in Task 1 and checking deltas in Tasks 2-4 is the right engineering practice for structure-preserving refreshes.
- **81-07 mdbook build check.** Replacing the bare-`error` substring grep with exit-code-driven verification removes a real false-positive risk.
- **Inline-excerpt strategy with cross-link.** Reader-friendly, low-maintenance — both properties use the same style.

### Agreed Concerns (highest priority)

- **HIGH: 81-07 Task 3 inline-excerpt audit is weaker than its objective implies.** Both reviewers flag this as the primary risk. Codex says "byte-for-byte" promise but only a stable-substring is checked; Gemini says the "nearest preceding Full example link" heuristic is fragile when an H2 section has multiple excerpts from different sources but only one link. **Both want Audit A tightened** — at minimum, an explicit `Full example: [...](URL)` anchor pattern (Gemini), at best, full-block normalized-string match (Codex).
- **MEDIUM: Cross-property drift risk between Wave 1 plans.** 81-01↔81-04 and 81-02↔81-05 author the same advanced-topic chapter independently. Both reviewers note the audit (81-07) catches version pins and excerpts but not the *prose-level* consistency that matters most for reader experience (dual-surface invariant wording, derive-macro-first ordering, language-to-feature table).

### Divergent Views (worth investigating)

- **Doctest in `src/server/skills.rs` — risk or asset?** Gemini sees the doctest as drift-prevention by design (a living example). Codex sees it as fragile (production source touched for docs; `Server::builder().build()?` may not match assumed API). Both points are valid; the resolution is "verify the snippet compiles first." Codex's suggestion to add a discovery step in 81-01 Task 3 before fixing the doctest body addresses this directly.
- **Overall risk: LOW (Gemini) vs MEDIUM (Codex).** Gemini weights the audit as a strong safety net; Codex weights the parts of the audit it considers weak. Both agree that the audit is the load-bearing artifact and that strengthening Audit A in 81-07 moves the suite firmly into LOW risk territory.

### Recommended actions before execute

If you want to incorporate review feedback into the plans, the highest-value changes are:

1. **81-07 Task 3 — strengthen Audit A.** Anchor on the explicit `Full example: [filename](URL)` markdown pattern and require contiguous block matching rather than single-substring grep (Codex + Gemini consensus).
2. **81-07 Task 3 — add prose-level cross-property consistency checks** (Codex):
   - Skills book/course both contain the same canonical dual-surface invariant sentence.
   - Both Code Mode chapters lead with `#[derive(CodeMode)]` before any manual handler discussion.
   - Both Code Mode chapters render the same language-to-feature table.
3. **81-04 / 81-05 — verify quiz TOML parses** (Codex): add `python -c 'import tomllib; tomllib.loads(open(...).read())'` or `taplo` to the verify blocks.
4. **81-01 Task 3 — confirm doctest compiles before fixing snippet** (Codex): small discovery step to validate `Server::builder().build()?` shape under `skills,full` before locking the snippet text.
5. **81-03 / 81-06 — add prose-drift search list** (Codex): explicit terms like `SSE`, `serverless`, `owner binding`, `experimental.tasks`, `TaskSupport::*` to catch stale behavioral claims that don't surface as type names.

Items 1 and 2 are the highest-value (both reviewers HIGH-flag Audit A); items 3-5 are quick wins that strengthen verification without expanding scope.

To incorporate: `/clear` then `/gsd-plan-phase 81 --reviews`. To proceed as-is and accept the risks above: `/gsd-execute-phase 81`.
