---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
plan: 11
subsystem: validate_widget_pair example extension + 78-HUMAN-UAT cycle-2 rewrite + Test 6 prod re-verification checkpoint
tags: [mcp-apps, validator, claude-desktop, gap-closure, cycle-2, always-coverage, human-uat, checkpoint, cost-coach-prod, calculator-cross-server]
gap_closure: true
requirements:
  - PHASE-78-AC-1
  - PHASE-78-AC-2
  - PHASE-78-AC-3
  - PHASE-78-AC-4
  - PHASE-78-AC-5
  - PHASE-78-ALWAYS-EXAMPLE

dependency-graph:
  requires:
    - "Plan 78-09 — captured 6 real-prod fixtures + RED-phase tests + root-cause documentation"
    - "Plan 78-10 — string-literal aware comment stripper + widened G2 regex (validator GREEN at lib level)"
  provides:
    - "validate_widget_pair example demonstrates cycle-2 GREEN end-to-end (3 cycle-1 + 6 cycle-2 real-prod runs, all pass)"
    - "78-HUMAN-UAT.md flipped status: re-verify → complete + gap_closure_validated: false → true after operator-confirmed Test 6 success"
    - "Cycle-2 prod evidence file proving cost-coach 33 Failed → 0 Failed"
    - "Cross-server validation evidence: Scientific Calculator MCP App also turned green after team adopted SDK via the migration document"
    - "MIGRATION-CALCULATOR-TO-MCP-APPS-SDK.md — bonus deliverable produced from operator's Test 6 supplemental run; the calculator team used it to migrate"
  affects:
    - "Phase 78 ROADMAP — gap-closure cycle 2 wave 3 closed; phase 78 ready for /gsd-verify-work"
    - "External: Scientific Calculator MCP App team migrated their widget to the canonical SDK"

tech-stack:
  added: []
  patterns:
    - "Example as observability proof: extending the working example with real-prod fixture runs surfaces validator state to operators without requiring them to read test code"
    - "HUMAN-UAT branching: explicit pass/fail recipes with concrete commands, expected outputs, and route decisions; eliminates ambiguity at the operator-decision boundary"
    - "Cross-server validation as bonus evidence: testing cycle-2 fix against an independent second server (calculator) revealed both validator accuracy (failures were real for non-SDK widgets) AND a follow-up improvement opportunity (G1 false-positive on widgets that contain protocol-level method-name strings without loading the SDK)"
    - "Migration document as executable artifact: turned a TestFailed status into a workable plan for an external team in ~30 min; the team migrated and the validator turned green"

key-files:
  created:
    - "crates/mcp-tester/examples/validate_widget_pair.rs (extended with 6 cycle-2 real-prod runs + tally + success-path summary literal)"
    - ".planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/78-11-SUMMARY.md (this file)"
    - ".planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/MIGRATION-CALCULATOR-TO-MCP-APPS-SDK.md (calculator team migration guide — bonus deliverable from operator's supplemental run)"
    - ".planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/uat-evidence/2026-05-02-cost-coach-prod-cycle2-rerun.md (Test 6 PASSED evidence)"
    - ".planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/uat-evidence/2026-05-02-scientific-calculator-cycle2-rerun.md (cross-server validation evidence)"
  modified:
    - ".planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/78-HUMAN-UAT.md (cycle-2 rewrite + close-out: status complete, gap_closure_validated true, all 6 tests pass, G6 closed)"

decisions:
  - "Wrote MIGRATION-CALCULATOR-TO-MCP-APPS-SDK.md as a bonus deliverable from the operator's supplemental Test 6 run against the Scientific Calculator. The calculator's 48 Failed rows were real (widget used legacy window.mcpBridge instead of the canonical SDK); rather than weaken the validator to recognize legacy patterns, the right move was to document the migration to the canonical pattern. The calculator team applied it the same day."
  - "Cross-server validation finding documented: G1 SDK detection has a false-positive class for widgets that contain protocol-level method-name string literals (e.g. 'ui/notifications/tool-result' inside their own message filter) without actually loading the SDK runtime. NOT addressed in cycle 2 — flagged in uat-evidence/2026-05-02-scientific-calculator-cycle2-rerun.md as a follow-up phase consideration if the project decides legacy bridge widgets should be recognized as a valid alternative integration."
  - "validate_widget_pair example uses a wrapping run_one helper around run_one_and_count_failed. Preserves cycle-1 baseline call shape (3 first calls don't tally) while introducing the cycle-2 tally for the 6 new real-prod runs. Slightly less elegant than refactoring all 9 calls to return failed counts, but preserves backward-compatibility of the existing cycle-1 line semantics in case downstream readers depended on them."
  - "Plan 11 Task 3 checkpoint resume signal: operator chose `approved` based on cost-coach 96/96 PASS. Cross-server calculator finding was a separate investigation that did not block cycle-2 closure (the cycle-2 acceptance bar was specifically zero Failed rows on cost-coach prod, which was met)."

