---
phase: 69
reviewers: [gemini, codex]
reviewed_at: 2026-04-16T20:00:00Z
plans_reviewed: [69-01-PLAN.md, 69-02-PLAN.md, 69-03-PLAN.md]
reviewer_versions:
  gemini: "0.38.1"
  codex: "codex-cli 0.121.0 (gpt-5.4)"
---

# Cross-AI Plan Review — Phase 69

Phase 69: rmcp parity research — ergonomics gap analysis + follow-on phase proposals.

---

## Gemini Review

The plans for Phase 69 are exceptionally well-structured and reflect a high degree of engineering discipline. They correctly treat research as a rigorous, evidence-gathering exercise that must produce actionable follow-on phases. The dependency chain between generating a factual matrix (Plan 01), deriving proposals with a clear severity bar (Plan 02), and integrating those findings into project management documents (Plan 03) ensures that the research result is high-quality and "GS-able."

### Strengths
- **Evidence-Link Discipline:** Requiring file:line citations for `pmcp` and GitHub/docs.rs links with line anchors for `rmcp` prevents hand-waving and ensures finding reproducibility.
- **Strict Severity Rubric:** The "High severity = clean articulable fix" rule is a powerful guardrail that ensures follow-on phase proposals are concrete and plannable rather than vague architectural desires.
- **Robust Automated Verification:** The use of `awk` for row counting and `python3` for cross-document bijection checks is a professional implementation of the "Trust but Verify" mandate.
- **Edge Case Sensitivity:** Explicitly handling the "zero High-severity gaps" scenario prevents the agent from hallucinating work just to fill the 2–5 proposal range.
- **Identity Protection:** The "Anti-pattern guard" in Plan 02 Task 1 correctly prevents "rmcp-architecture-mimicry," ensuring `pmcp` maintains its own design philosophy while seeking ergonomic parity.

### Concerns
- **None (HIGH/MEDIUM)**.
- **Citation Drift (LOW):** While line numbers are pinned to a captured short-SHA, researchers should be aware that `rmcp` source tags are more stable than `pmcp` main-branch line numbers during long-running research. *Plan 01 Task 1 correctly mitigates this by pinning the pmcp baseline SHA.*
- **Namespace Collision (LOW):** Plan 02 Task 1 includes a grep for `PARITY-` in `REQUIREMENTS.md` to ensure the prefix is new, but Plan 03 should re-verify this before the final land. *Mitigation is already present in Plan 03 Task 1 check C1/C2.*

### Suggestions
- **GitHub Search Strategy:** When populating `rmcp` evidence, focus on the `crates/rmcp/src/` path in their repository, as `rmcp` uses a workspace structure. Citing the top-level crate version is good, but the internal file links should point into the specific crate source for precision.
- **Succinct Goals:** In Plan 02, when writing the `### Goal` for proposals, ensure the "Sentence 1: what the phase delivers" is phrased as a functional capability (e.g., "Add support for X") rather than a research goal.

### Risk Assessment: LOW
This phase is documentation-only, posing zero risk to the SDK's runtime stability. The primary risk is a "shallow research" outcome, which is effectively mitigated by the minimum row count requirements and the evidence-link standard.

**Plan Review Result: APPROVED** — Plans are ready for execution.

---

## Codex Review

### Summary

The plan set is strong overall: it has clear phase separation, good dependency ordering (`69-01` research → `69-02` proposals → `69-03` integration), and unusually explicit evidence standards for a documentation-only phase. The main weaknesses are in traceability mechanics and integration assumptions, not in intent. In particular, the proposal/requirements mapping is internally inconsistent, the "bijection" logic is not robust without stable row IDs, and Plan 03 hardcodes some document math that should be derived from file state. Those issues are fixable, but should be tightened before execution.

### Strengths

