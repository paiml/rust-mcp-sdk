# Phase 44: Improving mcp-preview to Support ChatGPT Version - Research

**Researched:** 2026-03-08
**Domain:** mcp-preview local development tool -- ChatGPT protocol emulation + protocol diagnostics
**Confidence:** HIGH

## Summary

This phase adds a `--mode chatgpt` flag to the `mcp-preview` crate that enables ChatGPT-specific protocol emulation (postMessage data delivery, `window.openai` stub injection) and a new Protocol diagnostics tab visible in both modes. The codebase is well-structured for these changes: `PreviewConfig` accepts new fields trivially, the `McpProxy` already returns all needed data (`_meta`, `structuredContent`), the `AppBridge` class already sends `ui/notifications/tool-result` via postMessage, and the `index.html` SPA has a clear DevTools tab pattern to extend.

The key technical challenge is bifurcating the widget HTML wrapper (`wrapWidgetHtml`) to inject either the current `mcpBridge` pattern (Standard mode) or a ChatGPT-compatible `window.openai` stub + raw postMessage delivery (ChatGPT mode). The protocol validation logic can live entirely in the browser-side JavaScript since the API handlers already return `_meta` and `structuredContent` data -- no Rust-side validation middleware is needed.

**Primary recommendation:** Implement in two plans -- (1) Rust-side mode plumbing + API changes (PreviewConfig, ConfigResponse, CLI flag, ResourceInfo `_meta`, banner) and (2) Browser-side Protocol tab + ChatGPT mode emulation in index.html.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- CLI flag: `--mode chatgpt` or `--mode standard` (default: standard)
- Mode is fixed for the session -- no runtime switching
- Mode badge displayed in browser UI header
- Terminal startup banner shows mode prominently
- New "Protocol" tab in DevTools area alongside existing panels
- Protocol tab always available in both modes -- informational in Standard, error-highlighting in ChatGPT mode
- Validates on connect (tools/list, resources/list) and after each tool call (tools/call, resources/read)
- Shows pass/fail per check with expandable details
- Specific checks: 4 descriptor keys on tools/list, resources/list, resources/read _meta; mimeType text/html+skybridge; 2 invocation keys on tools/call; structuredContent present
- Key diff on failure showing extra/missing keys
- ChatGPT mode: full postMessage emulation with JSON-RPC ui/notifications/tool-result
- ChatGPT mode: inject window.openai stub (toolOutput, toolInput, theme, callTool())
- Standard mode: continue using existing mcpBridge pattern
- Warn only -- never block tool execution or widget rendering
- Red warnings in Protocol tab for violations

### Claude's Discretion
- How to inject window.openai stub (script injection vs postMessage-based setup)
- Protocol tab UI layout and styling details
- How to intercept proxy responses for validation (middleware vs handler-level)
- Whether to cache validation results or recompute per-view

### Deferred Ideas (OUT OF SCOPE)
None
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.8 | HTTP server framework | Already used by mcp-preview |
| serde / serde_json | latest | Serialization | Already used throughout |
| clap | 4.x | CLI argument parsing | Already used by cargo-pmcp |
| reqwest | latest | HTTP client for MCP proxy | Already used by McpProxy |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tower-http | latest | CORS middleware | Already in use |
| tokio | latest | Async runtime | Already in use |

No new dependencies required. All changes use existing stack.

## Architecture Patterns

### Current Architecture (Relevant to Changes)

```
cargo-pmcp CLI (main.rs)
  └── Commands::Preview { url, port, ... }
        └── preview::execute()
              └── mcp_preview::PreviewServer::start(config)
                    ├── PreviewConfig (server.rs)
                    ├── AppState { config, proxy, wasm_builder }
                    ├── McpProxy (proxy.rs) -- JSON-RPC to MCP server
                    ├── handlers/api.rs -- /api/tools, /api/resources, etc.
                    └── assets/index.html -- SPA with DevTools

Browser SPA (index.html)
  ├── PreviewRuntime class -- tool calls, widget loading, DevTools
  ├── AppBridge (from widget-runtime.mjs) -- host-side bridge
  └── Widget iframe -- srcdoc with wrapWidgetHtml()
```

### Pattern 1: Mode Plumbing (Rust side)

