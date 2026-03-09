# Phase 34: Fix MCP Apps ChatGPT Compatibility - Context

**Gathered:** 2026-03-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix SDK UI element handling to be compatible with ChatGPT's MCP Apps implementation. Also fix the mcp-preview server panic caused by axum 0.8 wildcard route syntax. Scope: review SDK output against OpenAI's published docs and fix discrepancies.

Reference docs:
- https://developers.openai.com/apps-sdk/build/chatgpt-ui
- https://developers.openai.com/apps-sdk/mcp-apps-in-chatgpt
- https://developers.openai.com/apps-sdk/deploy/testing

</domain>

<decisions>
## Implementation Decisions

### MIME Type Verification
- OpenAI docs show `text/html;profile=mcp-app` for widget resources
- SDK uses `text/html+mcp` (MCP standard) and `text/html+skybridge` (ChatGPT)
- Need to verify which MIME type ChatGPT actually accepts and update accordingly
- May need to add `text/html;profile=mcp-app` as an alias or replacement

### Tool _meta Format Consistency
- **Critical bug**: Two code paths produce different `_meta` formats for the same field:
  - `ToolInfo::with_ui()` (protocol.rs:331): flat key `"ui/resourceUri": "uri"`
  - `TypedTool::metadata()` (typed_tool.rs:232): nested `"ui": { "resourceUri": "uri" }`
- OpenAI docs show nested format `_meta.ui.resourceUri` matching TypedTool
- OpenAI docs also show `_meta["openai/outputTemplate"]` as ChatGPT-specific alias
- **Fix**: Unify both paths to use the same format, and optionally add `openai/outputTemplate`

### Widget Meta in Resource _meta
- OpenAI docs show `_meta: { ui: { prefersBorder: true } }` (nested under `ui` key)
- SDK currently uses `openai/widgetPrefersBorder` (flat OpenAI-namespaced key)
- Need to verify which format ChatGPT accepts; may need both for compatibility

### mcp-preview Panic Fix
- Axum 0.8 requires `{*path}` not `*path` for wildcard routes
- Lines 106-108 in `crates/mcp-preview/src/server.rs` use old syntax
- Fix the route syntax and bump mcp-preview version

### Already Fixed
- `ToolInfo._meta` serde rename (commit 992aa8d) — `rename_all = "camelCase"` was stripping the underscore, serializing as `meta` instead of `_meta`

### Claude's Discretion
- Whether to add `openai/outputTemplate` alongside `ui.resourceUri` in typed tools
- Exact version bump numbers for affected crates
- Whether to update examples to demonstrate both MCP and ChatGPT patterns

</decisions>

<specifics>
## Specific Ideas

- The pmcp.run dev team is actively building MCP Apps and found these issues
- Primary MCP client supporting MCP Apps today is ChatGPT — compatibility is critical
- OpenAI's testing guidance recommends MCP Inspector (`npx @modelcontextprotocol/inspector@latest`) for debugging
- The three-tier response model (content/structuredContent/_meta) is architecturally sound — issues are in metadata formatting

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `WidgetMeta` struct with builder pattern (mcp_apps.rs:191-271) — needs format adjustment
- `ChatGptToolMeta` struct (mcp_apps.rs:302-370) — has correct `openai/*` key serialization
- `WidgetResponseMeta` struct (mcp_apps.rs:449-500) — response-level metadata
- `WidgetCSP` struct (mcp_apps.rs:60-103) — CSP fields match OpenAI docs (snake_case)
- `ExtendedUIMimeType` enum (mcp_apps.rs:620-700) — MIME type handling
- `UIAdapter` trait (adapter.rs) — per-platform transformation

### Established Patterns
- Serde `rename` attributes for protocol field mapping (e.g., `rename = "openai/widgetPrefersBorder"`)
- Builder pattern for metadata construction (`.prefers_border(true).domain(...)`)
- `to_meta_map()` for converting typed structs to JSON maps
- Three separate adapter implementations: ChatGpt, McpApps, McpUi

### Integration Points
- `ToolInfo::with_ui()` in protocol.rs — tool-level _meta construction
- `TypedTool::metadata()` in typed_tool.rs — typed tool _meta construction
- `CallToolResult` structured_content and _meta fields
- `ReadResourceResult` contents with MIME type
- mcp-preview server routes in crates/mcp-preview/src/server.rs

### Files to Modify
- `src/types/protocol.rs` — ToolInfo::with_ui() _meta format
- `src/types/mcp_apps.rs` — MIME types, WidgetMeta format
- `src/types/ui.rs` — UIMimeType enum
- `src/server/typed_tool.rs` — TypedTool metadata consistency
- `crates/mcp-preview/src/server.rs` — wildcard route fix

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 34-fix-mcp-apps-chatgpt-compatibility*
*Context gathered: 2026-03-06*
