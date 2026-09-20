# Phase 72 — Decision Rubric (RMCP-EVAL-04, reviews-mode revised)

**Baseline pin:** rmcp 1.5.0 / pmcp current main.
**Purpose:** provide falsifiable thresholds so the Plan 03 recommendation is derivable from measurable inputs rather than from taste.
**Rule:** every threshold is numeric or boolean, and every threshold line is immediately followed by a `Data source:` line naming a command, deliverable, or CONTEXT.md entry.

**Reviews-mode expansion (per 72-REVIEWS.md):**
- Added **T8** (historical churn on src/types/ + src/shared/ over last 180 days) replacing "LOC deleted" as the primary maintenance-burden signal (replan action #5).
- Added **T9** (enterprise-feature-preservation per-feature checklist) to bind the Strategy Matrix "Enterprise feature preservation" column to concrete extension points (replan action #5).
- Updated **T2** to include PR merge latency in addition to issue close time (replan action #5).
- Updated **T4** to subcount broken examples and broken downstream crates separately from compile errors (replan action #5).
- **T3 and T4 data sources now cite 72-POC-RESULTS.md** — Slice 1 was executed in Plan 02 Task 1b, so T4 subcount (a) is RESOLVED, not UNRESOLVED (replan action #2 / HIGH-2).
- Codified the `gh issue list` fallback with an exact WebFetch URL for T2 (replan action #8).
- The Plan 03 decision tree has been rewritten to REMOVE any default-to-B logic (replan HIGH-1). On unresolved evidence the tree defers or lands on the most conservative option (D), not B.

---

## Thresholds

### T1 — Protocol-types LOC delta

- **Threshold:** If Full-adopt (A) would delete ≥5,000 LOC from `src/types/` + `src/shared/` (per 72-INVENTORY.md Totals), and Hybrid (B) would delete ≥3,000 LOC, then maintenance savings are material enough to accept migration cost. C1 (types-only) requires ≥3,500 LOC; C2 (transports-only) requires ≥800 LOC.
  Data source: 72-INVENTORY.md §"Totals" (Strong-overlap / deletable rows). Corroborate at Plan 03 time with `wc -l src/types/*.rs src/shared/*.rs`.

### T2 — rmcp governance responsiveness (issue close time + PR merge latency)

- **Threshold:** rmcp median time-to-close on closed issues over the last 50 issues MUST be <14 days AND rmcp median PR merge latency over the last 50 merged PRs MUST be <7 days for A/B/C1/C2 to be recommendable. If either metric is ≥14 days (issues) or ≥7 days (PRs), prefer Option D (Status quo); E (Fork) may attach as an escape hatch.
  Data source: Primary — `gh issue list -R modelcontextprotocol/rust-sdk --state closed --limit 50 --json closedAt,createdAt` for issue close time; `gh pr list --repo modelcontextprotocol/rust-sdk --state merged --limit 50 --json mergedAt,createdAt` for PR merge latency. Fallback (per reviews replan action #8) if `gh` CLI is unavailable or unauthenticated: WebFetch `https://api.github.com/repos/modelcontextprotocol/rust-sdk/issues?state=closed&per_page=100&page=1` (iterate pages 1..N until 50 records collected; compute median of `closed_at - created_at`) AND WebFetch `https://api.github.com/repos/modelcontextprotocol/rust-sdk/pulls?state=closed&per_page=100&page=1` filtered to records where `merged_at != null` (iterate pages similarly; compute median of `merged_at - created_at`).

### T3 — Enterprise extension-point coverage percent

- **Threshold:** PoC Slice 2 (proposal in 72-POC-PROPOSAL.md) must produce an adapter of ≤400 LOC AND pass the mcp-tester conformance suite with 0 failures for Option B (Hybrid) to be declared viable. Coverage percent = (passing conformance checks / total conformance checks) × 100 and must be ≥95 percent.
  Data source: PoC Slice 2 execution output written into 72-POC-RESULTS.md (Slice 2 section, when run). **STATUS AT PLAN 03 SYNTHESIS: UNRESOLVED** (Slice 2 is proposal-only in Phase 72; execution deferred to migration-scoping phase). Plan 03 treats T3 as UNRESOLVED unless a future phase updates 72-POC-RESULTS.md with Slice 2 data.

### T4 — SemVer break count (3 subcounts: compile errors + broken examples + broken downstream crates)

- **Threshold:** Option A is feasible if ALL three subcounts are within budget: (a) `cargo check --workspace --all-features` compile errors ≤50, (b) broken examples ≤3 of ~30 total examples, (c) broken downstream workspace crates ≤1 of {mcp-tester, mcp-preview, cargo-pmcp, pmcp-macros, pmcp-widget-utils, pmcp-macros-support}. Option A is disqualified if compile errors >200 OR broken examples >10 OR broken downstream crates >3.
  Data source: Primary — 72-POC-RESULTS.md `T4_compile_errors` field (Slice 1 executed in Plan 02 Task 1b; subcount (a) RESOLVED at value `0`). Subcounts (b) and (c) remain UNRESOLVED until a future full-migration spike is run. Commands for future resolution: `for ex in examples/*.rs; do cargo check --example "$(basename "$ex" .rs)" --features full 2>&1 | grep -c '"level":"error"'; done`; downstream crates: `cargo check -p mcp-tester; cargo check -p cargo-pmcp; cargo check -p pmcp-macros`.

### T5 — PoC time-box

- **Threshold:** At least one PoC slice must have a declared Time-box ≤3 days, and the aggregate Time-box of all PoC slices chosen for execution must be ≤6 days. This caps Phase 72 (and its migration-scoping successor) from drifting into implementation.
  Data source: 72-POC-PROPOSAL.md `Time-box:` declarations per slice. Aggregate = sum of Time-box values for slices selected in §"Slice Selection Recommendation".

### T6 — pmcp v2.x breaking-change window

- **Threshold:** pmcp v2.x MUST still be in a declared breaking-change window for Option A, B, or C1 to be recommendable. If window is closed, recommend C2 or D and defer foundation adoption to the next major window.
  Data source: 72-CONTEXT.md `breaking_change_window:` field (set in Plan 01 Task 0). Values: `open` / `closed` / `closing_YYYY-MM-DD` / `UNKNOWN`. If UNKNOWN, threshold counts against the Plan 03 net-resolved count.

### T7 — Downstream pmcp-user tolerance for v3.0

- **Threshold:** If production_user_tolerance is `0` OR `1-2` with explicit "no v3.0", Options A/B/C1 are disqualified — prefer D or C2. If `3-5` with ≥2 tolerating, A/B/C1 eligible. If `6-20` or `21+`, Plan 03 MUST add a user-communication subtask to Next Steps.
  Data source: 72-CONTEXT.md `production_user_tolerance:` field (set in Plan 01 Task 0).

### T8 — Historical churn on src/types/ and src/shared/ (last 180 days)  [NEW per reviews replan action #5]

- **Threshold:** If the number of git commits touching `src/types/` OR `src/shared/` in the last 180 days is ≥30, pmcp's protocol layer is actively churning — maintenance savings from transferring to rmcp are MATERIAL. If 10–29, savings are MODERATE. If <10, savings are MARGINAL (reconsider whether migration cost is justified). This replaces the weak "LOC deleted" proxy (reviews feedback: LOC deleted is not maintenance burden).
  Data source: `git log --since=180.days.ago --oneline -- src/types/ src/shared/ | wc -l` executed at Plan 03 synthesis time. Record the count directly in 72-RECOMMENDATION.md Criterion-1 justification.

### T9 — Enterprise-feature-preservation checklist  [NEW per reviews replan action #5]

- **Threshold:** Each of the seven pmcp enterprise extension-points MUST have a named preservation mechanism in the evaluated option, OR that option scores 0 on T9 (binary per-feature). The seven features (per 72-STRATEGY-MATRIX.md "Enterprise feature preservation" column):
  1. `TypedTool<I, O>` (src/server/typed_tool.rs)
  2. Workflow engine (src/server/workflow/)
  3. `mcp_apps` + UI (src/types/mcp_apps.rs, src/types/ui.rs)
  4. Auth (src/types/auth.rs, auth middleware)
  5. Middleware framework (src/shared/middleware.rs)
  6. `mcp-preview` crate
  7. `cargo-pmcp` binary
  An option passes T9 if and only if all 7 features have a named extension-point under that option. For example, Option A passes T9 only if each feature has an explicit "stays first-party in pmcp crate" or "re-exposed through rmcp facade" note in the plan.
  Data source: 72-STRATEGY-MATRIX.md "Enterprise feature preservation" column for the chosen option. Plan 03 Criterion-4 justification MUST enumerate the 7 features and state PRESERVED/BROKEN per feature.

---

## Decision Tree (reviews-mode revised — removes default-to-B bias per HIGH-1)

```
STEP 1: Count net-resolved thresholds.
  Let R = number of T1..T9 thresholds that have a concrete measured value
          (not UNRESOLVED, not UNKNOWN).
  Net-resolved count N := R.

STEP 2: If N < 3:
    Recommendation = DEFER
    Produce a next-measurements checklist naming what would close
    each UNRESOLVED threshold.
    STOP.

STEP 3: If N is 3..4:
    Recommendation = D (Status quo — most conservative)
    Rationale: insufficient evidence to justify breaking changes.
    List the 2 measurements that would flip the recommendation.
    STOP.

STEP 4: If N >= 5:
    Score each option (A, B, C1, C2, D) against resolved thresholds:
      - T1 weight: +1 if option meets LOC threshold, 0 otherwise.
      - T2 weight: -inf (disqualify) if governance fails AND option in {A, B, C1, C2}.
      - T3 weight: +2 if resolved AND passing; 0 if resolved AND failing (disqualify A/B); ignored if UNRESOLVED.
      - T4 weight: 0 if compile errors <=50 AND examples <=3 AND downstream <=1;
                    -inf (disqualify A) if compile errors >200 OR examples >10 OR downstream >3.
      - T5 weight: 0 for all options (precondition, not differentiator).
      - T6 weight: -inf (disqualify A, B, C1) if window is closed.
      - T7 weight: -inf (disqualify A, B, C1) if tolerance is 0 or "1-2 with no v3.0"; 0 otherwise.
      - T8 weight: +2 if >=30 commits (churn justifies migration); +1 if 10-29; 0 if <10.
      - T9 weight: +3 if all 7 features preserved; 0 if any feature broken (disqualify).
    Recommendation = highest-scoring non-disqualified option.
    Ties broken by lower migration cost (D > C2 > C1 > B > A).
    STOP.

NEVER:
  - Default to any option when evidence is unresolved.
  - Select E as the primary recommendation (E is contingency-only; see 72-STRATEGY-MATRIX.md footnote).
```

## Mapping to Strategy Matrix Criteria

| Rubric threshold | Strategy matrix criterion |
|------------------|---------------------------|
| T1 | Maintenance reduction |
| T2 | Spec-upgrade agility (governance is the gating risk) |
| T3 | Enterprise feature preservation |
| T4 | Breaking-change surface |
| T5 | Migration cost |
| T6 | Breaking-change surface (SemVer window gating) |
| T7 | Migration cost (user-borne) |
| T8 | Maintenance reduction (replaces weak LOC proxy) |
| T9 | Enterprise feature preservation (binds 7 features to option choice) |

## Feeds Into

- 72-RECOMMENDATION.md: Plan 03 walks the Decision Tree above. Each `### Criterion` subsection cites the relevant T-IDs. If N<3 → DEFER. If 3..4 → D. If >=5 → highest-scoring non-disqualified option.
- Any UNRESOLVED threshold at Plan 03 synthesis time becomes an explicit entry in the `## UNRESOLVED Thresholds` section of the recommendation.