**What:** Thread `mode` from CLI through PreviewConfig -> AppState -> ConfigResponse -> browser
**When to use:** For all mode-aware behavior

The existing pattern is:
1. CLI arg parsed in `main.rs` Commands::Preview enum
2. Passed to `preview::execute()` function
3. Set on `PreviewConfig` struct
4. Shared via `Arc<AppState>`
5. Exposed to browser via `GET /api/config` -> `ConfigResponse`

Add `mode: PreviewMode` following this exact chain. Use a Rust enum:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewMode {
    Standard,
    ChatGpt,
}
```

For clap, use `#[arg(long, default_value = "standard")]` with a string, then validate/convert in execute().

### Pattern 2: Protocol Validation (Browser side)

**What:** Validate _meta keys, mimeType, structuredContent after each proxy response
**When to use:** In the Protocol tab, after tools/list, resources/list, tools/call, resources/read

Validation runs entirely in JavaScript. The API handlers already return `_meta` on ToolInfo and ToolCallResult. ResourceInfo needs `_meta` added (currently missing from proxy.rs struct).

Expected descriptor keys (from `CHATGPT_DESCRIPTOR_KEYS`):
```javascript
const DESCRIPTOR_KEYS = [
  'openai/outputTemplate',
  'openai/toolInvocation/invoking',
  'openai/toolInvocation/invoked',
  'openai/widgetAccessible'
];
const INVOCATION_KEYS = [
  'openai/toolInvocation/invoking',
  'openai/toolInvocation/invoked'
];
```

### Pattern 3: ChatGPT postMessage Emulation

**What:** In ChatGPT mode, after tool call returns structuredContent, send postMessage to widget iframe
**When to use:** Only in ChatGPT mode, replaces the AppBridge sendToolResult path

The current flow in Standard mode:
1. Tool call returns -> handleToolResponse()
2. Finds widgetUri in _meta
3. loadResourceWidget(widgetUri) loads HTML into iframe via srcdoc
4. After 300ms timeout, appBridge.sendToolResult({ structuredContent, _meta })

For ChatGPT mode, replace step 4 with:
```javascript
iframe.contentWindow.postMessage({
  jsonrpc: "2.0",
  method: "ui/notifications/tool-result",
  params: { structuredContent: result.structuredContent }
}, "*");
```

### Pattern 4: window.openai Stub Injection

**What:** In ChatGPT mode, inject a window.openai object into the widget iframe
**When to use:** In wrapWidgetHtml() when mode is chatgpt

