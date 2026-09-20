# Phase 46: MCP Bridge Review and Fixes - Context

**Gathered:** 2026-03-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix the mcpBridge data delivery pipeline so widgets receive structuredContent from tool responses across all MCP hosts. Comprehensive bridge audit aligning with both ext-apps spec and OpenAI reference implementations. Primary deliverable: Claude Desktop support alongside existing ChatGPT support. Secondary deliverable: mcp-preview Bridge diagnostics tab and reproducible test harness for cross-host bridge validation.

</domain>

<decisions>
## Implementation Decisions

### Bridge data delivery fix
- Claude Desktop loads widget iframes successfully but tool result data never reaches the widget — bridge handshake or data delivery mechanism is broken
- Root cause is unknown — research must trace the full host-to-widget data delivery path for Claude Desktop
- Bridge must auto-detect the host environment and adapt its listening/delivery behavior automatically
- Widget developers write host-agnostic code using `mcpBridge.onToolResult(callback)` — bridge handles all host differences internally
- Treat MCP Apps as a common standard (like HTML) but be ready for vendor variants (like browser wars) — support the majority of variants to serve the widest developer market

### Reference alignment
- ext-apps spec and OpenAI examples are BOTH ground truth — neither takes precedence
- Don't break ChatGPT (existing, well-supported), ADD Claude Desktop/spec support alongside it
- ChatGPT was first to market; MCP spec is converging and cleaning up the early OpenAI implementation
- Claude Desktop represents the "standard" MCP Apps host — its protocol follows the ext-apps spec
- Hands-on testing required: run reference examples (threejs-server, OpenAI SDK examples) against both Claude Desktop and ChatGPT to map actual protocol differences
- Sources: https://modelcontextprotocol.io/extensions/apps/build, https://github.com/modelcontextprotocol/ext-apps, https://github.com/openai/openai-apps-sdk-examples

### Local-to-real-client testing
- User handles ngrok tunneling and Claude Desktop setup (already configured)
- Workflow: user downloads ext-apps + OpenAI reference examples locally → shares directory → phase executor builds MCP servers → user tunnels to real hosts
- Ship a reproducible test harness for bridge validation — script or test suite that can re-run bridge tests against reference examples for regression testing
- Test harness helps when hosts update their protocol

### mcp-preview Bridge diagnostics
- New dedicated "Bridge" tab in mcp-preview (separate from existing Protocol tab)
- Bridge tab includes:
  1. PostMessage traffic log — every postMessage between host and widget iframe (direction, payload, origin, timestamp)
  2. Bridge handshake trace — step-by-step visualization: widget loads → bridge registers listeners → host sends init → widget acknowledges
  3. Data flow end-to-end — full path: tool call → server response → host receives structuredContent → host delivers to widget → widget processes → widget renders
- Mode remains CLI flag only (--mode standard/chatgpt) — no live switching in UI
- Bridge tab shows current mode but cannot change it

### Strategic context
- MCP Apps is the extensibility layer for enterprise AI interfaces (ChatGPT, Claude Desktop, future: Gemini, CoPilot)
- mcp-preview is positioned as the universal development and debugging tool for MCP Apps across all hosts
- Enterprise-first market, consumer later
- SDK absorbs vendor differences so developers write once, deploy to many hosts

### Claude's Discretion
- How to implement host auto-detection in the bridge (environment sniffing strategy)
- Internal architecture of the Bridge diagnostics tab (data capture, rendering)
- Test harness implementation details (script vs test suite vs both)
- How to trace Claude Desktop's specific bridge protocol (reverse-engineering approach)

</decisions>

<specifics>
## Specific Ideas

- "The MCP protocol and MCP Apps extensions are new and evolving. They are positioned as a standard that all providers can adopt (like HTTP or HTML), but we should expect variants like the early browser wars where vendors push their own variations"
- "ChatGPT is still the most common chat interface with UI — we should serve it well while adding standard/Claude Desktop support"
- "The focus is on non-technical business manager AI interfaces that large providers are pushing to the enterprise market"
- Phase 44's discovery of ChatGPT's undocumented 4-key strictness (found by diffing against Pizzaz reference server) — same investigative approach needed for Claude Desktop
- The open-images reference implementation at pmcp-run/built-in/sql-api/servers/open-images demonstrates the current developer experience and pain points

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppBridge` class (packages/widget-runtime/src/app-bridge.ts): Current bridge abstraction — needs host auto-detection refactor
- `widget-runtime.mjs` (crates/mcp-preview/assets/): Compiled bridge runtime used in mcp-preview iframes
- `McpProxy` (crates/mcp-preview/src/proxy.rs): Proxy that fetches tool results, extracts structuredContent
- `index.html` (crates/mcp-preview/assets/): Main SPA with Protocol tab, widget iframe management, postMessage delivery (lines 1649-1657)
- `PreviewMode` enum: Already has Standard/ChatGpt variants — extensible for new modes
- `CHATGPT_DESCRIPTOR_KEYS` (src/types/ui.rs): Canonical key whitelist — model for per-host key definitions
- `HostType` enum (mcp_apps.rs): Already has Claude variant
- `UIAdapter` trait (adapter.rs): Per-platform transformation pattern

### Established Patterns
- Standard-only metadata default + host layer enrichment (Phase 45)
- Extensions namespace on mcpBridge for host-specific APIs (Phase 45)
- Protocol diagnostics tab with pass/fail validation checks (Phase 44)
- Builder pattern: ServerCoreBuilder::with_host_layer(HostType)
- Dual-emit metadata for multi-host support

### Integration Points
- `packages/widget-runtime/src/app-bridge.ts` — Add host auto-detection and multi-host listener registration
- `crates/mcp-preview/assets/index.html` — Add Bridge tab alongside Protocol tab
- `crates/mcp-preview/src/proxy.rs` — May need to capture postMessage traffic for Bridge tab
- Reference examples directory (user-provided) — Build and test against real hosts

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 46-mcp-bridge-review-and-fixes*
*Context gathered: 2026-03-10*
