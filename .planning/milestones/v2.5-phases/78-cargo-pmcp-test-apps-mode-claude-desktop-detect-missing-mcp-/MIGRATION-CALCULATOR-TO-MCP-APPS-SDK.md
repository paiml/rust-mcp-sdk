# Migrating the Scientific Calculator widget to `@modelcontextprotocol/ext-apps`

**For:** Scientific Calculator MCP App team
**Source:** `cargo pmcp test apps --mode claude-desktop https://scientific-calculator-mcp-app.us-east.true-mcp.com/mcp` reported 48 Failed rows + 8 Warnings on 8 tools (2026-05-02)
**Why:** The widget currently uses a legacy `window.mcpBridge.*` pattern that is not the MCP Apps spec's canonical SDK. To work reliably across Claude Desktop, ChatGPT, and any future MCP Apps host, switch to the official `@modelcontextprotocol/ext-apps` SDK. This document maps each `mcpBridge` call you have today to its SDK equivalent and shows the minimum code change.

## TL;DR — the gap in one sentence

You're listening for `ui/notifications/tool-result` JSON-RPC messages directly via `window.addEventListener('message', ...)` and calling tools through `window.mcpBridge.callTool`. The MCP Apps SDK gives you `app.ontoolresult` and `app.callServerTool` for the same job, plus a connect handshake (`app.connect()`) the host expects before pushing notifications. Without `app.connect()` and the four required handlers, hosts that strictly follow the spec (e.g. Claude Desktop) won't initialize the widget — that's why the validator flags every wiring row as missing.

## What works today (keep it — it migrates straight across)

| Calculator code | Where in `widgets/keypad.html` | What it does |
|---|---|---|
| `window.mcpBridge.callTool(name, args)` | `callTool` helper, line 794 | Calls a server tool and gets the structured result back |
| `window.addEventListener('message', ...)` filtered on `msg.method === 'ui/notifications/tool-result'` | line 589 | Receives results from LLM-decomposed tool calls |
| `applyToolResult(data, /*fromLLM=*/...)` | line 596, 459, 480 etc. | Renders a tool result into the keypad UI |
| `window.mcpBridge.notifyIntrinsicHeight(h)` | line 776 | Tells the host how tall the iframe content is |
| `window.mcpBridge.theme` | line 869 | Reads the host theme (`'dark'` / `'light'`) |
| `window.mcpBridge.setState({...})` / `getState()` | lines 803, 815 | Persists local widget state across tool calls |

The structure is fine. The plumbing underneath needs to swap from `mcpBridge` to `app`. State persistence (`setState` / `getState`) is the only API without a direct SDK equivalent — see the bottom of this doc.

## The minimum migration

### 1. Add an SDK import + the `App` instance

At the top of `widgets/keypad.html`'s `<script>` block (or in a new `<script type="module">` block — see step 6), replace nothing (you're adding) and put:

```html
<script type="module">
  import { App } from "@modelcontextprotocol/ext-apps";

  const app = new App({
    name: "scientific-calculator",
    version: "1.0.0"
  });

  // ... rest of your widget code goes here, using `app.*` for host calls ...
</script>
```

The `name` and `version` strings are what the validator's G2 constructor regex checks for — they must be string literals (or string-concatenations like `"scientific-calculator-" + suffix`).

### 2. Register the four required handlers BEFORE calling `app.connect()`

The MCP Apps spec requires these four handlers — they MUST be set before `connect()` because the host queues notifications and replays them on connect, and unhandled events are an error.

```js
app.onteardown = async () => {
  // Called when the host wants to dispose the widget. Persist any pending
  // state and resolve. Returning {} is fine for a stateless widget.
  saveState();
  return {};
};

app.ontoolinput = (params) => {
  // Called when the LLM is about to invoke a tool but hasn't sent the
  // result yet. Useful for "tool starting…" UI states. Stub it out if
  // you don't need it — but it MUST exist.
};

app.ontoolcancelled = (params) => {
  // Called if the LLM aborts a tool call. params.reason is a string.
  console.debug("Tool cancelled:", params && params.reason);
};

app.onerror = (err) => {
  // Called when the SDK runtime encounters an error. Surface to the
  // user or log; do not swallow silently.
  console.error("[calculator] App error:", err);
  setError(err && err.message ? err.message : String(err));
};

app.ontoolresult = (result) => {
  // *** This replaces your window.addEventListener('message', ...) handler. ***
  // result.structuredContent is what your existing applyToolResult expects.
  if (result && result.structuredContent) {
    applyToolResult(result.structuredContent, /*fromLLM=*/true);
  }
};
```

