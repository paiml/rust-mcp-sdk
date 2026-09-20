---
phase: 75-fix-pmat-issues
plan: 00
subsystem: testing
tags: [pmat, insta, cargo-expand, regression-tests, ci-pin, snapshot-tests, semantic-baseline]

requires:
  - phase: pre-existing
    provides: Phase 76 (cargo-pmcp IAM) baseline already on main; pmcp-macros + pmcp-code-mode crates exist
provides:
  - Empirical resolution of D-09 (PMAT path-filter behavior on PMAT 3.15.0)
  - Empirical resolution of D-10 (PMAT honors `#[allow(clippy::cognitive_complexity)]`)
  - Empirical resolution of D-11 (badge gate vs CI gate semantic alignment)
  - Macro-expansion snapshot baselines for the 4 worst Wave 1b offenders (cargo-expand + insta)
  - Semantic regression baseline for the 2 worst Wave 3 offenders + 30-entry corpus
  - PMAT 3.15.0 pinned in `.github/workflows/quality-badges.yml` (Pitfall 1)
  - Authoritative machine-readable PMAT inventory snapshot (single source of truth for wave deltas)
  - SCOPE EXPANSION ALERT for Waves 1-4 (D-10-B: no `#[allow]` shortcut)
  - WAVE 5 ADDITIONAL EDIT REQUIRED (D-11-B: `quality-badges.yml` must be patched)
affects: [75-01, 75-02, 75-03, 75-04, 75-05]

tech-stack:
  added: [cargo-expand (CI install), insta snapshot fixtures]
  patterns:
    - "Per-fixture sub-project pattern for proc-macro crate snapshot tests (avoids the proc_macro = true reachability constraint)"
    - "Semantic-regression-via-corpus pattern: compile-then-evaluate round-trip via PlanCompiler::compile_code"
    - "Empirical-spike-doc pattern: machine-greppable outcome lines (e.g. `outcome: D-10-B`) drive downstream wave conditional logic"
    - "Single-source-of-truth inventory JSON pattern: every wave's delta calculation reads from one committed file via jq"

key-files:
  created:
    - .planning/phases/75-fix-pmat-issues/75-W0-SPIKE-RESULTS.md
    - .planning/phases/75-fix-pmat-issues/pmat-allow-behavior.md
    - .planning/phases/75-fix-pmat-issues/badge-vs-gate-verification.md
    - .planning/phases/75-fix-pmat-issues/pmat-inventory-2026-04-22.json
    - .planning/phases/75-fix-pmat-issues/pmat-inventory-summary.md
    - .planning/phases/75-fix-pmat-issues/deferred-items.md
    - pmcp-macros/tests/expansion_snapshots.rs
    - pmcp-macros/tests/fixtures/.gitignore
    - pmcp-macros/tests/fixtures/example_mcp_tool/Cargo.toml
    - pmcp-macros/tests/fixtures/example_mcp_tool/src/main.rs
    - pmcp-macros/tests/fixtures/example_mcp_server/Cargo.toml
    - pmcp-macros/tests/fixtures/example_mcp_server/src/main.rs
    - pmcp-macros/tests/fixtures/example_mcp_resource/Cargo.toml
    - pmcp-macros/tests/fixtures/example_mcp_resource/src/main.rs
    - pmcp-macros/tests/fixtures/example_mcp_prompt/Cargo.toml
    - pmcp-macros/tests/fixtures/example_mcp_prompt/src/main.rs
    - pmcp-macros/tests/snapshots/expansion_snapshots__snapshot_expand_mcp_tool.snap
    - pmcp-macros/tests/snapshots/expansion_snapshots__snapshot_expand_mcp_server.snap
    - pmcp-macros/tests/snapshots/expansion_snapshots__snapshot_expand_mcp_resource.snap
    - pmcp-macros/tests/snapshots/expansion_snapshots__snapshot_expand_mcp_prompt.snap
    - crates/pmcp-code-mode/tests/eval_semantic_regression.rs
  modified:
    - .github/workflows/quality-badges.yml
    - .github/workflows/ci.yml
    - crates/pmcp-code-mode/src/lib.rs

