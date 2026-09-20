---
phase: 91
reviewers: [codex, gemini]
reviewed_at: 2026-06-10T03:28:05Z
plans_reviewed: [91-01-PLAN.md, 91-02-PLAN.md, 91-03-PLAN.md]
---

# Cross-AI Plan Review — Phase 91

## Codex Review

## Summary

The plan set is strong on phase sequencing and captures the main architectural intent: runtime first, dialect contract second, purity gate third. It correctly avoids pulling the reader/linter execution into Phase 91 and is unusually explicit about the reader-free boundary. The main issues are around enforcement soundness: the purity-check script as described can produce false passes, the deferred `cargo-deny` layer conflicts with the written success criteria unless formally re-scoped, and the plans do not explicitly prove the Toyota Way constraints, especially cognitive complexity <=25 and zero SATD, for the lifted lighthouse code.

## Strengths

- Clear dependency order: `pmcp-workbook-runtime` before `pmcp-workbook-dialect`, then purity/traceability wiring.
- Good scope discipline: `WorkbookMap`, `linter.rs`, `umya`, and `quick-xml` stay out of Phase 91.
- Correct SDK convention awareness: literal `0.1.0`, `thiserror = "2"`, no unnecessary `pmcp` dependency, writer-only `rust_xlsxwriter`.
- Good non-vacuous purity intent: negative reader/JS checks plus positive `rust_xlsxwriter` assertion.
- The dialect doc to const binding test is the right control for WBDL-01.
- The `cargo-deny` limitation is surfaced honestly instead of pretending the layer is cleanly enforceable.

## Concerns

- **HIGH: `make purity-check` can false-pass on `cargo tree` failure.**  
  The proposed `cargo tree ... | grep ...` pattern, especially with `2>/dev/null`, treats a failed `cargo tree` the same as “no banned dependency found” unless the shell explicitly checks the cargo command status. This weakens the core security property.

- **HIGH: `cargo-deny Layer 2 deferred` is honest but not reconciled with the phase success criteria.**  
  The roadmap says the purity gate is backed by a `cargo-deny [bans]` declaration. The plan chooses not to edit `deny.toml`, which may be the right engineering call, but it needs an explicit accepted deviation or roadmap/success-criteria update. Otherwise Phase 91 can be marked green while missing a stated gate layer.

- **HIGH: Cognitive complexity <=25/function is not explicitly enforced.**  
  A near-verbatim lift of large evaluator/semantics/render files is risky under the stated Toyota Way constraint. Clippy does not prove cognitive complexity. The plans need an explicit complexity check and a refactor path if lifted functions exceed 25.

- **MEDIUM: Zero SATD is not checked.**  
  The plans do not require a scan for `TODO`, `FIXME`, `HACK`, `XXX`, or existing lighthouse comments that may count as SATD.

- **MEDIUM: “Per feature-combination” is only partially sound.**  
  The negative check loops across feature modes, but the positive `rust_xlsxwriter` assertion appears to run only for the default tree. If a future feature combination removes or bypasses the renderer, the positive assertion may not catch it.

- **MEDIUM: The served-binary part of WBRT-04 is under-specified.**  
  Runtime and dialect trees are checked, but “any served-binary dependency tree” is not modeled. Phase 91 may not yet have a workbook served binary, but the plan should explicitly define the current protected package set and how Phase 92/95 extend it.

- **MEDIUM: WBDL-03 appears in Plan 03 `requirements` metadata.**  
  Since WBDL-03 is deliberately re-mapped, including it as a plan requirement risks tooling or humans treating it as satisfied in Phase 91. Use a separate “traceability remap” field or remove it from the requirements list.

- **MEDIUM: `make purity-check` vs `just purity-check` is inconsistent with the roadmap wording.**  
  The plan justifies `make`, but the phase success criteria mention `just purity-check`. Add a `just purity-check` wrapper or update the roadmap wording.

- **LOW: Adding cargo-deny notes to `docs/workbook-dialect-spec.md` pollutes the dialect spec.**  
  The spec is BA/auditor-facing for whitelist/refuse-set governance. The cargo-deny limitation belongs in a planning artifact, purity-gate doc, or phase summary.

- **LOW: Verification commands for ROADMAP updates are weak.**  
  `grep -A1 ... | grep -v WBDL-03` can succeed without proving the Phase 91 requirements line was fixed. Use a section-aware check.

## Suggestions

