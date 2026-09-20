# Phase 46: MCP Bridge Review and Fixes - Research

**Researched:** 2026-03-10
**Domain:** MCP Apps bridge protocol, cross-host widget communication, postMessage transport
**Confidence:** MEDIUM

## Summary

Phase 46 fixes the data delivery pipeline between MCP hosts and widget iframes so that structuredContent from tool responses reaches widgets across all host environments (Claude Desktop, ChatGPT, mcp-preview). The core problem is a disconnect between what the official ext-apps specification defines as the host-to-widget protocol and what the current PMCP bridge implementations listen for/send.

The official `@modelcontextprotocol/ext-apps` App class (widget-side) listens for notifications with method names `ui/toolResult`, `ui/toolInput`, `ui/toolCancelled`, `ui/hostContextChanged`, and `ui/teardown`. The PMCP `AppBridge` class (host-side) already sends these correctly (`ui/toolResult` via `PostMessageTransport.notify()`). However, the injected bridge scripts in `adapter.rs` (both ChatGptAdapter and McpAppsAdapter) listen for `ui/notifications/tool-result` -- a different method name. The mcp-preview `index.html` also sends `ui/notifications/tool-result` as a supplemental postMessage. This mismatch means widgets using the ext-apps App class never receive host-initiated tool results.

**Primary recommendation:** Align all bridge scripts and host-side notification senders to the ext-apps protocol method names (`ui/toolResult`, `ui/toolInput`). Add host auto-detection to the widget-side bridge. Build a Bridge diagnostics tab in mcp-preview for real-time postMessage inspection.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Claude Desktop loads widget iframes successfully but tool result data never reaches the widget -- bridge handshake or data delivery mechanism is broken
- Root cause is unknown -- research must trace the full host-to-widget data delivery path for Claude Desktop
- Bridge must auto-detect the host environment and adapt its listening/delivery behavior automatically
- Widget developers write host-agnostic code using `mcpBridge.onToolResult(callback)` -- bridge handles all host differences internally
- Treat MCP Apps as a common standard (like HTML) but be ready for vendor variants (like browser wars) -- support the majority of variants to serve the widest developer market
- ext-apps spec and OpenAI examples are BOTH ground truth -- neither takes precedence
- Don't break ChatGPT (existing, well-supported), ADD Claude Desktop/spec support alongside it
- ChatGPT was first to market; MCP spec is converging and cleaning up the early OpenAI implementation
- Claude Desktop represents the "standard" MCP Apps host -- its protocol follows the ext-apps spec
- Hands-on testing required: run reference examples against both Claude Desktop and ChatGPT to map actual protocol differences
- User handles ngrok tunneling and Claude Desktop setup (already configured)
- Ship a reproducible test harness for bridge validation
- New dedicated "Bridge" tab in mcp-preview (separate from existing Protocol tab)
- Bridge tab includes: PostMessage traffic log, Bridge handshake trace, Data flow end-to-end
- Mode remains CLI flag only (--mode standard/chatgpt) -- no live switching in UI
- Bridge tab shows current mode but cannot change it

### Claude's Discretion
- How to implement host auto-detection in the bridge (environment sniffing strategy)
- Internal architecture of the Bridge diagnostics tab (data capture, rendering)
- Test harness implementation details (script vs test suite vs both)
- How to trace Claude Desktop's specific bridge protocol (reverse-engineering approach)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@modelcontextprotocol/ext-apps` | 1.1.2 | Reference MCP Apps protocol implementation | Official spec SDK; ground truth for method names and protocol flow |
| `PostMessageTransport` | (internal) | JSON-RPC 2.0 over postMessage | Already exists in widget-runtime; aligns with ext-apps transport |
| `AppBridge` | (internal) | Host-side bridge for widget iframes | Already exists; needs host auto-detection refactor |
| `App` | (internal) | Widget-side protocol client | Already exists; mirrors ext-apps App class API |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `widget-runtime.mjs` | (internal) | Compiled bridge runtime injected into widget iframes | Used by mcp-preview to bootstrap widget communication |
| Axum | 0.8 | mcp-preview web server | Already used; Bridge tab adds new SSE/websocket endpoint for diagnostics |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom postMessage interception | Chrome DevTools protocol | DevTools requires manual inspection; embedded Bridge tab gives in-app diagnostics |
| Per-host adapter pattern | Single universal bridge | Adapter pattern already established in codebase; extend it |

