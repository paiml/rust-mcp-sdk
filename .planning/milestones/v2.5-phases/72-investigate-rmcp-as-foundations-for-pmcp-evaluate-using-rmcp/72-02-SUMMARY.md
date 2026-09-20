---
phase: 72
plan: 02
subsystem: research/decision
tags:
  - research
  - decision
  - rmcp
  - foundations
  - poc
  - rubric
  - spike-execution
  - reviews-mode
requires:
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-RESEARCH.md
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-REVIEWS.md
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-VALIDATION.md
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-CONTEXT.md
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-INVENTORY.md
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-STRATEGY-MATRIX.md
  - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-01-SUMMARY.md
provides:
  - 72-POC-PROPOSAL.md (3 PoC slices; LOC/Files/Pass/Fail/Time-box per slice)
  - 72-POC-RESULTS.md (measured output from Slice 1 throwaway spike)
  - 72-DECISION-RUBRIC.md (9 falsifiable thresholds T1..T9, gh fallback URL codified, no default-to-B logic)
affects: []
tech-stack:
  added: []
  patterns:
    - "Throwaway time-boxed spike pattern: scratch branch `spike/72-poc-slice-1` + scratch dir `examples/spike_72_rmcp_types/` created, measured, deleted — only markdown artifact survives on main"
    - "Per-threshold Data source convention: every rubric threshold followed by a reproducible command, named deliverable, or CONTEXT.md field"
    - "Decision tree with explicit DEFER outcome and no default-to-any-option bias"
key-files:
  created:
    - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-POC-PROPOSAL.md
    - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-POC-RESULTS.md
    - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-DECISION-RUBRIC.md
    - .planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-02-SUMMARY.md
  modified: []
decisions:
  - "Slice 1 spike ran in ~15 minutes wall-clock (well under 4-hour hard time-box); produced T4_compile_errors=0 and T4_loc_delta=537"
  - "Serde-shape divergence discovered: rmcp::model::JsonRpcRequest<Request> requires params field present as object; canonical JSON-RPC 2.0 shapes with params:null or omitted params FAIL. This downgrades 72-INVENTORY.md row 1 serde-compat rating from EXACT to compatible-via-adapter — Plan 03 must surface"
  - "Rubric decision tree explicitly removes any default-to-B or default-to-any-option logic per reviews HIGH-1; on unresolved evidence tree defers (N<3) or lands on D most-conservative (N=3..4), never B"
  - "Task commits squash-combined into single aggregate commit per Plan 02 Task 3 prescribed verify block (same pattern used in Plan 01)"
metrics:
  duration: "single session, ~25m wall-clock"
  completed: "2026-04-19"
---

# Phase 72 Plan 02: PoC Proposal + Executed Slice 1 Spike + Decision Rubric Summary

Produce Phase 72's mid-phase deliverables — the PoC proposal, the EXECUTED Slice 1 spike results, and the expanded decision rubric — that bridge the inventory/matrix (Plan 01) to the final recommendation (Plan 03). All in reviews-mode revised form addressing HIGH-1 (default-to-B removal), HIGH-2 (Slice 1 execution gap), and replan actions #5 and #8 (rubric expansion + gh fallback codification).

## Reviews-Mode Revision Summary

| Reviews finding | How addressed in this plan |
|---|---|
| **HIGH-1 (consensus)** default-to-B bias in Plan 03 | 72-DECISION-RUBRIC.md §"Decision Tree" explicitly removes default-to-B logic. On `N<3` unresolved: DEFER. On `N=3..4`: D (most conservative). On `N>=5`: highest-scoring non-disqualified option. Tie-break prefers lower migration cost (D > C2 > C1 > B > A). Verified by grep: no occurrence of `default.*B (conditional)` or `default to B` in the rubric. |
| **HIGH-2 (Gemini)** PoC execution gap — T3/T4 would remain UNRESOLVED | Plan 02 Task 1b EXECUTED PoC Slice 1 as throwaway time-boxed spike. Branch `spike/72-poc-slice-1` created, `examples/spike_72_rmcp_types/` scratch crate added with `rmcp = "=1.5.0"` dep, `cargo check` + `cargo run` executed, measurements captured to 72-POC-RESULTS.md, branch + scratch dir DELETED before commit. T4 subcount (a) RESOLVED at `0`. |
| **Replan action #5 (Codex)** weak-proxy thresholds | Added T8 (historical churn: 180-day git log on src/types/ + src/shared/) + T9 (enterprise-feature-preservation per-feature checklist over 7 features: TypedTool, workflow, mcp_apps, auth, middleware, mcp-preview, cargo-pmcp). Updated T2 to include PR merge latency in addition to issue close time. Updated T4 to subcount compile errors + broken examples + broken downstream crates separately. |
| **Replan action #8 (Gemini)** gh CLI fallback not codified | T2 Data source line now includes verbatim WebFetch URLs `https://api.github.com/repos/modelcontextprotocol/rust-sdk/issues?state=closed&per_page=100&page=1` and the pulls equivalent, with pagination + median-calculation instructions. |