- The phase boundary is disciplined. `69-01` explicitly locks the six ergonomics surfaces and keeps Tower/service integration and transport construction out of scope in both the header and methodology requirements (69-01-PLAN.md:114, 69-01-PLAN.md:267).
- The evidence bar is good by default: every matrix row requires evidence on both sides, with pinned rmcp version/tag and pmcp commit SHA for reproducibility (69-01-PLAN.md:96, 69-01-PLAN.md:98, 69-01-PLAN.md:110).
- The matrix design avoids forced gap inflation. Requiring `Parity` rows and `Strength to preserve` rows is the right antidote to cherry-picking (69-01-PLAN.md:99, 69-01-PLAN.md:220).
- The High-only promotion rule is sound. `69-02` should prevent Medium/Low findings from turning into bloated roadmap work (69-02-PLAN.md:117, 69-02-PLAN.md:165).
- Plan ordering is correct. Plan 03 quality-gates the research/proposals before landing IDs into shared planning docs, which reduces propagation of bad findings (69-03-PLAN.md:120).

### Concerns

- **HIGH** — The requirement-ID model in Plan 03 does not match the proposal template in Plan 02. Plan 03 says "Each ID corresponds to one Success Criterion in a proposal" (69-03-PLAN.md:68), but Plan 02 only requires `2–4` requirement IDs per proposal and `3–5` success criteria (69-02-PLAN.md:89, 69-02-PLAN.md:97, 69-02-PLAN.md:186). That breaks one-to-one traceability.
- **HIGH** — The proposal coverage check is not actually a bijection and is brittle because row identity is only `(surface, subtopic)` text (69-02-PLAN.md:222). If a section uses similar subtopic names twice, or a proposal bundles rows with edited wording, the set comparison becomes ambiguous. Stable row IDs are missing.
- **MEDIUM** — Plan 03 hardcodes REQUIREMENTS coverage math instead of deriving it from the document. The prescribed parenthetical `(14 original + 6 inserted via Phase 67.1 + K seeded by Phase 69)` will drift as soon as another insertion happens (69-03-PLAN.md:215). The current file happens to match that formula today, but the plan should not rely on that staying true.
- **MEDIUM** — The pmcp baseline split between published `v2.3.0` and "post-v2.3.0 main-only" APIs is not enforced strongly enough. The must-have says that distinction must be recorded (69-01-PLAN.md:18), but the required header only records the commit SHA (69-01-PLAN.md:113). That makes it too easy to blur published vs main-only evidence in the matrix.
- **MEDIUM** — The rmcp evidence standard is strong in theory but weaker in enforcement. Plan 01 allows docs.rs anchors as primary evidence (69-01-PLAN.md:96), and Plan 03 mostly validates via heuristics/spot checks (69-03-PLAN.md:137). For nuanced ergonomics claims, source-line citations should be preferred over rustdoc anchors.
- **MEDIUM** — The client/error discovery steps in `69-01` are too search-order-dependent. "Read the first two client-construction sites found" and "likely `src/error.rs`" are weak instructions for a document that is supposed to be evidence-backed (69-01-PLAN.md:190, 69-01-PLAN.md:191).
- **MEDIUM** — Plan 03 only partially updates `STATE.md`. It adds a Phase 69 bullet and session continuity, but it does not reconcile already-stale "Current focus" / "Current Position" fields, which currently still point at earlier work (STATE.md:22, STATE.md:27). That weakens the "discoverable from state" goal.
- **LOW** — The zero-proposal branch in Plan 03 assumes there are no `PARITY-*` IDs anywhere in `REQUIREMENTS.md` (69-03-PLAN.md:237). That is true today, but it will be false on reruns or if another parity phase later uses the same prefix.
- **LOW** — Each plan writes a `69-0X-SUMMARY.md` output, but those files are not declared in `files_modified` (69-01-PLAN.md:311, 69-02-PLAN.md:308, 69-03-PLAN.md:387).
- **LOW** — Plan 03's note "not expected — v2.0 is already 'Milestone complete'" does not match the actual current `STATE.md`, which says `status: Ready to execute` for milestone `v2.0` (69-03-PLAN.md:302, STATE.md:3, STATE.md:5). It's probably harmless, but it shows the integration logic is not fully grounded in the current file.