key-decisions:
  - "D-09 resolved: PMAT 3.15.0 quality-gate has NO --include/--exclude flag; .pmatignore (gitignore-style) is the only gate-honored path-filter mechanism. include_works=false."
  - "D-10 resolved D-10-B: PMAT IGNORES #[allow(clippy::cognitive_complexity)]. P5 (allow-with-Why) is REMOVED from Phase 75 toolkit. All in-tree complexity hotspots must reduce ≤25 by extraction."
  - "D-11 resolved D-11-B: bare `pmat quality-gate --fail-on-violation` fails on 5 dimensions today (complexity 94, duplicate 1545, satd 33, entropy 13, sections 2). Wave 5 MUST patch quality-badges.yml with --checks complexity OR badge stays red post-Wave-4."
  - "Snapshot path for pmcp-macros: per-fixture cargo expand sub-projects (the __test_internal re-export plan was deleted as physically impossible per Rust's proc-macro export prohibition)"
  - "PMAT pin: =3.15.0 --locked in quality-badges.yml; ci.yml gets a comment for Wave 5"
  - "crates/pmcp-code-mode/src/lib.rs: `mod eval` → `pub mod eval` (smallest change to expose evaluator for integration tests; alternative re-export would clutter crate root)"

patterns-established:
  - "Per-fixture cargo expand sub-projects: each fixture is a self-contained Cargo workspace under tests/fixtures/<name>/ with empty `[workspace]` to avoid parent-workspace absorption"
  - "Spike-result frontmatter convention: `outcome: <decision-id-letter>` (e.g. D-10-A) is grep-friendly so downstream waves can branch on outcome via shell"
  - "Inventory-as-truth: ONE committed JSON file is the source for all wave deltas; CONTEXT.md / RESEARCH.md prose counts are explicitly superseded"

requirements-completed: []

duration: 92 min
completed: 2026-04-23
---

# Phase 75 Plan 00: Wave 0 baseline + spike Summary

**Established Wave 0 regression-detection baseline (4 cargo-expand snapshots + 32 evaluator semantic tests + 30-entry corpus), pinned PMAT 3.15.0 in CI, and resolved D-09/D-10/D-11 — both load-bearing assumptions (D-10-B: PMAT ignores `#[allow]`; D-11-B: bare gate fails on 5 dimensions) resolved unfavorably, triggering scope-expansion + Wave 5 alerts for the operator.**

## SCOPE EXPANSION DETECTED (D-10-B)

PMAT 3.15.0 IGNORES `#[allow(clippy::cognitive_complexity)]`. Empirical fixture: a function with cog 41 was annotated per the project-wide `// Why:` template; PMAT continued to flag it (`pmat analyze` value=41, `pmat quality-gate --checks complexity` exit=1).

**Implications for Waves 1-4:** Plans 75-01 through 75-05 were authored under the optimistic D-10-A assumption that `#[allow]`-with-`// Why:` would suppress flagged functions from the gate. That shortcut is now removed — every flagged function must reduce ≤25 by real refactor. Concrete deltas (vs original P5-included projections):

- **Wave 1a** (`src/server/streamable_http_server.rs`, 6 fns): all need helper extraction; estimated moderate additional effort.
- **Wave 1b** (`pmcp-macros/`, 7 fns including 4 expand_* + 3 collect_*_methods): higher risk because macro expansion has interlocking parser/generator state — Task 1's snapshot baseline becomes the load-bearing safety net.
- **Wave 2** (`cargo-pmcp/` 51 fns): the dominant bucket; all must reach ≤25 (no `#[allow]` shortcut for the dispatch-style `execute`/`main` functions).
- **Wave 3** (`crates/pmcp-code-mode/`): the two highest-cog functions in the entire repo (`evaluate_with_scope` cog 123, `evaluate_array_method_with_scope` cog 117) must reach ≤25, not the original ≤50 D-03 ceiling. This is a 5x reduction. Task 2's semantic regression baseline + 30-entry corpus is the critical safety net for that magnitude of restructuring.
- **Wave 4** (examples/, fuzz/, packages/): SMALLER than budgeted — `.pmatignore` absorbs all 8 out-of-tree violations (5 fuzz + 3 packages; examples are already 0). Re-scoped to "configure .pmatignore + verify".