## Slice 1 Spike Outcome

**Status:** COMPLETED (not TIMED_OUT, not ABORTED)
**Wall-clock time:** ~15 minutes (hard time-box was 4 hours)

**Measurements:**

| Metric | Value | Interpretation |
|---|---|---|
| `T4_compile_errors` | **0** | Well under the 50-error A-feasibility threshold; types-layer compile-check passes cleanly |
| `T4_loc_delta` | **537** | pmcp `src/types/jsonrpc.rs` (615 LOC) − spike total (78 LOC = 65 main.rs + 13 Cargo.toml) |
| Serde roundtrip ATTEMPT 1 | FAIL | canonical `{jsonrpc, id, method, params: null}` → rmcp rejects `null` (expects `WithMeta` struct) |
| Serde roundtrip ATTEMPT 2 | FAIL | canonical `{jsonrpc, id, method}` (no params) → rmcp rejects missing field |
| Serde roundtrip ATTEMPT 3 | PASS | `{jsonrpc, id, method, params: {}}` (empty object) → parses cleanly |

**Side-effect finding:** 72-INVENTORY.md row 1 serde-compat rating should be downgraded from **EXACT** to **compatible-via-adapter** (requires params-normalizer that converts missing/null params to `{}`). Plan 03 "Strongest Counterargument" section should surface this.

## Cleanup Verification

| Check | Command | Result |
|---|---|---|
| Spike branch deleted | `git branch -a \| grep -c 'spike/72-poc-slice-1'` | `0` |
| Scratch dir absent | `test ! -d examples/spike_72_rmcp_types` | absent |
| No source/Cargo.toml changes in Plan 02 commit | `git log -1 --name-only \| grep -E '^(src/\|Cargo\.toml)'` | empty |
| Plan 02 commit lists 3 markdown files only | `git log -1 --name-only` | 72-POC-PROPOSAL.md, 72-POC-RESULTS.md, 72-DECISION-RUBRIC.md |

## Rubric Thresholds Shipped (9 total — target met, minimum 7)

| T-ID | Criterion | Data source | Status at Plan 02 completion |
|---|---|---|---|
| T1 | Protocol-types LOC delta | 72-INVENTORY.md Totals | RESOLVED (directional numbers in inventory) |
| T2 | Governance responsiveness (issue close + PR merge) | `gh` + WebFetch fallback URL | UNRESOLVED (run at Plan 03 synthesis) |
| T3 | Enterprise extension-point coverage % | PoC Slice 2 output in 72-POC-RESULTS.md | UNRESOLVED (Slice 2 is proposal-only) |
| T4 | SemVer break count (3 subcounts) | 72-POC-RESULTS.md (subcount a resolved) + future spike | PARTIAL (a)=0 RESOLVED; (b)(c) UNRESOLVED |
| T5 | PoC time-box | 72-POC-PROPOSAL.md | RESOLVED (Slice 1=4h, Slice 2=2d, Slice 3=3d) |
| T6 | pmcp v2.x breaking-change window | 72-CONTEXT.md T6 | UNRESOLVED (locked UNKNOWN w/ Resolution path) |
| T7 | Production-user tolerance for v3.0 | 72-CONTEXT.md T7 | UNRESOLVED (locked UNKNOWN w/ Resolution path) |
| T8 | Historical churn on src/types/ + src/shared/ (180d) | `git log --since=180.days.ago --oneline -- src/types/ src/shared/ \| wc -l` | UNRESOLVED (run at Plan 03 synthesis) |
| T9 | Enterprise-feature-preservation checklist (7 features) | 72-STRATEGY-MATRIX.md column + Plan 03 Criterion-4 | RESOLVED (matrix cells in place for all 5 options) |

**Count of UNRESOLVED at Plan 02 completion:** 6 of 9 (T2, T3, T4 partial, T6, T7, T8).
**Count of RESOLVED at Plan 02 completion:** 3 full + 1 partial (T1, T5, T9 full; T4 subcount (a) partial).

Plan 03 decision tree will count T4 as RESOLVED if it consumes T4 subcount (a) and documents (b)/(c) as deferred evidence, or as UNRESOLVED if it insists on all three subcounts. Plan 03 is expected to resolve T2, T6, T7, and T8 via `/gsd-verify-work` user input + `gh`/git-log commands, which would bring net-resolved count to 7 of 9 and activate Step 4 (highest-scoring option) of the decision tree.

