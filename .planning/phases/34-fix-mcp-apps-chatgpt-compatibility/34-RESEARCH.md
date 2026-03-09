# Phase 34: Fix MCP Apps ChatGPT Compatibility - Research

**Researched:** 2026-03-06
**Domain:** MCP Apps protocol metadata format, ChatGPT compatibility
**Confidence:** HIGH

## Summary

This phase addresses five concrete discrepancies between the PMCP SDK's MCP Apps implementation and the actual format expected by ChatGPT (and the MCP Apps specification). Research confirms all five issues identified in CONTEXT.md are real and actionable.

The core problems are: (1) MIME type mismatch -- OpenAI docs specify `text/html;profile=mcp-app` but SDK uses `text/html+mcp` and `text/html+skybridge`, (2) inconsistent `_meta` format between two code paths (`ToolInfo::with_ui()` uses flat `"ui/resourceUri"` while `TypedTool::metadata()` uses nested `"ui": { "resourceUri" }`), (3) resource `_meta` uses flat `openai/*` keys where OpenAI docs show nested `ui` object, (4) mcp-preview panics due to axum 0.8 wildcard route syntax, and (5) `TypedTool` lacks `openai/outputTemplate` support. The `_meta` serde rename issue (commit 992aa8d) is already fixed.

**Primary recommendation:** Unify all `_meta` output to use nested `ui` object format (`_meta.ui.resourceUri`, `_meta.ui.prefersBorder`) as the MCP standard, and add `openai/outputTemplate` as a ChatGPT-specific alias alongside the standard key.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- MIME type: Verify which MIME type ChatGPT actually accepts and update accordingly; may need `text/html;profile=mcp-app` as alias or replacement
- Tool `_meta` format: Unify both code paths (ToolInfo::with_ui and TypedTool::metadata) to use the same format; optionally add `openai/outputTemplate`
- Widget meta in resource `_meta`: Verify which format ChatGPT accepts; may need both for compatibility
- mcp-preview: Fix axum 0.8 wildcard route syntax (`*path` to `{*path}`) on lines 106-108
- Already fixed: `ToolInfo._meta` serde rename (commit 992aa8d)

### Claude's Discretion
- Whether to add `openai/outputTemplate` alongside `ui.resourceUri` in typed tools
- Exact version bump numbers for affected crates
- Whether to update examples to demonstrate both MCP and ChatGPT patterns

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

No new dependencies required. All fixes are internal to existing crates:

| Crate | Current | Purpose | Action |
|-------|---------|---------|--------|
| `pmcp` | workspace | Core SDK types | Fix _meta format, MIME types |
| `mcp-preview` | workspace | Preview server | Fix axum wildcard routes |
| `axum` | 0.8 | HTTP framework (mcp-preview dep) | Already correct version, just fix route syntax |

## Architecture Patterns

### Issue 1: MIME Type Mismatch

**Current SDK state:**
- `UIMimeType::HtmlMcp` -> `"text/html+mcp"`
- `UIMimeType::HtmlSkybridge` -> `"text/html+skybridge"`
- `ExtendedUIMimeType::HtmlMcp` -> `"text/html+mcp"`
- `ExtendedUIMimeType::HtmlSkybridge` -> `"text/html+skybridge"`

**What OpenAI docs say:**
- MCP standard: `text/html;profile=mcp-app` (from official MCP Apps extension docs and OpenAI chatgpt-ui docs)
- No mention of `text/html+skybridge` in current OpenAI docs

**Analysis (HIGH confidence):**
The MCP Apps extension specification (modelcontextprotocol.io/docs/extensions/apps) does NOT specify a MIME type directly in the docs page. However, the OpenAI chatgpt-ui docs reference `text/html;profile=mcp-app` as the profile for MCP App resources. The `+skybridge` variant was likely an earlier internal name.

**Recommendation:**
- Add `text/html;profile=mcp-app` as a new variant (`HtmlMcpApp`)
- Keep `text/html+skybridge` for backward compatibility (some deployed ChatGPT apps may still use it)
- Make `text/html;profile=mcp-app` the default for ChatGPT adapter
- Update `FromStr` to accept the new MIME type
- Consider making `is_chatgpt()` return true for the new profile type

### Issue 2: Tool `_meta` Format Inconsistency (CRITICAL)

**Current SDK state -- TWO DIFFERENT FORMATS for the same concept:**

Path 1: `ToolInfo::with_ui()` (protocol.rs line 329-333):
```rust
meta.insert("ui/resourceUri".to_string(), Value::String(ui_resource_uri.into()));
// Produces: { "_meta": { "ui/resourceUri": "ui://widget/foo.html" } }
```

Path 2: `TypedTool::metadata()` (typed_tool.rs line 230-233):
```rust
meta.insert("ui".to_string(), serde_json::json!({ "resourceUri": uri }));
// Produces: { "_meta": { "ui": { "resourceUri": "ui://widget/foo.html" } } }
```

