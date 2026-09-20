---
phase: 66
reviewers: [gemini]
reviewed_at: 2026-04-11
plans_reviewed:
  - 66-01-poc-include-str-gate-PLAN.md
  - 66-02-delete-deprecated-macros-PLAN.md
  - 66-03-downstream-markdown-fixup-PLAN.md
  - 66-04-readme-and-rustdoc-rewrite-PLAN.md
  - 66-05-changelog-version-bump-PLAN.md
---

# Cross-AI Plan Review — Phase 66

## Gemini Review

This review covers the implementation plans for **Phase 66: Macros Cleanup + Documentation Rewrite**.

The overall quality of the plans is **excellent**. They are surgically precise, faithfully implement all 25 locked decisions from the context, and proactively address subtle technical risks (like the proc-macro doctest cyclic dependency) identified during research.

### 1. Summary
Phase 66 is a high-impact hygiene and documentation phase. The 5-plan sequence logically progresses from a technical proof-of-concept (Wave 0) through aggressive cleanup of legacy code (Wave 1), into a high-quality prose rewrite (Wave 2), and finally to release coordination (Wave 3). The plans exhibit a deep understanding of the Rust ecosystem's documentation idiomatics (`include_str!`, hidden doctest structs) and maintain strict adherence to the project's "Toyota Way" quality standards.

### 2. Strengths
- **Risk-First Wave 0:** Plan 01 correctly prioritizes validating the `use pmcp_macros::mcp_resource` import path. This is the single most subtle technical assumption in the phase; proving it early prevents significant rework in Plan 04.
- **Surgical Deletion:** Plan 02 provides precise anchor strings and region descriptions for the `lib.rs` cleanup, ensuring that while 600+ lines of code are removed, the remaining real macros stay intact and functional.
- **Downstream Consistency:** Plan 03 demonstrates great ownership by including the `pmcp-course` and migration guide updates in the same phase, preventing the spread of obsolete patterns.
- **Documentation Quality:** Plan 04's use of absolute GitHub URLs for examples is a "pro tip" that solves the known issue of relative links breaking on `docs.rs`.
- **Release Precision:** Plan 05 follows the `CLAUDE.md` release workflow exactly, including the specific publish order and the multi-crate `CHANGELOG.md` entry pattern.

### 3. Concerns
- **Build Breakage Point (LOW):** Plan 02 Task 1 deletes files before Task 2 cleans up the module declarations. This results in a temporary broken build state between tasks. While normal for GSD, the executor should be warned not to run full-workspace checks until Task 2 is complete.
- **mcp_resource Asymmetry (LOW):** As noted in research, the `#[mcp_resource]` import in the README will be asymmetric (`pmcp_macros` vs `pmcp`). Plan 04 handles this well with a note to users, but it remains a minor visual wart that should be prioritized for a follow-up "bug fix" phase.
- **Line Number Drift (LOW):** Plans 02 and 04 rely on approximate line numbers. Given the deletions in Plan 02, the anchors in Plan 04 will shift. The plans wisely instruct the executor to "read first" and "anchor to strings," which mitigates this.

### 4. Suggestions
- **Plan 02 Task 2:** When deleting the `//!` lines, double-check for any `//!` lines that might have been added by workspace-level tooling (though unlikely in this repo).
- **Plan 04 Task 1:** In the `#[mcp_resource]` section of the README, explicitly mention that URI template variables (e.g., `{topic}`) are automatically extracted and passed as `String` parameters to the function. This is a key feature of that macro.
- **Plan 05 Task 2:** In the root `Cargo.toml` edit for line 147, the plan already fixes the `63_mcp_tool_macro` comment. Ensure this same thoroughness is applied if any other renamed examples are found in comments.

### 5. Risk Assessment: LOW
The phase is primarily documentation and deletion. No new runtime logic is introduced.
- **Technical Risk:** De-risked by the Wave 0 POC.
- **Regression Risk:** Mitigated by `make quality-gate` and the fact that `#[mcp_*]` implementations are untouched.
- **User Impact:** Breaking changes are correctly signaled by a minor version bump (0.4 -> 0.5) and documented in a high-quality migration guide.

**The plans are approved for execution.**

---

## Consensus Summary

Only one reviewer (Gemini) — no consensus across multiple reviewers. See Gemini's findings above as the sole external opinion.

### Strengths (from Gemini)
- Wave 0 POC correctly prioritizes the riskiest technical assumption (`use pmcp_macros::mcp_resource` import path)
- Plan 02's surgical deletion uses anchor strings + region descriptions, not fragile line numbers
- Plan 03 prevents obsolete patterns from spreading by bundling downstream fixups in-phase
- Plan 04 uses absolute GitHub URLs to avoid docs.rs relative-link breakage
- Plan 05 follows CLAUDE.md release workflow precisely (publish order, multi-crate CHANGELOG)

### Concerns (all LOW severity)
1. **Temporary build breakage between Plan 02 Task 1 and Task 2** — files deleted before module decls cleaned up. Executor should avoid full-workspace checks between tasks.
2. **mcp_resource asymmetric import** — visual wart in README, flagged for follow-up phase (already deferred per D-03).
3. **Line number drift** — Plan 04 anchors may shift after Plan 02's deletions; mitigated by "read first" + string-anchor pattern.

### Actionable Suggestions
1. **Plan 04 Task 1:** Add URI template variable extraction (`{topic}` → `String` param) callout to the `#[mcp_resource]` section.
2. **Plan 05 Task 2:** Extend the `63_mcp_tool_macro` comment sweep to any other renamed-example references in `Cargo.toml`.
3. **Plan 02:** Consider a note warning the executor not to run `cargo check --workspace` between Task 1 and Task 2.

### Divergent Views
N/A — only one reviewer.

### Overall Verdict
**LOW risk, approved for execution.** The review surfaced three minor suggestions worth folding into the plans via `/gsd-plan-phase 66 --reviews`, but no blockers.