- Rewrite the purity script to capture `cargo tree` output and fail if `cargo tree` itself fails before grepping banned tokens.
- Run both negative and positive assertions for each relevant feature mode, at least for `pmcp-workbook-runtime`.
- Add `just purity-check` as a thin wrapper around `make purity-check`, or update the roadmap/success criteria before execution.
- Treat the missing `cargo-deny` layer as a formal decision: either update Phase 91 success criteria or add a tracked follow-up with explicit owner and acceptance.
- Add explicit validation for `cognitive complexity <=25/function` and zero SATD before accepting the verbatim lift.
- Add a golden deserialization fixture or sample bundle check if available, so WBRT-01 proves compatibility with offline-emitter output rather than only compiling copied types.
- Move the cargo-deny deferred note out of `docs/workbook-dialect-spec.md`.
- Remove `WBDL-03` from Plan 03’s `requirements` metadata and describe it as a traceability correction instead.

## Risk Assessment

**Overall risk: MEDIUM.**  
The architecture and sequencing are sound, and the plans are close to achieving the phase goal. The remaining risk is concentrated in enforcement details: the purity gate must not false-pass, the cargo-deny deviation must be formally reconciled, and the quality constraints need explicit checks. Fixing those before execution would bring the plan set down to low risk.

---

## Gemini Review

# Plan Review: Phase 91 (Workbook Runtime + Purity Gate + Dialect Spec)

The implementation plans for Phase 91 provide a high-fidelity roadmap for extracting the governed Excel workbook components from the lighthouse app into the PMCP SDK. The research is exhaustive, correctly identifying SDK-specific conventions (literal versioning, `thiserror = "2"`) and addressing the critical "purity" security requirement with a multi-layered verification strategy.

## Strengths
- **Rigorous Purity Enforcement:** The "Purity Gate" (Plan 91-03) is exceptionally well-designed. By using `cargo tree` to perform negative assertions (reader absence) and positive assertions (writer presence), the plan moves beyond simple text-grepping to a structurally sound link-boundary proof.
- **Decision Traceability:** The migration of `WBDL-03` to Phase 93 (Decision D-02) is handled as a blocking documentation update, ensuring that requirements are re-mapped rather than lost.
- **Adherence to Conventions:** The plans correctly identify and apply SDK-specific patterns, such as literal versions instead of workspace inheritance, avoiding the common "Pitfall 2" identified in research.
- **Deep Research:** The identification of `zip 7.2.0` as a legitimate transitive dependency of the writer (and therefore permitted) shows a high level of technical diligence.
- **Toyota Way Integration:** The use of `--test-threads=1` and the inclusion of panic-freedom lints (`#![deny(clippy::unwrap_used)]`) aligns perfectly with the project's quality standards.

## Concerns
- **Cognitive Complexity & SATD (MEDIUM):** The "Toyota Way" requires cognitive complexity $\le$ 25 per function and zero SATD. Plan 91-01 specifies a "verbatim lift" from the lighthouse app. If the lighthouse source contains complex functions or `TODO/FIXME` comments (SATD), a verbatim lift will violate these constraints. The plan lacks an explicit step to audit or refactor the lifted code for these specific quality gates.
- **`quality-gate` Integration (LOW):** Plan 91-03 creates a `purity-check` Makefile target but does not explicitly state that `quality-gate` should depend on it. To ensure `make quality-gate` remains the single source of truth for pre-commit validation, `purity-check` should be added to the `quality-gate` dependency list.
- **`cargo-deny` Layer 2 Honesty (LOW):** The plan correctly identifies that `deny.toml` is infra-managed and workspace-global, making a per-crate ban difficult. Deferring this is the "honest" choice as requested, though it leaves the "Three-layer gate" promise slightly diminished.

## Suggestions
- **Add SATD/CC Audit Task:** In Plan 91-01, Task 3, add a sub-step to:
    1. Scan lifted files for `TODO`, `FIXME`, or `XXX` and resolve or promote them to issues.
    2. Run a complexity check (e.g., `cargo clippy -- -A clippy::all -D clippy::cognitive_complexity`) to ensure the $\le$ 25 limit is met.
- **Package-Level `deny.toml`:** To resolve the "Layer 2" concern, consider invoking `cargo deny` with a specific config file for the runtime: `cargo deny --config crates/pmcp-workbook-runtime/deny.toml check bans`. This would allow a local ban on readers without affecting the global workspace or Phase 93.
- **Positive Grep Refinement:** In Plan 91-03, Task 2, ensure the positive assertion grep for `rust_xlsxwriter` targets the specific crate being checked rather than the whole tree, to prevent false positives from other workspace members.

## Risk Assessment
**Overall Risk: LOW**

The risk is low because the code being extracted is already "penny-reconciled" and proven in a production-like lighthouse environment. The primary technical risk—accidental introduction of the Excel reader into the served binary—is robustly mitigated by the Plan 91-03 purity gate. The remaining risks are purely stylistic/convention-based, which are well-addressed by the provided patterns.

---

## Consensus Summary

Both reviewers independently rate the plan set as architecturally sound and close to
achieving the phase goal. They diverge on overall risk (Codex: **MEDIUM**, Gemini:
**LOW**) — the gap is entirely about *enforcement soundness* of the purity gate and the
Toyota Way quality constraints, not about the design. The lift sequencing, scope
discipline (reader/linter stay out of Phase 91), SDK-convention deltas, and the
WBDL-03 re-map are praised by both.

