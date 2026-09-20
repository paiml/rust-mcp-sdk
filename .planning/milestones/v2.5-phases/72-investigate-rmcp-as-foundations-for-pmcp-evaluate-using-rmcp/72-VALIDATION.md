---
phase: 72
slug: investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-19
revised: 2026-04-19 (reviews-mode — added 72-CONTEXT.md, 72-POC-RESULTS.md; expanded T-IDs to T1..T9; added semantic-audit gate)
mode: decision-validation
---

# Phase 72 — Validation Strategy (reviews-mode revised)

> Phase 72 is a **research/decision phase**, not a code/implementation phase.
> Canonical Nyquist (test files → code requirements) is replaced by
> **decision-quality validation**: did the phase produce a recommendation
> that is traceable, falsifiable, and evidence-backed?

See 72-RESEARCH.md §"Validation Architecture" for full framing.
See 72-REVIEWS.md for the reviews-mode revision drivers.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Markdown audit + `awk` semantic audit (no test framework) |
| **Config file** | `.planning/phases/72-.../72-DECISION-RUBRIC.md` |
| **Quick run command** | `grep -c` checks on deliverable structure (per REQ-ID row below) |
| **Full suite command** | Plan-checker agent review against the per-REQ criteria + semantic-audit awk script (new) |
| **Estimated runtime** | Manual audit, ~5 min per deliverable |

---

## Sampling Rate

- **After every plan wave:** Plan-checker agent reads produced deliverables against their REQ-ID criterion (PASS/FAIL).
- **Before `/gsd-verify-work`:** All 7 deliverables present (was 5 pre-reviews; now includes 72-CONTEXT.md and 72-POC-RESULTS.md) and cross-document consistency confirmed.
- **Max feedback latency:** Per-plan review.

---

## Per-Task Verification Map (REQ-ID → Deliverable)

