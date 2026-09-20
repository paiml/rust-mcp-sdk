# Phase 93: Workbook Compiler + §5 Generalization Fixes + Promote Gate - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-11
**Phase:** 93-workbook-compiler-5-generalization-fixes-promote-gate
**Areas discussed:** First-build findings model, Manifest synthesis + ratification, Change-class diff presentation, Promote gate + --accept UX

**Framing (user, opening directive):** Focus on the business-analyst experience — a BA starts from an Excel sheet built off one of our examples, scaffolds an MCP server, builds it for the first time (errors + warnings), fixes errors and some warnings, builds, deploys, tests; weeks later updates the versioned spreadsheet, rebuilds, gets the main diffs/errors/warnings, fixes, deploys a new version, tests, iterates.

---

## First-build findings model

| Option | Description | Selected |
|--------|-------------|----------|
| Integrity-only blocks | Errors = penny-reconcile failure, formula parse failure, non-whitelist function, umya fabricated-provenance. Warnings = manifest ambiguity, enum-source-not-inline (dynamic fallback), style/Guide advisories. | ✓ |
| Strict (more blocks) | Also makes enum-source-not-inline and low-confidence manifest inference hard errors. | |

**User's choice:** Integrity-only blocks. Block semantics pre-settled by the user's narrative ("fix the errors and *some* warnings, build") — errors block emit, warnings are advisory.

| Option (reconcile) | Description | Selected |
|--------|-------------|----------|
| Always hard error | Any over-tolerance cell mismatch blocks emit. | |
| Named-outputs only | Named-output divergence blocks; intermediate/helper drift is a warning. | ✓ (refined) |
| — | — | |

**User's choice:** Free-text — asked what "Excel cached value" means and steered: "make it easy, not overly technical, for the BA…be tolerant to common issues that we can resolve with a warning." Resolved to a refinement of "named-outputs only": block only when a *published answer* is wrong; helper-cell numeric/rounding drift is a located warning; broken logic still errors via the dialect gate.
**Notes:** Confirmed "Yes, makes sense" after the cached-value explanation and the tolerant policy proposal.

---

## Manifest synthesis + ratification

| Option | Description | Selected |
|--------|-------------|----------|
| Stay in Excel | Infer from colour/Guide/headers; manifest.json emitted-only, never hand-edited; corrections made in the sheet + recompile; ratification = recorded approver + date. | ✓ |
| Edit the emitted manifest | BA edits a candidate manifest file directly; recompile re-reads it. | |

**User's choice:** Stay in Excel.

| Option (ambiguous cells) | Description | Selected |
|--------|-------------|----------|
| Warn + safe default (internal) | Unclassified cell stays in computation, not exposed; warn only when it looks like it should be exposed. | ✓ |
| Warn on every unclassified cell | Warning on every unclassified cell, including obvious helpers. | |

**User's choice:** Warn + safe default (internal).
**Notes:** Range-backed dropdowns captured as a consistent corollary — fall back to dynamic input with a warning, not a block (only inline ≤10 DV lists become closed enums).

---

## Change-class diff presentation

| Option | Description | Selected |
|--------|-------------|----------|
| By change-class (action buckets) | Safe — auto-applied / Needs your approval / New version required; each with a plain "what changed → what it means for consumers" line. | ✓ |
| By named output | Grouped by which published answer changed. | |

**User's choice:** By change-class (action buckets).

| Option (depth) | Description | Selected |
|--------|-------------|----------|
| Material changes only | Named-output + manifest + structural changes; helper noise summarized in one line. | ✓ |
| Everything, itemized | Every changed cell listed individually. | |

**User's choice:** Material changes only. (Aligns with the user's "give me the *main* diffs.")

---

## Promote gate + --accept UX

| Option (corpus) | Description | Selected |
|--------|-------------|----------|
| Auto-derived regression baseline | Replay manifest defaults + enum domains through prior vs candidate; flag over-tolerance named-output deltas; no BA-authored cases. | ✓ |
| BA-curated input→expected cases | BA authors representative cases the gate checks against. | |

**User's choice:** Auto-derived regression baseline.

| Option (block loop) | Description | Selected |
|--------|-------------|----------|
| Show deltas + exact approve command | Gate prints deltas + change class + copy-pasteable `--accept --approver <X> --effective-date <D>`; records a fingerprint-bound ApprovalRecord, re-baselines, proceeds. | ✓ |
| Prompt for approval inline | Interactive y/n + name during the run. | |

**User's choice:** Show deltas + exact approve command.

| Option (versioning) | Description | Selected |
|--------|-------------|----------|
| Declared in the workbook | BA bumps version in the workbook changelog/Guide; compiler writes @<version>; BUNDLE.lock version == declared == changelog.to_version. | ✓ |
| Auto-bump by change class | Compiler picks patch/minor/major from the diff. | |

**User's choice:** Declared in the workbook. (Consistent with stay-in-Excel.)

---

## Claude's Discretion

- Compiler module layout (mirror lighthouse `workbook-compiler/src` tree, reuse runtime shared types).
- Auto-generated input-case grid size/shape for the gate; reconcile-finding wording; ApprovalRecord on-disk format/location.
- Operand-anchored rounding detail (runtime `sheet_ir/rounding.rs`; never naïve abs-delta — grep-gated).
- Colour/Guide/header inference heuristics; quarantined provenance-reader internals.

## Deferred Ideas

- CLI subcommands + `pmcp.toml` (Phase 94); Shape A binary + deploy (Phase 95); scaffold + dialect-version + second-workbook gate (Phase 96).
- BA-curated test cases as a gate basis (rejected for 93 — auto-derived baseline).
- Auto-bump versioning by change class (rejected for 93 — workbook-declared).
- Interactive inline gate approval prompt (rejected — explicit auditable `--accept`).

## Derived (not separately discussed)

- First build has no prior baseline → promote gate is a no-op on v1; v1 correctness is enforced by penny-reconcile against Excel's cached values.
- `evidence/` bundle member records the reconciliation + provenance trail (seven-member contract from Phase 92).
