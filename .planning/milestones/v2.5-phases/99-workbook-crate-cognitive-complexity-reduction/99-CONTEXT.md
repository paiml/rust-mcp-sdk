# Phase 99: Workbook-Crate Cognitive-Complexity Reduction (PMAT gate debt) - Context

**Gathered:** 2026-06-16
**Status:** Ready for planning
**Source:** PR #279 CI failure (`pmat quality-gate --fail-on-violation --checks complexity` → 21 violations). Empirical PMAT-allow behavior: `.planning/phases/75-fix-pmat-issues/pmat-allow-behavior.md` (D-10-B).

<domain>
## Phase Boundary

Refactor the 21 cognitive-complexity violations in the v2.3 workbook crates so the org-required PMAT complexity gate passes workspace-wide. These are pre-existing milestone debt, surfaced on PR #279 because PMAT is **CI-only** (Phase 75 D-07) and the v2.3 milestone never ran through a PMAT-gated CI.

**Hard facts that constrain the approach:**
- **PMAT ignores `#[allow(clippy::cognitive_complexity)]`** — empirically verified (Phase 75 D-10-B). The annotation has ZERO effect on the PMAT gate (PMAT walks the AST directly; it does not consult Rust's `#[allow]` machinery). CLAUDE.md's "annotate irreducibly-complex functions" escape hatch applies only to clippy's own lint, NOT to this gate. **Do not rely on `#[allow]` to clear PMAT.**
- **No `.pmatignore` weakening of production crates.** `.pmatignore` today excludes only non-production dirs (`fuzz/`, `packages/`, `examples/`). Adding `crates/pmcp-workbook-*` would weaken the gate for production code — out of scope (the user chose "refactor", not "scope the gate").
- **The gate is the oracle.** Run `pmat quality-gate --fail-on-violation --checks complexity` (and/or `pmat analyze complexity --max-cognitive 25 --format json`) locally to confirm each function clears. Empirically the gate flags at two tiers: cog ≥26 against "maximum 25" AND cog ≥24 against "recommended 23" — so **target ≤23 per function** to guarantee a clean pass.

**In scope:** behavior-preserving refactor (helper extraction, table-driven dispatch, early-return flattening, sub-function decomposition) of the 21 named functions. **Out of scope:** any behavior change, any new feature, touching non-flagged functions beyond what extraction requires, the Phase 98 deploy code, Phase 97 GitHub-automation.
</domain>

<decisions>
## Implementation Decisions

### Refactor technique menu (Phase 75 P1–P6 lineage)
Apply the lightest technique that clears the gate while preserving behavior:
- **Extract cohesive blocks** into named private helpers (the workhorse — each `if`/`match` arm cluster or loop body that forms a sub-step becomes a function).
- **Table-driven / match-to-map** dispatch for long `match` ladders (tokenizers, evaluators).
- **Early-return guard clauses** to flatten nesting (cognitive complexity is dominated by nesting depth × branch count).
- **Split a multi-phase function** (parse → validate → build) into per-phase helpers with a thin orchestrator.

### The 5 over the cog-50 hard cap (genuine decomposition, not cosmetic)
These are parsers/evaluators/AST-walkers/renderers — the legitimately-complex category. Each needs real structural decomposition, with the existing test net as the regression guard:
- `pmcp-workbook-runtime/src/render/mod.rs::render_xlsx` (93)
- `pmcp-workbook-compiler/src/change_class/mod.rs::classify_cell_roles` (74)
- `pmcp-workbook-runtime/src/sheet_ir/executor.rs::eval_expr` (58)
- `pmcp-workbook-compiler/src/ingest/mod.rs::ingest` (57)
- `pmcp-workbook-compiler/src/formula/token.rs::tokenize` (52)

### Safety net (no behavior drift)
The milestone already ships a strong regression net — USE it as the gate against silent drift, run before and after each plan:
- Golden/reconcile fixtures (penny-reconcile, the quirk corpus — 1900-leap, empty-cell, half-rounding), `quirks_reconcile.rs`, `reemit_loan.rs`
- Dialect linter tests, provenance gate tests, `backward_compat_*` goldens
- `scalar_eval` unit tests in the runtime
- Each refactor task MUST run the affected crate's test suite (`cargo test -p <crate>`) and show it green; the FINAL plan runs the full workspace suite + `make quality-gate`.

### Per-crate grouping (to avoid intra-file merge conflicts)
Group plans by file/subsystem so two plans never edit the same function/file concurrently. `formula/token.rs` has 3 flagged fns (tokenize, lex_quoted_sheet_ref, scan_atom_run) → one plan. `provenance/raw_parts.rs` has 2 → one plan. etc.

### Claude's Discretion
- Exact helper names/granularity; whether a cog-93 function becomes 3 or 6 helpers — driven by what clears the gate at ≤23 while keeping each helper readable.
- Whether to add focused unit tests for newly-extracted helpers (encouraged where a helper has non-trivial logic, but not required if the existing golden/reconcile coverage already exercises the path).
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before refactoring.**

### The gate + its empirical behavior
- `.planning/phases/75-fix-pmat-issues/pmat-allow-behavior.md` — PROOF that PMAT ignores `#[allow]` (D-10-B); the refactor-only mandate flows from this
- `.github/workflows/ci.yml` (quality-gate job, ~:233) + `.github/workflows/quality-gate.yml` (~:70) — the exact gate command `pmat quality-gate --fail-on-violation --checks complexity`
- `.pmatignore` — current exclusions (fuzz/packages/examples only); DO NOT add production crates
- `CLAUDE.md` "CI Quality Gates" section — the P1–P6 refactor techniques pointer + cog-25/50 policy (note the `#[allow]` part is a no-op for PMAT)

### Flagged files (read each before editing)
- runtime: `crates/pmcp-workbook-runtime/src/render/mod.rs` (render_xlsx 93), `src/sheet_ir/executor.rs` (eval_expr 58), `src/bundle_loader.rs` (load 28), `src/sheet_ir/semantics.rs` (f_index 24, f_search 31)
- compiler: `crates/pmcp-workbook-compiler/src/change_class/mod.rs` (classify_cell_roles 74), `src/change_class/ir_identity.rs` (dependency_order 24), `src/ingest/mod.rs` (ingest 57, references_external_workbook 31), `src/formula/token.rs` (tokenize 52, lex_quoted_sheet_ref 33, scan_atom_run 30), `src/dialect/linter.rs` (extract_function_tokens 29), `src/fixture_author.rs` (author_xlsx 29), `src/dag/resolve.rs` (walk 25), `src/gate/corpus.rs` (derive_case_grid 34, no_seeded_value_outside_allowed 46), `src/provenance/gate.rs` (gate_inner 29), `src/provenance/raw_parts.rs` (parse_calc_pr 44, parse_app_props 39)
- server-toolkit: `crates/pmcp-server-toolkit/src/workbook/input.rs` (validate_input 33)
</canonical_refs>

<specifics>
## Specific Ideas

- Verify the live cog of each function FIRST with `pmat analyze complexity --max-cognitive 25 --format json | jq '.violations[] | select(.path | test("workbook"))'` — line numbers in this doc are from the PR #279 CI run and may drift.
- The 3 functions at cog 24–25 (`dependency_order`, `f_index`, `walk`) are flagged against the "recommended 23" tier — a single guard-clause extraction each likely clears them; do not over-engineer.
- `make quality-gate` is the merge bar (fmt/clippy pedantic+nursery/build/test/audit) — the refactor must also keep clippy clean (extracted helpers need docs if `pub`, but most will be private `fn` with no doc requirement).
- Commit per function-group; keep each diff reviewable on the already-large PR #279.
</specifics>

<deferred>
## Deferred Ideas

- Scoping the PMAT gate to root `pmcp` `src/` only (matching CLAUDE.md's `startswith("src/")` jq filter) — the user explicitly chose refactor over gate-scoping; revisit only if the refactor proves intractable.
- A broader sweep of non-workbook crates for latent complexity debt — out of scope; this phase closes only the 21 that block #279.
</deferred>

---

*Phase: 99-workbook-crate-cognitive-complexity-reduction*
*Context derived 2026-06-16 from PR #279 CI failure*