### Suggestions

- Add stable row IDs to `69-RESEARCH.md`, e.g. `MACRO-01`, `CLIENT-03`, and require proposals to reference those IDs instead of free-text surface/subtopic names. That makes the High-row coverage check precise and bundle-safe.
- Fix the traceability model by choosing one of two rules and using it consistently:
  1. One requirement ID per proposal, with success criteria nested under it.
  2. One requirement ID per success criterion, and require the counts to match.
- Tighten rmcp evidence requirements to "GitHub blob URL with line fragment by default; docs.rs anchor only when no source line is meaningful." Then make Plan 03 validate row-by-row, not just via grep heuristics.
- Make the pmcp baseline explicit in each relevant row, e.g. append `[v2.3.0]` or `[main-only]` in the pmcp evidence cell, or add a short header note enumerating which APIs are main-only. Right now that distinction can disappear in the matrix.
- Replace search-order instructions like "first two client-construction sites found" with named canonical sources. If those sources are unknown, add a preparatory discovery step that records which files were selected and why.
- In Plan 03, compute REQUIREMENTS coverage counts from actual list length / traceability table length rather than hardcoding `(14 + 6 + K)`.
- Expand the `STATE.md` integration step to update `Current focus` and `Current Position` if they are stale, or explicitly say they are intentionally left historical. Right now the plan lands new context without reconciling older misleading fields.
- Add the `69-01-SUMMARY.md`, `69-02-SUMMARY.md`, and `69-03-SUMMARY.md` files to each plan's `files_modified` list.

### Risk Assessment: MEDIUM

The plans are well-structured and should produce the two intended deliverables, but there are a few structural traceability problems that could reduce actionability if left unchanged. The biggest risks are not scope creep or ordering; those are handled well. The risks are ambiguity in mapping research rows to proposals, and inconsistency in how proposal requirements are later landed into `REQUIREMENTS.md`. Tighten those two areas and the phase should reliably achieve its goal of an evidence-backed gap matrix plus a credible set of follow-on phases.

---

## Consensus Summary

Two external reviewers (Gemini 0.38.1, Codex 0.121.0 / gpt-5.4) analyzed the three plans. Risk ratings diverged — Gemini: LOW (approved for execution), Codex: MEDIUM (tighten traceability first). The divergence is driven by Codex catching traceability-mechanics issues that Gemini did not surface.

### Agreed Strengths (both reviewers)

- **Evidence discipline** — file-path + line-number citations for pmcp, URL + anchor for rmcp; pmcp SHA + rmcp version pinned in the gap-matrix header.
- **Phase-boundary discipline** — Tower integration (Phase 56 handled) and transport construction correctly kept out of scope in both the matrix header and the proposal template's required "Out of scope" bullets.
- **Severity rubric quality** — three-level High/Medium/Low scale with "High = typical user path + clean articulable fix" is tight enough to filter gap inflation.
- **Edge-case handling** — zero-proposal path (CONTEXT.md D-15 allows it) is explicitly handled in Plan 03's automated verify with a `proposal_count == 0` branch.
- **Dependency ordering** — 69-01 (matrix) → 69-02 (proposals) → 69-03 (integration + quality gate) correctly sequences factual foundation before derived artifacts before project-wide landings.

### Agreed Concerns (≥ 1 reviewer flagged; priority for resolution)

None raised by both reviewers. The divergence is total — Gemini raised no HIGH/MEDIUM concerns; Codex raised 2 HIGH + 5 MEDIUM. This is itself informative: it suggests Gemini reviewed for *scope correctness and quality discipline*, while Codex reviewed for *file-level mechanical rigor*.

