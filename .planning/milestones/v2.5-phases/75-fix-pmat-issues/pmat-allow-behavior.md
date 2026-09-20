---
verification: D-10 PMAT allow-suppression behavior
pmat_version: pmat 3.15.0
date: 2026-04-23
fixture_path: /tmp/pmat-allow-fixture/ (deleted post-test)
---

# PMAT allow-suppression empirical verification

## Baseline

- Function: `deeply_nested_for_pmat_test`
- Cognitive complexity (PMAT-reported): **41** (well above the 25 threshold)
- Cyclomatic complexity (PMAT-reported): 16
- `pmat analyze complexity --max-cognitive 25` reports violation: **yes** (cognitive-complexity rule, value 41, threshold 25)
- `pmat quality-gate --fail-on-violation --checks complexity` exit code: **1** (violation present)

## After adding `#[allow(clippy::cognitive_complexity)]` with `// Why:` comment

The exact annotation form per the project-wide `// Why:` template in 75-00-PLAN.md `<context>`:

```rust
// Why: test fixture for PMAT allow-suppression verification per Phase 75 D-10
#[allow(clippy::cognitive_complexity)]
pub fn deeply_nested_for_pmat_test(input: i32) -> i32 { /* unchanged */ }
```

Re-running PMAT against the same fixture file:

- `pmat analyze complexity --max-cognitive 25` reports violation: **YES — STILL FLAGGED** (cognitive-complexity rule, value 41, threshold 25; identical to baseline)
- `pmat quality-gate --fail-on-violation --checks complexity` exit code: **1** (still fails — identical to baseline)

The annotation has **zero effect** on PMAT's complexity check. Both rules (cyclomatic-complexity and cognitive-complexity) continue to flag the function.

This is consistent with PMAT's design as an external static-analysis tool that walks the source AST directly — it does not consult Rust's `#[allow]` attribute machinery (those attributes are only honored by `rustc`'s built-in lints and by clippy's own attribute parsing). PMAT computes complexity from raw structure and applies its own threshold; it has no "suppression by allow attribute" concept.

## Decision (per CONTEXT.md D-10)

outcome: D-10-B

(D-10-B: PMAT IGNORES the allow → P5 removed from toolkit; all functions must reduce to ≤25 to clear the gate.)

## Implications for Waves 1-4 (D-10-B branch)

This is a **scope-expansion event** for Phase 75. Plans 75-01 through 75-05 were authored under the optimistic D-10-A assumption that `#[allow(clippy::cognitive_complexity)]` with a `// Why:` justification would suppress flagged functions from the gate. That assumption is now disproven.

**Concrete delta per wave:**

- **Wave 1a (`src/server/streamable_http_server.rs`)** — projected count drop **was -8** under P5 (some hotspots planned to be retro-justified rather than refactored). Under D-10-B every flagged function must reduce ≤25 by extraction. Hotspots that were going to receive `#[allow]` annotations now require real refactor: e.g. `validate_headers` (cog 40), `build_response` (cog 30), `validate_protocol_version` (cog 34), `handle_post_fast_path` (cog 48), `handle_post_with_middleware` (cog 59), `handle_get_sse` (cog 35) — all 6 violations in this file must reach ≤25. Estimated additional refactor effort: moderate (these are protocol-dispatch fns; helper extraction is workable but adds plumbing).

- **Wave 1b (`pmcp-macros/`)** — projected count drop **was -7** under P5. Under D-10-B every macro `expand_*` and `collect_*_methods` function must reduce ≤25. The 4 expand functions snapshotted in Task 1 (`expand_mcp_server` cog 36, `expand_mcp_tool` cog 40, `expand_mcp_resource` cog 71, `expand_mcp_prompt` cog 42) plus the 3 collect functions (`collect_tool_methods` cog 44, `collect_prompt_methods` cog 42, `collect_resource_methods` cog 80) all need helper extraction. Higher risk because macro expansion has interlocking parser/generator state — Task 1's snapshot baseline becomes load-bearing for catching regressions.

- **Wave 2 (`cargo-pmcp/src/pentest/` + `cargo-pmcp/src/deployment/` + scattered cargo-pmcp/commands)** — projected count drop **was -19**. The 51 cargo-pmcp violations dominate the inventory. Many are dispatch-style (`execute`, `main`) that are naturally branchy; under D-10-B all must reach ≤25 by extraction. Wave 2 likely the longest sub-budget.

- **Wave 3 (`crates/pmcp-code-mode/`)** — projected count drop **was -5**. The two highest-cog functions in the entire repo live here: `evaluate_with_scope` cog 123 and `evaluate_array_method_with_scope` cog 117. Both must reach ≤25 (not the original ≤50 D-03 ceiling). This is a 5x reduction, requiring substantial AST-walker decomposition. Task 2's semantic-regression baseline becomes critical — the corpus + per-variant tests are the only safety net against silent behavior drift across that magnitude of restructuring.

- **Wave 4 (examples/, fuzz/, packages/)** — examples/ count is **0** today (D-09 spike result), so no examples-handling changes. fuzz/ has 5 violations; D-09 result confirmed `.pmatignore` works (fuzz/ excluded cleanly). packages/ (3 TypeScript violations) is also `.pmatignore`-handled. Wave 4 is therefore SMALLER than originally budgeted — `.pmatignore` absorbs all out-of-tree violations without code changes. Net effect: Wave 4 plan should be re-scoped to "configure .pmatignore for fuzz/, packages/, examples/ defensively + verify" (a one-task plan).

## Recommendation to user (D-10-B branch)

**SCOPE EXPANSION DETECTED**: surface in `75-00-SUMMARY.md` and present to operator before any Wave 1 refactor commences.

The phase budget no longer fits the original wave structure cleanly. The operator should decide between:

1. **Split Phase 75 into 75 + 75.5** — keep Phase 75 scope to "configure `.pmatignore` + Wave 1a (streamable_http) + verification", and move Wave 1b/2/3 into a follow-on Phase 75.5. This preserves shippable cadence and lets the badge flip green incrementally.
2. **Accept additional refactor effort in Phase 75** — keep the original wave structure but expect each wave plan to grow (no `#[allow]` shortcut). Wave 3's cog-123 → ≤25 reduction in particular is a multi-day effort.
3. **Raise the cog threshold from 25 to 35 (or 50)** — REJECTED in 75-CONTEXT.md "deferred ideas" (would break the CLAUDE.md "complexity ≤25 per function" promise). Listed only for completeness; should not be revisited.

The recommendation is **option 1 (split)** because:
- Wave 1a alone is enough to flip the badge for the most-visible file (`src/server/streamable_http_server.rs`) via real refactor.
- The Wave 3 effort (cog 123 → ≤25 on `evaluate_with_scope`) is itself a meaningful design-grade refactor of the AST evaluator and merits its own discussion + review cycle, not a sub-bullet inside Phase 75.
- `.pmatignore` configuration is decoupled from refactor effort and can ship on its own.

The Wave 1b snapshot baseline (Task 1) and Wave 3 semantic-regression baseline (Task 2) remain valuable artifacts regardless of whether the refactors land in Phase 75 or 75.5 — they provide the safety net for the eventual decomposition work.

## Cleanup

Fixture directory `/tmp/pmat-allow-fixture/` deleted post-test (per Task 4 step 7). Only this artifact persists.
