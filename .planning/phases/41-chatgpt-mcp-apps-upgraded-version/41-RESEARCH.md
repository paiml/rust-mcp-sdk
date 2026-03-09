# Phase 41: ChatGPT MCP Apps Upgraded Version - Research

**Researched:** 2026-03-06
**Domain:** MCP Apps protocol alignment (SDK types, bridge protocol, scaffold template)
**Confidence:** HIGH

## Summary

Phase 41 addresses critical compatibility gaps between the PMCP SDK's MCP Apps implementation and ChatGPT's actual MCP Apps protocol as documented at developers.openai.com. The SDK has evolved through Phases 34-40 (adding dual-emit keys, nested CSP, visibility, etc.) but still has four significant gaps:

1. **MIME type**: The scaffold template and `ChatGptAdapter` still emit `text/html+skybridge` instead of `text/html;profile=mcp-app`. The `UIMimeType::HtmlMcpApp` variant exists but is not used by default anywhere.
2. **Widget metadata placement**: `WidgetMeta` fields (widgetDescription, widgetPrefersBorder, widgetCSP, widgetDomain) are designed for tool `_meta` via `ToolInfo::with_widget_meta()`, but the official ChatGPT protocol places them on **resource content `_meta`** (in the `resources/read` response). The `Content::Resource` enum variant and `UIResourceContents` struct have no `_meta` field.
3. **Bridge protocol mismatch**: The preview's `AppBridge` host-side implementation uses `ui/toolInput`, `ui/toolResult`, `ui/hostContextChanged`, `ui/teardown`, `ui/ready` but ChatGPT actually uses `ui/initialize`, `ui/notifications/initialized`, `ui/notifications/tool-input`, `ui/notifications/tool-result`, `ui/message`, `ui/open-link`, `ui/resource-teardown`.
4. **Scaffold template**: `mcp_app.rs` scaffold doesn't call `.with_ui()` on the tool or emit `openai/outputTemplate`, and uses `HtmlSkybridge` MIME type.

**Primary recommendation:** Fix all four issues in order: (1) add `_meta` to `Content::Resource` for resource-level metadata, (2) update `ChatGptAdapter` to use `HtmlMcpApp` MIME type, (3) update the bridge protocol method names in both widget-runtime.mjs and AppBridge, (4) update scaffold template.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde/serde_json | 1.x | JSON serialization for `_meta` on Content::Resource | Already in use throughout |
| axum | 0.8 | mcp-preview HTTP server | Already in use |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| pmcp-widget-utils | workspace | Bridge script injection | Already in use for inject_bridge_script |

No new dependencies needed. All changes are internal to existing types and bridge code.

## Architecture Patterns

### Issue 1: Content::Resource _meta field

The `Content` enum in `src/types/protocol.rs` currently has:

```rust
pub enum Content {
    Text { text: String },
    Image { data: String, mime_type: String },
    Resource {
        uri: String,
        text: Option<String>,
        mime_type: Option<String>,
        // MISSING: _meta field
    },
}
```

ChatGPT's `resources/read` response requires `_meta` on each content item:

```json
{
  "contents": [{
    "uri": "ui://app/main.html",
    "mimeType": "text/html;profile=mcp-app",
    "text": "<!doctype html>...",
    "_meta": {
      "openai/widgetDescription": "Interactive view",
      "ui": {
        "prefersBorder": true,
        "domain": "myapp",
        "csp": { "connectDomains": ["https://api.example.com"] }
      }
    }
  }]
}
```

**Pattern:** Add an optional `_meta` field to `Content::Resource`:

```rust
Resource {
    uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    _meta: Option<serde_json::Map<String, serde_json::Value>>,
},
```

Also add `_meta` to `UIResourceContents` in `src/types/ui.rs` since that struct mirrors this concept.

### Issue 2: MIME type correction

The `ChatGptAdapter` in `src/server/mcp_apps/adapter.rs` returns `ExtendedUIMimeType::HtmlSkybridge`. Per official ChatGPT documentation, the correct MIME type is `text/html;profile=mcp-app`.

