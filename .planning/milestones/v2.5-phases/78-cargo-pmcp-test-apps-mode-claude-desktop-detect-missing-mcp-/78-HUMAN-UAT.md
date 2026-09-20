---
status: complete
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
source: [78-VERIFICATION.md, 78-05-PLAN.md, 78-06-PLAN.md, 78-07-PLAN.md, 78-09-PLAN.md, 78-10-PLAN.md]
started: 2026-05-02T00:00:00Z
updated: 2026-05-02T22:00:00Z
gap_closure_landed: 2026-05-02
cycle_2_landed: 2026-05-02
gap_closure_validated: true
validated: 2026-05-02
---

## Current Test

[complete — Test 6 passed against cost-coach prod 2026-05-02; Phase 78 ready for /gsd-verify-work]

## Tests

### 1. AC-78-1 at CLI binary boundary — broken widget fails claude-desktop mode
expected: Run `cargo pmcp test apps --mode claude-desktop --widgets-dir <path>` against a directory containing a deliberately-broken widget (no SDK presence signals: no `@modelcontextprotocol/ext-apps`, no `[ext-apps]` log prefix, no `ui/initialize`, no `ui/notifications/tool-result`; no `new <id>({name:"...",version:"..."})` constructor in any form; no protocol handlers; no `app.connect()`). The simplest reproducer is to copy `crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html` into a tempdir and point `--widgets-dir` at it. Process exits non-zero AND stdout/stderr names at least one missing handler (e.g. `onteardown`).

Why this is testable without a fixture binary: Plan 78-07 added `--widgets-dir`, which bypasses the `cli_acceptance.rs` fixture-binary skip-gate.

Cycle-2 note: this test is unchanged from cycle 1 — Plan 10's regex generalization makes the comment-stripper string-literal aware and widens the constructor regex, but does NOT make broken widgets pass. The synthetic broken fixture remains broken.

result: [pass]

### 2. AC-78-2 at CLI binary boundary — corrected widget passes claude-desktop mode
expected: Run the same `--widgets-dir` command against a directory containing `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html`. Process exits zero.

Cycle-2 note: cycle-1 corrected fixture continues to pass. Plan 10's broader regex doesn't reject the unminified `App` constructor + import literal that the corrected fixture uses.

result: [pass]

### 3. AC-78-3 at CLI binary boundary — Standard mode is permissive
expected: Run `cargo pmcp test apps --widgets-dir <path>` (no `--mode` flag) against BOTH the broken and corrected widgets. Both invocations exit zero — no regression for the permissive default.

result: [pass]

### 4. AC-78-4 at CLI binary boundary — chatgpt mode is unchanged
expected: Run `cargo pmcp test apps --mode chatgpt --widgets-dir <path>` against both fixtures. Both exit zero AND stderr/stdout MUST NOT contain any of the four protocol handler names — chatgpt mode is a no-op for widget validation.

result: [pass]

### 5. AC-78-5 — UX review of READMEs and `--help` text
expected: Visual review of `cargo pmcp test apps --help` and both READMEs (`cargo-pmcp/README.md`, `crates/mcp-tester/README.md`). Reader can answer:
- "What does `--mode claude-desktop` do, when should I use it, and what does it check?"
- "What does `--widgets-dir` do, when should I prefer source scan over bundle scan?"

Both questions answerable from the docs alone.

result: [pass]

### 6. Re-verify against cost-coach prod (the cycle-2 binding proof)
expected: Run

```sh
cargo run -p cargo-pmcp -- test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp
```

Expected outcome (cycle-2 acceptance bar):
- Process exits **zero**.
- Output reports **zero Failed rows on the 8 production widgets**.
- Compare to cycle-1 result captured in `uat-evidence/2026-05-02-cost-coach-prod-rerun.md` (97 tests, 60 passed, **33 failed**, 4 warnings — all 33 confirmed false positives). Post-cycle-2 result: **96 tests, 96 passed, 0 failed, 0 warnings**. See `uat-evidence/2026-05-02-cost-coach-prod-cycle2-rerun.md` for the full evidence file.

Optional supplemental run (offline-fallback / drift-check):

```sh
cargo run -p mcp-tester --example validate_widget_pair
```

This runs the validator against the 6 real-prod fixtures committed in Plan 78-09 (captured from the same prod source, SHA-256s recorded in `tests/fixtures/widgets/bundled/real-prod/CAPTURE.md`). Expected last line: `All 6 cycle-2 real-prod widgets produced zero Failed rows. Cost-coach v2 false-positive class CLOSED.` — proves the captured fixtures match the live prod and that the same bytes pass under the live binary.

Source feedback driving this re-verification: `/Users/guy/projects/mcp/cost-coach/drafts/feedback-pmcp-test-apps-v1-false-positives.md` (8.4 KB, dated 2026-05-02). The original feedback documents the v1 false-positive class. The cycle-1 re-verify (`uat-evidence/2026-05-02-cost-coach-prod-rerun.md`) showed cycle-1 didn't generalize. This cycle-2 re-verify confirms cycle-2 does.

**Acceptance bar (load-bearing):** Test 6 is the single load-bearing acceptance gate for closing Phase 78 cycle 2. If Test 6 reports >0 Failed rows on cost-coach prod, the gap closure is INCOMPLETE and this UAT routes back to `/gsd-plan-phase 78 --gaps` for cycle 3. If Test 6 reports 0 Failed rows, the gap closure is COMPLETE and this UAT routes to `/gsd-verify-work` for final phase verification.