## Mapping to Strategy Matrix Criteria

| Rubric threshold | Strategy matrix criterion |
|---|---|
| T1, T8 | Maintenance reduction |
| T2 | Spec-upgrade agility (governance gate) |
| T3, T9 | Enterprise feature preservation |
| T4, T6 | Breaking-change surface |
| T5, T7 | Migration cost |

## Cross-Doc Consistency Check

- **T4 data source in 72-DECISION-RUBRIC.md cites 72-POC-RESULTS.md (not UNRESOLVED):** PASS — verified by `grep '72-POC-RESULTS.md' 72-DECISION-RUBRIC.md` → 3 matches, including the T4 Data source line.
- **T8 data-source command is reproducible:** PASS — `git log --since=180.days.ago --oneline -- src/types/ src/shared/ | wc -l` is a standard git invocation with no env-dependent arguments.
- **T9 enumerates exactly 7 features:** PASS — TypedTool, workflow engine, mcp_apps+UI, auth, middleware framework, mcp-preview, cargo-pmcp.
- **gh fallback URL is a syntactically valid GitHub API URL:** PASS — `https://api.github.com/repos/modelcontextprotocol/rust-sdk/issues?state=closed&per_page=100&page=1` matches GitHub's documented issues API shape.
- **Every PoC slice name appears as a Data source reference in the rubric:** PASS — Slice 1 via 72-POC-RESULTS.md (T4); Slice 2 via 72-POC-PROPOSAL.md (T3); all slice Time-box declarations feed T5.

## Deviations from Plan

### Task commits squash-combined into aggregate commit (Rule 3 blocking-issue auto-fix)

- **What:** GSD executor protocol prescribes per-task atomic commits; Plan 02 Task 3's prescribed verify block requires `git log -1 --name-only` to list all 3 deliverable files AND the subject to match `docs(72):` + `reviews-mode`. I committed each task atomically (3 commits: 976f7d41 = Task 1, 243196ad = Task 1b, 3d1b8794 = Task 2), then `git reset --soft HEAD~3` and re-committed as a single aggregate commit `2fb2573f` with the prescribed message so the Task 3 verify block passes.
- **Why:** Satisfies both protocols — per-task commits during execution gave me atomic rollback points; the final aggregate satisfies the plan-prescribed verify invariant. Exactly the same pattern used in Plan 01 (see 72-01-SUMMARY.md §"Deviations from Plan" → "Task 4 commit shape").
- **Risk:** Zero — soft reset operated only on commits I created in this session.
- **Classification:** Rule 3 (blocking issue for Task 3 verify) — auto-fixed.

### Spike binary test expanded beyond minimal roundtrip

- **What:** The plan's seed code for `main.rs` suggested a single `serde_json::from_value` roundtrip test on canonical `{params: null}` shape. That attempt panicked with `invalid type: null, expected struct WithMeta`. Rather than recording a panic as the only signal, I expanded the spike to test three shapes (params: null, no params, params: {}) so the results file captures which shapes rmcp accepts and which it rejects. This is stronger signal for Plan 03 than a single-shape failure would have been.
- **Why:** Rule 2 — auto-add missing critical functionality. Single-shape failure is under-informative; testing three canonical JSON-RPC 2.0 shapes covers the actual interop question (what does rmcp's type demand vs. what does the wire spec allow?).
- **Classification:** Rule 2 (auto-add missing critical functionality) — auto-added.

### No framing changes from 72-REVIEWS.md

All revisions in this plan (default-to-B removal, Slice 1 execution, T8/T9 addition, T2 PR-merge-latency addition, gh fallback URL codification) mirror 72-REVIEWS.md replan actions #1/#2/#5/#8 faithfully. No fresh framing decisions were introduced.

## Self-Check

**Files:**
- `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-POC-PROPOSAL.md` — FOUND (104 lines)
- `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-POC-RESULTS.md` — FOUND (86 lines)
- `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-DECISION-RUBRIC.md` — FOUND (133 lines)

**Commits:**
- `2fb2573f` — aggregate Plan 02 commit (`docs(72): PoC proposal, Slice 1 spike results, expanded decision rubric (T1..T9, gh fallback codified) [reviews-mode]`) — FOUND on HEAD with all 3 markdown files.

**Spike cleanup:**
- `git branch -a | grep -c 'spike/72-poc-slice-1'` → 0 (branch deleted)
- `test ! -d examples/spike_72_rmcp_types` → absent (scratch dir deleted)
- `git log -1 --name-only | grep -E '^(src/|Cargo.toml)'` → empty (no source pollution)

## Self-Check: PASSED
