---
phase: 96
reviewers: [gemini, codex]
reviewed_at: 2026-06-15
plans_reviewed: [96-01-PLAN.md, 96-02-PLAN.md, 96-03-PLAN.md, 96-04-PLAN.md, 96-05-PLAN.md]
---

# Cross-AI Plan Review — Phase 96

## Gemini Review

# Plan Review: Phase 96 (Shape B Scaffold + Dialect-Version + Generalization)

This review covers plans **96-01** through **96-05**, which finalize the v2.3 milestone for the PMCP Workbook CodeLanguage.

## 1. Summary
The plan set is exceptionally well-structured, demonstrating a deep understanding of the existing "Shape A" and "Toolkit" patterns established in Phases 91–95. It successfully navigates the "authoring landmine" (the inability to generate valid `.xlsx` fixtures in-repo without specific Excel identity) by building a dedicated test helper. The strategy for generalization validation—using a divergent loan calculator and a quirk corpus—is robust and directly addresses the RFC §5 requirements. The dependency ordering is logical, moving from foundational versioning to authoring infrastructure, and finally to the specific fixtures and generalization proofs.

## 2. Strengths
- **Surgical Integration:** Plan 01 correctly identifies the risk of conflating the *dialect* version with the *bundle* version and mandates a sibling reader, preserving the stability of the Phase 94 CLI.
- **Purity-First Scaffolding:** Plan 02's focus on `default-features = false` in the scaffolded `Cargo.toml` ensures that new users don't accidentally pull `umya` or `swc` into their served binary tree, maintaining the project's strict purity invariants.
- **Critical Path De-risking:** Plan 03 proactively resolves the fixture-authoring gap. By building a `#[cfg(test)]` helper that injects genuine Excel identity, it unblocks the authoring of the ~10 new workbooks required for validation.
- **Divergence Strategy:** The choice of a "Rate-Tier Table" for the loan workbook (Plan 04) is excellent. It exercises `VLOOKUP` and `INDEX-MATCH`, which are structurally distinct from `tax-calc`'s percentage-based arithmetic, providing a strong proof of manifest-driven generalization.
- **Two-Layer Quirk Validation:** Plan 05's approach of testing quirks at both the `scalar_eval` level (logic) and the `reconcile` level (file integration) ensures that edge cases are correct both in isolation and when compiled through the full pipeline.

## 3. Concerns
- **Serialized `lib.rs` Edits (LOW):** Edits to `pmcp-workbook-compiler/src/lib.rs` are required in Plans 01, 03, 04, and 05. While the plans acknowledge this, the executor must ensure that module declarations and re-exports don't collide or get duplicated during wave-based implementation.
- **1900-Leap Quirk Expressibility (MEDIUM):** There is a risk that the 1900-leap serial quirk cannot be meaningfully expressed using only the 13 whitelisted functions without `DATE()` or `DATEVALUE()`. Plan 03's spike is the correct mitigation, but the plan should be prepared to accept a "scalar-only" disposition if the formula DAG cannot reach the serial logic.
- **Scaffold Payload Size (LOW):** Plan 02 carries both the `.xlsx` and the compiled bundle in the template. While necessary for the "works out of the box" experience, ensure these are tracked as `include_bytes!` assets so they don't bloat the `cargo-pmcp` source code unnecessarily.

## 4. Suggestions
- **Version Advisory Severity:** In Plan 01, for the "absent -> baseline" path, use a `Severity::Info` finding that explicitly recommends adding a `pmcp_dialect_version` cell for future-proofing. This encourages good habits without breaking existing fixtures.
- **Loan Workbook Metadata:** For the loan workbook (Plan 04), ensure at least one output has a custom unit (e.g., `USD` or `percent`) to verify that the manifest-driven `calculate` tool correctly projects unit metadata for non-golden workbooks.
- **Scaffold Test Portability:** In Plan 02, ensure the golden test for `main.rs` drift handles line-ending differences (LF vs CRLF) if the templates are authored on different operating systems, as this often trips up string-equality assertions in scaffolds.

## 5. Risk Assessment
**Overall Risk: LOW**