Then **delete** the line-589 `window.addEventListener('message', function (event) { ... ui/notifications/tool-result ... })` block — `app.ontoolresult` does the same job with proper SDK lifecycle.

### 3. Replace `window.mcpBridge.callTool` with `app.callServerTool`

The SDK's tool-invocation API takes a single object with `name` and `arguments`:

```js
// Before:
async function callTool(name, args) {
  if (window.mcpBridge && typeof window.mcpBridge.callTool === 'function') {
    return await window.mcpBridge.callTool(name, args);
  }
  throw new Error('MCP bridge not available');
}

// After:
async function callTool(name, args) {
  const out = await app.callServerTool({
    name,
    arguments: args,
  });
  // app.callServerTool returns the full CallToolResult; your existing
  // applyToolResult expects the structuredContent payload.
  return (out && out.structuredContent) ?? out;
}
```

The rest of your code that invokes `callTool('add', {a, b})` etc. doesn't need to change.

### 4. Call `app.connect()` once, after handlers are registered

Add to the bottom of the `<script type="module">` block, AFTER all `app.on…` assignments and AFTER `init()` has wired up DOM listeners:

```js
init();              // your existing DOMContentLoaded body
app.connect();       // initiate the MCP Apps handshake; do this LAST
```

`app.connect()` (with default `autoResize: true`) sets up a ResizeObserver that automatically sends size-changed notifications — which means **you can delete your `notifyIntrinsicHeight` block** (line 776). The SDK does it for you.

### 5. Read the host theme from `app.getHostContext()`, not `mcpBridge.theme`

```js
// Before:
const theme = (window.mcpBridge && window.mcpBridge.theme) || 'dark';

// After (do this AFTER app.connect() resolves):
app.connect().then(() => {
  const ctx = app.getHostContext();
  const theme = (ctx && ctx.theme) || 'dark';
  document.body.classList.add('theme-' + theme);
  if (theme === 'light') {
    document.body.style.background = '#f5f5f5';
    document.body.style.color = '#222';
  }
});
```

The SDK also gives you a host-context-changed handler if the user toggles theme mid-session:

```js
app.onhostcontextchanged = (partial) => {
  if (partial.theme) {
    document.body.className = 'theme-' + partial.theme;
  }
};
```

### 6. Convert your existing `<script>` block to `<script type="module">`

The `import { App } from ...` line requires module syntax. Change:

```html
<script>
  // ... your code ...
</script>
```

to:

```html
<script type="module">
  // ... your code ...
</script>
```

