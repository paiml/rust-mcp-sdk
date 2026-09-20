# UAT Evidence — 2026-05-02 Scientific Calculator MCP App cycle-2 cross-server validation (PASSED)

## Background

While the cost-coach prod re-verification was the load-bearing acceptance gate for Phase 78 cycle 2 (Test 6 of `78-HUMAN-UAT.md`), the operator also ran the validator against an independent second MCP App server — Scientific Calculator MCP App — to test how cycle 2's fix generalizes beyond a single server.

This evidence file documents both the pre-migration result (which surfaced an interesting validator-accuracy question) and the post-migration result (which confirms the validator works on a second canonical-SDK widget).

## Pre-migration result

```sh
cargo pmcp test apps --mode claude-desktop https://scientific-calculator-mcp-app.us-east.true-mcp.com/mcp
```

- Total: 104
- Passed: 48
- Failed: **48**
- Warnings: 8
- Overall Status: **FAILED**

### Per-row diagnosis

The calculator widget at `~/projects/mcp/Scientific-Calculator-MCP-App/widgets/keypad.html` was implementing a **legacy `window.mcpBridge.*` pattern** rather than the canonical `@modelcontextprotocol/ext-apps` SDK. Probing its source confirmed:

| Signal | Hits in keypad.html source |
|---|---|
| `@modelcontextprotocol/ext-apps` import | 0 |
| `[ext-apps]` log prefix | 0 |
| `ui/initialize` | 0 |
| `ui/notifications/tool-result` | 4 (in own message-listener filter, not SDK runtime) |
| `new App(` | 0 |
| `.onteardown` / `.ontoolinput` / `.ontoolcancelled` / `.onerror` | 0 each |
| `.connect(` | 0 |

Validator findings categorized:

- **48 Failed rows: REAL.** The widget genuinely lacked the canonical SDK constructs — no `new App(...)`, no handler assignments, no `app.connect()`. These were correct identifications.
- **8 ontoolresult Warnings: SOFT-CORRECT.** The widget DID handle `ui/notifications/tool-result` via direct `window.addEventListener('message', ...)`, just not via `app.ontoolresult = ...`. The validator's WARN-tier was the right severity.
- **8 G1 PASSED rows ("MCP Apps SDK wiring"): FALSE POSITIVE.** The validator's heuristic fired on the literal string `"ui/notifications/tool-result"` in the widget's own filter — but that literal was being used to FILTER incoming messages, not because an SDK runtime was loaded. Specificity gap to address in a future phase.

## Migration

A migration document was produced: `MIGRATION-CALCULATOR-TO-MCP-APPS-SDK.md` in this phase directory. It mapped each `mcpBridge.*` call in the calculator's existing code to its `@modelcontextprotocol/ext-apps` SDK equivalent and showed the minimum diff (~30 lines added / ~15 removed).

The calculator team applied the migration on 2026-05-02.

## Post-migration result

```sh
cargo pmcp test apps --mode claude-desktop https://scientific-calculator-mcp-app.us-east.true-mcp.com/mcp
```

Result: **PASSED — calculator now green** (operator-confirmed).

The validator's findings transitioned from 48 Failed rows to all-pass once the calculator widget:
- Imported `@modelcontextprotocol/ext-apps`
- Constructed `new App({ name: "scientific-calculator", version: "1.0.0" })`
- Registered the four required handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`) plus `ontoolresult`
- Called `app.connect()` after handler registration

Plus replaced the legacy plumbing:
- `window.mcpBridge.callTool(name, args)` → `app.callServerTool({ name, arguments: args })`
- The line-589 `window.addEventListener('message', ...)` block → `app.ontoolresult = (result) => { ... }`
- `window.mcpBridge.notifyIntrinsicHeight` → automatic (`autoResize: true` default in `app.connect()`)
- `window.mcpBridge.theme` → `app.getHostContext().theme` + `app.onhostcontextchanged`

## What this proves

Cycle 2's validator fix generalizes beyond cost-coach to a second independent MCP App server. The migration document was the load-bearing artifact for the calculator team; once they followed it, their widget passed.

The validator's specificity is now:
- **High accuracy on canonical-SDK widgets** (cost-coach: 96/96 PASS; calculator post-migration: PASS)
- **Correctly identifies non-SDK widgets** (calculator pre-migration: 48 Failed rows that the team confirmed were real and chose to fix by adopting the SDK)
- **One residual false-positive class:** G1 SDK detection over-reports for widgets that contain protocol-level method-name string literals without actually loading the SDK. Marked for follow-up phase consideration if the project decides legacy `window.mcpBridge` widgets should be recognized as a valid alternative pattern.

## Source data

- Operator commands: pre-migration + post-migration runs against `https://scientific-calculator-mcp-app.us-east.true-mcp.com/mcp`
- Calculator source: `~/projects/mcp/Scientific-Calculator-MCP-App/`
- Migration document: `.planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/MIGRATION-CALCULATOR-TO-MCP-APPS-SDK.md`
- Validator binary: `cargo-pmcp` from main branch post Plan 78-10 (commit `dd15b7db`)