The phase is primarily additive and utilizes existing, proven patterns (the `sql-server` scaffold, the `tax-calc` reconcile path, and the `reemit_golden` proof structure). The highest technical risk is the fixture-authoring recipe, which is addressed early in Wave 2 (Plan 03). The strict adherence to the "purity invariant" and the "fail-closed" philosophy established in earlier phases makes this a safe and predictable conclusion to the v2.3 milestone.

---

## Codex Review

## Overall Assessment

The plan set is strong and mostly aligned with Phase 96's four success criteria. It correctly separates WBDL-02, WBCL-05, the fixture-authoring spike, the WBEX-01 generalization gate, and the WBEX-02 quirk corpus. The biggest risks are not architectural scope creep; they are verification fidelity and fixture authoring mechanics. In particular, the WBEX-01 plan currently proves `load_bundle`/cell-map structure but does not clearly exercise served `tools/list` and `get_manifest`, and the `#[cfg(test)]` fixture-author helper may be awkward or insufficient for creating committed `.xlsx` fixtures without source-tree-mutating tests.

## 96-01 Dialect Version

### Summary

Good plan. It correctly isolates dialect version from bundle version, keeps absent declaration backward-compatible, adds doc/const drift protection, and includes property/fuzz coverage. Main risk is underspecified integration into the compile pipeline and a fuzz verification command that can silently pass without actually checking the target.

### Strengths

- Keeps `read_workbook_version` untouched, avoiding Phase 94 regression.
- Uses fail-closed compatibility for incompatible or malformed declarations.
- Adds spec-doc drift guard parallel to existing whitelist guard.
- Includes unit, property, and fuzz coverage for the new parser/compat surface.

### Concerns

- **MEDIUM:** "Typed `CompileError`" is implemented as `CompileError::Lint(format!(...))`; if the codebase has or expects structured lint findings, this may be weaker than the plan language implies.
- **MEDIUM:** Patch-version behavior is ambiguous. Accepting `1.0.999` when supported is `1.0` may be fine, but the rule should be explicit.
- **MEDIUM:** The plan says "prefer stage-1 collect-all" but does not define the exact finding type or how it participates in existing stage-1 diagnostics.
- **LOW:** The fuzz verify command includes `|| echo "fuzz target present..."`, which can mask a broken fuzz target.

### Suggestions

- Define version comparison precisely: accepted grammar, optional patch behavior, whitespace handling, leading zero handling, and maximum integer size.
- Make the parser/compat function public or `pub(crate)` in a way the fuzz target can actually call.
- Replace the fuzz verification with a non-masking command, or explicitly mark fuzz execution as a manual phase gate.
- Add an integration test with a real or synthetic workbook map for compatible, absent, newer minor, different major, and malformed versions.

### Risk Assessment

**MEDIUM.** The design is sound, but parser policy and pipeline integration need tighter acceptance criteria.

## 96-02 Shape B Scaffold

### Summary

The scaffold plan is well targeted and mirrors existing CLI patterns. It correctly emphasizes `default-features = false`, `workbook-embedded`, drift-locking, and byte equality for the embedded bundle. The main risks are publishability of template assets, hardcoded crate versions, and whether the proposed example can actually call private template modules.

### Strengths

- Mirrors the existing `sql-server` scaffold rather than inventing a new path.
- Explicitly guards against `code-mode`/served-tree dependency leakage.
- Drift-locks emitted `main.rs` to the canonical toolkit example.
- Adds bundle-byte equality tests to prevent stale or corrupted scaffold assets.

### Concerns

- **HIGH:** The example `cargo-pmcp/examples/workbook_server_scaffold.rs` may not compile if `templates::workbook_server` is not exposed through a library target. Examples cannot normally import private binary modules.
- **HIGH:** Template asset strategy needs to work after publishing `cargo-pmcp`. Copying from workspace fixture paths at generate-time may fail outside the monorepo unless assets are embedded or packaged.
- **MEDIUM:** Hardcoded versions `pmcp = 2.9.0` and `pmcp-server-toolkit = 0.1.0` may drift from release state.
- **MEDIUM:** The plan says "runnable crate" but only makes full server boot/build smoke optional.
- **LOW:** Drift-lock normalizer may become brittle if the example gains unrelated harness code.

