# UAT Evidence — 2026-05-02 cost-coach prod cycle-2 re-verification (Test 6 PASSED)

## Command

```sh
cargo pmcp test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp
```

## Result

- Total: 96
- Passed: **96**
- Failed: **0**
- Warnings: **0**
- Overall Status: **PASSED**

Cycle-1 baseline (`uat-evidence/2026-05-02-cost-coach-prod-rerun.md`): 97 tests, 60 passed, 33 failed, 4 warnings.
Cycle-2 result: 96 tests, 96 passed, 0 failed, 0 warnings.

(The total drop 97 → 96 reflects the validator's deduplication of widget-bodies-by-URI: the cost-coach `savings-summary.html` is shared by 3 tools but only validated once in the post-cycle-2 path. Per-tool pass count remains 8/8 widgets.)

## Per-tool breakdown (all PASS)

| Tool | Widget URI | Result |
|---|---|---|
| `track_realized_savings` | `ui://cost-coach/savings-summary.html` | PASS |
| `get_spend_summary` | `ui://cost-coach/cost-summary.html` | PASS |
| `connect_aws_account` | `ui://cost-coach/connect-account.html` | PASS |
| `find_quick_wins` | `ui://cost-coach/savings-summary.html` | PASS |
| `find_savings_opportunities` | `ui://cost-coach/savings-summary.html` | PASS |
| `get_spend_over_time` | `ui://cost-coach/cost-over-time.html` | PASS |
| `get_service_breakdown` | `ui://cost-coach/service-sankey.html` | PASS |
| `assess_tag_strategy` | `ui://cost-coach/tag-coverage.html` | PASS |

All 8 App-capable tools' widget bodies pass every check under claude-desktop mode:
- `MCP Apps SDK wiring` — PASS
- `App constructor` — PASS
- 4 handler member assignments (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`) — PASS each
- `connect() call` — PASS
- `ontoolresult` handler — PASS

## Diagnosis vs cycle-1

Cycle 1 (Plans 78-05/06/07/08) shipped a fix tuned to synthetic minified fixtures and missed the actual root cause: the cycle-1 `strip_js_comments` was destroying ~21 KB of SDK code in cost-over-time and savings-summary because a JS string `"/*.example.com..."` (a CSP frame-src directive value) opened a phantom block comment that closed at a real `*/` license-header banner thousands of bytes later.

Cycle 2 (Plans 78-09/10/11):
- Plan 09 captured 6 real cost-coach prod widget bundles + 7 RED-phase integration tests + step-by-step probe that revealed the comment-stripper bug
- Plan 10 replaced the 3-regex stripper with a ~110-LOC string-literal aware state machine + widened the G2 constructor regex from `name:"<lit>",version:"<lit>"` to accept any value expression (real prod uses `name:"cost-coach-"+t,version:"1.0.0"` — string concatenation)
- Plan 11 extended the working example + rewrote HUMAN-UAT + checkpointed for this re-verification

## Closes

Gap G6 from `78-VERIFICATION.md` — closed.
Phase 78 cycle 2 acceptance bar (HUMAN-UAT Test 6: zero Failed rows on 8 cost-coach prod widgets) — met.

## Source data

- Operator command: `cargo pmcp test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp`
- Operator local CLI: rebuilt from main HEAD post Plan 78-10 (commit `dd15b7db`)
- Test environment: cost-coach prod (us-west)
- Validator binary: `cargo-pmcp` from this repo's main branch
