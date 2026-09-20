# UAT Evidence — 2026-05-02 cost-coach prod re-verification (Test 6)

## Command
```
cargo pmcp test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp
```

## Result
- Total: 97
- Passed: 60
- **Failed: 33** (identical count to v1 pre-fix)
- Warnings: 4
- Overall Status: **FAILED**

## Diagnosis (per-widget breakdown)

8 App-capable tools, 8 widgets:

| Widget | SDK signal (G1) | Constructor (G2) | Handler cascade (G3) | Connect | Notes |
|---|---|---|---|---|---|
| `get_spend_summary` / cost-summary.html | PASSED | **FAILED** | — | — | Only G2 fails |
| `find_savings_opportunities` / savings-summary.html | **FAILED** | **FAILED** | **FAILED** (4 handlers + soft warn ontoolresult) | **FAILED** | Full cascade |
| `get_spend_over_time` / cost-over-time.html | **FAILED** | **FAILED** | **FAILED** (4 handlers + soft warn) | **FAILED** | Full cascade + window.openai/window.mcpBridge heuristic flag |
| `find_quick_wins` / savings-summary.html | **FAILED** | **FAILED** | **FAILED** (4 handlers + soft warn) | **FAILED** | Full cascade |
| `track_realized_savings` / savings-summary.html | **FAILED** | **FAILED** | **FAILED** (4 handlers + soft warn) | **FAILED** | Full cascade |
| `assess_tag_strategy` / tag-coverage.html | PASSED | **FAILED** | — | — | Only G2 fails |
| `connect_aws_account` / connect-account.html | PASSED | **FAILED** | — | — | Only G2 fails |
| `get_service_breakdown` / service-sankey.html | PASSED | **FAILED** | — | — | Only G2 fails |

## Failure mode summary

1. **G2 false-positive** (8/8 widgets): Constructor regex still doesn't match the real Vite-singlefile minified pattern produced by cost-coach prod. The synthetic fixture in Plan 05 used `new yl({name:..., version:...})` shape, but the actual prod bundles use a different mangled-id shape that the regex misses.

2. **G1 false-positive** (4/8 widgets): The 4 SDK-presence signals (`[ext-apps]` log prefix, `ui/initialize` method literal, `ui/notifications/tool-result` method literal, `@modelcontextprotocol/ext-apps` import literal) are not present in 4 of 8 prod widgets, even though those widgets render correctly in Claude Desktop.

3. **G3 design behavior** (4/8 widgets): When G1 false-negatives, G3's correct cascade-elimination still yields handler/connect failure rows since handlers and connect are *also* not detected in those bundles. This is consistent with G3's design (independence — emit per-signal failures regardless of SDK status), but it means G1 misses amplify into 6 extra failures per widget.

4. **window.openai/window.mcpBridge heuristic**: get_spend_over_time also flags this — possibly another false positive worth investigating.

## What this means

The gap-closure cycle (Plans 78-05 through 78-08) was tested against synthetic fixtures and passed, but did NOT generalize to real cost-coach prod bundles. The fixtures captured in Plan 05 were based on a model of what minified Vite output looks like, but the actual prod output diverges in two ways:

- Different constructor name-mangling pattern than `yl`
- Different SDK-loading mechanism (the 4 G1 signals don't appear)

The fix as landed is INCOMPLETE — the false-positive class is not eliminated against prod.

## Next step

Route to `/gsd-plan-phase 78 --gaps` to create another gap-closure cycle. The new plan should:

1. Sample a real cost-coach prod widget bundle (`fetch https://cost-coach.us-west.pmcp.run/.../cost-summary.html`) and inspect the actual minified shape
2. Update fixtures in `crates/mcp-tester/tests/fixtures/widgets/bundled/` to match real-prod-shape (replace synthetic guesses)
3. Generalize G1 SDK-detection signals to cover whatever pattern the 4 failing prod widgets use
4. Generalize G2 constructor regex to handle the actual mangled-id pattern in prod
5. Re-run the full 6 UAT items end-to-end against cost-coach prod after the new fixes land

The 5 CLI-boundary items (Tests 1-5) remain valid and should still be operator-verified — they exercise the local `--widgets-dir` path which has not been disproven.

## Source data
- Operator command: `cargo pmcp test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp`
- Operator local CLI: rebuilt from main HEAD (commit `7138b06f` after Wave 4 merge)
- Test environment: cost-coach prod (us-west)