### Suggestions

- Prefer embedding scaffold assets inside `cargo-pmcp` with `include_bytes!`/static asset lists, and add a packaging test or `cargo package --list` check.
- Replace the example with either an integration test invoking the CLI or expose a small public scaffold API intentionally.
- Add a scaffold-build smoke test that runs `cargo check` inside the generated crate, even if server boot remains optional.
- Derive scaffold dependency versions from workspace/package metadata where feasible, or add a test that fails when hardcoded versions drift.

### Risk Assessment

**MEDIUM-HIGH.** The implementation pattern is right, but publishability and example visibility are real execution risks.

## 96-03 Fixture Author + 1900 Spike

### Summary

This plan addresses the correct critical-path risk: `.xlsx` authoring. The spike for the 1900 date quirk is also prudent. The biggest issue is operational: a `#[cfg(test)]` helper is useful for tests, but it is not a clean mechanism for generating committed fixture files unless there is a deterministic generator workflow outside normal tests.

### Strengths

- Directly tackles the known `.xlsx` fixture-authoring gap.
- Keeps fixture override `#[cfg(test)]`, preserving production freshness behavior.
- Requires cached formula results via `Formula::set_result`.
- Explicitly prevents adding `DATE`/`DATEVALUE` to the dialect.

### Concerns

- **HIGH:** A `#[cfg(test)]` helper does not by itself provide a clean way to create committed `loan-calc.xlsx` and quirk fixtures. Tests should not mutate the source tree.
- **HIGH:** The claim "genuine Excel identity" needs a direct assertion against the actual provenance classifier, not just a compile success through the trusted-fixture path.
- **MEDIUM:** The 1900 leap-year quirk may become a documented limitation, but WBEX-02 explicitly names it. If downgraded, the phase may need a clear traceability note explaining how the requirement is still satisfied.
- **MEDIUM:** Committing `leap1900-probe.xlsx` may be unnecessary if the spike ends in scalar-only or limitation disposition.
- **LOW:** The plan does not say how generated binary fixtures remain reproducible.

### Suggestions

- Add a dev-only generator path, such as an ignored test with explicit env var, an example, or an xtask-style command, that writes fixtures deterministically when intentionally run.
- Add tests that inspect provenance classification directly: Excel-trusted identity, expected stale/fullCalc behavior, and production refusal.
- Record generation metadata for committed fixtures: generator function/name, inputs, expected output cells, and reason for override.
- For the 1900 spike, require the disposition to map explicitly to WBEX-02 traceability.

### Risk Assessment

**HIGH.** The concept is right, but fixture generation mechanics need tightening before Plans 04 and 05 depend on them.

## 96-04 Loan Workbook Generalization Gate

### Summary

The workbook design direction is good: loan/rate-tier domain, whitelist-legal formulas, multiple outputs, and a compile/load proof. However, the current proof may stop one layer short of the roadmap wording. WBEX-01 specifically requires served `get_manifest` / `tools/list` schema reflection, while the plan mostly asserts `load_bundle` and `cell_map`.

### Strengths

- Uses a genuinely different workbook domain from `tax-calc`.
- Avoids deferred features like PMT, POWER, exponentiation, and arbitrary row iteration.
- Requires multiple `out_*` outputs, preventing a single privileged answer.
- Includes production-refusal counter-test for the test-authored fixture.

### Concerns

- **HIGH:** The plan does not clearly exercise served `tools/list` and `get_manifest`; `load_bundle`/`cell_map` alone is not the full WBEX-01 gate.
- **MEDIUM:** "Zero per-workbook Rust" should be clarified as "zero per-workbook served/runtime Rust"; test-only fixture-authoring/assertion Rust is acceptable but should not be confused with generated server logic.
- **MEDIUM:** The loan calculation could become a toy lookup rather than a credible mortgage/loan model unless formulas are specified enough to be meaningful.
- **LOW:** "No privileged headline" is asserted mostly by output count; names/schema should also be checked.

### Suggestions

- Add a test that builds the generic workbook server/toolkit and invokes or inspects `tools/list` and `get_manifest`, asserting loan-specific input/output schema.
- Assert the generic five tool names remain unchanged while their manifest/schema payload differs from `tax-calc`.
- Add negative assertions that tax-specific fields are absent and loan-specific fields are present.
- Document the formula DAG and exact named inputs/outputs in the test or fixture metadata.