On pass:
- Update this file's frontmatter `gap_closure_validated: false` → `true`.
- Update Test 6 result from `[pending]` to `[pass]` with link to the new evidence file.
- Capture the prod re-run output in `uat-evidence/<date>-cost-coach-prod-cycle2-rerun.md`.
- Tests 1-5 results updated to `[pass]` if the operator confirmed each via local `--widgets-dir` runs.

On fail:
- Update Test 6 result with the failed-row count + diagnosis.
- Capture the prod re-run output in `uat-evidence/<date>-cost-coach-prod-cycle3-rerun.md`.
- Add a Gap entry below documenting the new failure mode.
- DO NOT flip `gap_closure_validated` — it stays `false` until cycle 3 closes the residual gap.

result: [pass]

## Summary

total: 6
passed: 6
issues: 0
pending: 0
skipped: 0
blocked: 0

## Re-verification context

Cycle 1 (Plans 78-05/06/07/08, completed 2026-05-02):
- **Plan 05** (Wave 1) captured 3 synthetic bundled fixtures + RED-phase integration tests encoding the v1 false-positive class.
- **Plan 06** (Wave 2) replaced fragile regexes with cycle-1 minification-resistant patterns + eliminated the SDK→handler/connect cascade. The 5 RED-phase tests turned GREEN.
- **Plan 07** (Wave 3) added `--widgets-dir <path>` source-scan mode + 3 CLI-boundary integration tests via `assert_cmd`.
- **Plan 08** (Wave 4) extended the property test corpus + bundled-fixture example + READMEs and rewrote this UAT.
- **Cycle-1 outcome:** library-level GREEN against synthetic fixtures, but the 2026-05-02 cost-coach prod re-run reported 33 Failed rows on 8 prod widgets — same count as v1. The synthetic fixtures didn't match real Vite-singlefile prod output.

Cycle 2 (Plans 78-09/10/11, this iteration):
- **Plan 09** (Wave 1) captured 6 REAL cost-coach prod widget bundles into `tests/fixtures/widgets/bundled/real-prod/` + added 6 RED-phase integration tests bound to those bytes + 1 cycle-1 no-regression sentinel. Step-by-step probe revealed the load-bearing root cause: `strip_js_comments` was destroying ~21 KB of SDK code in cost-over-time and savings-summary because a JS string `"/*.example.com..."` (a CSP frame-src directive value) opened a phantom block comment that closed at a real `*/` license-header banner thousands of bytes later.
- **Plan 10** (Wave 2) replaced the 3-regex comment stripper with a ~110-LOC string-literal aware state machine (out / block_comment / line_comment / single_string / double_string / template_string) AND widened the G2 constructor regex from `name:"<lit>",version:"<lit>"` to `[^}]{0,200}\bname\s*:[^,}]{0,100},\s*version\s*:` (real prod uses `name:"cost-coach-"+t,version:"1.0.0"` — string concatenation, not literal). 12 cycle-2 unit tests + 1 G2 false-positive guard property test added. PMAT cog ≤ 25 maintained.
- **Plan 11** (Wave 3 — this plan) extended `validate_widget_pair.rs` example with 6 cycle-2 real-prod widget runs + rewrote this UAT for cycle-2 expectations + checkpointed for operator Test 6 re-verification.

Pre-cycle-2 evidence: 8 cost-coach prod widgets produced 33 false-positive Failed rows (cycle-1 outcome).
Post-cycle-2 expected: 8 cost-coach prod widgets produce zero Failed rows.

## Gaps

(No new gaps unless Test 6 fails. The cycle-1 G6 entry below is preserved for history.)

### G6 — Plan 05/06 fix did not generalize to real cost-coach prod bundles (CLOSED by cycle 2)
- **Date:** 2026-05-02 (opened) / 2026-05-02 (closed)
- **Source:** Test 6 operator re-verification against `https://cost-coach.us-west.pmcp.run/mcp`
- **Evidence (cycle-1):** `uat-evidence/2026-05-02-cost-coach-prod-rerun.md` (97 tests, 60 passed, 33 failed, 4 warnings — all confirmed false positives)
- **Evidence (cycle-2 closure):** `uat-evidence/2026-05-02-cost-coach-prod-cycle2-rerun.md` (96 tests, 96 passed, 0 failed, 0 warnings — Status PASSED)
- **Cycle-2 closure:** Plans 78-09/10/11. Plan 09 captured the actual prod bytes; library-level probe revealed the comment-stripper as the load-bearing root cause (a JS string `"/*.example.com..."` opened a phantom block comment that destroyed ~21 KB of SDK code in cost-over-time and savings-summary); Plan 10 replaced the stripper with a string-literal aware state machine + widened the G2 regex; Plan 11 wired the binding proof.
- **Cross-server validation:** A second independent server (Scientific Calculator MCP App) was tested in parallel. Pre-migration: 48 Failed rows (real — widget didn't use the SDK). The team adopted the SDK using `MIGRATION-CALCULATOR-TO-MCP-APPS-SDK.md` and the post-migration run also passed. See `uat-evidence/2026-05-02-scientific-calculator-cycle2-rerun.md`.
- **Status:** **closed** (cycle-2 Test 6 PASSED 2026-05-02)