## Architecture Patterns

### Protocol Method Name Alignment (CRITICAL)

The ext-apps spec and PMCP widget-runtime have TWO naming conventions that must both be supported:

| Spec Method Name | PMCP Internal Name | Who Uses It |
|------------------|--------------------|-------------|
| `ui/notifications/tool-result` | `ui/toolResult` | Spec uses long form in docs; App class handles short form |
| `ui/notifications/tool-input` | `ui/toolInput` | Same pattern |
| `ui/notifications/tool-cancelled` | `ui/toolCancelled` | Same pattern |
| `ui/notifications/host-context-changed` | `ui/hostContextChanged` | Same pattern |
| `ui/notifications/initialized` | `ui/notifications/initialized` | Same in both (notification sent BY widget) |

**Key Finding:** The PMCP `App._handleNotification()` method dispatches on SHORT names (`ui/toolResult`). The `AppBridge.sendToolResult()` sends SHORT names via `PostMessageTransport.notify('ui/toolResult', ...)`. The injected bridge scripts in `adapter.rs` listen for LONG names (`ui/notifications/tool-result`). **This is likely why Claude Desktop delivery fails** -- Claude Desktop likely sends the short form following the ext-apps SDK pattern.

**Recommendation:** The bridge scripts must listen for BOTH forms, or standardize on one. Since the ext-apps SDK App class (widget-side) handles short forms, and AppBridge (host-side) sends short forms, standardize on short forms and add fallback listeners for long forms.

### Host Auto-Detection Strategy

```
Host Detection Priority:
1. window.openai exists AND has toolOutput/callTool → ChatGPT
2. window.parent !== window (in iframe) → MCP Apps host
3. document.referrer contains known host domains → classify
4. Default → Generic MCP Apps (standard protocol)
```

**Environment signals by host:**

| Host | Detection Signal | Data Delivery Method |
|------|-----------------|---------------------|
| ChatGPT | `window.openai` present | `window.openai.toolOutput` property + `openai:set_globals` event |
| Claude Desktop | No `window.openai`; in iframe; ext-apps protocol | `ui/toolResult` notification via postMessage |
| mcp-preview (standard) | No `window.openai`; in srcdoc iframe | `ui/toolResult` notification + `ui/notifications/tool-result` fallback |
| mcp-preview (chatgpt) | Injected `window.openai` stub | Both ChatGPT path and postMessage |

### Data Flow: Tool Call to Widget Render

```
Standard Mode (Claude Desktop / ext-apps):
1. User triggers tool call → host calls MCP server
2. Server returns { content: [...], structuredContent: {...} }
3. Host fetches ui:// resource → loads HTML in sandboxed iframe
4. Widget sends ui/initialize REQUEST → host responds with hostContext
5. Widget sends ui/notifications/initialized NOTIFICATION
6. Host sends ui/toolResult NOTIFICATION with { structuredContent, content }
7. Widget's app.ontoolresult callback receives data
8. Widget renders UI from structuredContent

ChatGPT Mode:
1. Same steps 1-2
3. Host sets window.openai.toolOutput directly on iframe window
4. Host fires openai:set_globals CustomEvent
5. Widget reads window.openai.toolOutput or listens for set_globals
6. No postMessage-based tool result delivery (ChatGPT uses window properties)
```

### Recommended Bridge Diagnostics Architecture

```
mcp-preview
├── index.html
│   ├── Bridge Tab (NEW)
│   │   ├── PostMessage Traffic Log
│   │   │   └── Captured via MessageEvent listener on window
│   │   ├── Handshake Trace
│   │   │   └── Step-by-step: load → init request → init response → initialized notification
│   │   └── Data Flow Visualization
│   │       └── Full path: tool call → server response → widget delivery → render
│   ├── Protocol Tab (existing)
│   └── Widget iframe
└── assets/
    └── widget-runtime.mjs (enhanced with diagnostic hooks)
```

