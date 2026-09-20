---
phase: 93
reviewers: [gemini, codex]
reviewed_at: 2026-06-11T17:47:43Z
plans_reviewed: [93-01-PLAN.md, 93-02-PLAN.md, 93-03-PLAN.md, 93-04-PLAN.md, 93-05-PLAN.md, 93-06-PLAN.md, 93-07-PLAN.md]
---

# Cross-AI Plan Review — Phase 93

## Gemini Review

# Phase 93 Plan Review

## Summary
The provided plans are exceptionally well-structured, comprehensive, and accurately reflect the "lift-scrub-and-verify" nature of this phase. The planner successfully internalized the critical insight from the research: the §5 "fixes" (CR-01, CR-02, WR-01) are already implemented in the lighthouse and must be treated as invariants to verify rather than net-new code to write. The plans proactively navigate all six identified pitfalls, with particularly strong handling of the purity boundary, the `Manifest.annotations` struct delta, and the strict customer-identifier scrubbing requirements. 

## Strengths
- **Accurate Scope Framing:** Correctly treats the CR-01, CR-02, and WR-01 fixes as existing code to lift and verify via their test suites (Pitfall 1 avoidance), saving significant re-implementation effort.
- **Purity Boundary Enforcement:** Plan 01 brilliantly extends `make purity-check` with a positive `cargo tree` assertion for the compiler crate while maintaining negative assertions for the served tree, ensuring `umya` remains strictly quarantined.
- **Proactive Delta Handling:** Plan 04 explicitly addresses the `Manifest.annotations` struct mismatch between the lighthouse and the in-repo runtime (Pitfall 2), avoiding inevitable compile errors on hand-built manifest literals.
- **Robust Quality Gates:** Extensive use of `grep` gates to enforce invariants (zero customer identifiers, absence of `delta.abs()`, absence of `build_reference_manifest`) provides strong automated guarantees against regressions or incomplete scrubbing.
- **Smart Decision Deferral:** Plan 07 uses a `checkpoint:decision` for the producer/consumer proof shape (O-2), acknowledging the potential difficulty of byte-for-byte `.xlsx` reverse-engineering and offering a viable structural-equivalence fallback.

## Concerns
- **[LOW] Fuzz Target Dependency:** Plans 02 and 03 introduce fuzz targets. While the plans verify that `cargo fuzz build` succeeds, running a short smoke test (`-runs=10000`) is mentioned in the acceptance criteria but not explicitly captured in the automated `<verify>` block (only the build is automated there). If the smoke run is required to pass the plan, it should be in the automated verify script.
- **[LOW] Tempdir Cleanup in Tests:** Plan 02 and Plan 05 involve file I/O and writing candidate bundles. Ensure that the lifted tests use `tempfile::TempDir` or similar mechanisms so that `cargo test` doesn't leave artifacts behind in the workspace, which could cause state leakage between test runs.