**What OpenAI docs say (HIGH confidence):**
- MCP standard key: `_meta.ui.resourceUri` (nested)
- ChatGPT alias: `_meta["openai/outputTemplate"]` (flat with slash)
- The MCP Apps spec page explicitly says: "Declare your UI using `_meta.ui.resourceUri`"

**Recommendation:**
- Unify BOTH paths to use nested format: `{ "ui": { "resourceUri": "..." } }`
- Fix `ToolInfo::with_ui()` to match `TypedTool::metadata()`
- Also add `"openai/outputTemplate"` as a sibling key for ChatGPT compatibility
- Update `ToolUIMetadata` struct in `ui.rs` (currently uses flat `"ui/resourceUri"`)

**Code fix for `ToolInfo::with_ui()`:**
```rust
pub fn with_ui(
    name: impl Into<String>,
    description: Option<String>,
    input_schema: Value,
    ui_resource_uri: impl Into<String>,
) -> Self {
    let uri = ui_resource_uri.into();
    let mut meta = serde_json::Map::new();
    meta.insert("ui".to_string(), serde_json::json!({ "resourceUri": &uri }));
    // ChatGPT alias
    meta.insert("openai/outputTemplate".to_string(), Value::String(uri));

    Self {
        name: name.into(),
        description,
        input_schema,
        annotations: None,
        _meta: Some(meta),
        execution: None,
    }
}
```

### Issue 3: Widget Meta in Resource `_meta` (Resource-Level)

**Current SDK state:**
`WidgetMeta` uses flat OpenAI-namespaced keys:
```rust
#[serde(rename = "openai/widgetPrefersBorder")]
pub prefers_border: Option<bool>,
#[serde(rename = "openai/widgetDomain")]
pub domain: Option<String>,
#[serde(rename = "openai/widgetCSP")]
pub csp: Option<WidgetCSP>,
#[serde(rename = "openai/widgetDescription")]
pub description: Option<String>,
```
This produces: `{ "openai/widgetPrefersBorder": true, "openai/widgetDomain": "..." }`

**What OpenAI docs say (HIGH confidence):**
The chatgpt-ui docs show response metadata with flat `openai/*` keys for response-level items like `openai/closeWidget`, `openai/widgetDomain`, `openai/widgetCSP`. But for the `prefersBorder` concept, the docs also reference `ui.prefersBorder` under the MCP standard metadata.

**Analysis:**
The OpenAI docs show a mixed approach:
- **MCP standard fields** use nested `ui` object: `_meta.ui.prefersBorder`, `_meta.ui.resourceUri`
- **ChatGPT-specific fields** use flat `openai/*` keys: `_meta["openai/closeWidget"]`, `_meta["openai/widgetDomain"]`
- ChatGPT accepts BOTH formats for shared fields

**Recommendation:**
- For fields that have MCP standard equivalents (`prefersBorder`), emit BOTH:
  - Nested: `"ui": { "prefersBorder": true }` (MCP standard)
  - Flat: `"openai/widgetPrefersBorder": true` (ChatGPT backward compat)
- For ChatGPT-only fields (`widgetDomain`, `widgetCSP`, `widgetDescription`), keep flat `openai/*` format
- This dual-emit approach ensures compatibility with both standard MCP hosts and ChatGPT

### Issue 4: mcp-preview Axum 0.8 Wildcard Route Panic

**Current state (server.rs lines 106-108):**
```rust
.route("/wasm/*path", get(handlers::wasm::serve_artifact))
.route("/assets/*path", get(handlers::assets::serve))
```

**Problem (HIGH confidence):**
Axum 0.8 changed wildcard route syntax from `*path` to `{*path}`. The old syntax causes a panic at startup.

**Fix:**
```rust
.route("/wasm/{*path}", get(handlers::wasm::serve_artifact))
.route("/assets/{*path}", get(handlers::assets::serve))
```

**Impact:** Handler functions using `Path<String>` extractors should work unchanged since axum 0.8 still extracts the wildcard value the same way.

### Issue 5: TypedTool Missing `openai/outputTemplate`

**Current state:**
`TypedTool::metadata()` emits `{ "ui": { "resourceUri": "..." } }` but does NOT include `openai/outputTemplate`.

**OpenAI docs confirm:**
ChatGPT uses `_meta["openai/outputTemplate"]` as its primary way to find the UI template URI. If only `_meta.ui.resourceUri` is present, ChatGPT may not render the widget.

**Recommendation:**
When `ui_resource_uri` is set on a TypedTool, emit BOTH keys:
```rust
let meta = self.ui_resource_uri.as_ref().map(|uri| {
    let mut meta = serde_json::Map::new();
    meta.insert("ui".to_string(), serde_json::json!({ "resourceUri": uri }));
    meta.insert("openai/outputTemplate".to_string(), Value::String(uri.clone()));
    meta
});
```