### Agreed Strengths
- **Dependency order & scope discipline** — runtime → dialect → purity gate; `WorkbookMap`,
  `linter.rs`, `umya`, `quick-xml` correctly excluded (both reviewers).
- **Purity gate intent is non-vacuous** — negative reader/JS absence checks *plus* a positive
  `rust_xlsxwriter` presence assertion (both reviewers).
- **SDK-convention awareness** — literal `0.1.0`, `thiserror = "2"`, no gratuitous `pmcp`
  dep, `zip 7.2.0` correctly identified as a permitted writer-transitive (both reviewers).
- **WBDL-03 re-map handled as a blocking doc edit**, not silently dropped (both reviewers).
- **doc↔WHITELIST binding test** is the right control for WBDL-01 (both reviewers).
- **cargo-deny Layer 2 surfaced honestly** rather than faked (both reviewers).

### Agreed Concerns (highest priority — both reviewers raised these)
1. **Toyota Way quality not explicitly enforced on the lifted code (Codex HIGH / Gemini MEDIUM).**
   A near-verbatim lift of large evaluator/`semantics`/`render` files can carry functions
   over cognitive-complexity ≤25 and import lighthouse SATD (`TODO`/`FIXME`/`XXX`). Clippy
   does NOT prove cognitive complexity. **Action:** add an explicit SATD scan + complexity
   check (PMAT or `clippy::cognitive_complexity`) to 91-01 Task 3, with a refactor/`// Why:`
   `#[allow]` path (per CLAUDE.md Phase-75 template, hard cap cog 50) for irreducible lifted
   functions.
2. **cargo-deny Layer 2 deferral not reconciled with the stated success criteria (Codex HIGH / Gemini LOW).**
   ROADMAP Success-Criterion 3 promises a `cargo-deny [bans]` backstop. Deferring it may be
   the right call, but it needs either a recorded accepted-deviation or a ROADMAP/criteria
   edit so the phase cannot be marked green while a *stated* gate layer is absent. Gemini
   offers a concrete reconciliation: a **crate-local `crates/pmcp-workbook-runtime/deny.toml`**
   invoked via `cargo deny --config … check bans`, sidestepping the infra-managed
   workspace-global file and the Phase-93 conflict — worth evaluating vs. pure deferral.

### Divergent Views
- **Overall risk: Codex MEDIUM vs Gemini LOW.** Codex weights the enforcement-soundness gaps
  (false-pass risk, unproven complexity) as material pre-execution risk; Gemini weights the
  "already penny-reconciled, production-proven lighthouse code" as de-risking, treating the
  remainder as stylistic. Both agree the design is correct.
- **Codex-only HIGH: `make purity-check` can false-pass.** `cargo tree … | grep … 2>/dev/null`
  treats a *failed* `cargo tree` invocation identically to "no banned dep found." Codex wants
  an explicit `cargo tree` exit-status check (`set -o pipefail` / capture status) so a broken
  tree command can't silently green the gate. **This is the single most actionable finding —
  not raised by Gemini, and it directly undermines the core security property.**
- **Positive-assertion scoping (both, slightly different framing).** Codex: the positive
  `rust_xlsxwriter` assertion may run only on the default tree, so a feature combo that drops
  the renderer might not be caught. Gemini: scope the positive grep to the specific crate, not
  the whole workspace tree, to avoid a false-positive from another member pulling
  `rust_xlsxwriter`. **Both point at the same fix: run the positive assertion per-crate AND
  per-feature-combination, same matrix as the negative checks.**
- **`make quality-gate` wiring (Gemini-only LOW).** `purity-check` is wired into CI's
  merge-blocking `gate` job but Gemini notes it should also be a dependency of the local
  `make quality-gate` target so the pre-commit single-source-of-truth stays complete.

### Recommended next step
Several findings are concrete and low-cost to fold in before execution. Run:

```
/gsd:plan-phase 91 --reviews
```

to replan incorporating this feedback. Priority order for the replanner:
1. (Codex HIGH) Harden `purity-check` against `cargo tree` false-pass — exit-status check, no bare `2>/dev/null` swallow.
2. (Both) Run the **positive** `rust_xlsxwriter` assertion per-crate + per-feature-combination, matching the negative matrix.
3. (Both) Add SATD scan + cognitive-complexity check to the 91-01 lift, with the CLAUDE.md `// Why:` allow path for irreducible functions.
4. (Both) Reconcile the cargo-deny Layer-2 deferral — either record an accepted deviation/ROADMAP edit, or adopt Gemini's crate-local `deny.toml` approach.
5. (Gemini LOW) Add `purity-check` to the local `make quality-gate` dependency list.