Inside a module, `let`/`const`/`function` declarations are scoped to the module. If you reference globals from elsewhere on the page (you don't appear to), you'd assign them to `window.*` explicitly.

## What the SDK does NOT provide: `setState` / `getState`

Your current `saveState()` / `loadState()` use `window.mcpBridge.setState({...})` and `window.mcpBridge.getState()`. The MCP Apps SDK does not expose a direct state-persistence API — state in the spec is implicit in the conversation history and the host context.

Three options, in order of recommended:

1. **Drop `saveState`/`loadState` entirely.** Render directly from `app.ontoolresult` calls. The chat transcript IS your history; the widget renders the latest tool result and that's enough. Many MCP App widgets work this way.

2. **Use `localStorage` keyed by an iframe-survival mechanism.** Inside the SDK's runtime, the iframe is recreated on each chat turn, so `sessionStorage` won't survive. `localStorage` survives but is shared across all instances of the widget served from the same origin — fine for "the user's last theme preference" but wrong for "the user's last calculator state in THIS conversation." Use only for global preferences.

3. **Keep your `mcpBridge.setState`/`getState` calls as best-effort fallbacks** alongside the new SDK code. Test for `window.mcpBridge` defensively (you already do). When the host injects `mcpBridge` (e.g. `cargo pmcp preview`), state persists; in hosts that don't inject it (Claude Desktop), the calls silently no-op. This is what your current code already does.

We recommend option 1: keep the calculator stateless across tool calls, render purely from incoming `ontoolresult` events. The keypad UI's local token list (`state.tokens`) doesn't need to survive a host page reload — the chat transcript is the source of truth.

## Full minimal post-migration shape

After all six steps, the bottom of `widgets/keypad.html` looks roughly like:

```html
<script type="application/json" id="initial-data">{"version":"1.0.0"}</script>
<script type="module">
  import { App } from "@modelcontextprotocol/ext-apps";

  const app = new App({
    name: "scientific-calculator",
    version: "1.0.0"
  });

  // ... your existing DOM setup, event listeners, render(), pressDigit(),
  //     pressOp(), pressEquals(), pressNegate(), pressSqrt(), pressSquare(),
  //     pressLog10(), pressLn(), and the local state object ...

  // Tool invocation goes through the SDK now:
  async function callTool(name, args) {
    const out = await app.callServerTool({ name, arguments: args });
    return (out && out.structuredContent) ?? out;
  }

  // Required handlers (must be set before connect):
  app.onteardown = async () => ({});
  app.ontoolinput = () => {};
  app.ontoolcancelled = (p) => console.debug("Tool cancelled:", p && p.reason);
  app.onerror = (err) => {
    console.error("[calculator] App error:", err);
    setError(err && err.message ? err.message : String(err));
  };
  app.ontoolresult = (result) => {
    if (result && result.structuredContent) {
      applyToolResult(result.structuredContent, /*fromLLM=*/true);
    }
  };

  // Wire up DOM, then connect:
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
      init();
      app.connect();
    });
  } else {
    init();
    app.connect();
  }
</script>
```

## Verifying the migration

After deploying the updated widget, re-run the validator:

```sh
cargo pmcp test apps --mode claude-desktop \
  https://scientific-calculator-mcp-app.us-east.true-mcp.com/mcp
```

Expected post-migration output:

```
Total Tests: 104
Passed: 104
Failed: 0
Warnings: 0
Overall Status: PASSED
```

(The 8 G1 false-positive PASSEDs you currently see — `MCP Apps SDK wiring` — will become real PASSEDs once the SDK is actually loaded. The 48 Failed rows for App constructor + handlers + connect become PASSED. The 8 `ontoolresult` warnings disappear because `app.ontoolresult` is now registered explicitly.)

For local pre-deploy verification without hitting the live endpoint, you can scan the raw HTML directly:

```sh
cargo pmcp test apps --mode claude-desktop \
  --widgets-dir widgets/ \
  http://informational
```

## Why each row failed today (for completeness)

| Validator row | Current calculator behavior | Post-migration behavior |
|---|---|---|
| `MCP Apps SDK wiring` (PASSED) | False positive — heuristic fired on the literal string `"ui/notifications/tool-result"` in your `addEventListener` filter | True positive — SDK is genuinely imported |
| `App constructor` (FAILED) | Real — no `new App({name, version})` exists | PASSED — new App constructor on line ~3 of the migrated module |
| `handler: onteardown` (FAILED) | Real — no `.onteardown` assignment | PASSED — `app.onteardown = async () => ({})` |
| `handler: ontoolinput` (FAILED) | Real | PASSED — `app.ontoolinput = () => {}` |
| `handler: ontoolcancelled` (FAILED) | Real | PASSED |
| `handler: onerror` (FAILED) | Real | PASSED |
| `handler: ontoolresult` (WARN, soft) | Soft fail — you handle tool-results via raw `addEventListener`, not `app.ontoolresult` | PASSED |
| `connect() call` (FAILED) | Real — no `app.connect()` call | PASSED |

## Reference: canonical minimal widget

The cycle-2 cycle-1 fixture `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html` in the `paiml/rust-mcp-sdk` repo is the smallest valid MCP Apps widget. It's 24 lines total. Copy it as a sanity check that a stripped-down keypad widget passes the validator before adding back your calculator-specific UI logic.

## Help anchors in the official guide

- [MCP Apps SDK GUIDE — handlers before connect](https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#handlers-before-connect)
- [Vite singlefile minification considerations](https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#vite-singlefile)
- [Common failures running widgets in Claude Desktop](https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#common-failures-claude)

## Summary

- **Add 1 import**, **1 constructor**, **5 handler assignments**, **1 `app.connect()` call**.
- **Delete 1 message-listener** (the line-589 `window.addEventListener('message', ...)` block).
- **Update 1 helper** (`callTool` swaps `mcpBridge.callTool` for `app.callServerTool`).
- **Replace 1 line** (theme detection moves from `mcpBridge.theme` to `app.getHostContext().theme`).
- **Remove 1 line** (`mcpBridge.notifyIntrinsicHeight` becomes automatic via `autoResize: true`).
- **Optionally drop** `saveState`/`loadState` (the spec doesn't have a direct equivalent).

Total diff: ~30 lines added, ~15 lines removed. Time to migrate: ~30 minutes including a Claude Desktop test session. The validator goes from 48 Failed → 0 Failed.
