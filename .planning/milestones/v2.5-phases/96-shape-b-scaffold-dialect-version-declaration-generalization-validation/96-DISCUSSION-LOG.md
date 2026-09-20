# Phase 96: Shape B Scaffold + Dialect-Version Declaration + Generalization Validation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-14
**Phase:** 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
**Areas discussed:** Second-workbook domain, Dialect-version mechanism, Scaffold sample content, Excel-quirk corpus shape

---

## Second-workbook domain (WBEX-01)

### Q1 — What domain should the second, non-lighthouse workbook model?

| Option | Description | Selected |
|--------|-------------|----------|
| Loan/mortgage payment calc | Compounding/PMT family, half-rounding quirks; maximally different + quirk-rich | ✓ |
| Unit/currency converter | Simple fixed DAG, very different shape, but light on formula variety | |
| Tip / bill splitter | Simple division + rounding, but too close to tax-calc's percentage-of-base shape | |
| You decide | Claude picks for strongest generalization proof | |

**User's choice:** Loan/mortgage payment calc

### Q2 — How hard should it diverge from tax-calc?

| Option | Description | Selected |
|--------|-------------|----------|
| Maximal divergence | Distinct input arity + formula families absent from tax-calc, multiple outputs | ✓ |
| Moderate divergence | Different inputs/outputs but reuse existing formula families | |
| You decide | Claude picks the convincing level | |

**User's choice:** Maximal divergence

### Q3 — Given the 13-fn whitelist (no PMT/POWER), how to achieve maximal divergence?

| Option | Description | Selected |
|--------|-------------|----------|
| Rate-table lookup model | VLOOKUP/INDEX-MATCH rate-tier table + IFERROR + nested IF + ROUND/CEILING; whitelist-legal, quirk-rich | ✓ |
| Fixed-term unrolled schedule | Small fixed term unrolled into per-period cells; risks looking like deferred row-iteration | |
| Annuity-factor as input | payment = principal / factor; pushes the math outside the workbook, weakest proof | |
| You decide | Claude picks the whitelist-legal max-divergence approach | |

**User's choice:** Rate-table lookup model
**Notes:** Claude flagged mid-discussion that the dialect whitelist (`crates/pmcp-workbook-dialect/src/lib.rs:35`) has no PMT/POWER/exponent, so arbitrary-term amortization isn't expressible in a fixed-cell DAG — the divergence lever was re-pinned to whitelisted lookup families. This grounding corrected the earlier (inaccurate) "PMT/POWER compounding" framing.

---

## Dialect-version mechanism (WBDL-02)

### Q1 — Where should a workbook declare the dialect version?

| Option | Description | Selected |
|--------|-------------|----------|
| Named cell/range in workbook | Reserved named-range inside the .xlsx; self-describing, workbook-as-spec | ✓ |
| Field in pmcp.toml | Centralized project config, but separates declaration from workbook | |
| Both (workbook wins) | Workbook authoritative, pmcp.toml default; more surface, precedence ambiguity | |
| You decide | Claude picks the most philosophy-consistent location | |

**User's choice:** Named cell/range in workbook

### Q2 — Mismatch policy vs the compiler's supported dialect?

| Option | Description | Selected |
|--------|-------------|----------|
| Semver-compatible, fail-closed | Same major + declared minor ≤ supported = ok; else hard typed error | ✓ |
| Exact-match only | Declared must equal compiler version; defeats forward-compatible goal | |
| Warn-and-proceed | Log warning but compile anyway; violates fail-closed ethos | |
| You decide | Claude picks forward-compatible + fail-closed | |

**User's choice:** Semver-compatible, fail-closed

### Q3 — What when a workbook has NO declaration (e.g. tax-calc golden)?

| Option | Description | Selected |
|--------|-------------|----------|
| Default to baseline version | Treat as targeting baseline (1.0), compile normally, optional advisory; existing fixtures untouched | ✓ |
| Require declaration (error) | Hard error; breaks existing golden + every prior fixture | |
| Default to current version | Silently re-targets old workbooks onto new dialects | |
| You decide | Claude picks the non-breaking default | |

**User's choice:** Default to baseline version

---

## Scaffold sample content (WBCL-05)

### Q1 — What should the scaffold ship?

| Option | Description | Selected |
|--------|-------------|----------|
| Source .xlsx + bundle + toml | Pre-compiled embedded bundle + source .xlsx + pmcp.toml; full edit→compile→rerun round-trip | ✓ |
| Pre-compiled bundle only | Minimal, runs immediately, but dead-ends the edit loop | |
| You decide | Claude picks the best-DX payload | |

**User's choice:** Source .xlsx + bundle + toml

### Q2 — Which workbook should the scaffold sample?

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse tax-calc golden | Existing proven minimal workbook; least new authoring (only loan .xlsx is new) | ✓ |
| New minimal 'hello' workbook | Clearest onboarding artifact, but a third .xlsx to author/maintain | |
| Reuse the loan workbook | One workbook serves scaffold + gate, but a rate-table mortgage calc is heavy for 'getting started' | |
| You decide | Claude balances onboarding clarity vs authoring scope | |

**User's choice:** Reuse tax-calc golden

---

## Excel-quirk corpus shape (WBEX-02)

### Q1 — How should the corpus be encoded?

| Option | Description | Selected |
|--------|-------------|----------|
| Mini reconcile fixtures | Tiny fixture workbooks with cached oracle values through the real penny-reconcile path | |
| scalar_eval unit tests | Targeted evaluator unit tests; lighter but isolated from the pipeline | |
| Both layers | Unit tests for pinpoint coverage + reconcile fixtures for end-to-end determinism | ✓ |
| You decide | Claude picks the most direct satisfaction of "reconcile determinism" | |

**User's choice:** Both layers

### Q2 — How exhaustive?

| Option | Description | Selected |
|--------|-------------|----------|
| Four named + curated few | 4 roadmap quirks + text→number, #DIV/0!, float boundary, negative-rounding sign; ~7–9 total | ✓ |
| Exactly the four named | Tightest scope, but leaves obvious quirks untested | |
| Broad sweep | Comprehensive but large authoring effort that could balloon the phase | |
| You decide | Claude picks the bounded-but-convincing breadth | |

**User's choice:** Four named + curated few

---

## Claude's Discretion

- Exact reserved named-range identifier (WBDL-02 D-03), the baseline version value (D-05).
- Scaffolded crate package name + precise file layout (WBCL-05).
- Loan workbook's exact rate-tier table contents and input/output cell names (WBEX-01).
- Precise fixture file locations for the WBEX-02 corpus.

## Deferred Ideas

None — discussion stayed within phase scope. Scope-creep guardrails held; row-block iteration,
capability cells, named-range validation lists, and registry bundle stores remain deferred-by-design
v2.3 items and were not pulled in.