| Req ID | Plan | Deliverable | Validation Criterion | Automated Command | File Exists | Status |
|---------|------|-------------|----------------------|-------------------|-------------|--------|
| — (CONTEXT.md — pre-REQ) | 01 Task 0 | `72-CONTEXT.md` | Explicit `breaking_change_window:` and `production_user_tolerance:` values (concrete or UNKNOWN + Resolution path) | `grep -qE '^breaking_change_window: (open\|closed\|closing_[0-9]{4}-[0-9]{2}-[0-9]{2}\|UNKNOWN)' 72-CONTEXT.md`; `grep -qE '^production_user_tolerance: (0\|1-2\|3-5\|6-20\|21\+\|UNKNOWN)' 72-CONTEXT.md`; `! grep -qE '<[a-z_]+>' 72-CONTEXT.md` | ❌ W0 | ⬜ pending |
| RMCP-EVAL-01 | 01 Task 2 | `72-INVENTORY.md` | ≥15 pmcp module families on the 9-column evidence schema (pmcp defining file:line, rmcp evidence, exact symbols, public API surface, owned impls/macros, serde compat risk, feature flag, downstream crates) | `grep -cE '\| [a-zA-Z_/.]+:[0-9]+ \|' 72-INVENTORY.md` ≥ 15; `! grep -qE '<line of' 72-INVENTORY.md`; grep for `Exact symbols touched`, `Serde compat risk`, `Downstream crates` headers (each ≥ 1) | ❌ W0 | ⬜ pending |
| RMCP-EVAL-02 | 01 Task 3 | `72-STRATEGY-MATRIX.md` | 5 options × 5 criteria = 25 cells; option rows are {A, B, C1, C2, D}; E appears only as `## Contingency (not scored): E. Fork`; no `TBD` | `grep -cE '^\| (A\. Full adopt\|B\. Hybrid\|C1\. Selective\|C2\. Selective\|D\. Status quo)' 72-STRATEGY-MATRIX.md` = 5; `! grep -qE '^\| E\. Fork' 72-STRATEGY-MATRIX.md`; `grep -q '^## Contingency (not scored): E\. Fork' 72-STRATEGY-MATRIX.md`; `! grep -i 'TBD' 72-STRATEGY-MATRIX.md` | ❌ W0 | ⬜ pending |
| RMCP-EVAL-03 | 02 Task 1 | `72-POC-PROPOSAL.md` | ≥2 PoC slices, each with `LOC: <N>` (<500), `Files:`, `Pass:`, `Fail:` sections, and at least one slice executable in ≤3 days | `grep -c '^### PoC Slice' 72-POC-PROPOSAL.md` ≥ 2; every slice block contains `LOC:`, `Files:`, `Pass:`, `Fail:`, `Time-box:` | ❌ W0 | ⬜ pending |
| — (POC-RESULTS.md — Slice 1 spike output) | 02 Task 1b | `72-POC-RESULTS.md` | Real measured `T4_compile_errors` and `T4_loc_delta` values from the executed Slice 1 spike; scratch branch `spike/72-poc-slice-1` DELETED; scratch dir `examples/spike_72_rmcp_types/` REMOVED | `grep -qE '^T4_compile_errors: [0-9]+' 72-POC-RESULTS.md`; `grep -qE '^T4_loc_delta: ' 72-POC-RESULTS.md`; `git branch -a \| grep -c 'spike/72-poc-slice-1'` = 0; `test ! -d examples/spike_72_rmcp_types` | ❌ W0 | ⬜ pending |
| RMCP-EVAL-04 | 02 Task 2 | `72-DECISION-RUBRIC.md` | ≥7 falsifiable thresholds (target 9 — T1..T9); every threshold followed by `Data source:`; includes T8 (historical churn) AND T9 (enterprise-feature-preservation checklist); `gh` fallback URL codified; default-to-B rule REMOVED | `grep -c '^- \*\*Threshold:\*\*' 72-DECISION-RUBRIC.md` ≥ 7; `grep -q 'T8'`; `grep -q 'T9'`; `grep -q 'PR merge latency'`; `grep -q 'api.github.com/repos/modelcontextprotocol/rust-sdk'`; `! grep -q 'default.*B (conditional)'` | ❌ W0 | ⬜ pending |
| RMCP-EVAL-05 | 03 Task 1 | `72-RECOMMENDATION.md` | Starts with `**Recommendation:** <A\|B\|C1\|C2\|D\|DEFER>` (E PROHIBITED); ≥5 `### Criterion` subsections; every criterion passes the semantic audit | `head -5 72-RECOMMENDATION.md \| grep -E '^\*\*Recommendation:\*\* (A\|B\|C1\|C2\|D\|DEFER)'`; `grep -c '^### Criterion' 72-RECOMMENDATION.md` ≥ 5; `! grep -qE '^\*\*Recommendation:\*\* E'`; semantic-audit awk script (see below) exits 0 | ❌ W0 | ⬜ pending |
| RMCP-EVAL-05 semantic audit | 03 Task 1b | `72-RECOMMENDATION.md` | Every `### Criterion` subsection contains ≥1 T-ID reference matching `T[1-9]` AND ≥1 inventory-row reference (`(row\|Row) [0-9]+`) OR matrix-option reference (`A.\|B.\|C1.\|C2.\|D.\|Full adopt\|Hybrid\|Selective\|Status quo`) | `awk` script below (see §"Semantic Audit Script") exits 0 | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

All 7 deliverables are created fresh in this phase — nothing pre-exists.

- [ ] `72-CONTEXT.md` — Plan 01 Task 0 Wave 1 (NEW per reviews-mode)
- [ ] `72-INVENTORY.md` — Plan 01 Task 2 Wave 1
- [ ] `72-STRATEGY-MATRIX.md` — Plan 01 Task 3 Wave 1
- [ ] `72-POC-PROPOSAL.md` — Plan 02 Task 1 Wave 2
- [ ] `72-POC-RESULTS.md` — Plan 02 Task 1b Wave 2 (NEW per reviews-mode — product of an EXECUTED spike)
- [ ] `72-DECISION-RUBRIC.md` — Plan 02 Task 2 Wave 2
- [ ] `72-RECOMMENDATION.md` — Plan 03 Task 1 Wave 3