### Risk Assessment

**MEDIUM-HIGH.** The fixture idea is strong, but the served-schema proof must be upgraded to fully satisfy WBEX-01.

## 96-05 Excel Quirk Corpus

### Summary

This is a good two-layer test plan and stays within scope. It correctly uses runtime scalar tests plus compiler reconcile fixtures. The main risks are ambiguity in what some quirks mean without date/string functions, and whether the reconcile harness actually compares computed executor values to cached workbook oracles through the same path as the golden gate.

### Strengths

- Implements both required layers: fast scalar tests and penny-reconcile fixtures.
- Uses `within_tol` rather than exact float equality for money-like values.
- Explicitly anchors half-rounding to `excel_round(1594.925, 2) == 1594.93`.
- Defers 1900 handling to the Plan 03 disposition instead of adding date functions.

### Concerns

- **MEDIUM:** "Empty-cell coercion" and "text→number coercion" need formula-context definitions; Excel behavior differs by operator/function context.
- **MEDIUM:** If the 1900 quirk becomes scalar-only or limitation-only, the corpus may not fully meet the roadmap wording unless traceability is explicit.
- **MEDIUM:** The plan says "compile then verify through `within_tol`," but should specify how computed values and cached oracle values are retrieved to avoid accidentally testing only compile success.
- **LOW:** Plan 05 depends on 96-04 even though it mostly depends on 96-03; this is safe but serializes more than necessary.

### Suggestions

- Define each quirk as a small table: formula, cached Excel oracle, runtime expected value, reconcile cell key.
- Require at least one reconcile assertion per named roadmap quirk unless Plan 03 explicitly marks it impossible.
- Add production-refusal spot checks for the quirk fixtures, or one shared test proving the quirk fixture class only passes via trusted override.
- Keep the corpus capped, but make the final summary map each quirk back to WBEX-02.

### Risk Assessment

**MEDIUM.** The structure is good; correctness depends on precise fixture/oracle definitions.

## Cross-Plan Risks

- **HIGH:** The plans do not mention the repo's contract-first requirement: update contract YAML under `../provable-contracts/contracts/<crate>/` and run `pmat comply check`. If enforced for this phase, that is a planning gap.
- **HIGH:** WBEX-01 must prove served `get_manifest` / `tools/list`, not only bundle loading.
- **HIGH:** Fixture generation needs a non-mutating, reproducible workflow for committed `.xlsx` files.
- **MEDIUM:** Final phase verification should include `make quality-gate` and `make purity-check`, not just per-plan crate tests.
- **MEDIUM:** Multiple plans touch `crates/pmcp-workbook-compiler/src/lib.rs`, but the wave dependencies serialize them, so conflict risk is manageable.
- **LOW:** No visible scope creep into row iteration, capability cells, validation lists, registry storage, or `pmcp-code-mode`.

## Final Risk

**Overall risk: MEDIUM-HIGH.** The plans are directionally correct and well researched, but two acceptance gaps should be fixed before execution: make fixture authoring reproducible without source-tree-mutating tests, and upgrade the loan gate to assert actual served `tools/list` / `get_manifest` schema for the second workbook.

---

## Consensus Summary

Two independent reviewers (Gemini, Codex) agree the plan set is well-structured, pattern-faithful, and free of scope creep. They diverge sharply on overall risk: **Gemini → LOW** (sees a primarily additive phase on proven rails), **Codex → MEDIUM-HIGH** (sees real verification-fidelity and fixture-mechanics gaps that should be closed before execution). The divergence is substantive, not stylistic — Codex performed a deeper per-plan acceptance-criteria audit and surfaced four HIGH items Gemini did not weight.

