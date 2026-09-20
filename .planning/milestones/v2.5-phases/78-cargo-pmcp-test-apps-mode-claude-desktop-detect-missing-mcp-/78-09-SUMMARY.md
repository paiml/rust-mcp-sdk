---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
plan: 09
subsystem: mcp-tester real-prod fixtures + RED-phase integration tests + cycle-2 root-cause documentation
tags: [mcp-apps, validator, claude-desktop, gap-closure, cycle-2, red-phase, real-prod-fixtures, root-cause-discovery]
gap_closure: true
requirements:
  - PHASE-78-AC-1
  - PHASE-78-AC-2
  - PHASE-78-AC-3
  - PHASE-78-ALWAYS-UNIT

dependency-graph:
  requires:
    - "Plan 78-05 — cycle-1 synthetic fixtures (preserved untouched, consumed by no-regression test)"
    - "Plan 78-06 — current cycle-1 validator code (the file Plan 10 modifies)"
    - "uat-evidence/2026-05-02-cost-coach-prod-rerun.md — per-widget failure breakdown"
  provides:
    - "6 real cost-coach prod widget bundles bound as regression evidence"
    - "CAPTURE.md with SHA-256 + per-fixture grep evidence + step-by-step probe of the cycle-1 stripper bug"
    - "7 RED-phase integration tests (6 prod-fixture asserts + 1 cycle-1 no-regression sentinel)"
    - "Root-cause finding that rescoped Plan 78-10 from 'add new G1 signals' to 'fix strip_js_comments + widen G2 regex'"
  affects:
    - "Plan 78-10 — the GREEN-phase fix is now bound to real prod bytes; success criterion is all 7 tests pass"
    - "Plan 78-11 — Test 6 re-verification recipe gains concrete acceptance bar (zero Failed rows on prod)"

tech-stack:
  added: []
  patterns:
    - "Capture provenance as a first-class artifact: CAPTURE.md records SHA-256 + bytes + per-fixture grep evidence so future drift detection can confirm fixtures still match real prod"
    - "RED-phase test layout mirrors cycle-1 (`app_validator_widgets_bundled.rs`): same `validate(mode, tool_name, html)` + `count_status` helpers, one `#[test]` per fixture, plus a no-regression sentinel covering all cycle-1 synthetic fixtures"
    - "Step-by-step validator probe: writing a temporary example that loads each fixture and prints byte counts + signal hits at each strip stage made the comment-stripper bug visible in 30 seconds. Worth keeping as a debug technique."

key-files:
  created:
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/cost-summary.html (488,838 bytes, captured from cost-coach 29f46efd)"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/cost-over-time.html (507,371 bytes)"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/savings-summary.html (486,846 bytes)"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/tag-coverage.html (358,822 bytes)"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/connect-account.html (346,887 bytes)"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/service-sankey.html (374,804 bytes)"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/CAPTURE.md (provenance + per-fixture grep evidence + 'Root cause discovered' section)"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/README.md (cycle-2 mode-emission table + 'Why a separate real-prod/ subdirectory')"
    - "crates/mcp-tester/tests/app_validator_widgets_real_prod.rs (7 RED-phase tests)"
  modified:
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/README.md (cycle-2 update tail section pointing to real-prod/)"

decisions:
  - "Path A (local cost-coach checkout) used over Path B (resources/read against prod). Operator confirmed Path A; the local widget/dist/ output had been built recently and contained the same shape patterns as deployed prod. Validator probe confirmed: 1+8+7+1+1+1=19 distinct Failed rows in 6 fixtures, multiplied across 8 prod tool→widget mappings = 33, IDENTICAL to the prod re-run total."
  - "ROOT CAUSE PIVOT: the cycle-2 plan's original direction ('add new G1 signals') was the wrong fix. Step-by-step probe revealed strip_js_comments destroys 21 KB of SDK code in cost-over-time and savings-summary because a JS string `\"/*.example.com...\"` (a CSP frame-src directive) opens a phantom block comment that closes at a real `*/` license-header banner thousands of bytes later. The cycle-1 G1 4-signal OR is correct; the cycle-1 G2 regex shape is wrong; the cycle-1 comment-stripper is the load-bearing bug. Plan 78-10 was rescoped accordingly (commit 6a4ec9b8)."
  - "Wrote CAPTURE.md 'Root cause discovered' section as the load-bearing artifact for Plan 78-10. The section includes the byte-by-stage probe table, the offending CSP-string pattern, and the per-widget cascade explanation. This means Plan 78-10's executor sees the bug evidence directly without needing to re-derive it."
  - "RED-phase tests bind correctly: 6 fail today (matching prod cascade), 1 cycle-1 no-regression sentinel passes today. The RED contract is provable, not aspirational."
  - "Cycle-1 fixtures preserved untouched (`bundled/cost_summary_minified.html`, `bundled/cost_over_time_minified.html`, `bundled/synthetic_cascade_repro.html`). Both regression sets coexist."