**PostMessage capture approach:** Install a `window.addEventListener('message', ...)` on the HOST window (mcp-preview) that logs ALL messages in both directions. Also hook `PostMessageTransport.notify()` and `PostMessageTransport.send()` to capture outgoing messages. Display in a scrollable log with:
- Timestamp
- Direction (host->widget or widget->host)
- Method name
- Payload (truncated, expandable)
- Origin

### Anti-Patterns to Avoid
- **Dual naming without mapping:** Don't have two different method name conventions without a translation layer. Pick one canonical form and map the other.
- **Mode-conditional bridge scripts:** The injected bridge in `adapter.rs` should NOT have separate code paths for ChatGPT vs standard. Host-side adapters handle differences; widget-side bridge should be universal.
- **setTimeout-based delivery:** The current 300ms setTimeout for tool result delivery after widget load is fragile. Use the `ui/notifications/initialized` notification from the widget as the signal that it's ready to receive data.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON-RPC 2.0 transport | Custom postMessage parser | Existing `PostMessageTransport` class | Already handles correlation IDs, timeouts, origin validation |
| Protocol method dispatch | Switch statements in inline scripts | `App._handleNotification()` pattern | Centralized, testable, matches ext-apps API |
| Host detection | Inline `if (window.openai)` checks | `detectHost()` utility from widget-runtime | Already exported; needs enhancement for Claude Desktop |
| Widget readiness signaling | setTimeout delays | `ui/notifications/initialized` notification | Spec-defined signal that widget is ready for data |

**Key insight:** The ext-apps `App` class and `AppBridge` class already implement the correct protocol. The problem is that the INJECTED bridge scripts in `adapter.rs` implement a DIFFERENT protocol (different method names, different data delivery). The fix is to make the injected scripts use the shared widget-runtime library (App class) OR align the injected scripts to match the same protocol.

## Common Pitfalls

### Pitfall 1: Method Name Mismatch Between Spec Versions
**What goes wrong:** Different hosts use different notification method names. Claude Desktop uses ext-apps spec names; ChatGPT uses its own mechanism entirely.
**Why it happens:** The ext-apps spec evolved. Early versions used `ui/notifications/tool-result` (long form); the SDK implementation uses `ui/toolResult` (short form). Both may exist in the wild.
**How to avoid:** Listen for BOTH forms in widget-side bridges. Map long form to short form in a normalization layer.
**Warning signs:** Widget loads but shows no data; postMessage traffic shows notifications being sent but never handled.

### Pitfall 2: srcdoc iframe Origin
**What goes wrong:** `postMessage` origin validation fails because srcdoc iframes have origin `"null"` (the string, not null).
**Why it happens:** srcdoc iframes are same-document and don't inherit the parent's origin.
**How to avoid:** Already handled -- use `targetOrigin: '*'` for srcdoc iframes. The App class has `_resolveTargetOrigin()` that handles this.
**Warning signs:** No messages received despite being sent; no errors in console (silently dropped by browser).

### Pitfall 3: Widget Not Ready When Tool Result Arrives
**What goes wrong:** Host sends tool result before widget's bridge script has initialized its listener.
**Why it happens:** iframe srcdoc load is async; host sends data immediately after setting srcdoc.
**How to avoid:** Wait for `ui/notifications/initialized` from the widget before sending tool results. The current 300ms setTimeout is a fragile workaround.
**Warning signs:** First tool call shows blank widget; subsequent calls work fine.

### Pitfall 4: Breaking ChatGPT While Fixing Standard Mode
**What goes wrong:** Refactoring bridge scripts to fix Claude Desktop breaks ChatGPT's `window.openai` integration.
**Why it happens:** ChatGPT uses a completely different data delivery mechanism (window properties + custom events) that bypasses postMessage.
**How to avoid:** Keep ChatGPT's `window.openai` path intact. Add standard postMessage path alongside it, not replacing it.
**Warning signs:** ChatGPT widgets show "undefined" for tool output after bridge refactor.