**Operator decision required.** Recommended option (per `pmat-allow-behavior.md`): split Phase 75 into 75 + 75.5 — keep Wave 1a + `.pmatignore` + Wave 5 in 75; move Wave 1b/2/3 into 75.5 for cleaner shippable cadence. Wave 3's evaluator decomposition in particular is design-grade work that merits its own plan + review cycle.

## WAVE 5 ADDITIONAL EDIT REQUIRED (D-11-B)

The bare `pmat quality-gate --fail-on-violation` (the BADGE command) currently fails on 5 dimensions: complexity (94), duplicate (1545), satd (33), entropy (13), sections (2). Even after Waves 1-4 reduce complexity to 0, the bare gate will STILL exit 1 (duplicates/SATD continue to fail) — meaning the README badge will stay RED.

**Wave 5 must update BOTH `ci.yml` (add gate job) AND `quality-badges.yml` (add `--checks complexity` to the existing badge gate command at line ~72).** Recommended patch shape documented in `badge-vs-gate-verification.md`. Without the `quality-badges.yml` patch, Phase 75's stated goal (D-01: "badge flips green") is unmet regardless of refactor effort.

## Performance

- **Duration:** 92 min
- **Started:** 2026-04-23T19:55Z
- **Completed:** 2026-04-23T21:27Z
- **Tasks:** 7 (all completed)
- **Files created:** 23
- **Files modified:** 3

## Accomplishments

- **D-09 empirically resolved:** PMAT 3.15.0 `quality-gate` has no `--include`/`--exclude` flag; `.pmatignore` is the only gate-honored mechanism (drops 5 fuzz violations cleanly). Wave 5 implementation locked in.
- **D-10 empirically resolved (UNFAVORABLY):** PMAT ignores `#[allow(clippy::cognitive_complexity)]`. Surfaced as scope-expansion event for operator decision.
- **D-11 empirically resolved (UNFAVORABLY):** bare gate fails on 5 dimensions; Wave 5 must patch quality-badges.yml.
- **Macro snapshot baseline shipped:** 4 `cargo-expand` snapshots committed for `expand_mcp_{tool,server,resource,prompt}`. Wave 1b can refactor with byte-identical regression detection.
- **Semantic regression baseline shipped:** 22 per-variant + 10 per-method + 30-entry corpus tests for `evaluate_with_scope` and `evaluate_array_method_with_scope`. Wave 3's cog 123→25 refactor has a real safety net.
- **PMAT 3.15.0 pinned in CI:** `cargo install pmat --version =3.15.0 --locked` + version-assertion step in `quality-badges.yml`; comment seeded in `ci.yml` for Wave 5.
- **Authoritative inventory committed:** `pmat-inventory-2026-04-22.json` (166 violations / 91 cog + 75 cyc) is now the single source of truth; CONTEXT.md prose counts (94/73/21/3) are explicitly superseded.

## Task Commits

1. **Task 0: PMAT path-filter empirical spike (D-09)** — `4847d110` (feat)
2. **Task 4: PMAT allow-suppression verification (D-10)** — `2970bf16` (feat)
3. **Task 5: Badge vs CI gate alignment (D-11)** — `52af6cdd` (feat)
4. **Task 6: Authoritative PMAT inventory snapshot** — `8279e379` (feat)
5. **Task 3: Pin PMAT version in CI workflows** — `14cb5fec` (chore)
6. **Task 1: pmcp-macros cargo-expand snapshot baselines** — `ccf8d20a` (test)
7. **Task 2: pmcp-code-mode semantic regression baseline** — `1ca541bd` (test)

_Tasks 4/5/6 were authored as post-review additions (D-10/D-11 + inventory reconciliation) and were executed in spike-first order to surface the scope-expansion alerts before the test-baseline tasks landed._

## Files Created/Modified