## Suggestions
- **Automate Fuzz Smoke Runs:** In Plans 02 and 03, update the `<verify><automated>` blocks to actually run the short `cargo fuzz run ... -- -runs=10000` smoke test rather than just building the target, ensuring it executes cleanly without panics.
- **Scrubbing Verification:** In Plan 07, Task 4's verification block runs `! grep -rEi 'ufh|towelrad...`. Consider adding `.github/workflows/` or the `Makefile` itself to a broader check to ensure no customer names accidentally leaked into CI configurations or build scripts during the extraction.

## Risk Assessment
**LOW RISK**. The plans are highly detailed, perfectly aligned with the architectural constraints, and demonstrate a deep understanding of the source material. The division into waves and the strict sequencing minimize integration risks, and the reliance on existing, proven lighthouse code (rather than rewriting) significantly reduces the implementation risk.

---

## Codex Review

## Summary

The plan set is strong overall: it respects the lift-and-scrub nature of the phase, keeps the compiler/server purity boundary central, correctly treats CR-01/CR-02/WR-01 as “lift with tests” invariants, and isolates the true new work. The main risks are execution-level: a few plan steps would not compile as written, some tasks depend on artifacts from parallel plans without declaring that dependency, and the provenance/fixture requirements may be harder than the plans imply. Overall risk: **MEDIUM-HIGH**, mostly due to the size of the lift, private-source scrubbing, and the net-new WBCO-07 + D-09 + fixture proof.

## Cross-Plan Strengths

- Clear phase boundary: CLI, Shape-A server, scaffold, second-workbook gate, and `pmcp-code-mode` are kept out of scope.
- Correct dependency cone: `umya`/`quick-xml`/`zip` live only in `pmcp-workbook-compiler`.
- Good security posture: untrusted `.xlsx` ZIP/XML and formula text get fuzz targets, fail-closed behavior, and no-panic expectations.
- Good invariant framing: CR-01, CR-02, WR-01 are planned as lifted tested behavior rather than speculative rewrites.
- Good BA-lifecycle alignment: warnings vs errors, collect-all findings, auto-corpus, and stay-in-Excel manifest correction all match the context.

## Plan 93-01

**Risk: MEDIUM**

**Strengths**
- Establishes the crate and purity boundary before the lift begins.
- Re-export-only rule is the right guard against runtime/compiler schema drift.
- Human verification for `[ASSUMED]` packages is appropriate.

**Concerns**
- **HIGH:** Task ordering is inconsistent. Task 2 says pin `quick-xml`/`zip` to versions printed by Task 3, but Task 3 runs later.
- **HIGH:** `mod ingest; mod dialect; ...` in `lib.rs` will not compile unless empty module files are created in Plan 01.
- **MEDIUM:** `todo!()`/`unimplemented!()` may violate the crate’s panic-deny/clippy posture. Return a typed `CompileError::NotImplemented` instead.
- **MEDIUM:** `cargo tree -p pmcp-workbook-compiler -i umya` likely should be `umya-spreadsheet`.
- **LOW:** Direct `quick-xml`/`zip` pins can create duplicate versions if they do not exactly match the resolver. This needs a precise workflow.

**Suggestions**
- Split pin derivation into Task 2a: add `umya-spreadsheet`, run resolver, inspect resolved transitive versions, then add direct pins only if needed.
- Create empty `mod.rs` files for declared modules, or delay module declarations until each plan.
- Make the purity check use package names exactly: `umya-spreadsheet`, `quick-xml`, `zip`.

## Plan 93-02

**Risk: HIGH**

**Strengths**
- Correctly treats provenance as a security boundary.
- Explicitly avoids leaking `umya` types through public APIs.
- Adds the necessary net-new umya-fabrication refusal test.

**Concerns**
- **HIGH:** “Real Excel accepted” may be hard to test reproducibly in CI unless a known real-Excel fixture is committed.
- **HIGH:** Detecting umya fabrication via `calcId=122211` and/or missing app-version may produce false positives for some real Excel files. This needs a well-documented confidence model.
- **MEDIUM:** Fuzz target over `pub(crate)` raw reader may require exposing test/fuzz-only hooks; the plan mentions this but does not specify the pattern.
- **MEDIUM:** Ingest tests requiring cached formula values may fail if test fixtures were written by a library that does not store cached values.

**Suggestions**
- Define provenance classes explicitly: `ExcelTrusted`, `NonExcel`, `UmyaFabricated`, `UnknownStale`, etc.
- Commit one minimal known-good Excel-saved fixture, or soften “real_excel_accepted” to “trusted_fixture_accepted” with documented provenance.
- Add hard size limits for ZIP entry size, total decompressed bytes, XML depth, and cell count.

## Plan 93-03

**Risk: MEDIUM**

**Strengths**
- Correct focus on whitelist-at-parse-time.
- Avoids `petgraph` and local WHITELIST drift.
- Adds cycle detection and parser fuzzing.

**Concerns**
- **HIGH:** Plan 93-03 depends only on 93-01 but consumes `WorkbookMap` from Plan 02. Since 93-02 and 93-03 are same wave, this is a hidden dependency.
- **MEDIUM:** “Parser returns an error / pushes a dialect ERROR finding” mixes parser errors and lint findings. The API boundary should be crisp.
- **MEDIUM:** Formula parsing in Excel has many edge cases: sheet-qualified refs, quoted sheet names, ranges, absolute refs, unary operators, string literals, percent, and error values. The plan should list which are supported vs rejected.

**Suggestions**
- Either make 93-03 independent of `WorkbookMap` by testing parser/DAG on synthetic inputs, or declare dependency on 93-02.
- Add a supported-formula matrix tied to `pmcp-workbook-dialect::WHITELIST`.
- Add recursion/depth limits before fuzzing, not only as a reaction to fuzz crashes.

## Plan 93-04

**Risk: HIGH**

**Strengths**
- Correctly identifies `Manifest.annotations` as a likely compile-time and golden-output trap.
- Good handling of warning-vs-error split for reconcile.
- Strong grep gate for `delta.abs()`.

**Concerns**
- **HIGH:** “Confirm pure-Rust reconcile parity” is underspecified. It needs a concrete oracle or fixture set; otherwise it becomes subjective.
- **HIGH:** Manifest synthesis is likely the most customer-specific part of the lighthouse. Scrubbing while preserving behavior will be difficult.
- **MEDIUM:** The threat model mentions length-cap and sanitization of BA strings, but Task 1 does not define exact limits or escaping rules.
- **MEDIUM:** “Range/named-range DV falls back to dynamic input” must ensure default values are schema-valid and do not later conflict with WR-01.

**Suggestions**
- Define metadata limits now: max `meaning`, `unit`, enum label length, input/output count, sheet name length.
- Add tests for formula cached values with text, bool, blank, and Excel error cells.
- Make O-1 parity a named test suite, not just a summary note.

## Plan 93-05

**Risk: MEDIUM**

**Strengths**
- Correctly ties bundle hashing to runtime helpers.
- Good emphasis on `bundle_id`, annotations, and Phase 92 consumer compatibility.
- Treats classifier completeness as a security property.

**Concerns**
- **HIGH:** “Verified against the COMMITTED manifest” may require a committed manifest before 93-07’s fixture exists. The plan should clarify whether it uses the Phase 92 golden manifest.
- **MEDIUM:** Byte-stable JSON output is not explicitly specified. If golden diffing is expected later, key ordering and pretty formatting matter.
- **MEDIUM:** `bundle_lock.rs` may be unnecessary if runtime already owns the lock builder; duplicating wrapper logic risks drift.

**Suggestions**
- Standardize JSON serialization settings in artifact emit: deterministic ordering, pretty/no-pretty, newline policy.
- Add a direct loader test using `pmcp-server-toolkit::workbook` against an emitted temp bundle.
- Treat any wrapper around runtime hash helpers as thin and heavily tested.

## Plan 93-06

**Risk: HIGH**

**Strengths**
- Correctly identifies D-09 auto-corpus as net-new.
- Content-hash-bound approvals and promote-twice tests are well scoped.
- Good first-version no-op handling.

**Concerns**
- **HIGH:** Auto-corpus replay requires a stable evaluator for both prior and candidate bundles. The plan assumes this is already available but does not name the exact execution API.
- **HIGH:** Numeric boundary generation is underspecified. “Default ± step” requires step/min/max/units or heuristics.
- **MEDIUM:** “No cases.json” grep may be too strict if test fixtures or migration code mention it; better gate production paths.
- **MEDIUM:** Approval storage location and atomic write behavior are not defined.

**Suggestions**
- Define corpus cap and policy concretely, e.g. max 50 cases, deterministic truncation order, default case always first.
- Add atomic write requirements for approvals and new bundle dirs: write temp dir, fsync where practical, rename.
- Include tests for approval mismatch when only prior hash changes, only candidate hash changes, and only region deltas change.

## Plan 93-07

**Risk: HIGH**

**Strengths**
- Correct capstone: it proves the generic driver and manifest-driven path.
- Good explicit decision point for byte-identical vs structural-regenerate.
- Includes full quality/purity/grep/fuzz phase gate.

**Concerns**
- **HIGH:** Authoring `tax-calc.xlsx` “with cached values” using `rust_xlsxwriter` may not produce cached formula results unless explicitly supported and set. Real Excel may be required.
- **HIGH:** `rust_xlsxwriter` may produce non-Excel provenance that Plan 02’s gate refuses, depending on WBCO-07 detection design.
- **HIGH:** A blocking decision in Wave 6 means the phase cannot be fully autonomous. That is acceptable, but it should be surfaced earlier because it can change artifact strategy.
- **MEDIUM:** Byte-identical output may be unrealistic after moving from synthetic runtime fixture to compiler-emitted artifact. Structural equivalence may be the practical default.
- **MEDIUM:** `make quality-gate` after a 13K LOC lift may uncover broad clippy issues. The plans do not reserve scope for mechanical refactors.

**Suggestions**
- Move the O-2 decision earlier, before artifact emit design hardens.
- Prefer structural equivalence unless byte-identical is known feasible from a spike.
- Define “structural equivalence” precisely: normalized JSON equality, required members, lock hash recomputation, server-toolkit loader acceptance, and named output behavior.
- Add a fallback fixture provenance rule: test fixtures may carry a committed provenance override only if production compile paths reject fabricated provenance.

## Highest-Priority Fixes Before Execution

1. Fix 93-01 compile hazards: module files, `todo!()`, package names, and pin derivation order.
2. Resolve the hidden 93-03 dependency on 93-02’s `WorkbookMap`.
3. Specify WBCO-07 provenance classes and false-positive handling before coding.
4. Move or pre-decide the tax-calc proof shape earlier.
5. Define deterministic serialization and structural-equivalence rules before artifact/gate work.
6. Specify auto-corpus execution API, grid cap, numeric boundary policy, and approval storage location.

## Overall Risk Assessment

**Overall risk: MEDIUM-HIGH.** The architecture is sound and the phase is well decomposed, but the implementation is a large private-code lift with strict scrubbing, security-sensitive parsing, and several newly designed behaviors. The riskiest pieces are not CR-01/CR-02/WR-01; they are WBCO-07 fabricated-provenance refusal, D-09 auto-corpus generation, and the neutral workbook proof. With the dependency-order fixes and earlier decisions above, the plans should be executable and aligned with the phase goal.

---

## Consensus Summary

Two independent reviewers (Gemini, Codex) agree the **architecture and decomposition are sound** and that the planner correctly internalized the headline insight — CR-01/CR-02/WR-01 are lift-with-tests-and-verify-green invariants, not net-new rewrites. They **diverge on residual risk**: Gemini rates the set **LOW** (highly detailed, well-aligned, proven source); Codex rates it **MEDIUM-HIGH**, focused on *execution-level* hazards Gemini did not surface (compile-order bugs in 93-01, a hidden cross-plan dependency, and the difficulty of the net-new fixture/provenance work). The divergence is reconcilable: the *design* is low-risk; the *net-new execution surface* (WBCO-07 refuse, D-09 auto-corpus, neutral-fixture byte-identical proof) is where the real risk concentrates — exactly the pieces both reviewers independently single out.

### Agreed Strengths
- Scope framing: CR-01/CR-02/WR-01 treated as lifted+tested invariants, not speculative rewrites (both).
- Purity boundary: `umya`/`quick-xml`/`zip` quarantined to the compiler; positive `cargo tree` assertion + served-tree negatives (both).
- `Manifest.annotations` struct delta proactively handled to avoid compile/golden-output traps (both).
- Security posture: untrusted `.xlsx` ZIP/XML + formula text get fuzz targets, fail-closed, no-panic; classifier completeness framed as a security property (both).
- Strong grep-gates (zero customer identifiers, no `delta.abs()`, no `build_reference_manifest`) (both).

### Agreed Concerns (highest priority — raised or implied by both)
- **[HIGH] Neutral-fixture authoring is the load-bearing risk.** Can `rust_xlsxwriter` emit *cached formula values* (needed as the reconcile oracle) AND will its own provenance survive the WBCO-07 refuse gate? Real Excel may be required. Codex flags this across 93-02/93-07; Gemini flags fixture/test-artifact hygiene. **→ resolve the O-2 proof shape EARLIER (Codex: move it before artifact-emit design hardens), and pre-decide the fixture provenance-override rule for test paths.**
- **[HIGH] WBCO-07 false-positive model.** Detecting umya via `calcId=122211` / missing app-version risks rejecting some real Excel files. Needs an explicit provenance-class model (`ExcelTrusted` / `NonExcel` / `UmyaFabricated` / `UnknownStale`) and a documented confidence/false-positive policy before coding (Codex HIGH).
- **[MEDIUM] Determinism unspecified.** Byte-identical golden diffing depends on deterministic JSON serialization (key ordering, pretty/newline policy) — not yet pinned in 93-05 (Codex). Define "structural equivalence" precisely as the fallback.

### Codex-only Concerns Worth Acting On (execution hazards)
- **[HIGH] 93-01 compile-order bug:** Task 2 pins `quick-xml`/`zip` to versions printed by Task 3, which runs *later*. Split pin-derivation (add `umya-spreadsheet` → resolve → inspect → pin only if needed).
- **[HIGH] 93-01 module declarations won't compile:** `mod ingest; mod dialect; …` in `lib.rs` needs empty `mod.rs` stubs created in Plan 01, or defer declarations per-plan. Replace `todo!()`/`unimplemented!()` with a typed `CompileError::NotImplemented` (panic-deny posture).
- **[MEDIUM] Package name:** `cargo tree -i umya` should be `-i umya-spreadsheet`.
- **[HIGH] Hidden 93-03 → 93-02 dependency:** 93-03 consumes `WorkbookMap` (from 93-02) but they share Wave 2. Either make 93-03 test parser/DAG on synthetic inputs, or declare the dependency (which breaks the parallelism).
- **[MEDIUM] D-09 underspecified:** name the exact bundle-evaluation API for replay; define numeric-boundary generation (step/min/max) and the grid cap concretely; define approval storage location + atomic-write (temp→rename).

### Divergent Views
- **Overall risk:** Gemini **LOW** vs Codex **MEDIUM-HIGH**. Gemini judges the design; Codex judges the execution of a ~13K-LOC private-code lift with scrubbing + net-new security-sensitive parsing. Treat Codex's MEDIUM-HIGH as the operative figure for *execution planning* (reserve scope for mechanical clippy refactors after the lift; expect the fixture/provenance work to need a spike).
- **Fuzz automation:** Gemini wants the `cargo fuzz run -runs=N` smoke test in the automated `<verify>` block (not just `cargo fuzz build`); Codex treats build-green as sufficient at plan-gate. Low-cost to adopt Gemini's stricter bar.

### Recommended Action
Most concerns are **execution-detail refinements that strengthen the plans without changing scope.** The two structural items that justify a `--reviews` replan pass: (1) the **93-01 compile-order / module-stub / package-name fixes**, and (2) the **93-03↔93-02 `WorkbookMap` dependency** (decide: synthetic-input independence vs. declared dependency + wave change). The net-new-risk items (move O-2 earlier; WBCO-07 provenance-class model; deterministic serialization; D-09 concretization) are best folded into the same replan or pinned as explicit task decisions.

→ `/gsd:plan-phase 93 --reviews`