metrics:
  duration: "~30 minutes wall-clock (Path A capture + SHA-256 + grep evidence + CAPTURE.md + READMEs + test file + bug discovery via step-by-step probe)"
  completed: "2026-05-02"
  tasks_completed: "3 of 3 (Task 1 = checkpoint:human-action satisfied via Path A, Task 2 = README files, Task 3 = test file)"
  commits:
    - "dba5aa1c — test(78-09): capture real cost-coach prod widgets + RED-phase tests + root-cause documentation"
---

# Phase 78 Plan 09: Real cost-coach prod fixtures + RED-phase integration tests Summary

Plan 78-09 captured 6 real cost-coach prod widget bundles, wrote CAPTURE.md provenance with per-fixture grep evidence, added 7 RED-phase integration tests, and surfaced a major root-cause finding that rescoped Plan 78-10. The original cycle-2 plan was tuned to "generalize G1 signals + G2 regex"; the actual bug is the comment stripper destroying SDK code before signals can be detected.

## Objective Recap

Capture real cost-coach prod widget bundles as durable regression fixtures so cycle 2's validator generalizations are tuned against actual minified shapes. Encode the regression contract: every captured prod fixture must report zero Failed rows under `--mode claude-desktop` after Plan 78-10 lands the fix.

## What Landed

### Task 1 (checkpoint:human-action) — Real prod fixture capture

Operator chose Path A (local cost-coach checkout). Captured 6 widget bundles from `~/projects/mcp/cost-coach/widget/dist/` at commit `29f46efd` (subject: "fix(tools,demo): align date defaults with documented tool descriptions"):

| File | SHA-256 (truncated) | Bytes |
|---|---|---|
| cost-summary.html | `e6368274...` | 488,838 |
| cost-over-time.html | `d585a695...` | 507,371 |
| savings-summary.html | `0f75e778...` | 486,846 |
| tag-coverage.html | `3e8b6879...` | 358,822 |
| connect-account.html | `4d182fcf...` | 346,887 |
| service-sankey.html | `7000b072...` | 374,804 |

CAPTURE.md records full SHA-256 + provenance + per-fixture grep evidence + drift-detection recipe.

### Task 2 — README files

`real-prod/README.md` documents the cycle-2 mode-emission table (post-Plan-10 expected: zero Failed rows per fixture under ClaudeDesktop) and explains why `real-prod/` is a separate subdirectory from cycle-1's `bundled/`.

`bundled/README.md` gets a tail section pointing readers to `real-prod/` for cycle-2 fixtures. Cycle-1 README content preserved untouched.

### Task 3 — RED-phase integration tests

`crates/mcp-tester/tests/app_validator_widgets_real_prod.rs` (190 lines) contains:
- 6 `#[test]` functions, one per real-prod fixture, asserting zero Failed rows under `AppValidationMode::ClaudeDesktop`
- 1 no-regression sentinel `test_real_prod_no_regression_on_cycle1_synthetic_fixtures` that exercises all 3 cycle-1 synthetic fixtures and asserts the existing post-Plan-06 expected emission
- File-level docstring that lists which 6 tests will fail until Plan 78-10 lands and describes the cycle-2 scope

Mirrors cycle-1's `app_validator_widgets_bundled.rs` style (`validate(mode, tool_name, html)` + `count_status` helpers).

### ⭐ ROOT CAUSE FINDING (rescopes Plan 78-10)

While verifying the RED contract, I ran the validator step-by-step against cost-over-time.html with a temporary probe example:

```
Raw script body:        505,381 bytes,  2 [ext-apps] hits
After HTML comment strip: 505,381 bytes, 2 [ext-apps] hits
After block comment strip: 484,064 bytes, 0 [ext-apps] hits  ← BUG
After line comment strip:  383,745 bytes, 0 [ext-apps] hits
```