### Spike + verification artifacts
- `.planning/phases/75-fix-pmat-issues/75-W0-SPIKE-RESULTS.md` — D-09 path-filter empirical record (`include_works: false`)
- `.planning/phases/75-fix-pmat-issues/pmat-allow-behavior.md` — D-10 allow-suppression empirical record (`outcome: D-10-B`)
- `.planning/phases/75-fix-pmat-issues/badge-vs-gate-verification.md` — D-11 badge/gate alignment empirical record (`outcome: D-11-B`)
- `.planning/phases/75-fix-pmat-issues/pmat-inventory-2026-04-22.json` — authoritative violation snapshot (166 violations, normalized for `jq '.violations | length'`)
- `.planning/phases/75-fix-pmat-issues/pmat-inventory-summary.md` — human-readable summary + per-directory breakdown + Phase-76 reconciliation
- `.planning/phases/75-fix-pmat-issues/deferred-items.md` — pre-existing pmcp-code-mode clippy/dead-code issues logged for Wave 3 intake

### Snapshot baseline (pmcp-macros — Wave 1b safety net)
- `pmcp-macros/tests/expansion_snapshots.rs` — 4 `#[test] fn snapshot_expand_*` shelling out to `cargo expand`
- `pmcp-macros/tests/fixtures/{example_mcp_tool,example_mcp_server,example_mcp_resource,example_mcp_prompt}/{Cargo.toml,src/main.rs}` — 4 self-contained sub-projects (each with empty `[workspace]` to avoid parent absorption)
- `pmcp-macros/tests/fixtures/.gitignore` — ignores per-fixture `target/` (Cargo.lock excluded by repo-root .gitignore)
- `pmcp-macros/tests/snapshots/expansion_snapshots__snapshot_expand_*.snap` — 4 accepted insta snapshots (4.4 KB to 24.8 KB each)

### Semantic regression baseline (pmcp-code-mode — Wave 3 safety net)
- `crates/pmcp-code-mode/tests/eval_semantic_regression.rs` — 32 `#[test]` functions + 30-entry corpus
- `crates/pmcp-code-mode/src/lib.rs` — modified `mod eval` → `pub mod eval` (justification documented in commit + inline comment)

### CI / workflow pins
- `.github/workflows/quality-badges.yml` — pinned `cargo install pmat --version =3.15.0 --locked` + added `pmat --version` assertion step
- `.github/workflows/ci.yml` — added Wave 5 pin-reminder comment near `quality-gate` job

## Decisions Made

All decisions documented in detail in `key-decisions:` frontmatter above. Compressed:

- **Snapshot path:** per-fixture `cargo expand` sub-projects (not `__test_internal` re-export — the latter is impossible per `proc-macro = true`). Both Gemini HIGH and Codex MEDIUM reviewers were correct.
- **`mod eval` → `pub mod eval`:** smallest change to expose the eval functions for the integration test target. Alternative (per-symbol re-export at crate root) would clutter the public surface with internal helpers like `is_truthy`, `to_number`, `evaluate_binary_op`.
- **D-09 chosen path:** `.pmatignore` (Mechanism 6 from spike). The plan's preferred path (a) `--include` flag does not exist on PMAT 3.15.0 `quality-gate`. `--project-path` works but rescopes to a single subtree, not viable for the multi-root gate Wave 5 needs.
- **Numeric promotion pinning:** evaluator promotes integer arithmetic to f64; tests use `5.0` not `5` for arithmetic results. Pinning current behavior (Wave 3 must keep this byte-identical or document an intentional break).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Made `pmcp-code-mode` `mod eval` public**
- **Found during:** Task 2 (semantic regression baseline)
- **Issue:** Task 2 requires calling `evaluate_with_scope` and `evaluate_array_method_with_scope` from a separate `tests/` integration target. The `eval` module was declared `mod eval` (private), so the public `pub fn` symbols inside it were not reachable from outside the crate.
- **Fix:** Changed `mod eval;` → `pub mod eval;` in `crates/pmcp-code-mode/src/lib.rs`. Documented justification inline (smallest change vs cluttering the crate root with per-symbol re-exports of internal helpers).
- **Files modified:** `crates/pmcp-code-mode/src/lib.rs`
- **Verification:** Test file compiles with `use pmcp_code_mode::eval::{evaluate_with_scope, evaluate_array_method_with_scope};` and all 34 tests pass.
- **Committed in:** `1ca541bd` (Task 2 commit)