**Pattern:** Change `ChatGptAdapter::mime_type()` to return `ExtendedUIMimeType::HtmlMcpApp` (which maps to `HtmlMcpApp` on `UIMimeType`). The `HtmlSkybridge` variant should be kept for backward compatibility but deprecated.

### Issue 3: Bridge protocol methods

Current SDK bridge uses non-standard method names:

| Current (SDK) | Correct (ChatGPT) | Direction |
|---|---|---|
| `ui/ready` | `ui/initialize` (request) | Widget -> Host |
| (none) | `ui/notifications/initialized` (response) | Host -> Widget |
| `ui/toolInput` | `ui/notifications/tool-input` | Host -> Widget |
| `ui/toolResult` | `ui/notifications/tool-result` | Host -> Widget |
| `ui/hostContextChanged` | (no standard equivalent) | Host -> Widget |
| `ui/sendMessage` | `ui/message` | Widget -> Host |
| `ui/openLink` | `ui/open-link` | Widget -> Host |
| `ui/teardown` | `ui/resource-teardown` | Host -> Widget |

**Files to update:**
1. `crates/mcp-preview/assets/widget-runtime.mjs` - App class (`_handleNotification` cases and `sendToolInput`/etc method names)
2. `crates/mcp-preview/assets/index.html` - AppBridge host-side class
3. `src/server/mcp_apps/adapter.rs` - McpAppsAdapter bridge script

The widget-runtime.mjs App class already sends `ui/initialize` and handles `ui/toolInput` etc. The AppBridge host-side class in index.html also uses these names. Both need updating.

### Issue 4: Scaffold template

`cargo-pmcp/src/templates/mcp_app.rs` has these problems:
- Uses `ExtendedUIMimeType::HtmlSkybridge` in both `read()` and `list()`
- Does not call `.with_ui("ui://app/hello.html")` on the tool
- Does not emit `openai/outputTemplate` in tool `_meta`
- Resource read does not emit `_meta` on the content (since `Content::Resource` has no `_meta` field currently)

### Recommended Project Structure (changes only)

```
src/
  types/
    protocol.rs          # Add _meta to Content::Resource variant
    ui.rs                # Add _meta to UIResourceContents
  server/
    mcp_apps/
      adapter.rs         # Fix ChatGptAdapter MIME type; update bridge scripts
crates/
  mcp-preview/
    assets/
      index.html         # Fix AppBridge method names
      widget-runtime.mjs # Fix App class method names
cargo-pmcp/
  src/
    templates/
      mcp_app.rs         # Fix MIME type, add .with_ui(), resource _meta
```

### Anti-Patterns to Avoid

- **Breaking Content::Resource API:** Adding `_meta` as a required field would break all existing `Content::Resource` construction sites. Use `Option<>` with `skip_serializing_if`.
- **Removing HtmlSkybridge:** Some existing servers may depend on it. Deprecate but don't remove.
- **Changing bridge methods without backward compat:** The AppBridge host should accept both old and new method names during the transition, or the widget-runtime.mjs should send the new names while the AppBridge host accepts both.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON metadata merging | Manual field-by-field merge | `deep_merge()` from `ui.rs` | Already handles recursive object merge correctly |
| Triple-key resource URI | Manual map construction | `emit_resource_uri_keys()` from `ui.rs` | Single source of truth, already tested |
| MIME type strings | Hardcoded string literals | `UIMimeType::HtmlMcpApp.as_str()` | Centralized, type-safe |

## Common Pitfalls

### Pitfall 1: Enum variant field addition is not additive in serde tagged enums
**What goes wrong:** Adding a field to a `#[serde(tag = "type")]` enum variant can change serialization format if not careful.
**Why it happens:** The `Content::Resource` variant uses internal tagging, so all fields must be at the same level.
**How to avoid:** The `_meta` field is just another sibling of `uri`, `text`, `mime_type` within the internally-tagged object. Standard `#[serde(rename = "_meta")]` works fine.
**Warning signs:** Deserialization tests fail for existing Content::Resource values.

