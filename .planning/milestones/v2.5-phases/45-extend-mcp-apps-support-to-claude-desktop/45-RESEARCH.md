# Phase 45: Extend MCP Apps Support to Claude Desktop - Research

**Researched:** 2026-03-09
**Domain:** MCP Apps cross-host compatibility (SDK metadata, preview modes, widget bridge)
**Confidence:** HIGH

## Summary

The MCP Apps specification (ext-apps v1.1.2, spec 2026-01-26) defines a standard protocol for interactive UIs in MCP hosts. The spec uses `_meta.ui.resourceUri` as the canonical tool-to-widget linkage, `text/html;profile=mcp-app` as the MIME type, and JSON-RPC over postMessage for iframe-host communication. Claude Desktop implements the standard spec directly -- there is no Claude-specific protocol variation. ChatGPT adds proprietary `openai/*` flat keys alongside the standard nested keys.

The current PMCP SDK emits a "triple-key" format by default (`_meta.ui.resourceUri` + `_meta["ui/resourceUri"]` + `_meta["openai/outputTemplate"]`), which works for both hosts but conflates standard and host-specific keys. The phase goal is to refactor this to emit ONLY standard keys by default, with ChatGPT-specific keys added via an opt-in `with_host_layer(HostType::ChatGpt)` builder method. This is a breaking change for existing ChatGPT users.

