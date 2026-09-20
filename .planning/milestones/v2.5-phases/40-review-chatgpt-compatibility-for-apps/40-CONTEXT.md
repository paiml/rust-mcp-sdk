# Phase 40: Review ChatGPT Compatibility for Apps - Context

**Gathered:** 2026-03-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Audit the PMCP SDK's MCP Apps output against the official `@modelcontextprotocol/ext-apps` reference implementation (v1.x) and ChatGPT developer docs. Identify all gaps between what our SDK produces and what the official spec expects. Produce a gap analysis and implement fixes for compatibility issues.

Reference sources:
- Official ext-apps SDK: https://github.com/modelcontextprotocol/ext-apps
- MCP Apps build guide: https://modelcontextprotocol.io/extensions/apps/build
- ChatGPT UI docs: https://developers.openai.com/apps-sdk/build/chatgpt-ui
- Apps SDK UI components: https://openai.github.io/apps-sdk-ui/?path=/docs/overview-introduction--docs
- Example repos: https://github.com/openai/openai-apps-sdk-examples

</domain>

<decisions>
## Implementation Decisions

### Legacy Flat Key Dual-Emit
- The official ext-apps `registerAppTool` sets BOTH `_meta.ui.resourceUri` (nested) AND `_meta["ui/resourceUri"]` (legacy flat key) for backward compat
- `RESOURCE_URI_META_KEY = "ui/resourceUri"` is the legacy constant
- Our SDK currently only emits nested format — need to also emit the legacy flat key to match official behavior
- The flat key is deprecated but still emitted by the reference implementation

### MIME Type
- `RESOURCE_MIME_TYPE = "text/html;profile=mcp-app"` in ext-apps — our SDK already has this as `HtmlMcpApp` variant (Phase 34)
- No gap here

### Tool _meta Format
- Official format: `_meta: { ui: { resourceUri: "ui://..." } }` — our SDK matches (Phase 34)
- ChatGPT also reads `openai/outputTemplate` — our SDK emits this (Phase 34)
- Need to verify: does the official SDK also emit `openai/outputTemplate`? Or is that ChatGPT-specific and we're the only ones doing it?

### New ui Fields from ext-apps Spec
- `ui.visibility`: `["model"]`, `["app"]`, or `["model", "app"]` — controls whether tool is visible to AI model, UI app, or both. **Not in our SDK yet.**
- `ui.csp`: `{ resourceDomains: [...], connectDomains: [...] }` — CSP for widget iframe. We have `WidgetCSP` but using flat `openai/*` keys.
- `ui.domain`: stable CORS origin for widget. We have this in `WidgetMeta` as flat `openai/widgetDomain`.
- `ui.prefersBorder`: boolean layout hint. We dual-emit this (Phase 34).

### Capability Negotiation
- Extension ID: `"io.modelcontextprotocol/ui"` used in `extensions` field of client capabilities
- `McpUiClientCapabilities` includes `mimeTypes` array for clients to advertise which MIME types they support
- `getUiCapability()` helper checks if client supports MCP Apps
- Our SDK doesn't have capability negotiation for MCP Apps — servers can't conditionally register app tools based on client support
- This is important for graceful degradation (text-only fallback when client doesn't support apps)

### registerAppTool / registerAppResource Equivalents
- Official SDK provides helper functions that normalize metadata automatically
- Our SDK has `ToolInfo::with_ui()` and typed tool `.with_ui()` builders — similar concept
- Gap: our builders don't set the legacy flat key
- Gap: no resource registration helper that defaults MIME type to `text/html;profile=mcp-app`

### Claude's Discretion
- Exact implementation approach for adding legacy flat key (in `build_meta_map` vs separate step)
- Whether to add `ui.visibility` as a new field on `ToolInfo` or as a separate builder method
- How to structure capability negotiation (new trait method, builder option, or runtime check)
- Priority ordering of gaps (what to fix first)

</decisions>

<specifics>
## Specific Ideas

- The official ext-apps server example uses `registerAppTool` and `registerAppResource` from `@modelcontextprotocol/ext-apps/server` — these are convenience wrappers, not new protocol
- The `App` class (client-side) uses `app.connect()`, `app.ontoolresult`, and `app.callServerTool()` — our bridge library (`packages/widget-runtime/`) should be compared against this API
- The server example in the build guide shows `inputSchema: {}` (empty object) which is valid
- Resource contents use `{ uri, mimeType: RESOURCE_MIME_TYPE, text: html }` format
- The official SDK normalizes in both directions: if you set nested, it also sets flat; if you set flat, it also sets nested

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ToolUIMetadata::build_meta_map()` in `src/types/ui.rs` — builds the nested `_meta` map, needs update to also emit flat key
- `ToolInfo::with_ui()` in `src/types/protocol.rs` — calls `build_meta_map`, will inherit the fix
- `WidgetMeta` in `src/types/mcp_apps.rs` — already has `to_meta_map()` with dual-emit pattern for `prefersBorder`
- `WidgetCSP` in `src/types/mcp_apps.rs` — existing CSP struct, may need mapping to `ui.csp` nested format
- All four typed tool variants have `.with_ui()` builders (Phase 37/39)

### Established Patterns
- Dual-emit pattern: both flat `openai/*` keys and nested `ui` object (Phase 34)
- `deep_merge` for composable `_meta` construction (Phase 39)
- Builder pattern with `with_*` methods on `ToolInfo`
- Feature-gated types behind `mcp-apps` flag

### Integration Points
- `ToolUIMetadata::build_meta_map()` — single point to add legacy flat key
- `ServerCoreBuilder` capability handling — where to add MCP Apps capability negotiation
- `WidgetMeta::to_meta_map()` — where CSP and domain could be migrated to nested `ui` format
- `mcp-preview` proxy — needs to understand both flat and nested metadata formats

</code_context>

<deferred>
## Deferred Ideas

- Widget runtime library (`packages/widget-runtime/`) API parity with `@modelcontextprotocol/ext-apps` `App` class — separate phase
- `cargo pmcp app` CLI updates for new spec fields — separate phase
- E2E testing against ChatGPT live environment — manual testing, not automatable in CI

</deferred>

---

*Phase: 40-review-chatgpt-compatibility-for-apps*
*Context gathered: 2026-03-07*