**2. [Rule 3 - Blocking] Inventory JSON normalization**
- **Found during:** Task 6 (inventory snapshot)
- **Issue:** Plan's `<verify>` regex expects `jq '.violations | length'` to work at the top level. Raw `pmat analyze complexity --format json` output puts violations under `.summary.violations`; top-level keys are `["files", "summary", "top_files_limit"]`. The verify check would have failed with the unmodified output.
- **Fix:** Wrapped the raw output with `jq '{pmat_version, generated_at, source_command, summary: .summary.summary, violations: .summary.violations, files}'` so `.violations` is reachable at the top level. The original `summary` object is preserved.
- **Files modified:** `.planning/phases/75-fix-pmat-issues/pmat-inventory-2026-04-22.json`
- **Verification:** `jq '.violations | length'` returns 166. The plan's `<verify>` regex now passes.
- **Committed in:** `8279e379` (Task 6 commit)

**3. [Rule 1 - Bug] Corpus test arithmetic literals corrected**
- **Found during:** Task 2 (corpus_evaluator_semantic_baseline test failure)
- **Issue:** Initial corpus expected `1 + 2` to evaluate to `3` (integer JSON), but the evaluator promotes integer arithmetic to f64, producing `3.0`. Likewise for unary negation, `mul`, etc. Per-variant binop tests had the same issue.
- **Fix:** Updated 7 expected values to use `.0`-suffixed JSON literals (`5.0`, `28.0`, `-7.0`, `[2.0, 4.0, 6.0]`, `10.0`). Documented as "current behavior pinning, not desired behavior" in the test file's CORPUS comment block.
- **Files modified:** `crates/pmcp-code-mode/tests/eval_semantic_regression.rs`
- **Verification:** All 34 tests pass.
- **Committed in:** `1ca541bd` (Task 2 commit)