### Pitfall 5: AppBridge initialization timing with srcdoc
**What goes wrong:** AppBridge tries to add listener via iframe.contentWindow but contentWindow changes when srcdoc is set.
**Why it happens:** Setting iframe.srcdoc causes a navigation that can replace the contentWindow object.
**How to avoid:** The current code initializes AppBridge BEFORE setting srcdoc, which works because PostMessageTransport listens on `window` (the host window), not on `iframe.contentWindow`. The transport sends TO `iframe.contentWindow` -- this reference updates automatically.
**Warning signs:** AppBridge.initialize() called but no messages received from widget.

## Code Examples

### Aligned notification handler (widget-side)

```typescript
// Source: packages/widget-runtime/src/app.ts (existing pattern)
private _handleNotification(method: string, params?: Record<string, unknown>): void {
  // Normalize long-form spec names to short form
  const normalized = method
    .replace('ui/notifications/tool-result', 'ui/toolResult')
    .replace('ui/notifications/tool-input', 'ui/toolInput')
    .replace('ui/notifications/tool-input-partial', 'ui/toolInputPartial')
    .replace('ui/notifications/tool-cancelled', 'ui/toolCancelled')
    .replace('ui/notifications/host-context-changed', 'ui/hostContextChanged');

  switch (normalized) {
    case 'ui/toolInput':
      if (this.ontoolinput && params) this.ontoolinput(params);
      break;
    case 'ui/toolResult':
      if (this.ontoolresult && params) this.ontoolresult(params as unknown as CallToolResult);
      break;
    // ... other cases
  }
}
```

### Host auto-detection utility

```typescript
// Source: packages/widget-runtime/src/utils.ts (enhance existing detectHost)
export function detectHost(): HostType {
  if (typeof window === 'undefined') return 'unknown';

  // ChatGPT: window.openai is the primary signal
  if (window.openai && (window.openai.callTool || window.openai.toolOutput !== undefined)) {
    return 'chatgpt';
  }

  // In an iframe (widget context)
  if (window.parent !== window) {
    // MCP-UI: has sendIntent capability
    if (window.mcpBridge?.sendIntent) return 'mcp-ui';
    // Default: standard MCP Apps (Claude Desktop, basic-host, mcp-preview)
    return 'mcp-apps';
  }

  return 'generic';
}
```

### Bridge diagnostics capture (host-side)

```javascript
// Source: new code for mcp-preview Bridge tab
class BridgeDiagnostics {
  constructor() {
    this.log = [];
    this._originalPostMessage = null;
  }

  startCapture(iframe) {
    // Capture incoming messages from widget
    window.addEventListener('message', (event) => {
      if (event.source === iframe.contentWindow) {
        this.log.push({
          timestamp: Date.now(),
          direction: 'widget->host',
          data: event.data,
          origin: event.origin,
        });
        this.render();
      }
    });

    // Intercept outgoing messages to widget
    const origPostMessage = iframe.contentWindow.postMessage.bind(iframe.contentWindow);
    iframe.contentWindow.postMessage = (data, origin) => {
      this.log.push({
        timestamp: Date.now(),
        direction: 'host->widget',
        data,
        origin,
      });
      this.render();
      return origPostMessage(data, origin);
    };
  }
}
```

### Correct tool result delivery with readiness signal