### Agreed Strengths (raised by both)
- **Sibling dialect-version reader** (96-01) cleanly isolated from the Phase-94 bundle-version reader — no regression risk.
- **Purity-first scaffold** (96-02): `default-features = false` prevents `code-mode`/`umya`/`swc` leaking into the served tree.
- **Critical-path de-risking** (96-03): the `.xlsx` fixture-authoring gap is tackled early, before 04/05 depend on it.
- **Divergent loan workbook** (96-04): rate-tier `VLOOKUP`/`INDEX-MATCH` is structurally distinct from `tax-calc`, a credible generalization proof; multiple `out_*` outputs avoid a privileged headline.
- **Two-layer quirk validation** (96-05): `scalar_eval` units + penny-reconcile fixtures; `within_tol` instead of float `==`.
- **No scope creep** into the deferred-by-design items (row iteration, capability cells, validation lists, registry store, `pmcp-code-mode`).

### Agreed Concerns (raised by both — highest priority)
1. **1900-leap-year quirk expressibility (MEDIUM, both).** May not be expressible over the 13-fn whitelist without `DATE()`/`DATEVALUE()`. The 96-03 spike is the right mitigation, but if it lands "scalar-only"/"documented limitation", the phase needs an **explicit WBEX-02 traceability note** explaining how the named requirement is still satisfied.
2. **Serialized `lib.rs` edits (LOW, both).** 01→03→04→05 each append a module declaration to `pmcp-workbook-compiler/src/lib.rs`; wave ordering makes conflict risk manageable, but the executor must avoid duplicate/colliding `mod` declarations.
3. **Fixture-authoring mechanics (Gemini LOW / Codex HIGH).** Both flag how committed `.xlsx` assets are produced and tracked — Gemini wants `include_bytes!` asset tracking; Codex wants a **non-mutating, reproducible generator path** (ignored/env-gated test, example, or xtask) plus a **direct provenance-classifier assertion** rather than relying on compile-through-the-trusted-path as the only proof.

### Divergent Views (worth investigating before execute)
- **Overall risk: LOW (Gemini) vs MEDIUM-HIGH (Codex).** Resolve by addressing Codex's HIGH items; if they prove cheap (they appear to be), the phase converges toward LOW.
- **WBEX-01 proof depth (Codex HIGH, Gemini silent).** Codex argues 96-04 currently asserts `load_bundle`/`cell_map` but the roadmap wording requires the *served* `get_manifest`/`tools/list` schema to reflect the loan workbook's own inputs (with the five generic tool names unchanged, payload differing, tax-specific fields absent). **This is the single most important item** — it is the literal WBEX-01 generalization gate. Recommend upgrading the 96-04 proof.
- **Scaffold example visibility & publishability (Codex HIGH, Gemini silent).** Codex warns `cargo-pmcp/examples/workbook_server_scaffold.rs` may not compile if `templates::workbook_server` is a private binary module, and that generate-time copying from workspace fixture paths breaks once `cargo-pmcp` is published outside the monorepo. Recommend `include_bytes!`-embedded assets + a `cargo package --list` / scaffold-`cargo check` smoke test, and either an integration test or an intentional public scaffold API instead of an example.
- **Contract-first gap (Codex HIGH, Gemini silent).** Codex notes CLAUDE.md's contract-first requirement (`../provable-contracts/contracts/<crate>/` + `pmat comply check`) is unmentioned. Verify whether it is enforced for this phase; if so, it is a planning gap to close.
- **96-01 parser policy (Codex MEDIUM, Gemini silent).** Codex wants explicit patch-version/grammar/whitespace rules, a non-masking fuzz verify command, and a callable `pub(crate)` parser for the fuzz target.

### Recommended Action
The bulk of the value is in Codex's HIGH items. Suggest feeding this back via `/gsd:plan-phase 96 --reviews`, focusing the replan on:
1. **96-04** — upgrade the WBEX-01 proof to assert served `tools/list`/`get_manifest` schema (loan-specific in/out, generic tool names unchanged, tax fields absent).
2. **96-03** — make committed-fixture generation non-mutating + reproducible, and assert the provenance classifier directly.
3. **96-02** — resolve example/private-module visibility + post-publish asset packaging (embed via `include_bytes!`, add `cargo package`/`cargo check` smoke).
4. **96-01** — tighten version-parser acceptance criteria + non-masking fuzz verify.
5. Confirm whether **contract-first (`pmat comply check`)** applies; add the final **`make quality-gate` + `make purity-check`** gate explicitly.