metrics:
  duration: "~45 minutes wall-clock — Task 1 example extension (~15 min) + Task 2 HUMAN-UAT rewrite (~15 min) + Task 3 operator Test 6 + cross-server investigation + migration document (~15 min orchestrator + ~30 min operator-side calculator migration which happened separately)"
  completed: "2026-05-02"
  tasks_completed: "3 of 3 (Task 1 + Task 2 autonomous, Task 3 = checkpoint:human-verify resolved with `approved`)"
  commits:
    - "5cb3871b — test(78-11): extend validate_widget_pair example with 6 cycle-2 real-prod widgets"
    - "e83b3f9c — docs(78-11): rewrite HUMAN-UAT for cycle-2 expected outcomes + Test 6 acceptance bar"
    - "7619b742 — docs(78): migration guide for Scientific Calculator team — switch to @modelcontextprotocol/ext-apps SDK (bonus, between Tasks 2 and 3)"
    - "14944c86 — docs(78-11): record cost-coach prod cycle-2 GREEN — Test 6 PASSED + cross-server validation"
---

# Phase 78 Plan 11: ALWAYS-coverage extension + HUMAN-UAT cycle-2 rewrite + Test 6 re-verification Summary

Plan 78-11 closed Phase 78 cycle-2 gap closure on the cost-coach binding proof: Test 6 against `https://cost-coach.us-west.pmcp.run/mcp` returned **96 tests passed, 0 failed, 0 warnings** — Status PASSED. Cycle-1's 33-Failed-row false-positive class is gone. A second independent server (Scientific Calculator MCP App) was used as bonus cross-server validation; the calculator's pre-migration failures were real, the team adopted the SDK via the migration document, and post-migration also passed.

## Objective Recap

Close gap-closure cycle 2 by extending the ALWAYS-requirement working example to demonstrate cycle-2 GREEN, rewriting the HUMAN-UAT with cycle-2-explicit expected outcomes, and checkpointing for operator re-verification of Test 6 against cost-coach prod.

## What Landed

### Task 1 (commit `5cb3871b`) — validate_widget_pair example extended

Added 6 new `include_str!` constants for the real-prod fixtures + 6 new `run_one_and_count_failed` calls in `main()` + a tally `real_prod_failed_total` + a branching success/warning summary line. Output post-Plan-78-10:

```
Done. broken: many Failed; corrected: zero Failed; cycle-1 synthetic: zero Failed; cycle-2 real-prod: 0 Failed total across 6 widgets.
All 6 cycle-2 real-prod widgets produced zero Failed rows. Cost-coach v2 false-positive class CLOSED.
```

`cargo run -p mcp-tester --example validate_widget_pair` exits 0 with 9 widget reports (3 cycle-1 baseline + 6 cycle-2 real-prod). The existing `run_one` helper is preserved as a thin wrapper around the new count-returning variant.

### Task 2 (commit `e83b3f9c`) — 78-HUMAN-UAT.md cycle-2 rewrite

Rewrote `78-HUMAN-UAT.md` with:
- Frontmatter: added `cycle_2_landed: 2026-05-02`; source list extended with `78-09-PLAN.md` + `78-10-PLAN.md`
- Test 6 acceptance bar made explicit: "zero Failed rows on the 8 production widgets"
- "Re-verification context" section narrating v1 → cycle-1 → cycle-2 progression with the actual root-cause finding (comment-stripper bug) documented
- Cycle-1 G6 entry preserved with `closing — pending Test 6` status
- `gap_closure_validated: false` until operator confirmed Test 6
- Optional supplemental `validate_widget_pair` run documented as offline-fallback / drift-check

### Bonus deliverable (commit `7619b742`) — Calculator migration guide

The operator ran Test 6 against an additional second server (Scientific Calculator MCP App at `https://scientific-calculator-mcp-app.us-east.true-mcp.com/mcp`) and got 48 Failed rows + 8 Warnings on 8 tools. Per-row analysis showed:
- **48 Failures: REAL** — the calculator widget used a legacy `window.mcpBridge.*` pattern instead of the canonical `@modelcontextprotocol/ext-apps` SDK. No `new App({...})`, no handler assignments, no `app.connect()`.
- **8 ontoolresult Warnings: SOFT-CORRECT** — widget DID handle `ui/notifications/tool-result` via direct `addEventListener('message', ...)`, just not via `app.ontoolresult`.
- **8 G1 PASSED rows: FALSE POSITIVE** — the validator's heuristic over-reported because the widget contains the literal string `"ui/notifications/tool-result"` for its own message filter.

Wrote `MIGRATION-CALCULATOR-TO-MCP-APPS-SDK.md` mapping each `mcpBridge.*` call in their existing code to its SDK equivalent, with concrete before/after diffs:
- Add 1 import + 1 `new App({...})` + 5 handler assignments + 1 `app.connect()`
- Delete 1 message-listener block (becomes `app.ontoolresult`)
- Update `callTool` helper to use `app.callServerTool({name, arguments})`
- Replace `mcpBridge.theme` → `app.getHostContext().theme` + `app.onhostcontextchanged`
- Drop `notifyIntrinsicHeight` (autoResize is automatic)
- Address `setState`/`getState` (no direct SDK equivalent — recommended: drop and render purely from `ontoolresult` events)