### Pitfall 2: Bridge protocol version mismatch between host and widget
**What goes wrong:** If widget-runtime.mjs sends `ui/initialize` but AppBridge in index.html still expects `ui/ready`, the handshake fails silently.
**Why it happens:** Two separate files implement opposite sides of the same protocol.
**How to avoid:** Update both files atomically in the same plan. Add protocol version negotiation or accept both old and new names.
**Warning signs:** Widget loads but bridge calls timeout.

### Pitfall 3: Content::Resource construction sites
**What goes wrong:** Every place that constructs `Content::Resource { uri, text, mime_type }` now needs to also specify `_meta`.
**Why it happens:** Struct literal syntax requires all fields.
**How to avoid:** Since `_meta` is `Option`, existing construction sites just need `_meta: None` appended. Alternatively, add a builder method.
**Warning signs:** Compilation errors in many files after adding the field.

### Pitfall 4: WidgetMeta placement confusion
**What goes wrong:** WidgetMeta is placed on tool _meta instead of resource content _meta.
**Why it happens:** Phases 34-40 built infrastructure for tool _meta; the protocol actually wants resource _meta.
**How to avoid:** Keep tool _meta for tool-level fields (openai/outputTemplate, openai/toolInvocation/*) and resource _meta for resource-level fields (openai/widgetDescription, ui.csp, ui.domain, ui.prefersBorder). WidgetMeta.to_meta_map() should be usable for either context.
**Warning signs:** ChatGPT ignores widget metadata because it looks for it in resource read response, not tool list.

## Code Examples

### Adding _meta to Content::Resource

```rust
// src/types/protocol.rs
pub enum Content {
    #[serde(rename_all = "camelCase")]
    Text { text: String },
    #[serde(rename_all = "camelCase")]
    Image { data: String, mime_type: String },
    #[serde(rename_all = "camelCase")]
    Resource {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        _meta: Option<serde_json::Map<String, serde_json::Value>>,
    },
}
```

### Resource read with _meta

```rust
// In scaffold template or resource handler
let widget_meta = WidgetMeta::new()
    .prefers_border(true)
    .description("Interactive hello widget");

Ok(ReadResourceResult::new(vec![Content::Resource {
    uri: uri.to_string(),
    text: Some(html),
    mime_type: Some(UIMimeType::HtmlMcpApp.as_str().to_string()),
    _meta: Some(widget_meta.to_meta_map()),
}]))
```

### Updated bridge protocol (widget-runtime.mjs excerpt)

```javascript
// App class - sending initialization
await this._transport.send("ui/initialize", {
    capabilities: { /* ... */ }
});

// App class - handling notifications
case "ui/notifications/tool-input":
    this._onToolInput(params);
    break;
case "ui/notifications/tool-result":
    this._onToolResult(params);
    break;
case "ui/resource-teardown":
    this._onTeardown();
    break;
```

### Updated AppBridge host-side (index.html excerpt)

```javascript
// AppBridge - sending notifications to widget
sendToolInput(params) {
    this._transport.notify("ui/notifications/tool-input", params);
}
sendToolResult(result) {
    this._transport.notify("ui/notifications/tool-result", result);
}
// AppBridge - handling requests from widget
case "ui/initialize":
    // respond with capabilities and send ui/notifications/initialized
    break;
case "ui/message":
    // handle follow-up message
    break;
case "ui/open-link":
    // handle external link
    break;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| `text/html+skybridge` MIME type | `text/html;profile=mcp-app` | OpenAI Apps SDK launch (Jan 2026) | All resources must use new MIME type |
| `window.openai` proprietary bridge | `ui/*` JSON-RPC postMessage bridge | MCP Apps spec (Jan 2026) | Standard bridge protocol, ChatGPT supports both |
| Widget metadata on tool `_meta` | Widget metadata on resource content `_meta` | MCP Apps spec (Jan 2026) | widgetDescription, CSP, domain go on resources/read |
| `ui/toolInput`, `ui/ready` | `ui/notifications/tool-input`, `ui/initialize` | MCP Apps spec (Jan 2026) | Standardized method naming |

