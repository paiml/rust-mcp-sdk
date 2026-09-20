---
phase: 71
reviewers: [codex]
reviewed_at: 2026-04-17T15:55:00Z
plans_reviewed: [71-01-PLAN.md, 71-02-PLAN.md, 71-03-PLAN.md]
skipped: [gemini]
skip_reasons:
  gemini: "Returned empty stdout on 3 attempts with the full review prompt (~3300 lines); short-prompt smoke test passed. Likely gemini CLI agent-mode / stdin size interaction. Retry with `--experimental-acp` or chunked plans if a second opinion is required."
---

# Cross-AI Plan Review — Phase 71

Phase 71 — Rustdoc fallback for `#[mcp_tool]` tool descriptions (PARITY-MACRO-01)
Plans reviewed: `71-01-PLAN.md`, `71-02-PLAN.md`, `71-03-PLAN.md`
Artifacts considered: `71-RESEARCH.md`, `71-PATTERNS.md`, `71-VALIDATION.md`, `69-PROPOSALS.md` Proposal 3, PARITY-MACRO-01 REQUIREMENTS entry.

---

## Codex Review

### Summary

The plans are strong on research quality, test intent, and scope discipline: they correctly identify both `#[mcp_tool]` parse sites, define a clear precedence rule, and map the phase back to a concrete requirement with explicit verification. The main problems are execution-level, not conceptual. Two stand out: the proposed `__fuzz_support` public re-export from a proc-macro crate is likely invalid and could block both property/fuzz work, and the release/versioning plan audits the `pmcp-macros` ripple but not the `pmcp` ripple through downstream workspace crates and publish flow. With those corrected, the phase looks achievable.

### Strengths

- The phase goal is narrow and well-held. It does not drift into `#[mcp_prompt]`, `#[mcp_resource]`, schema overrides, or transport work.
- The research is unusually thorough. It finds the second parse site in `mcp_server.rs`, which is the key implementation trap here.
- Precedence is specified clearly: explicit `description = "..."` wins, rustdoc is fallback only, and neither present is a hard error.
- Backwards-compatibility thinking is good. The plans explicitly call out the three example sites where rustdoc already coexists with an explicit description.
- The normalization contract is concrete and testable. The trim/drop-empty/join-with-`\n` rule is easy to reason about and close to rmcp parity.
- The validation matrix is strong overall: unit, integration, trybuild, property, examples, docs, and release verification are all considered.
- Using a shared error-message constant is a good move; it reduces accidental wording drift.
- `trim()` already covers Unicode whitespace reasonably well, so that corner is mostly handled without extra complexity.

### Concerns

- **HIGH:** The `__fuzz_support` plan is likely invalid for a proc-macro crate. `pub mod __fuzz_support` exported from `pmcp-macros` may fail because proc-macro crates cannot expose arbitrary public items. This affects `71-01` Task 1, `71-01` Task 3, and `71-02` Task 3.
- **HIGH:** `71-03` checks the `pmcp-macros` version ripple, but not the `pmcp` version ripple. Bumping root `pmcp` from `2.3.0` to `2.3.1` may require updates in downstream workspace crates or release metadata if they pin `pmcp` exactly. The publish-order note makes this a real release risk.
- **MEDIUM:** The symmetry requirement is stated, but the implementation still duplicates logic in two places. That invites future drift even if Phase 71 lands correctly.
- **MEDIUM:** The trybuild coverage is weaker than the plan claims. Both missing-description UI fixtures use `#[mcp_tool()]`, so they do not lock the "non-empty args but still no description" path, e.g. `#[mcp_tool(name = "x")]`.
- **MEDIUM:** Edge cases like `#[doc = include_str!(...)]` and possibly `cfg_attr(doc = ... )` are not resolved. The helper only accepts string-literal `#[doc = "..."]` attributes. That is fine if intentional, but it should be documented or tested as unsupported.
- **MEDIUM:** The semver posture is not fully coherent. `pmcp-macros 0.5 -> 0.6` makes sense, but `pmcp 2.3.0 -> 2.3.1` may under-signal a newly accepted source form of `pmcp::mcp_tool`. That is additive, but still user-visible behavior.
- **LOW:** The README task is internally inconsistent about whether the doctest should import `mcp_tool` from `pmcp` or `pmcp_macros`.
- **LOW:** The explicit-empty case `description = ""` is not discussed. Current plan likely treats it as present and winning, which may be correct, but it is worth making explicit.
- **LOW:** The fuzz target is narrow. It mostly fuzzes newline-split string literals after escaping, not mixed attributes or AST-shape variation like `#[doc(hidden)]` plus normal docs.

### Suggestions

- Replace the `__fuzz_support` export approach. Better options:
  - keep property tests inside `mcp_common.rs` under `#[cfg(test)]`,
  - move the pure normalization helper into a non-proc-macro support crate/module,
  - or fuzz a tiny internal support binary instead of exporting from the proc-macro crate.
- Extract one shared resolver, not just shared leaf helpers. Something like "parse nested metas + synthesize rustdoc description + emit canonical error" should live in one function used by both parse sites.
- Add a compile-fail case for `#[mcp_tool(name = "x")]` with no rustdoc and no description. That covers the real "args present, description absent" path.
- Add one impl-block negative case, not just impl-block success. That proves the `Meta::Path(_)` change in `mcp_server.rs` behaves as intended.
- Decide and document the support boundary for:
  - `#[doc = include_str!(...)]`,
  - `cfg_attr(doc = ...)`,
  - code fences losing indentation,
  - explicit empty descriptions.