Total diff: ~30 lines added / ~15 removed; estimated 30-min migration. Calculator team applied it the same day.

### Task 3 (commit `14944c86`) — Operator Test 6 + cross-server validation + close-out

Operator ran Test 6 and received the resume signal `approved` based on:

**Cost-coach (load-bearing):** 96 tests, 96 passed, 0 failed, 0 warnings. Compared to cycle-1's 97/60/33/4: a clean elimination of the false-positive class.

**Calculator (bonus cross-server):** post-migration result also PASSED. Validates the cycle-2 fix works on a second canonical-SDK widget.

Updates committed:
- `78-HUMAN-UAT.md`: `status: re-verify → complete`, `gap_closure_validated: false → true`, added `validated: 2026-05-02`, all 6 tests `[pending] → [pass]`, summary block updated to `passed: 6, pending: 0`, Gap G6 marked `closed` with cross-references to both evidence files.
- `uat-evidence/2026-05-02-cost-coach-prod-cycle2-rerun.md`: full Test 6 PASSED capture with per-tool breakdown.
- `uat-evidence/2026-05-02-scientific-calculator-cycle2-rerun.md`: cross-server pre/post-migration capture with diagnosis of false-positive vs real-failure rows.

## Verification Evidence

```sh
$ cargo run -p mcp-tester --example validate_widget_pair 2>&1 | tail -2
Done. broken: many Failed; corrected: zero Failed; cycle-1 synthetic: zero Failed; cycle-2 real-prod: 0 Failed total across 6 widgets.
All 6 cycle-2 real-prod widgets produced zero Failed rows. Cost-coach v2 false-positive class CLOSED.

$ cargo pmcp test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp
... (96 ✓ rows) ...
Total Tests: 96, Passed: 96, Failed: 0, Warnings: 0
Overall Status: PASSED

$ grep -E '^(status|gap_closure_validated|validated):' .planning/phases/78-*/78-HUMAN-UAT.md
status: complete
gap_closure_validated: true
validated: 2026-05-02

$ grep -E 'total:|passed:|pending:|G6.*closed' .planning/phases/78-*/78-HUMAN-UAT.md | head -5
total: 6
passed: 6
pending: 0
- **Status:** **closed** (cycle-2 Test 6 PASSED 2026-05-02)
```

## Deviations from Plan

### Beyond plan scope (handled per operator request)

**1. Calculator cross-server validation + migration document**
- **Found during:** Task 3 checkpoint — operator ran Test 6 against an additional second server beyond the cost-coach acceptance bar.
- **Result:** 48 Failed rows on calculator; per-row analysis showed they were real (calculator wasn't using the SDK).
- **Decision:** Operator requested a migration document for the calculator team. Wrote `MIGRATION-CALCULATOR-TO-MCP-APPS-SDK.md` as a bonus phase-78 artifact. Calculator team applied the migration; their server now passes too.
- **Rationale:** The migration document was the right artifact to produce — it converts a "your widget fails" verdict into a workable plan. Took ~15 min; team migrated in ~30 min separately. Strong cross-server validation of cycle-2 fix.

### Out-of-scope (logged for follow-up)

**1. G1 SDK-detection false-positive on non-SDK widgets**
- **Found during:** Calculator pre-migration diagnosis.
- **Issue:** The validator's G1 heuristic (`[ext-apps]` log prefix OR `ui/initialize` OR `ui/notifications/tool-result` OR `@modelcontextprotocol/ext-apps` import) over-reports for widgets that contain a protocol-level method-name string literal in their own code without actually loading the SDK. The calculator's pre-migration widget had `ui/notifications/tool-result` 4× in its message filter and the validator falsely reported "MCP Apps SDK wiring: PASSED" for all 8 tools.
- **Why out of scope:** The 48 Failed rows downstream of the false G1 PASSED were ALL real — the cascade caught the actual missing wiring. So the false G1 PASSED was cosmetically misleading but didn't change the operator-actionable verdict (the widget needed migration either way). Fixing this would require either (a) requiring TWO of 4 G1 signals OR (b) adding a stronger SDK-runtime detection (e.g. a unique runtime literal that only the inlined SDK contains). Defer to a future phase if the project decides false-positive G1 PASSEDs are worth addressing.

## Auth Gates

None.

## Self-Check: PASSED

- All 7 cycle-2 RED tests pass at lib level (Plan 78-10).
- All cycle-1 invariants preserved (5 RED + 7 widget + 4 property + 81 lib unit tests).
- `validate_widget_pair` example exits 0 and prints the success-path summary literal.
- 78-HUMAN-UAT.md frontmatter `gap_closure_validated: true` (`status: complete`).
- All 6 numbered tests in 78-HUMAN-UAT.md `result: [pass]`.
- Summary block: `passed: 6, pending: 0`.
- Gap G6 marked `closed` with cross-references.
- 2 cycle-2 evidence files committed (cost-coach + calculator).
- 4 commits on main: `5cb3871b` + `e83b3f9c` + `7619b742` + `14944c86`.
- Phase 78 ready for `/gsd-verify-work` final phase verification.