**4. [Rule 1 - Bug] Removed nullish-coalescing entries from corpus (compiler unsupported)**
- **Found during:** Task 2 (corpus_evaluator_semantic_baseline test failure)
- **Issue:** Corpus included `null ?? 42` and `0 ?? 99`, expecting them to round-trip via `PlanCompiler::compile_code`. The compiler returns `UnsupportedExpression("nullish coalescing")` even though the evaluator HAS a `ValueExpr::NullishCoalesce` variant.
- **Fix:** Removed those two corpus entries; added inline comment explaining the compiler/evaluator asymmetry. The variant-level tests `variant_nullish_coalesce_left_is_null` and `variant_nullish_coalesce_left_is_zero_keeps_zero` cover the evaluator path.
- **Files modified:** `crates/pmcp-code-mode/tests/eval_semantic_regression.rs`
- **Verification:** Corpus-size sanity test ensures ≥20 entries; current count is 30, well above the floor.
- **Committed in:** `1ca541bd` (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (1 missing critical, 1 blocking, 2 bugs).
**Impact on plan:** All four were necessary for the plan's `<verification>` and `<acceptance_criteria>` to pass. None expanded scope; one (Rule 2 making `eval` public) is a 1-line crate-internal change that doesn't grow the documented public-API surface in any meaningful way.

## Issues Encountered

- **Pre-existing clippy errors in `pmcp-code-mode`** (out of scope): `cargo clippy -p pmcp-code-mode --features js-runtime --tests -- -D warnings` fails with 18 lib + 28 lib-test errors that pre-exist this Wave (verified by re-running with my changes stashed). Logged in `deferred-items.md` for Wave 3 intake. Did NOT block Wave 0 because the new test file itself has zero clippy issues — the pre-existing errors are in `eval.rs` and `executor.rs`.
- **Phase 76 dependency inversion:** Phase 76 (cargo-pmcp IAM) shipped to main BEFORE Phase 75, despite being a "depends-on-75" phase logically. Inventory recorded against post-Phase-76 HEAD. Material divergences from CONTEXT.md baseline (full table in `pmat-inventory-summary.md`):
  - **complexity gate count: UNCHANGED at 94** (Phase 76 added cargo-pmcp branchy code but the gate-relevant count is identical)
  - duplicates tripled (439 → 1545); entropy tripled (4 → 13) — Phase 76 side-effects, not gating
  - in-scope src/ count grew (73 → 86) — Phase 76 IAM-validator paths
  - examples/ count dropped 21 → 0 under the gate (gate-internal filter masks them)

## TDD Gate Compliance

This plan is `type: execute` (not `type: tdd`), so the RED→GREEN→REFACTOR commit sequence is not required. Tasks 1 and 2 are TDD-adjacent (test files added with no production-behavior changes), committed with `test(...)` prefix. No `feat(...)` follow-up was needed because no production code was added — Wave 1b and Wave 3 own the production refactors that these snapshot/regression tests will guard.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

Wave 0 complete. **Wave 1 is BLOCKED — awaiting operator decision** on the D-10-B scope-expansion alert above:

1. **Option A (recommended):** split Phase 75 into 75 + 75.5. Keep Wave 1a (streamable_http) + Wave 4 (.pmatignore) + Wave 5 (CI gate + badge patch) in 75 — flips badge for the most-visible file. Move Wave 1b + 2 + 3 into 75.5 for design-grade refactor cycles.
2. **Option B:** accept additional refactor effort in Phase 75; original wave structure stands but each wave plan grows substantially.
3. **Option C (rejected per CONTEXT.md):** raise the cog threshold from 25 to 35/50.

Wave 5 (whichever option lands) MUST also patch `quality-badges.yml` per D-11-B — without that, no amount of complexity reduction will flip the badge green.

## Self-Check: PASSED

Verified key files exist on disk:
- `.planning/phases/75-fix-pmat-issues/75-W0-SPIKE-RESULTS.md` — present
- `.planning/phases/75-fix-pmat-issues/pmat-allow-behavior.md` — present
- `.planning/phases/75-fix-pmat-issues/badge-vs-gate-verification.md` — present
- `.planning/phases/75-fix-pmat-issues/pmat-inventory-2026-04-22.json` — present, `jq '.violations | length'` = 166
- `.planning/phases/75-fix-pmat-issues/pmat-inventory-summary.md` — present
- `pmcp-macros/tests/expansion_snapshots.rs` — present, 4 `#[test]` functions
- `pmcp-macros/tests/snapshots/*.snap` — 4 files present
- `crates/pmcp-code-mode/tests/eval_semantic_regression.rs` — present, 34 `#[test]` functions

Verified all 7 task commits exist via `git log --oneline --grep '75-00'`:
- `4847d110` Task 0 (D-09)
- `2970bf16` Task 4 (D-10)
- `52af6cdd` Task 5 (D-11)
- `8279e379` Task 6 (inventory)
- `14cb5fec` Task 3 (CI pin)
- `ccf8d20a` Task 1 (snapshots)
- `1ca541bd` Task 2 (regression baseline)

Verified plan-level success criteria:
- Wave 0 commits include `pmat-complexity: NN (was NN)` showing count is unchanged (every commit message)
- `include_works:` boolean committed (`include_works: false`)
- `outcome: D-10-B` committed
- `outcome: D-11-B` committed
- inventory JSON + summary committed
- 4 expand-fn snapshots accepted + committed
- 32 semantic regression tests + 30 corpus entries committed (vs ≥18 + ≥20 plan minimums)
- PMAT pin landed in CI workflow

Verified `pmat quality-gate --fail-on-violation --checks complexity` count is unchanged from baseline: **94** (Wave 0 is purely additive — no production-code refactor).

Verified pmcp-macros test suite passes: `cargo test -p pmcp-macros --all-features` = 91 passed, 5 ignored, 6 suites.

Verified new test files pass: `cargo test -p pmcp-macros --test expansion_snapshots` = 4 passed; `cargo test -p pmcp-code-mode --features js-runtime --test eval_semantic_regression` = 34 passed.

---
*Phase: 75-fix-pmat-issues*
*Completed: 2026-04-23*
