# Phase 75: Fix PMAT issues - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-22
**Phase:** 75-fix-pmat-issues
**Areas discussed:** Definition of done, Complexity remediation strategy, SATD + duplicate policy, Wave structure + enforcement

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Definition of done | Gate badge only vs all 5 PMAT dimensions clean | ✓ |
| Complexity remediation strategy | Aggressive vs pragmatic vs rewrite vs threshold raise | ✓ |
| SATD + duplicate policy | Triage approach for the 33 SATD + 439 duplicates | ✓ |
| Wave structure + enforcement | Split strategy + going-forward CI gating | ✓ |

**User's choice:** All four areas selected.

---

## Definition of Done

| Option | Description | Selected |
|--------|-------------|----------|
| Gate badge green only | Drive complexity violations to gate-passing; other dimensions best-effort | ✓ |
| Badge + zero SATD | Above + remove all 33 SATD comments | |
| All 5 dimensions to baseline-zero | Maximalist Toyota Way | |
| Badge + define stretch goals | Badge is must-have; per-dimension stretch targets as separate plans | |

**User's choice:** Gate badge green only (Recommended).
**Notes:** Locks D-01. Establishes that complexity is the gating dimension and that SATD/duplicate/entropy/sections are best-effort improvements within waves but do not block phase closure.

---

## Complexity Remediation Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Pragmatic per-function | Default extract-method/types; #[allow] with Why: justification for irreducible cases | ✓ |
| Aggressive refactor everywhere | No #[allow] escape hatches | |
| Rewrite hotspot files | Whole-file restructure for files with multiple violations | |
| Threshold raise + selective fix | Raise pmat threshold from 25 to e.g. 35 | |

**User's choice:** Pragmatic per-function (Recommended).
**Notes:** Locks D-02. Every `#[allow(clippy::cognitive_complexity)]` requires a one-line `// Why:` justification. No bare allows.

### Follow-up: Allow ceiling

| Option | Description | Selected |
|--------|-------------|----------|
| Cap at 50, prefer 35 | #[allow] should still aim ≤35; absolute hard ceiling 50 | ✓ |
| No cap — trust the justification | Some functions might stay at 80+ | |
| Cap at 75 | Looser ceiling | |

**User's choice:** Cap at 50, prefer 35 (Recommended).
**Notes:** Locks D-03. Functions above 50 must be refactored regardless of how good the justification is.

---

## SATD + Duplicate Policy

### SATD policy

| Option | Description | Selected |
|--------|-------------|----------|
| Triage: fix-or-issue-or-delete | Per-comment three-way judgment | ✓ |
| Convert all to GitHub issues | Mechanical, fast, but creates 33 backlog items | |
| Fix all in place | Ambitious; some SATDs likely block on external decisions | |
| Deprioritize — not gating | Capture as Phase 76 backlog | |

**User's choice:** Triage: fix-or-issue-or-delete (Recommended).
**Notes:** Locks D-04. Three-way per-comment judgment: trivial/obsolete → delete; real follow-up → file issue + reference number in normal comment; cheap (<30min) → fix in this phase.

### Duplicate policy

| Option | Description | Selected |
|--------|-------------|----------|
| Triage by location | src/ refactor; examples/tests/benches → PMAT exclusion config | ✓ |
| Refactor all in src/ | Per-instance refactor for every duplicate inside production | |
| Exclusion config only | Tune PMAT detection until count drops; don't touch code | |
| Deprioritize — not gating | Skip entirely in Phase 75 | |

**User's choice:** Triage by location (Recommended).
**Notes:** Locks D-05. PMAT config (likely `.pmat/project.toml` or `.pmatignore`) gets path exclusions for examples/, tests/, benches/, fuzz/. Real duplicates inside src/ get refactored.

---

## Wave Structure + Enforcement

### Wave structure

| Option | Description | Selected |
|--------|-------------|----------|
| By hotspot directory | Wave 1: streamable_http_server + pmcp-macros; Wave 2: pentest + deployment; Wave 3: pmcp-code-mode | ✓ |
| By PMAT dimension | Wave 1: complexity, Wave 2: SATD, Wave 3: duplicate | |
| One mega-wave parallel-by-file | Single wave, one plan per hotspot file, all parallel | |
| Iterative — measure after each | Top 20 worst-offender functions per wave; re-measure | |

**User's choice:** By hotspot directory (Recommended).
**Notes:** Locks D-06. Lets each wave ship independently; badge probably flips green mid-phase. Wave order chosen so cross-cutting dependencies (`pmcp-macros`) land first and the largest contained hotspot (`pmcp-code-mode`) lands last.

### Enforcement

| Option | Description | Selected |
|--------|-------------|----------|
| Block CI on regression | PR check runs `pmat quality-gate --fail-on-violation`, blocks merge on fail | ✓ |
| Pre-commit hook + CI block | Above + pre-commit integration | |
| Snapshot baseline, alert on increase | Non-blocking CI alert on regression | |
| Just fix, no enforcement | Rely on existing make quality-gate + clippy | |

**User's choice:** Block CI on regression (Recommended).
**Notes:** Locks D-07. CI-only enforcement; pre-commit explicitly excluded to keep the local dev loop fast. Workflow likely lives in `.github/workflows/quality-badges.yml` or a new sibling workflow that decouples badges from gating.

---

## Claude's Discretion

- Which specific functions to refactor first within each wave (planner picks based on dependency order and file co-location).
- Whether to introduce shared types/traits to reduce complexity vs duplicate helpers per call site — judgment call per case.
- How to structure GitHub issues filed for SATDs (one issue per SATD vs grouped by area) — planner decides based on triage outcome.
- Exact PMAT config file location and syntax for path exclusion (verify against `pmat --help` output for the installed 3.11.x version).
- Whether the new CI gate workflow runs on PRs only or also nightly — pick whichever is least disruptive while preventing regression.

## Deferred Ideas

- Drive SATD/duplicate/entropy/sections to absolute zero — Phase 76+ if pursued.
- Add PMAT to pre-commit hook — revisit only if CI gate proves insufficient.
- Raise the cognitive_complexity threshold from 25 to ~35 — rejected; stays at 25 to keep CLAUDE.md promise honest.
- Whole-file rewrites of hotspot files — rejected; function-level refactor is lower risk.
- README sections badge fix — if trivially fixable during housekeeping, do it; otherwise defer.