*No test framework install — decision validation is markdown-audit + awk.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Cross-document consistency: CONTEXT ↔ inventory ↔ matrix ↔ PoC ↔ rubric ↔ spike-results ↔ recommendation | RMCP-EVAL-01..05 | Requires semantic judgment | Reviewer reads all 7 docs and fills the Falsifiability Checklist |
| Recommendation justification quality | RMCP-EVAL-05 | Requires judging whether the chosen option's rationale meaningfully engages all 5 rubric criteria (semantic audit catches the citation gate but does not judge depth) | Reviewer confirms each `### Criterion` in 72-RECOMMENDATION.md cites a specific threshold outcome AND supporting inventory/matrix row WITH a plausible argument |
| Spike branch cleanliness | reviews-mode Task 1b | Audit mandates the spike branch and scratch dir are deleted after measurement | `git branch -a \| grep -c 'spike/72-poc-slice-1'` returns 0; `test ! -d examples/spike_72_rmcp_types` |

---

## Semantic Audit Script (reviews-mode addition)

Per 72-REVIEWS.md MEDIUM consensus finding, every `### Criterion` subsection in 72-RECOMMENDATION.md MUST cite a T-ID AND an inventory/matrix reference. The following `awk` script enforces this. Used inline by Plan 03 Task 1b; re-runnable by plan-checker:

```awk
awk '
  /^### Criterion/ {
    if (cb != "") { if (t == 0 || r == 0) { print "FAIL: " cb; f++ } else { print "PASS: " cb } }
    cb = $0; t = 0; r = 0; next
  }
  /^## / && cb != "" {
    if (t == 0 || r == 0) { print "FAIL: " cb; f++ } else { print "PASS: " cb }
    cb = ""; t = 0; r = 0
  }
  cb != "" {
    if (match($0, /T[1-9]/)) t = 1
    if (match($0, /(row|Row) [0-9]+/) || match($0, /(A\.|B\.|C1\.|C2\.|D\.)/) || match($0, /(Full adopt|Hybrid|Selective|Status quo)/)) r = 1
  }
  END {
    if (cb != "") { if (t == 0 || r == 0) { print "FAIL: " cb; f++ } else { print "PASS: " cb } }
    if (f > 0) { print "AUDIT FAIL: " f " subsection(s)"; exit 1 } else { print "AUDIT PASS"; exit 0 }
  }
' 72-RECOMMENDATION.md
```

Failure → recommendation is auto-downgraded to DEFER per the Plan 03 Task 1b rule.

---

## Falsifiability Checklist (phase-gate — updated per reviews-mode)

- [ ] `72-CONTEXT.md` exists with explicit T6 and T7 values (concrete or UNKNOWN + Resolution path)
- [ ] Every row in `72-INVENTORY.md` cites a pmcp `file:line` at a DEFINING item (NOT `:1`) AND an rmcp docs.rs/GitHub URL; 9-column evidence schema populated
- [ ] `72-STRATEGY-MATRIX.md` has all 25 cells filled (5 options × 5 criteria) with option rows {A, B, C1, C2, D}; E appears only as footnote; no `TBD`
- [ ] `72-POC-PROPOSAL.md` names ≥1 slice ≤500 LOC touched AND ≥1 slice executable in ≤3 days
- [ ] `72-POC-RESULTS.md` exists with real `T4_compile_errors` from executed Slice 1 spike; spike branch + scratch dir DELETED
- [ ] `72-DECISION-RUBRIC.md` has ≥7 falsifiable thresholds (target 9 — T1..T9); T8 (historical churn) AND T9 (enterprise-feature preservation) present; `gh` fallback URL codified
- [ ] `72-RECOMMENDATION.md` picks one of {A, B, C1, C2, D, DEFER} (E PROHIBITED); passes the semantic audit; decision-tree traversal log present

---

## Validation Sign-Off

- [ ] All 7 deliverables produced in their target Wave
- [ ] Falsifiability Checklist above fully checked
- [ ] Semantic audit passes on 72-RECOMMENDATION.md
- [ ] Cross-document consistency confirmed by plan-checker
- [ ] `nyquist_compliant: true` set in frontmatter (decision-validation framing satisfies Nyquist intent)

**Approval:** pending