- Audit the workspace for `pmcp =` pins before locking the release plan. The current grep only proves the `pmcp-macros` ripple, not the `pmcp` ripple.
- Revisit the root crate version bump policy. If the project treats newly accepted macro syntax as a feature, `pmcp` may merit a minor bump rather than a patch bump.
- If fuzzing remains required, add a tiny corpus or second target that includes mixed attr shapes, not just `#[doc = "..."]` lines.

### Risk Assessment

**Overall risk: HIGH.** The core feature itself is low-complexity and the plans mostly target the right behavior, but the current execution plan has at least one likely blocker (`pub` support module from a proc-macro crate) and one release-process gap (root `pmcp` ripple not fully audited). Fix those two issues and the implementation risk drops substantially, probably to medium-low.

---

## Gemini Review

*Skipped — see frontmatter `skipped`/`skip_reasons`. The gemini CLI returned empty stdout on all attempts against the 3300-line review prompt (short-prompt smoke test passed, so the CLI itself works; the agent-mode + large-stdin path appears to be the issue).*

---

## Consensus Summary

Only one reviewer (Codex) produced substantive output. Treat this as a single-reviewer report rather than a consensus. Key action items surfaced:

### Agreed Strengths (single-reviewer, worth preserving)

- Scope discipline — no drift into `#[mcp_prompt]` / `#[mcp_resource]` / schema overrides / transport.
- Research quality — identified the second parse site (`mcp_server.rs::parse_mcp_tool_attr`) as the key trap.
- Clear precedence semantics + concrete, testable normalization contract.
- Validation matrix covers unit + integration + trybuild + property + examples + docs + release.

### Agreed Concerns (single-reviewer — highest priority for next planning iteration)

1. **HIGH — `__fuzz_support` public re-export from a proc-macro crate is likely invalid.** Proc-macro crates restrict what can be exported; `pub mod __fuzz_support` may not compile. Affects Plan 01 Task 1 (helper visibility), Plan 01 Task 3 (property tests importing the helper), and Plan 02 Task 3 (fuzz target). Replace with one of: (a) `#[cfg(test)]` internal tests in `mcp_common.rs`, (b) move the pure normalization helper to a non-proc-macro support crate/module, or (c) fuzz a tiny internal support binary.
2. **HIGH — `pmcp` version ripple not audited.** Plan 03 bumps `pmcp 2.3.0 → 2.3.1` but does not audit whether `cargo-pmcp`, `mcp-tester`, or `mcp-preview` pin `pmcp` exactly. Per CLAUDE.md publish order, this is a release risk. Needs explicit grep of `pmcp =` pins across the workspace.
3. **MEDIUM — Implementation logic duplicated across two parse sites despite "symmetry" claim.** Codex recommends extracting one shared resolver (parse nested metas + synthesize rustdoc description + emit canonical error) callable from both sites.
4. **MEDIUM — Trybuild coverage gap:** `#[mcp_tool()]` missing-desc fixture does not lock the `#[mcp_tool(name = "x")]` path. Add a compile-fail case with non-empty args + missing description + missing rustdoc.
5. **MEDIUM — Unsupported rustdoc forms undocumented:** `#[doc = include_str!(...)]`, `cfg_attr(doc = ...)`, indented code fences, explicit empty `description = ""`. Decide and document the support boundary (likely unsupported for this phase, but surface explicitly).
6. **MEDIUM — Semver posture.** Root `pmcp 2.3.0 → 2.3.1` may under-signal a newly accepted macro source form; consider minor bump.
7. **LOW — README doctest `use` ambiguity** — whether to import from `pmcp` or `pmcp_macros`. Pinned in revision but worth confirming against the existing `### Example` doctest.
8. **LOW — Explicit empty `description = ""` semantics** undefined. Add a test fixing the chosen behavior.
9. **LOW — Fuzz target is narrow** (string-literal newlines only, not AST-shape variation).

### Divergent Views

None — single reviewer.

---

## Next Steps

To incorporate this feedback:

```
/gsd-plan-phase 71 --reviews
```

The planner will replan with `71-REVIEWS.md` as input, applying the HIGH-severity fixes first (`__fuzz_support` validity + `pmcp` ripple audit), then the MEDIUM/LOW items.

**Minimum targeted fixes (if a full replan is overkill):**
- **HIGH-1:** Plan 01 Task 1 — replace `pub mod __fuzz_support` with `#[cfg(test)] mod tests` (or move the pure helper to a non-proc-macro crate). Plan 02 Task 3 — decide whether fuzz target stays, moves, or is dropped.
- **HIGH-2:** Plan 03 Task 1 — add explicit grep step for `pmcp =` pins across `cargo-pmcp/Cargo.toml`, `crates/mcp-tester/Cargo.toml`, `crates/mcp-preview/Cargo.toml` and conditionally bump them.
- **MEDIUM-1:** Plan 01 Task 2 — extract one shared resolver used by both parse sites rather than duplicating the call sequence.
- **MEDIUM-2:** Plan 02 Task 1 — add `mcp_tool_missing_description_nonempty_args.rs` trybuild fixture (`#[mcp_tool(name = "x")]`).

If Gemini second opinion is required, retry with chunked plans or `--experimental-acp` mode after the high-severity fixes land.