### Divergent Views (worth investigating before execution)

Four Codex findings that Gemini missed and that are worth treating as first-pass revision candidates before executing Phase 69:

1. **Traceability mismatch between Plan 02 and Plan 03 (Codex: HIGH)**
   Plan 03 line 68 says "Each REQ-ID corresponds to one Success Criterion in a proposal." Plan 02 allows 2–4 REQ-IDs per proposal and 3–5 success criteria (lines 89, 97, 186). A proposal with 3 REQ-IDs and 5 success criteria cannot satisfy "one-to-one."
   **Decision needed:** pick ONE rule — (a) one REQ-ID per proposal with nested success criteria OR (b) one REQ-ID per success criterion, counts must match. Apply consistently to both plans.

2. **Bijection uses free-text row identity (Codex: HIGH)**
   Plan 02's coverage check (line 222) compares rows by `(surface, subtopic)` strings. If two subtopics in different surfaces share a name, or wording drifts during revision, the bijection silently breaks.
   **Fix:** introduce stable row IDs in 69-RESEARCH.md (e.g. `MACRO-01`, `CLIENT-03`) and have proposals reference those IDs. Then bijection compares on stable IDs, not strings.

3. **Plan 03 hardcodes REQUIREMENTS math (Codex: MEDIUM)**
   The parenthetical `(14 original + 6 inserted + K seeded)` at 69-03-PLAN.md:215 happens to match the current file but will silently drift if another Phase inserts more requirements before 69 executes.
   **Fix:** derive the numbers from actual REQUIREMENTS.md line counts at execution time.

4. **STATE.md not reconciled, only appended (Codex: MEDIUM)**
   Plan 03 adds a Phase 69 bullet but leaves `Current focus: Phase 65` and `Current Position: Phase 67.2` intact — both stale. Downstream `/gsd:progress` reads these fields and will mislead.
   **Fix:** Plan 03 Task 3 should also update `Current focus` and `Current Position` (or set them to a "milestone in progress" placeholder).

### Non-critical Codex findings (can be addressed or deferred)

- LOW: SUMMARY.md files missing from `files_modified` — cosmetic, executor creates them anyway
- LOW: "zero-proposal branch assumes no pre-existing PARITY-* in REQUIREMENTS.md" — true today; matters only on reruns
- LOW: Plan 03 mentions "v2.0 milestone complete" but STATE.md says v2.0 "Ready to execute" — stale assumption in a comment, harmless
- MEDIUM: rmcp source-line citations preferred over docs.rs anchors — tightening evidence standard
- MEDIUM: client/error discovery "first two sites found" wording is weak — replace with named canonical sources
- Suggestion: point rmcp evidence at `crates/rmcp/src/` workspace path for precision (Gemini)

---

## Recommendation

**Do not execute as-is.** The two HIGH concerns from Codex (traceability mismatch, bijection brittleness) are real and would materially reduce the usefulness of the follow-on proposals. Both can be fixed with targeted edits to 69-02-PLAN.md and 69-03-PLAN.md — no replanning.

To incorporate feedback:

```
/gsd:plan-phase 69 --reviews
```

Priority order for fixes (author's suggestion, not from reviewers):
1. Pick one traceability rule (REQ-ID ↔ success-criterion) and apply to both plans — HIGH
2. Introduce stable row IDs in the matrix template; use them in bijection checks — HIGH
3. Derive REQUIREMENTS coverage math at execution time (no hardcoded counts) — MEDIUM
4. Add STATE.md reconciliation step (Current focus + Current Position) — MEDIUM
5. Strengthen rmcp evidence standard (prefer source URLs over rustdoc anchors) — MEDIUM
6. Replace "first two sites found" with named canonical files for client/error surfaces — MEDIUM
7. Add SUMMARY.md files to `files_modified`; fix the stale v2.0 comment — LOW (batch)