**Recommendation (Claude's discretion):** Use script injection in wrapWidgetHtml(). The stub should be injected as an inline script BEFORE the widget-runtime.mjs import so `_detectHost()` finds `window.openai` and identifies the host as "chatgpt".

```javascript
window.openai = {
  toolOutput: null,
  toolInput: null,
  theme: '${theme}',
  callTool: async (name, args) => {
    // Delegate to parent's toolCallHandler
    return window.parent.previewRuntime.createToolCallHandler()(name, args);
  }
};
```

Then update `toolOutput` via postMessage when tool results arrive.

### Anti-Patterns to Avoid
- **Rust-side validation middleware:** The validation is UI-centric (showing pass/fail in a tab). Do NOT add Axum middleware that validates responses. Keep it in JavaScript.
- **Mode switching at runtime:** The user decided mode is session-fixed. Do NOT add a mode toggle button in the UI.
- **Blocking on validation failure:** Validation is warn-only. Never prevent widget rendering or tool execution.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Descriptor key list | Hardcoded JS array | Mirror from CHATGPT_DESCRIPTOR_KEYS constant | Single source of truth; pass via /api/config |
| postMessage delivery | Custom protocol | Standard JSON-RPC 2.0 envelope in postMessage | Matches real ChatGPT behavior exactly |
| Widget HTML wrapping | Separate template files | Extend existing wrapWidgetHtml() with mode branch | Maintains single code path for iframe setup |

**Key insight:** The AppBridge already sends `ui/notifications/tool-result` via postMessage. In ChatGPT mode, the difference is: (a) `window.openai` is present so host detection yields "chatgpt", and (b) data is also pushed via the openai-style postMessage pattern directly (not just through AppBridge).

## Common Pitfalls

### Pitfall 1: ResourceInfo Missing _meta Field
**What goes wrong:** The proxy's `ResourceInfo` struct lacks `_meta`, so resources/list responses drop metadata. Protocol validation can't check resource descriptor keys.
**Why it happens:** ResourceInfo was defined before _meta propagation was added to the SDK.
**How to avoid:** Add `#[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)] pub meta: Option<Value>` to ResourceInfo. Also add it to ResourceContentItem for resources/read.
**Warning signs:** Protocol tab shows "missing" for all resource _meta checks.

### Pitfall 2: srcdoc Iframe Origin is "null"
**What goes wrong:** postMessage origin checks fail because srcdoc iframes have origin "null" (string).
**Why it happens:** Browser security model for srcdoc iframes.
**How to avoid:** Continue using `'*'` as the target origin for postMessage (already done in existing code).
**Warning signs:** Widget doesn't receive postMessage data.

### Pitfall 3: Race Condition Between iframe Load and postMessage
**What goes wrong:** postMessage sent before widget script has initialized its listener.
**Why it happens:** srcdoc parsing and script execution are async.
**How to avoid:** Keep the existing 300ms setTimeout pattern, or better, wait for a "ready" message from the widget before sending data. The AppBridge already handles this via ui/initialize handshake.
**Warning signs:** Widget shows empty state on first load but works on subsequent tool calls.

### Pitfall 4: window.openai Stub Must Exist Before Widget Runtime Initializes
**What goes wrong:** WidgetRuntime._detectHost() runs on DOMContentLoaded and checks window.openai. If the stub is injected after, host detection returns "mcp-apps" instead of "chatgpt".
**Why it happens:** Script execution order in the iframe.
**How to avoid:** In wrapWidgetHtml(), inject the window.openai stub script BEFORE the dynamic import of widget-runtime.mjs.
**Warning signs:** Host info shows "MCP Apps Host" instead of "ChatGPT" in chatgpt mode.

### Pitfall 5: ConfigResponse Must Include Mode for Browser-Side Logic
**What goes wrong:** Browser code has no way to know which mode is active.
**Why it happens:** ConfigResponse doesn't include mode field.
**How to avoid:** Add `mode: String` to ConfigResponse, populate from PreviewConfig.
**Warning signs:** Protocol tab and mode badge can't render correctly.

### Pitfall 6: Descriptor Keys Must Be Passed to Browser, Not Hardcoded
**What goes wrong:** If CHATGPT_DESCRIPTOR_KEYS changes in the SDK, hardcoded JS arrays become stale.
**Why it happens:** Duplicated source of truth.
**How to avoid:** Serve descriptor keys via /api/config so the Protocol tab uses the Rust-defined constant as source of truth. Import and re-export from the pmcp crate.
**Warning signs:** Validation passes in mcp-preview but fails in real ChatGPT.

## Code Examples

### Adding mode to PreviewConfig (server.rs)
```rust
// Source: Existing pattern in server.rs
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewMode {
    Standard,
    ChatGpt,
}

impl Default for PreviewMode {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Debug, Clone)]
pub struct PreviewConfig {
    pub mcp_url: String,
    pub port: u16,
    pub initial_tool: Option<String>,
    pub theme: String,
    pub locale: String,
    pub widgets_dir: Option<PathBuf>,
    pub mode: PreviewMode,  // NEW
}
```

### Adding mode to CLI (main.rs)
```rust
// Source: Existing pattern in main.rs Commands::Preview
Preview {
    // ... existing fields ...

    /// Preview mode: standard (default) or chatgpt (strict validation)
    #[arg(long, default_value = "standard")]
    mode: String,
},
```

### Adding _meta to ResourceInfo (proxy.rs)
```rust
// Source: Existing ToolInfo pattern in proxy.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    pub uri: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    pub meta: Option<Value>,  // NEW
}
```

### Extending ConfigResponse (handlers/api.rs)
```rust
// Source: Existing ConfigResponse pattern
#[derive(Serialize)]
pub struct ConfigResponse {
    pub mcp_url: String,
    pub theme: String,
    pub locale: String,
    pub initial_tool: Option<String>,
    pub mode: String,  // NEW: "standard" or "chatgpt"
    pub descriptor_keys: Vec<String>,  // NEW: from CHATGPT_DESCRIPTOR_KEYS
    pub invocation_keys: Vec<String>,  // NEW: subset for tools/call
}
```

### ChatGPT Mode Banner (server.rs)
```rust
// In startup banner, after existing lines:
println!(
    "\x1b[1;36m||\x1b[0m  Mode:       \x1b[1;31m{:<30}\x1b[0m   \x1b[1;36m||\x1b[0m",
    match config.mode {
        PreviewMode::ChatGpt => "ChatGPT Strict",
        PreviewMode::Standard => "Standard",
    }
);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Only mcpBridge pattern | AppBridge with postMessage transport | Phase 41 | Already supports postMessage internally |
| No _meta on ResourceInfo | _meta propagated in SDK (Phase 43) | 2026-03-08 | mcp-preview proxy struct must catch up |
| No protocol validation | Manual ChatGPT trial-and-error | Pre-Phase 44 | This phase adds automated validation |

## Open Questions

1. **Should descriptor keys be fetched from the pmcp crate or hardcoded in mcp-preview?**
   - What we know: CHATGPT_DESCRIPTOR_KEYS is defined in `pmcp::types::ui`. mcp-preview already depends on pmcp indirectly via cargo-pmcp.
   - What's unclear: Whether mcp-preview has a direct dependency on the pmcp crate.
   - Recommendation: Check Cargo.toml. If mcp-preview depends on pmcp, import directly. Otherwise, add a lightweight dependency or pass via /api/config from cargo-pmcp (which does depend on pmcp). Worst case, duplicate the 4-string constant with a code comment referencing the source.

2. **Should ChatGPT mode skip AppBridge entirely or layer on top?**
   - What we know: AppBridge already sends ui/notifications/tool-result via postMessage. ChatGPT's real behavior also uses postMessage with the same JSON-RPC envelope.
   - What's unclear: Whether widgets rely on window.openai properties (toolOutput) vs postMessage events.
   - Recommendation: Keep AppBridge active even in ChatGPT mode (it handles ui/initialize handshake). Additionally inject window.openai stub and send the ChatGPT-style postMessage. This matches "defense-in-depth" pattern from PMCP_SDK_FIXES.md.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p mcp-preview` |
| Full suite command | `make tests` |

### Phase Requirements -> Test Map

No formal requirement IDs specified for this phase. Key behaviors to validate:

| Behavior | Test Type | Automated Command | Notes |
|----------|-----------|-------------------|-------|
| PreviewMode enum serializes correctly | unit | `cargo test -p mcp-preview` | Rust unit test |
| ResourceInfo deserializes _meta | unit | `cargo test -p mcp-preview` | Rust unit test |
| ConfigResponse includes mode and keys | unit | `cargo test -p mcp-preview` | Rust unit test |
| CLI --mode flag accepts standard/chatgpt | manual | `cargo pmcp preview --mode chatgpt --help` | CLI validation |
| Protocol tab renders checks | manual | Browser visual inspection | UI-only |
| ChatGPT postMessage delivery works | manual | Browser + widget test | Requires running server |
| window.openai stub present in chatgpt mode | manual | Browser console check | UI-only |

### Wave 0 Gaps
- [ ] Unit tests for PreviewMode enum and ConfigResponse -- covers mode plumbing
- [ ] Unit tests for ResourceInfo _meta deserialization -- covers proxy struct changes

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `crates/mcp-preview/src/` -- all source files read directly
- Codebase inspection: `cargo-pmcp/src/commands/preview.rs` and `main.rs` -- CLI structure
- Codebase inspection: `src/types/ui.rs` -- CHATGPT_DESCRIPTOR_KEYS constant
- Reference doc: `pmcp-run/built-in/WIDGET_DEVELOPMENT_GUIDE.md` lines 305-329
- Reference doc: `pmcp-run/built-in/PMCP_SDK_FIXES.md` -- testing checklist and enhancement requests

### Secondary (MEDIUM confidence)
- AppBridge postMessage protocol -- inferred from widget-runtime.mjs source code

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, extending existing patterns
- Architecture: HIGH -- clear extension points identified in every file
- Pitfalls: HIGH -- based on direct code analysis and referenced ChatGPT integration experience

**Research date:** 2026-03-08
**Valid until:** 2026-04-08 (stable domain, internal tooling)