The block-comment regex `/\*.*?\*/` is destroying 21 KB of SDK code. The bundle contains a JS string literal — a CSP `frame-src` directive value — that begins with `"/*.example.com..."`. The non-string-literal-aware regex sees that `/*` as a block-comment opener and matches the next `*/` it finds, which is the `*/` at the end of an `@kurkle/color v0.3.4` license-header banner thousands of bytes later. Everything between — including `[ext-apps]`, `ui/initialize`, `ui/notifications/tool-result`, all 4 handler member-assignments, and `app.connect()` — gets stripped before the regex match runs.

This explains the per-widget pattern in uat-evidence:
- cost-summary / tag-coverage / connect-account / service-sankey: don't have the CSP string positioning that triggers the phantom block comment → SDK section preserved → only G2 (constructor regex) fails → 1 Failed row each
- cost-over-time / savings-summary: contain `"/*.example.com..."` → SDK section stripped → G1 + handlers + connect cascade fail → 8 and 7 Failed rows respectively

Total: 1+8+7+1+1+1=19 distinct Failed rows in 6 fixtures, multiplied across 8 prod tool→widget mappings = **33 Failed rows — IDENTICAL to the 2026-05-02 prod re-run total**.

Cycle 2's actual fix is now: (a) make `strip_js_comments` string-literal aware, (b) widen G2 constructor regex to accept non-literal `name`/`version` values (real prod uses `name:"cost-coach-"+t,version:"1.0.0"` — concatenation, not literal). The cycle-1 G1 4-signal OR is correct as-is. Plan 78-10 was rescoped (commit 6a4ec9b8).

## Verification Evidence

```sh
$ cargo test -p mcp-tester --test app_validator_widgets_real_prod
test result: FAILED. 1 passed; 6 failed; 0 ignored — RED contract confirmed (6 prod tests fail, 1 no-regression test passes)

$ cargo test -p mcp-tester --test app_validator_widgets_bundled
test result: ok. 5 passed; 0 failed — cycle-1 RED tests preserved

$ shasum -a 256 crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod/*.html
e6368274313d0a941dd3b9f548217f9bd28aa150609339e7dd705aebf2e43e55  cost-summary.html
d585a69564daf97e3293fd553aab28a355bdcece85a717df4ed9a24d29c339aa  cost-over-time.html
0f75e778afb20621981f4ce607739505078a72b41294738f43b99c45bbf95b10  savings-summary.html
3e8b6879ec0ed46eda2530be47cdb7a6481df93b43c508fd41f2f4536005e0c8  tag-coverage.html
4d182fcf3fd901977eedecbe7d19318cc8489a824c4d6ade7ed9428236b57077  connect-account.html
7000b072e9d412e177b123c791ca645bd406eeb169cce2b8ea297ab2ccb713c3  service-sankey.html
```

## Deviations from Plan

### Auto-fixed Issues (Rule 3 — blocking issues)

**1. Cycle-2 plan rescope due to root-cause finding**
- **Found during:** Validator probe of captured fixtures (Task 3 verification phase)
- **Issue:** The original cycle-2 plan body's "add new G1 signals" direction was addressing the wrong layer. The probe revealed the comment stripper destroys SDK code before any G1 signal can match.
- **Why blocking:** Following the original plan would have shipped a fix that doesn't address the root cause; cycle 3 would have re-discovered the same bug.
- **Fix:** Documented the finding in CAPTURE.md "Root cause discovered" section + rescoped Plan 78-10 in a separate commit (`6a4ec9b8`). Plan 78-09 deliverables are unchanged.

### Out-of-Scope (none for this plan)

## Auth Gates

None — all changes are file copies + test file authoring; no external services.

## Self-Check: PASSED

- 6 real-prod fixture HTML files exist, are non-empty, and contain `<script>` tags.
- CAPTURE.md exists with SHA-256, byte size, and per-fixture grep evidence.
- real-prod/README.md exists with cycle-2 mode-emission table.
- bundled/README.md has the cycle-2 update tail section.
- app_validator_widgets_real_prod.rs exists with 7 `#[test]` functions.
- Cycle-2 RED contract holds today (6 fail, 1 passes).
- Cycle-1 invariants preserved (5 RED tests in app_validator_widgets_bundled.rs still pass).
- Commit `dba5aa1c` is on main.