**Deprecated/outdated:**
- `text/html+skybridge`: Keep as fallback but `text/html;profile=mcp-app` is the standard
- `ui/toolInput`, `ui/toolResult`, `ui/hostContextChanged`, `ui/teardown`, `ui/ready`: Old method names, replaced by `ui/notifications/*` pattern and `ui/initialize`

## Open Questions

1. **Backward compatibility for Content::Resource**
   - What we know: Adding `_meta: Option<...>` requires updating all struct literal construction sites
   - What's unclear: How many sites exist across the codebase (need to grep for `Content::Resource`)
   - Recommendation: Grep and update all sites, defaulting to `_meta: None`

2. **WidgetMeta dual placement**
   - What we know: WidgetMeta fields go on resource content `_meta` per spec; tool `_meta` gets different fields (outputTemplate, toolInvocation, visibility)
   - What's unclear: Whether existing code that puts WidgetMeta on tool `_meta` via `with_widget_meta()` should be preserved
   - Recommendation: Keep `with_widget_meta()` on `ToolInfo` for backward compat but add documentation that it is for tool-level metadata only; create new helper for resource-level metadata

3. **Bridge backward compatibility**
   - What we know: Both index.html and widget-runtime.mjs need updates
   - What's unclear: Whether any external widgets depend on old method names
   - Recommendation: In AppBridge, accept both old and new names for one release; in widget-runtime.mjs, send new names only

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml |
| Quick run command | `cargo test --lib` |
| Full suite command | `make tests` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P41-01 | Content::Resource has _meta field | unit | `cargo test types::protocol::tests -x` | Needs new tests |
| P41-02 | WidgetMeta.to_meta_map() used for resource content _meta | unit | `cargo test types::mcp_apps::tests -x` | Existing tests cover partial |
| P41-03 | ChatGptAdapter uses HtmlMcpApp MIME type | unit | `cargo test server::mcp_apps::adapter::tests -x` | Existing test needs update |
| P41-04 | Bridge protocol uses correct method names | manual | Browser test with mcp-preview | Manual only |
| P41-05 | Scaffold template uses correct MIME type and with_ui | unit | `cargo test -p cargo-pmcp templates::mcp_app::tests -x` | Existing tests need update |

### Sampling Rate
- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `make tests`
- **Phase gate:** Full suite green before verify

### Wave 0 Gaps
- [ ] New test: `Content::Resource` serialization/deserialization with `_meta`
- [ ] Update test: `ChatGptAdapter` MIME type assertion (currently asserts `HtmlSkybridge`)
- [ ] Update test: Scaffold template assertions

## Sources

### Primary (HIGH confidence)
- [OpenAI Apps SDK - Build MCP Server](https://developers.openai.com/apps-sdk/build/mcp-server/) - Resource _meta fields, MIME type, tool _meta fields
- [OpenAI Apps SDK - Reference](https://developers.openai.com/apps-sdk/reference/) - Complete protocol reference including bridge methods and metadata fields
- [OpenAI Apps SDK - Build ChatGPT UI](https://developers.openai.com/apps-sdk/build/chatgpt-ui/) - Bridge protocol methods (ui/notifications/*)

### Secondary (MEDIUM confidence)
- Codebase analysis of `src/types/protocol.rs`, `src/types/ui.rs`, `src/types/mcp_apps.rs` - Current implementation state
- Codebase analysis of `crates/mcp-preview/assets/` - Current bridge protocol implementation
- Codebase analysis of `cargo-pmcp/src/templates/mcp_app.rs` - Current scaffold template

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - No new libraries needed, all changes in existing code
- Architecture: HIGH - Official OpenAI docs clearly specify target shapes and protocol
- Pitfalls: HIGH - Codebase thoroughly analyzed, construction sites and compatibility concerns identified

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (protocol is stable, OpenAI docs are authoritative)