### Additional Issue 6: `ToolUIMetadata` in ui.rs Uses Flat Key

**Found during research:**
`ToolUIMetadata` struct uses `#[serde(rename = "ui/resourceUri")]` (flat with slash), which is a third format variant. This struct is used in `from_metadata()` and `to_metadata()` for parsing/producing tool metadata.

**Recommendation:**
Update `ToolUIMetadata` to use nested format. Either:
- Change to manual serialization that produces `{ "ui": { "resourceUri": "..." } }`
- Or deprecate the struct in favor of direct `serde_json::Map` construction (since the struct is relatively thin)

### Additional Issue 7: TypedSyncTool Has No UI Support

**Found during research:**
`TypedSyncTool` has no `ui_resource_uri` field and no `with_ui()` method. If users want UI-enabled sync tools, they cannot use `TypedSyncTool`. Its `metadata()` always returns `_meta: None`.

**Recommendation (Claude's discretion):**
Add `ui_resource_uri` field and `with_ui()` method to `TypedSyncTool` matching `TypedTool`'s implementation. This is a feature gap, not a bug, so it could be deferred.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dual-format metadata | Two separate serialization paths | Single `to_meta_map()` that emits both standard and OpenAI keys | Consistency across all tool types |
| MIME type parsing | String matching in multiple places | `FromStr` implementation with all variants | One parsing path |

## Common Pitfalls

### Pitfall 1: Breaking Backward Compatibility
**What goes wrong:** Changing MIME types or metadata keys breaks existing deployed MCP Apps
**Why it happens:** SDK users have apps in production using current format
**How to avoid:** ADD new formats as primary but keep old formats accepted; use `FromStr` to accept both old and new MIME types
**Warning signs:** Tests that assert exact MIME type strings

### Pitfall 2: Incomplete Dual-Emit
**What goes wrong:** Adding `openai/outputTemplate` to one path but not another
**How to avoid:** Both `ToolInfo::with_ui()` and `TypedTool::metadata()` must emit the same keys. Add a shared helper function.

### Pitfall 3: Resource MIME Type vs Tool MIME Type Confusion
**What goes wrong:** Using ChatGPT MIME type in tool metadata but MCP standard type in resource
**How to avoid:** The MIME type goes on the RESOURCE contents, not in tool `_meta`. Tool `_meta` just has `ui.resourceUri` pointing to the resource.

### Pitfall 4: Axum Path Extractor After Wildcard Change
**What goes wrong:** Axum 0.8's `{*path}` extraction may differ from `*path`
**How to avoid:** Verify handler functions accept the path correctly. The extractor pattern is `Path(path): Path<String>`.

## Code Examples

### Correct Tool `_meta` Output (MCP Standard + ChatGPT)
```json
{
  "name": "chess_move",
  "description": "Make a chess move",
  "inputSchema": { ... },
  "_meta": {
    "ui": {
      "resourceUri": "ui://chess/board.html"
    },
    "openai/outputTemplate": "ui://chess/board.html"
  }
}
```

### Correct Resource `_meta` Output (Dual-Emit)
```json
{
  "uri": "ui://chess/board.html",
  "name": "Chess Board",
  "mimeType": "text/html;profile=mcp-app",
  "_meta": {
    "ui": {
      "prefersBorder": true
    },
    "openai/widgetPrefersBorder": true,
    "openai/widgetDescription": "Interactive chess board"
  }
}
```

### Correct Tool Response with structuredContent
```json
{
  "content": [{ "type": "text", "text": "Move applied" }],
  "structuredContent": { "state": { ... } },
  "_meta": {
    "openai/closeWidget": false,
    "openai/widgetSessionId": "session-123"
  }
}
```

## Files to Modify

| File | Changes | Impact |
|------|---------|--------|
| `src/types/protocol.rs` | `ToolInfo::with_ui()` -- change flat `"ui/resourceUri"` to nested `"ui": { "resourceUri" }` + add `"openai/outputTemplate"` | HIGH -- fixes tool metadata for all non-typed tools |
| `src/server/typed_tool.rs` | `TypedTool::metadata()` -- add `"openai/outputTemplate"` alongside existing nested `"ui"` | HIGH -- fixes ChatGPT compatibility for typed tools |
| `src/types/mcp_apps.rs` | Add `HtmlMcpApp` variant to `ExtendedUIMimeType`; update `FromStr` | MEDIUM -- new MIME type support |
| `src/types/ui.rs` | Update `UIMimeType` and `ToolUIMetadata` to use nested format; add MIME variant | MEDIUM -- consistency |
| `src/types/mcp_apps.rs` | `WidgetMeta` -- add nested `ui` output alongside flat `openai/*` keys | MEDIUM -- resource metadata dual-emit |
| `crates/mcp-preview/src/server.rs` | Lines 106, 108 -- `*path` to `{*path}` | LOW risk -- simple syntax fix |
| `examples/mcp-apps-chess/src/main.rs` | Update MIME type references if changed | LOW -- example alignment |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `text/html+skybridge` | `text/html;profile=mcp-app` | MCP Apps spec 2026 | ChatGPT now uses profile-based MIME |
| Flat `"ui/resourceUri"` in _meta | Nested `"ui": { "resourceUri" }` | MCP Apps spec | Standard metadata format |
| ChatGPT-only metadata keys | Dual standard + ChatGPT keys | MCP Apps spec | Cross-host compatibility |

## Open Questions

1. **Does ChatGPT still accept `text/html+skybridge`?**
   - What we know: OpenAI docs reference `text/html;profile=mcp-app`, no mention of skybridge
   - What's unclear: Whether skybridge is still accepted for backward compat
   - Recommendation: Add new MIME type as primary, keep skybridge as legacy accepted variant

2. **Should `ChatGptToolMeta.output_template` auto-set `ui.resourceUri`?**
   - What we know: They point to the same resource URI
   - Recommendation: Yes -- when `ChatGptToolMeta` is merged into tool `_meta`, it should set both keys

3. **CSP field names: `connect_domains` vs `connectDomains`?**
   - What we know: SDK uses snake_case (`connect_domains`), OpenAI docs show snake_case too
   - Recommendation: No change needed, snake_case is correct per OpenAI docs

## Validation Architecture

> `workflow.nyquist_validation` is not explicitly set to false in config.json, so including this section.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p pmcp --lib types::mcp_apps --lib types::ui -- --test-threads=1` |
| Full suite command | `cargo test --workspace -- --test-threads=1` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A-1 | MIME type parsing accepts new profile variant | unit | `cargo test -p pmcp --lib types::mcp_apps::tests -- --test-threads=1` | Existing tests need update |
| N/A-2 | ToolInfo::with_ui produces nested _meta format | unit | `cargo test -p pmcp --lib types::protocol -- --test-threads=1` | Need new test |
| N/A-3 | TypedTool metadata includes openai/outputTemplate | unit | `cargo test -p pmcp --lib server::typed_tool -- --test-threads=1` | Need new test |
| N/A-4 | WidgetMeta produces both nested ui and flat openai keys | unit | `cargo test -p pmcp --lib types::mcp_apps::tests -- --test-threads=1` | Existing test needs update |
| N/A-5 | mcp-preview routes compile with {*path} syntax | build | `cargo check -p mcp-preview` | Build verification |

### Sampling Rate
- **Per task commit:** `cargo test -p pmcp --lib -- --test-threads=1`
- **Per wave merge:** `cargo test --workspace -- --test-threads=1`
- **Phase gate:** Full suite green before verification

### Wave 0 Gaps
- [ ] Test for `ToolInfo::with_ui()` output format -- no existing test covers _meta structure
- [ ] Test for `TypedTool::metadata()` with `openai/outputTemplate` key
- [ ] Test for new MIME type variant `text/html;profile=mcp-app`

## Sources

### Primary (HIGH confidence)
- OpenAI Apps SDK docs: https://developers.openai.com/apps-sdk/build/chatgpt-ui -- MIME type `text/html;profile=mcp-app`, `_meta.ui.resourceUri`, `openai/outputTemplate` alias
- OpenAI Apps SDK docs: https://developers.openai.com/apps-sdk/mcp-apps-in-chatgpt -- Migration table: `_meta.ui.resourceUri` (standard) = `_meta["openai/outputTemplate"]` (ChatGPT alias)
- MCP Apps extension spec: https://modelcontextprotocol.io/docs/extensions/apps -- Architecture and `_meta.ui.resourceUri` reference
- Source code analysis of: protocol.rs, typed_tool.rs, mcp_apps.rs, ui.rs, adapter.rs, server.rs

### Secondary (MEDIUM confidence)
- OpenAI testing docs: https://developers.openai.com/apps-sdk/deploy/testing -- MCP Inspector workflow

### Tertiary (LOW confidence)
- Whether `text/html+skybridge` is still accepted by ChatGPT (no docs confirm or deny)

## Metadata

**Confidence breakdown:**
- Tool _meta format: HIGH -- OpenAI docs explicitly show nested `ui` and `openai/outputTemplate`
- MIME type: HIGH -- OpenAI docs reference `text/html;profile=mcp-app`
- Resource _meta format: MEDIUM -- OpenAI docs show both flat openai/* and nested ui for different fields
- mcp-preview fix: HIGH -- axum 0.8 breaking change is well documented
- Pitfalls: HIGH -- based on direct source code analysis

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (30 days -- OpenAI Apps SDK is actively evolving)