```javascript
// Source: mcp-preview index.html deliverToolResult pattern (improved)
// Instead of setTimeout(deliverToolResult, 300):

// Listen for widget readiness
const readyPromise = new Promise((resolve) => {
  const handler = (event) => {
    const msg = event.data;
    if (msg?.jsonrpc === '2.0' && msg.method === 'ui/notifications/initialized') {
      window.removeEventListener('message', handler);
      resolve();
    }
  };
  window.addEventListener('message', handler);
  // Timeout fallback
  setTimeout(resolve, 3000);
});

await this.loadResourceWidget(widgetUri);
await readyPromise;
deliverToolResult();
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| ChatGPT-only bridge (window.openai) | Multi-host bridge with extensions namespace | Phase 45 (2026-03) | Standard methods at root, ChatGPT under extensions.chatgpt |
| Inline bridge scripts per adapter | Shared widget-runtime.mjs library | Phase 41 (2026-03) | Centralized bridge code, installCompat() shim |
| `ui/notifications/tool-result` (long form) | `ui/toolResult` (short form) | ext-apps SDK 1.0+ | SDK uses short form; spec docs reference long form |
| Flat `openai/*` metadata keys only | Nested `ui.resourceUri` + host enrichment | Phase 45 (2026-03) | Standard-only by default; host layers add vendor keys |

**Key evolution note:** The ext-apps spec (2026-01-26) formalized the protocol that ChatGPT pioneered informally. The spec uses `ui/toolResult` as the canonical notification method, with `structuredContent` as the primary data field. The older `ui/notifications/tool-result` form appears in some documentation but the SDK implementation uses the short form.

## Open Questions

1. **What exact postMessage protocol does Claude Desktop send?**
   - What we know: Claude Desktop supports MCP Apps; it follows the ext-apps spec; it loads widget iframes successfully
   - What's unclear: The exact sequence of messages Claude Desktop sends after a tool call. Does it send `ui/toolResult` or `ui/notifications/tool-result`? Does it support `ui/initialize` handshake?
   - Recommendation: Hands-on testing with ngrok tunnel + Claude Desktop + reference ext-apps examples. Log all postMessage traffic.

2. **Does Claude Desktop use srcdoc or a blob: URL for widget HTML?**
   - What we know: mcp-preview uses srcdoc; ChatGPT loads from a remote URL
   - What's unclear: How Claude Desktop serves the widget HTML -- srcdoc, blob:, data:, or remote URL
   - Recommendation: Inspect Claude Desktop's iframe attributes during testing. This affects origin validation and postMessage targeting.

3. **Method name normalization -- should we canonicalize to short or long form?**
   - What we know: ext-apps SDK uses short form (`ui/toolResult`); some spec docs use long form (`ui/notifications/tool-result`)
   - What's unclear: Whether any real host uses long form exclusively
   - Recommendation: Accept both, send short form. This is the safest approach and matches ext-apps SDK behavior.

## Sources

### Primary (HIGH confidence)
- `@modelcontextprotocol/ext-apps` API docs (https://apps.extensions.modelcontextprotocol.io/api/classes/app.App.html) -- App class methods, notification handlers
- MCP Apps Build guide (https://modelcontextprotocol.io/extensions/apps/build) -- protocol flow, `ontoolresult`, `App.connect()` pattern
- ext-apps GitHub repo (https://github.com/modelcontextprotocol/ext-apps) -- SDK structure, AppBridge, PostMessageTransport
- Codebase analysis: `packages/widget-runtime/src/app.ts` -- PMCP App class that mirrors ext-apps
- Codebase analysis: `packages/widget-runtime/src/app-bridge.ts` -- Host-side bridge, sends `ui/toolResult`
- Codebase analysis: `src/server/mcp_apps/adapter.rs` -- Injected bridge scripts, listen for `ui/notifications/tool-result`
- Codebase analysis: `crates/mcp-preview/assets/index.html` -- mcp-preview host, deliverToolResult(), loadWidget()

### Secondary (MEDIUM confidence)
- ext-apps specification draft (https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/draft/apps.mdx) -- Protocol specification, method names
- MCP Apps blog announcement (https://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/) -- Ecosystem status

### Tertiary (LOW confidence)
- Claude Desktop exact postMessage behavior -- needs hands-on testing to confirm

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- well-understood codebase with clear ext-apps reference
- Architecture: MEDIUM -- protocol method name mismatch is a strong hypothesis for the Claude Desktop bug, but needs validation via hands-on testing
- Pitfalls: HIGH -- derived from direct code analysis and known iframe/postMessage behaviors

**Research date:** 2026-03-10
**Valid until:** 2026-04-10 (ext-apps spec is relatively stable at 2026-01-26)