**Primary recommendation:** Refactor `build_meta_map()` and `emit_resource_uri_keys()` to emit only `_meta.ui.resourceUri` (nested). Add a host-layer system to `ServerCoreBuilder` that enriches `_meta` with `openai/*` keys when `HostType::ChatGpt` is registered. Refactor `AppBridge` in widget-runtime to be host-agnostic with an `extensions` namespace for ChatGPT-specific APIs.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Start with standard MCP Apps spec (ext-apps reference patterns) as Claude Desktop baseline
- HostType::Claude already exists returning HtmlMcp -- assume correct unless research contradicts
- If Claude Desktop has its own protocol variations, add --mode claude-desktop to mcp-preview (don't force into standard mode)
- Research phase should compare ext-apps reference server output against SDK output to identify gaps
- Default SDK behavior: emit ONLY standard MCP spec keys (nested ui.resourceUri) -- no ChatGPT openai/* keys, no legacy flat ui/resourceUri key
- Drop the legacy flat key ui/resourceUri entirely -- clean break
- Host-specific keys added via ServerCoreBuilder::with_host_layer(HostType::ChatGpt)
- General pattern: .with_host_layer(HostType) -- extensible for future hosts without API changes
- When ChatGPT layer is added, openai/outputTemplate and other ChatGPT keys are emitted alongside standard keys
- This is a BREAKING CHANGE for existing ChatGPT users who relied on auto-emitted openai/* keys
- --mode standard becomes the new default (no flag needed)
- --mode chatgpt remains as explicit opt-in for ChatGPT-strict validation
- --mode claude-desktop added later if research reveals Claude-specific requirements
- Standard mode validation mirrors ext-apps reference patterns
- Standard mode uses McpApps postMessage bridge (JSON-RPC over postMessage)
- ChatGPT mode continues with ChatGPT postMessage emulation + window.openai stub
- window.mcpBridge is the canonical developer-facing API
- AppBridge refactored to be host-agnostic
- Host-specific capabilities via mcpBridge.extensions namespace (mcpBridge.extensions.chatgpt.requestDisplayMode())
- widget-runtime package updated in this phase (not deferred)
- SDK: Refactor metadata emission to standard-first + opt-in layers
- Preview: --mode standard as default, update validation rules
- Bridge: Normalize AppBridge, add extensions namespace
- Examples: Verify chess, map, dataviz render in standard mode
- Deferred: Book chapters, course content, quizzes/exercises

### Claude's Discretion
- Internal implementation of host layer registration and metadata enrichment pipeline
- How to detect which host layers are active during _meta construction
- Widget-runtime bridge refactoring approach (incremental vs rewrite)
- Test strategy for verifying cross-host compatibility

### Deferred Ideas (OUT OF SCOPE)
- Book chapter updates for cross-client MCP Apps
- Course content updates for standard-first pattern
- Quiz/exercise updates
- --mode claude-desktop in mcp-preview (add only if research reveals Claude-specific requirements)
</user_constraints>

## Standard Stack

### Core (Already in Project)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| pmcp | 1.16.x | Core MCP SDK | This project |
| serde_json | 1.x | JSON manipulation for _meta maps | Already used everywhere |
| axum | 0.8 | mcp-preview HTTP server | Already used in mcp-preview |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| @modelcontextprotocol/ext-apps | 1.1.2 | Reference for standard MCP Apps protocol | Validation reference, not a dependency |

**No new dependencies required.** This phase refactors existing code, not adding new libraries.

## Architecture Patterns

### Pattern 1: Host Layer Registration on ServerCoreBuilder

**What:** Add a `host_layers: Vec<HostType>` field to `ServerCoreBuilder` and a `with_host_layer(HostType)` method. During `build()`, store the active layers on `ServerCore`. The `build_uri_to_tool_meta()` function and all _meta emission paths check active layers.

**When to use:** All _meta construction paths.

**Implementation approach:**

```rust
// In ServerCoreBuilder (src/server/builder.rs)
pub struct ServerCoreBuilder {
    // ... existing fields ...
    #[cfg(feature = "mcp-apps")]
    host_layers: Vec<HostType>,
}

impl ServerCoreBuilder {
    /// Register a host-specific metadata layer.
    ///
    /// By default, only standard MCP Apps keys are emitted in `_meta`.
    /// Call this to add host-specific keys (e.g., `openai/*` for ChatGPT).
    #[cfg(feature = "mcp-apps")]
    pub fn with_host_layer(mut self, host: HostType) -> Self {
        if !self.host_layers.contains(&host) {
            self.host_layers.push(host);
        }
        self
    }
}
```

```rust
// In ServerCore (src/server/core.rs) -- store active layers
pub struct ServerCore {
    // ... existing fields ...
    #[cfg(feature = "mcp-apps")]
    host_layers: Vec<HostType>,
}
```

**Confidence:** HIGH -- follows established builder pattern in the codebase.

### Pattern 2: Standard-Only Default Metadata Emission

**What:** Refactor `build_meta_map()` in `src/types/ui.rs` and `emit_resource_uri_keys()` to emit ONLY the standard nested `ui.resourceUri` key. Create a separate enrichment step that adds host-specific keys based on active layers.

**Current behavior (to change):**
```rust
// emit_resource_uri_keys() currently emits 3 keys:
// 1. ui.resourceUri (nested) -- STANDARD, KEEP
// 2. ui/resourceUri (flat) -- LEGACY, DROP
// 3. openai/outputTemplate -- CHATGPT-SPECIFIC, CONDITIONAL
```

**New behavior:**
```rust
// Standard-only: emit_resource_uri_keys() emits 1 key:
// 1. ui.resourceUri (nested) -- STANDARD

// Host layer enrichment (separate function):
fn enrich_meta_for_host(meta: &mut Map, host: HostType, uri: &str) {
    match host {
        HostType::ChatGpt => {
            meta.insert("openai/outputTemplate".into(), Value::String(uri.into()));
            // Other ChatGPT keys from WidgetMeta/ChatGptToolMeta
        }
        _ => {} // No enrichment needed for standard hosts
    }
}
```

**Confidence:** HIGH -- the triple-key pattern is centralized in `emit_resource_uri_keys()` and `build_meta_map()`.

### Pattern 3: Post-Registration Meta Enrichment Pipeline

**What:** After tool registration in `ServerCoreBuilder::build()`, iterate all `tool_infos` and enrich their `_meta` based on active host layers. This keeps the enrichment at the boundary (construction time) rather than scattered through the code.

**Implementation:**
```rust
// In ServerCoreBuilder::build()
#[cfg(feature = "mcp-apps")]
{
    for (_name, info) in &mut self.tool_infos {
        if let Some(meta) = info.meta.as_mut() {
            for host in &self.host_layers {
                enrich_meta_for_host(meta, *host);
            }
        }
    }
}
```

**Confidence:** HIGH -- aligns with Phase 38's caching pattern where metadata is finalized at registration.

### Pattern 4: Widget Bridge Extensions Namespace

**What:** Refactor the `window.mcpBridge` API in widget-runtime to separate standard MCP Apps methods from host-specific extensions.

**Standard mcpBridge API (all hosts):**
- `callTool(name, args)` -- via JSON-RPC postMessage
- `readResource(uri)` -- via JSON-RPC postMessage
- `getPrompt(name, args)` -- via JSON-RPC postMessage
- `notify(method, params)` -- via postMessage notification
- `onToolResult(callback)` -- receive tool results
- `onToolInput(callback)` -- receive tool input args
- `onHostContextChanged(callback)` -- theme, locale changes

**Extensions namespace (host-specific):**
```javascript
mcpBridge.extensions = {
    chatgpt: {
        // Only available when running in ChatGPT
        requestDisplayMode: (mode) => { ... },
        requestClose: () => { ... },
        notifyIntrinsicHeight: (height) => { ... },
        uploadFile: (file) => { ... },
        getFileDownloadUrl: (fileId) => { ... },
        getState: () => { ... },
        setState: (state) => { ... },
        sendMessage: (message) => { ... },
        setOpenInAppUrl: (href) => { ... },
        // Environment context
        get theme() { ... },
        get locale() { ... },
        get displayMode() { ... },
        get maxHeight() { ... },
        get safeArea() { ... },
    },
    claude: {
        // Reserved for future Claude-specific APIs
        // Currently empty
    }
};
```

**Confidence:** HIGH -- clean separation, backward-compatible detection via `if (mcpBridge.extensions?.chatgpt)`.

### Recommended Refactoring Sequence

```
1. Refactor emit_resource_uri_keys() → standard-only (ui.rs)
2. Refactor build_meta_map() → standard-only (ui.rs)
3. Add host_layers to ServerCoreBuilder + ServerCore
4. Add enrich_meta_for_host() pipeline in build()
5. Update build_uri_to_tool_meta() for standard-only index
6. Update uri_to_tool_meta enrichment to use host layers
7. Refactor ChatGptAdapter bridge injection → McpAppsAdapter bridge
8. Refactor widget-runtime AppBridge → host-agnostic + extensions
9. Update mcp-preview standard mode validation
10. Verify examples render in standard mode
```

### Anti-Patterns to Avoid
- **Runtime host detection in _meta emission:** Do NOT detect the client type at request time and conditionally emit keys. Host layers are registered at construction time and _meta is fixed.
- **Branching on HostType throughout the codebase:** All host-specific enrichment should funnel through one pipeline, not scattered `if host == ChatGpt` checks.
- **Breaking the adapter pattern:** The existing `UIAdapter` trait and `ChatGptAdapter`/`McpAppsAdapter` should remain for HTML transformation and bridge injection. Don't conflate metadata emission (builder-level) with content transformation (adapter-level).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON-RPC postMessage protocol | Custom message format | Standard JSON-RPC 2.0 over postMessage | MCP Apps spec requirement; already implemented |
| Host detection in widget | Feature flag system | Runtime `window.openai` / `window.mcpBridge` detection | Already works, just needs cleanup |
| MIME type negotiation | Content negotiation middleware | Static MIME per HostType | `text/html;profile=mcp-app` for all hosts per spec |

## Common Pitfalls

### Pitfall 1: Breaking Existing ChatGPT Users Without Migration Path
**What goes wrong:** Removing `openai/*` keys from default emission breaks all existing ChatGPT MCP Apps servers that upgrade pmcp.
**Why it happens:** This is an intentional breaking change per CONTEXT.md decisions.
**How to avoid:** Document clearly in CHANGELOG. Provide a one-line fix: add `.with_host_layer(HostType::ChatGpt)` to their builder.
**Warning signs:** ChatGPT widgets stop rendering after upgrade.

### Pitfall 2: Forgetting to Update build_uri_to_tool_meta Index
**What goes wrong:** `resources/list` and `resources/read` still try to propagate `openai/*` keys from the tool meta index, but those keys no longer exist in standard-only mode.
**Why it happens:** `build_uri_to_tool_meta()` currently filters to `CHATGPT_DESCRIPTOR_KEYS` only.
**How to avoid:** Refactor `build_uri_to_tool_meta()` to index standard keys (`ui.*`) by default, and add ChatGPT keys only when ChatGPT host layer is active.
**Warning signs:** Resources missing `_meta` enrichment.

### Pitfall 3: Widget Bridge Regression in ChatGPT Mode
**What goes wrong:** After refactoring AppBridge to be host-agnostic, ChatGPT-specific features (window.openai passthrough) stop working.
**Why it happens:** The ChatGPT bridge in `adapter.rs` has ~340 lines of `window.openai` wrapping that cannot simply be removed.
**How to avoid:** The standard bridge handles postMessage JSON-RPC. The ChatGPT adapter's `inject_bridge()` adds the `window.openai` wrapper ON TOP of the standard bridge, not replacing it. This is additive.
**Warning signs:** `window.openai.callTool()` fails in ChatGPT.

### Pitfall 4: emit_resource_uri_keys Used in Multiple Call Sites
**What goes wrong:** Changing `emit_resource_uri_keys()` signature/behavior breaks callers.
**Why it happens:** Called from `ToolUIMetadata::build_meta_map()`, `WidgetMeta::to_meta_map()`, and tests.
**How to avoid:** Audit all callers. The function should be simplified to emit only the nested key. Remove `map` parameter (flat key insertion). Callers that need ChatGPT keys should call the enrichment function separately.
**Warning signs:** Compiler errors in test assertions expecting 3 keys.

### Pitfall 5: mcp-preview Standard Mode Missing Tool Result Delivery
**What goes wrong:** Standard mode preview doesn't deliver tool results to the widget because it was previously relying on ChatGPT's `widgetStateUpdate` custom event.
**Why it happens:** The ChatGPT bridge sends `widgetStateUpdate` events, but standard MCP Apps uses `ui/notifications/tool-result` JSON-RPC notification.
**How to avoid:** Standard mode preview must send `ui/notifications/tool-result` (and `ui/notifications/tool-input`) via postMessage. The `AppBridge` host-side class already does this via `sendToolResult()`.
**Warning signs:** Widget renders but never receives data in standard mode.

## Code Examples

### Current _meta Output (BEFORE -- triple-key)
```json
{
  "_meta": {
    "ui": { "resourceUri": "ui://chess/board" },
    "ui/resourceUri": "ui://chess/board",
    "openai/outputTemplate": "ui://chess/board"
  }
}
```

### Target _meta Output (AFTER -- standard-only default)
```json
{
  "_meta": {
    "ui": { "resourceUri": "ui://chess/board" }
  }
}
```

### Target _meta Output (AFTER -- with ChatGPT layer)
```json
{
  "_meta": {
    "ui": { "resourceUri": "ui://chess/board" },
    "openai/outputTemplate": "ui://chess/board",
    "openai/toolInvocation/invoking": "Loading...",
    "openai/toolInvocation/invoked": "Ready!",
    "openai/widgetAccessible": true
  }
}
```

### Standard MCP Apps JSON-RPC Protocol (iframe-to-host)
```javascript
// Widget sends to host via postMessage:
// Initialize
{ "jsonrpc": "2.0", "id": 1, "method": "ui/initialize", "params": {} }
// Host responds with host context:
{ "jsonrpc": "2.0", "id": 1, "result": { "theme": "light", "locale": "en-US" } }

// Call tool
{ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "get_time", "arguments": {} } }

// Host pushes notifications:
{ "jsonrpc": "2.0", "method": "ui/notifications/tool-result", "params": { "structuredContent": {...} } }
{ "jsonrpc": "2.0", "method": "ui/notifications/tool-input", "params": { "arguments": {...} } }
```

### MCP Apps Registration (ext-apps reference pattern)
```typescript
// From @modelcontextprotocol/ext-apps/server
registerAppTool(server, "get-time", {
    title: "Get Time",
    description: "Returns the current server time.",
    inputSchema: {},
    _meta: { ui: { resourceUri } }  // ONLY standard key
}, async () => { ... });

registerAppResource(server, "Time App", resourceUri,
    { mimeType: "text/html;profile=mcp-app" },  // RESOURCE_MIME_TYPE
    async () => ({ contents: [{ mimeType: RESOURCE_MIME_TYPE, text: html }] })
);
```

### ServerCoreBuilder Usage (AFTER refactor)
```rust
// Standard-only (works with Claude Desktop, VS Code, Goose, etc.)
let server = ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool("chess", ChessTool)
    .build()?;

// With ChatGPT support added
let server = ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool("chess", ChessTool)
    .with_host_layer(HostType::ChatGpt)
    .build()?;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `text/html+skybridge` MIME | `text/html;profile=mcp-app` | ext-apps v1.0 (Jan 2026) | All hosts use same MIME type |
| `window.openai` only | Standard JSON-RPC postMessage | ext-apps v1.0 (Jan 2026) | Hosts implement postMessage, not vendor APIs |
| `openai/outputTemplate` as primary | `_meta.ui.resourceUri` as primary | ext-apps v1.0 (Jan 2026) | Standard key is the canonical one |
| Triple-key dual-emit (PMCP) | Standard-only + opt-in layers | Phase 45 (this phase) | Breaking change for ChatGPT users |
| `ui/resourceUri` flat key | Dropped | Phase 45 (this phase) | Legacy key removed |

**Claude Desktop protocol:** Claude Desktop implements the standard MCP Apps spec directly. It looks for `_meta.ui.resourceUri` on tools, fetches `ui://` resources with `text/html;profile=mcp-app` MIME type, renders in sandboxed iframe, communicates via JSON-RPC over postMessage. No Claude-specific `_meta` keys are needed. The existing `HostType::Claude` returning `HtmlMcp` is correct.

**Research finding: NO --mode claude-desktop needed.** Claude Desktop uses the standard protocol. `--mode standard` is sufficient. This aligns with the CONTEXT.md deferred decision.

## Key Research Findings

### 1. Claude Desktop Uses Standard MCP Apps Spec (HIGH confidence)
Claude Desktop expects:
- `_meta.ui.resourceUri` on tools/list (nested, not flat)
- `text/html;profile=mcp-app` MIME type on resources
- JSON-RPC 2.0 over postMessage for iframe communication
- Standard methods: `ui/initialize`, `tools/call`, `ui/notifications/tool-result`, `ui/notifications/tool-input`
- No proprietary keys required

### 2. ext-apps registerAppTool Emits Standard Keys Only (HIGH confidence)
The official `registerAppTool` from `@modelcontextprotocol/ext-apps/server` sets ONLY `_meta: { ui: { resourceUri } }`. It does NOT emit `openai/outputTemplate` or `ui/resourceUri` flat key. ChatGPT knows to read `_meta.ui.resourceUri` directly.

### 3. RESOURCE_MIME_TYPE = "text/html;profile=mcp-app" (HIGH confidence)
The ext-apps SDK exports `RESOURCE_MIME_TYPE = "text/html;profile=mcp-app"`. This is used for both `registerAppResource` mimeType and resource content mimeType. PMCP already has `HtmlMcpApp` variant for this.

### 4. ChatGPT Also Reads Standard Keys (MEDIUM confidence)
The migration guide shows ChatGPT can read `_meta.ui.resourceUri` (the standard key). The `openai/outputTemplate` is the legacy/alias key. Servers using only standard keys should work with ChatGPT, but the `openai/*` keys provide additional features (invoking messages, widget accessibility flags) that have no standard equivalent.

### 5. CSP Format Difference (HIGH confidence)
Standard spec uses camelCase nested format: `_meta.ui.csp.connectDomains`. ChatGPT uses `openai/widgetCSP` with `connect_domains` (snake_case) at the top level. The existing `WidgetCSP` type serializes with snake_case. Need to ensure `to_spec_map()` emits camelCase for standard mode.

### 6. Impact Analysis: What Breaks When openai/* Keys Not Auto-Emitted
- `openai/outputTemplate`: ChatGPT won't link tool to widget (CRITICAL for ChatGPT)
- `openai/toolInvocation/invoking|invoked`: ChatGPT won't show loading/ready messages (degraded UX)
- `openai/widgetAccessible`: ChatGPT won't allow widget-to-tool calls (breaks interactivity)
- These are ALL required for ChatGPT but NONE for Claude Desktop, VS Code, Goose, etc.
- Fix: Add `.with_host_layer(HostType::ChatGpt)` to restore ChatGPT behavior

## Open Questions

1. **Does ChatGPT actually work with ONLY `_meta.ui.resourceUri` (no `openai/outputTemplate`)?**
   - What we know: Migration guide implies ChatGPT reads the standard key. The ext-apps SDK only emits the standard key.
   - What's unclear: Whether ChatGPT has fully migrated or still requires the openai alias for some features.
   - Recommendation: The ChatGPT host layer should emit BOTH standard and openai keys when active. This is the safe additive approach.

2. **How should WidgetMeta.to_meta_map() change?**
   - What we know: Currently emits both `openai/widgetCSP` (snake_case) and `ui.csp` (camelCase spec format).
   - What's unclear: Whether standard-only mode should strip all `openai/*` serialized fields.
   - Recommendation: `to_meta_map()` should have a `for_host` parameter or return standard-only by default with a separate method for ChatGPT enrichment.

3. **Widget-runtime refactoring scope**
   - What we know: Need to separate standard APIs from ChatGPT extensions in `window.mcpBridge`.
   - What's unclear: How deeply to refactor -- incremental (add extensions namespace, keep existing methods) vs rewrite.
   - Recommendation: Incremental. Add `mcpBridge.extensions.chatgpt` namespace. Keep existing standard methods on `mcpBridge`. Mark ChatGPT-only methods as deprecated on the root (or just move them).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) + inline tests |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test --lib -p pmcp -- --test-threads=1` |
| Full suite command | `cargo test --lib --tests -p pmcp -- --test-threads=1` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P45-01 | build_meta_map emits only standard ui.resourceUri | unit | `cargo test --lib -p pmcp types::ui::tests::test_build_meta_map -- --test-threads=1` | Exists (needs update) |
| P45-02 | with_host_layer(ChatGpt) adds openai/* keys | unit | `cargo test --lib -p pmcp -- host_layer --test-threads=1` | Wave 0 |
| P45-03 | build_uri_to_tool_meta indexes standard keys | unit | `cargo test --lib -p pmcp -- uri_to_tool_meta --test-threads=1` | Exists (needs update) |
| P45-04 | Standard mode preview serves standard bridge | integration | `cargo test --lib -p mcp-preview -- --test-threads=1` | Wave 0 |
| P45-05 | ChatGPT adapter still injects window.openai bridge | unit | `cargo test --lib -p pmcp -- test_chatgpt_adapter --test-threads=1` | Exists |
| P45-06 | Examples render in standard mode | smoke | Manual: `cargo pmcp preview` for each example | Manual |

### Sampling Rate
- **Per task commit:** `cargo test --lib -p pmcp -- --test-threads=1`
- **Per wave merge:** `cargo test --lib --tests -- --test-threads=1`
- **Phase gate:** Full suite green before verification

### Wave 0 Gaps
- [ ] Tests for `with_host_layer()` on ServerCoreBuilder
- [ ] Tests for standard-only `build_meta_map()` (update existing)
- [ ] Tests for host-layer enrichment pipeline
- [ ] Tests for updated `build_uri_to_tool_meta()` with/without host layers

## Sources

### Primary (HIGH confidence)
- [MCP Apps Specification](https://modelcontextprotocol.io/docs/extensions/apps) -- official docs confirming `_meta.ui.resourceUri`, postMessage JSON-RPC, iframe sandboxing
- [ext-apps GitHub](https://github.com/modelcontextprotocol/ext-apps) -- reference SDK, spec 2026-01-26
- [ext-apps Quickstart](https://apps.extensions.modelcontextprotocol.io/api/documents/Quickstart.html) -- `registerAppTool` emits `_meta: { ui: { resourceUri } }` only
- [ext-apps Migration Guide](https://apps.extensions.modelcontextprotocol.io/api/documents/Migrate_OpenAI_App.html) -- OpenAI-to-standard key mapping, MIME type changes
- [ext-apps Overview](https://apps.extensions.modelcontextprotocol.io/api/documents/Overview.html) -- JSON-RPC methods: ui/initialize, ui/notifications/tool-result, tools/call

### Secondary (MEDIUM confidence)
- [MCP Apps Blog Post](http://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/) -- Claude Desktop, VS Code, Goose all confirmed supporting MCP Apps
- [The Register](https://www.theregister.com/2026/01/26/claude_mcp_apps_arrives/) -- Claude Desktop MCP Apps support confirmation

### Codebase (HIGH confidence)
- `src/types/ui.rs` -- `build_meta_map()`, `emit_resource_uri_keys()`, `deep_merge()`
- `src/types/mcp_apps.rs` -- `HostType`, `WidgetMeta`, `ChatGptToolMeta`, `WidgetCSP`
- `src/server/mcp_apps/adapter.rs` -- `ChatGptAdapter`, `McpAppsAdapter`, `UIAdapter` trait
- `src/server/mcp_apps/builder.rs` -- `MultiPlatformResource`, `UIResourceBuilder`
- `src/server/builder.rs` -- `ServerCoreBuilder`
- `src/server/core.rs` -- `build_uri_to_tool_meta()`, `uri_to_tool_meta` index
- `packages/widget-runtime/src/app-bridge.ts` -- host-side bridge
- `packages/widget-runtime/src/types.ts` -- McpBridge interface, HostType
- `crates/mcp-preview/src/server.rs` -- `PreviewMode`, `PreviewConfig`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, refactoring existing code
- Architecture: HIGH -- patterns follow established codebase conventions (builder, caching, adapters)
- Pitfalls: HIGH -- thorough analysis of all call sites and breaking change impact
- Claude Desktop protocol: HIGH -- standard MCP Apps spec, confirmed by multiple official sources
- Widget bridge refactoring: MEDIUM -- incremental approach clear, but exact scope of changes in widget-runtime.mjs needs validation during implementation

**Research date:** 2026-03-09
**Valid until:** 2026-04-09 (stable spec, unlikely to change in 30 days)

---

## Migration Guide: ChatGPT-Only to Cross-Client

**Added:** 2026-03-09 (Phase 45-03 Task 2)

This guide documents the DX simplifications when migrating from a ChatGPT-only MCP Apps server to the standard-first cross-client pattern. Based on analysis of the Open Images MCP server (`pmcp-run/built-in/sql-api/servers/open-images`).

### Server-side (Rust): Before and After

**Before (ChatGPT-only, ~20 lines per widget resource):**

```rust
use pmcp::server::mcp_apps::{ChatGptAdapter, UIAdapter};
use pmcp::types::mcp_apps::ChatGptToolMeta;

fn widget_resource(uri: &str, name: &str, html: &str, invoking: &str, invoked: &str)
    -> ResourceConfig
{
    // Step 1: Manual adapter construction
    let adapter = ChatGptAdapter::new();
    let transformed = adapter.transform(uri, name, html);

    // Step 2: Manual ChatGptToolMeta construction (4 keys)
    let meta = ChatGptToolMeta::new()
        .output_template(uri)        // openai/outputTemplate
        .invoking(invoking)          // openai/toolInvocation/invoking
        .invoked(invoked)            // openai/toolInvocation/invoked
        .widget_accessible(true);    // openai/widgetAccessible

    // Step 3: Manual ResourceConfig assembly
    ResourceConfig {
        uri: transformed.uri,
        name: transformed.name,
        description: Some(description.to_string()),
        mime_type: transformed.mime_type.to_string(),
        content: Some(transformed.content),
        meta: Some(meta.to_meta_map()),
    }
}
```

**After (standard-first with optional ChatGPT layer):**

```rust
// Standard metadata is automatic via with_ui() on tool registration.
// No adapter construction, no manual meta map, no ChatGptToolMeta.
// The builder emits only _meta.ui.resourceUri by default.

ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool("search_images", SearchTool.with_ui())
    // ChatGPT support is ONE line (opt-in):
    .with_host_layer(HostType::ChatGpt)
    .build()?;
```

**What changes for Open Images specifically:**
| Before (ChatGPT-only) | After (cross-client) | Lines saved |
|------------------------|---------------------|-------------|
| `ChatGptAdapter::new()` + `.transform()` | Automatic via `with_ui()` | ~3 per widget |
| `ChatGptToolMeta::new().output_template().invoking().invoked().widget_accessible()` | Automatic via host layer enrichment | ~5 per widget |
| Manual `ResourceConfig` assembly with meta map | Builder handles it | ~8 per widget |
| 3 widget resources x ~20 lines each = ~60 lines | `.with_host_layer(HostType::ChatGpt)` = 1 line | ~59 lines |

### Widget-side (HTML/JS): Before and After

**Before (4-tier fallback, ~40 lines in image-explorer.html lines 726-773):**

```javascript
// TIER 1: Listen for tool results via postMessage JSON-RPC
window.addEventListener('message', function(event) {
    if (event.source !== window.parent) return;
    var msg = event.data;
    if (!msg || msg.jsonrpc !== '2.0') return;
    if (msg.method === 'ui/notifications/tool-result') {
        var data = msg.params && msg.params.structuredContent;
        if (data) handleData(data);
    }
}, { passive: true });

// TIER 2: Listen for openai:set_globals (ChatGPT context updates)
window.addEventListener('openai:set_globals', function(event) {
    var data = event.detail && event.detail.globals && event.detail.globals.toolOutput;
    if (data) handleData(data);
}, { passive: true });

// TIER 3: Check window.openai.toolOutput on load (may already be set)
function tryOpenAI() {
    if (window.openai && window.openai.toolOutput) {
        handleData(window.openai.toolOutput);
        return true;
    }
    return false;
}

// TIER 4: Legacy mcpBridge support (non-ChatGPT hosts)
window.addEventListener('mcpBridgeReady', function() {
    if (window.mcpBridge && window.mcpBridge.toolOutput) {
        handleData(window.mcpBridge.toolOutput);
    }
}, { once: true });

window.addEventListener('widgetStateUpdate', function(e) {
    var data = e.detail && e.detail.toolOutput;
    if (data) handleData(data);
});

// Try immediate sources
if (!tryOpenAI()) {
    if (window.mcpBridge && window.mcpBridge.toolOutput) {
        handleData(window.mcpBridge.toolOutput);
    }
}
```

**After (standard mcpBridge, ~5 lines):**

```javascript
// The widget-runtime bridge normalizes all delivery mechanisms.
// One callback handles all hosts (ChatGPT, Claude Desktop, VS Code, etc.)
window.mcpBridge.onToolResult(function(data) {
    handleData(data.structuredContent || data);
});
```

**DX improvement summary:**
- Server-side: ~60 lines of boilerplate reduced to 1 line (`.with_host_layer()`)
- Widget-side: ~40 lines of 4-tier fallback reduced to ~5 lines
- No more manual ChatGptToolMeta construction
- No more ChatGptAdapter instantiation per widget
- No more 4-tier event listener fallback in widget HTML
- Cross-client by default: works with Claude Desktop, VS Code, Goose without changes

### Example Status: Chess and Map

The chess and map examples (`examples/mcp-apps-chess`, `examples/mcp-apps-map`) currently use `ChatGptAdapter` directly in their server code. This is noted but not blocking for Phase 45-03 -- these examples work correctly in standard preview mode because:

1. `mcp-preview` in standard mode does NOT inject ChatGPT-specific keys, so the bridge operates in standard MCP Apps mode
2. The `ChatGptAdapter` in the examples handles HTML transformation (bridge injection, MIME type), which works for all hosts
3. The preview HTML files (`preview.html`) are standalone dev tools, not production widget HTML

**Future cleanup (out of scope for this plan):** Migrate chess/map examples to use `with_ui()` builder pattern instead of manual `ChatGptAdapter` construction, matching the migration guide above.
