# Phase 44: Improving mcp-preview to support ChatGPT version - Context

**Gathered:** 2026-03-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Add a `--mode chatgpt` flag to mcp-preview that enables strict ChatGPT protocol validation, full postMessage emulation, and a Protocol diagnostics tab. Standard mode (default) preserves current behavior. Both modes benefit from the new Protocol tab.

Reference docs from Open Images dev team:
- `/Users/guy/Development/mcp/sdk/pmcp-run/built-in/WIDGET_DEVELOPMENT_GUIDE.md` (lines 305-323: mcp-preview requirements)
- `/Users/guy/Development/mcp/sdk/pmcp-run/built-in/PMCP_SDK_FIXES.md` (testing checklist)

</domain>

<decisions>
## Implementation Decisions

### Mode switching
- CLI flag: `--mode chatgpt` or `--mode standard` (default: standard)
- Mode is fixed for the session — no runtime switching
- Mode badge displayed in browser UI header so developer always knows which mode is active
- Terminal startup banner shows mode prominently (e.g., "MCP Apps Preview — ChatGPT Strict")

### Protocol diagnostics panel
- New "Protocol" tab in the DevTools area alongside existing panels
- Always available in both modes — informational in Standard, error-highlighting in ChatGPT mode
- Validates on connect (tools/list, resources/list) and after each tool call (tools/call, resources/read)
- Shows pass/fail per check with expandable details showing actual keys vs expected keys
- Checks performed:
  - tools/list `_meta`: exactly 4 descriptor keys per widget tool
  - resources/list `_meta`: exactly 4 descriptor keys per widget resource
  - resources/read `_meta`: exactly 4 descriptor keys
  - resources/read `mimeType`: must be `text/html+skybridge`
  - tools/call `_meta`: exactly 2 invocation keys
  - tools/call: `structuredContent` must be present
- Key diff on failure: "Extra keys: openai/widgetCSP, openai/widgetPrefersBorder" or "Missing keys: openai/widgetAccessible"

### ChatGPT postMessage emulation
- In ChatGPT mode, fully emulate ChatGPT's data delivery:
  - Send `postMessage` with `{ jsonrpc: "2.0", method: "ui/notifications/tool-result", params: { structuredContent: ... } }` to widget iframe after tool call
  - Inject `window.openai` stub into iframe with: `toolOutput`, `toolInput`, `theme`, `callTool()`
- In Standard mode, continue using existing mcpBridge pattern

### Validation strictness
- Warn only — never block tool execution or widget rendering
- Red warnings in Protocol tab for violations
- Developers can iterate on fixes while still seeing their widget render

### Claude's Discretion
- How to inject window.openai stub (script injection vs postMessage-based setup)
- Protocol tab UI layout and styling details
- How to intercept proxy responses for validation (middleware vs handler-level)
- Whether to cache validation results or recompute per-view

</decisions>

<specifics>
## Specific Ideas

- "The biggest time sink was discovering that ChatGPT uses postMessage for data delivery, not window.openai.toolOutput. mcp-preview should replicate this exactly so devs can test locally."
- "A protocol validator mode that checks all 4 MCP methods and flags mismatches (wrong key count, wrong MIME type, missing structuredContent) would have saved hours of ChatGPT trial-and-error."
- "The 4-key strictness was completely undocumented by OpenAI — we only discovered it by diffing against the Pizzaz reference server. mcp-preview should enforce this as a lint."
- The 4 descriptor keys are defined in `CHATGPT_DESCRIPTOR_KEYS` constant in `src/types/ui.rs` — reuse this as the source of truth for validation

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `PreviewConfig` struct in `crates/mcp-preview/src/server.rs` — add `mode: PreviewMode` field
- `McpProxy` in `crates/mcp-preview/src/proxy.rs` — already has `list_tools()`, `call_tool()`, `list_resources()`, `read_resource()` — responses can be validated post-fetch
- `ResourceInfo` and `ToolInfo` structs in proxy.rs — need `_meta` field on ResourceInfo (currently missing)
- `ToolCallResult` in proxy.rs — already has `meta` and `structured_content` fields
- `CHATGPT_DESCRIPTOR_KEYS` in `src/types/ui.rs` — the canonical 4-key whitelist
- `index.html` in `crates/mcp-preview/assets/` — main SPA, needs Protocol tab addition
- `widget-runtime.mjs` — existing bridge runtime, needs ChatGPT mode variant

### Established Patterns
- Axum router with `Arc<AppState>` shared state
- Proxy pattern: browser → preview server API → MCP server
- Handler pattern in `handlers/api.rs`: fetch from proxy, transform, return JSON
- Asset serving via `include_str!` for index.html

### Integration Points
- `PreviewConfig` — add mode field, thread through to AppState
- `handlers/api.rs` — add validation logic after proxy responses
- New `/api/protocol` endpoint or extend existing endpoints with validation data
- `assets/index.html` — add Protocol tab UI and mode badge
- `cargo-pmcp` CLI — add `--mode` flag to preview subcommand

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 44-improving-mcp-preview-to-support-chatgpt-version*
*Context gathered: 2026-03-08*
