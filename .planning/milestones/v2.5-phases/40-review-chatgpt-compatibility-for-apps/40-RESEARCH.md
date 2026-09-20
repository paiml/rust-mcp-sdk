# Phase 40: Review ChatGPT Compatibility for Apps - Research

**Researched:** 2026-03-06
**Domain:** MCP Apps ext-apps spec compatibility, _meta format normalization, capability negotiation
**Confidence:** HIGH

## Summary

Phase 40 is an audit-and-fix phase comparing the PMCP SDK's MCP Apps output against the official `@modelcontextprotocol/ext-apps` v1.x reference implementation. The official reference repo was cloned and analyzed directly. Three concrete gaps were identified between our SDK and the official spec.

The most impactful gap is the **missing legacy flat key `ui/resourceUri`** in `build_meta_map()` -- the official `registerAppTool` normalizes both directions (nested to flat, flat to nested) for backward compatibility with older hosts. The second gap is **`ui.visibility`** -- a new field on `McpUiToolMeta` that controls whether a tool is visible to the model, the app, or both. The third gap is **capability negotiation** via the `extensions` field on client capabilities, which the MCP spec hasn't finalized yet (pending SEP-1724) but the ext-apps SDK already uses.

**Primary recommendation:** Add the legacy flat key to `build_meta_map()`, add `ui.visibility` support, and add nested `ui.csp`/`ui.domain` to `WidgetMeta::to_meta_map()`. Defer capability negotiation until the MCP spec adds `extensions` to `ClientCapabilities`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- The official ext-apps `registerAppTool` sets BOTH `_meta.ui.resourceUri` (nested) AND `_meta["ui/resourceUri"]` (legacy flat key) for backward compat
- `RESOURCE_URI_META_KEY = "ui/resourceUri"` is the legacy constant
- Our SDK currently only emits nested format -- need to also emit the legacy flat key to match official behavior
- The flat key is deprecated but still emitted by the reference implementation
- `RESOURCE_MIME_TYPE = "text/html;profile=mcp-app"` -- our SDK already has this as `HtmlMcpApp` variant (Phase 34). No gap here.
- Official format: `_meta: { ui: { resourceUri: "ui://..." } }` -- our SDK matches (Phase 34)
- ChatGPT also reads `openai/outputTemplate` -- our SDK emits this (Phase 34)
- Need to verify: does the official SDK also emit `openai/outputTemplate`? Or is that ChatGPT-specific?
- `ui.visibility`: `["model"]`, `["app"]`, or `["model", "app"]` -- controls tool visibility. Not in our SDK yet.
- `ui.csp`: `{ resourceDomains: [...], connectDomains: [...] }` -- CSP for widget iframe. We have `WidgetCSP` but using flat `openai/*` keys.
- `ui.domain`: stable CORS origin for widget. We have this in `WidgetMeta` as flat `openai/widgetDomain`.
- `ui.prefersBorder`: boolean layout hint. We dual-emit this (Phase 34).
- Extension ID: `"io.modelcontextprotocol/ui"` for capability negotiation
- `McpUiClientCapabilities` includes `mimeTypes` array
- Our SDK doesn't have capability negotiation for MCP Apps

### Claude's Discretion
- Exact implementation approach for adding legacy flat key (in `build_meta_map` vs separate step)
- Whether to add `ui.visibility` as a new field on `ToolInfo` or as a separate builder method
- How to structure capability negotiation (new trait method, builder option, or runtime check)
- Priority ordering of gaps (what to fix first)

### Deferred Ideas (OUT OF SCOPE)
- Widget runtime library (`packages/widget-runtime/`) API parity with `@modelcontextprotocol/ext-apps` `App` class -- separate phase
- `cargo pmcp app` CLI updates for new spec fields -- separate phase
- E2E testing against ChatGPT live environment -- manual testing, not automatable in CI
</user_constraints>

## Gap Analysis (from ext-apps reference repo)

### Verified by direct code inspection of ext-apps repo

| Gap | Our SDK | Official ext-apps | Priority | Confidence |
|-----|---------|-------------------|----------|------------|
| Legacy flat key `ui/resourceUri` | Not emitted by `build_meta_map()` | `registerAppTool` emits both nested and flat | HIGH | HIGH |
| `ui.visibility` field | `ToolVisibility` exists as `openai/visibility` only | `McpUiToolMeta.visibility: ["model"\|"app"][]` in nested `ui` | MEDIUM | HIGH |
| `ui.csp` nested format | `WidgetCSP` uses flat `openai/widgetCSP` only | `McpUiResourceCsp` in nested `ui.csp` with `frameDomains`, `baseUriDomains` | MEDIUM | HIGH |
| `ui.domain` nested format | `WidgetMeta` uses flat `openai/widgetDomain` only | `McpUiResourceMeta.domain` in nested `ui.domain` | MEDIUM | HIGH |
| `ui.permissions` | Not in our SDK | `McpUiResourcePermissions` (camera, microphone, geolocation, clipboardWrite) | LOW | HIGH |
| `openai/outputTemplate` | We emit it | ext-apps does NOT emit it -- it is ChatGPT-specific, not part of official spec | INFO | HIGH |
| Capability negotiation | No `extensions` field on `ClientCapabilities` | Uses `extensions["io.modelcontextprotocol/ui"]` (pending SEP-1724) | LOW | HIGH |

### Key Finding: `openai/outputTemplate` is ChatGPT-Only

The official ext-apps SDK does NOT emit `openai/outputTemplate`. This is a ChatGPT-specific key that our SDK correctly includes for ChatGPT compatibility. The official spec only uses `_meta.ui.resourceUri` and the legacy `_meta["ui/resourceUri"]`. Our SDK's decision to emit both nested ui and `openai/outputTemplate` is correct for maximum compatibility.

## Architecture Patterns

### Pattern 1: Legacy Flat Key in `build_meta_map()`

**What:** Add `"ui/resourceUri"` key alongside existing nested `"ui": { "resourceUri": ... }` in `ToolUIMetadata::build_meta_map()`.

**Where:** `src/types/ui.rs`, line 343-356

**Current code:**
```rust
pub fn build_meta_map(uri: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut meta = serde_json::Map::with_capacity(2);
    let mut ui_obj = serde_json::Map::with_capacity(1);
    ui_obj.insert("resourceUri".to_string(), serde_json::Value::String(uri.to_string()));
    meta.insert("ui".to_string(), serde_json::Value::Object(ui_obj));
    meta.insert("openai/outputTemplate".to_string(), serde_json::Value::String(uri.to_string()));
    meta
}
```

**Fix:** Change capacity from 2 to 3, add the flat key:
```rust
pub fn build_meta_map(uri: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut meta = serde_json::Map::with_capacity(3);
    let mut ui_obj = serde_json::Map::with_capacity(1);
    ui_obj.insert("resourceUri".to_string(), serde_json::Value::String(uri.to_string()));
    meta.insert("ui".to_string(), serde_json::Value::Object(ui_obj));
    // Legacy flat key for older hosts (deprecated but emitted by official ext-apps SDK)
    meta.insert("ui/resourceUri".to_string(), serde_json::Value::String(uri.to_string()));
    // ChatGPT-specific alias
    meta.insert("openai/outputTemplate".to_string(), serde_json::Value::String(uri.to_string()));
    meta
}
```

**Why single point:** All four typed tool variants (`TypedTool`, `TypedSyncTool`, `TypedToolWithOutput`, `WasmTypedTool`) call `build_ui_meta()` which calls `build_meta_map()`. `ToolInfo::with_ui()` also calls `build_meta_map()`. One fix propagates everywhere.

**Test update:** The existing test `test_tool_ui_metadata_to_nested_format` at line 497 explicitly asserts `ui/resourceUri` is NOT present. This assertion must be inverted.

### Pattern 2: `ui.visibility` as Nested Array

**What:** The official spec uses `visibility: ["model" | "app"][]` inside the `ui` object on tool `_meta`. Our SDK has `ToolVisibility` enum (Public/Private) on `ChatGptToolMeta` with flat `openai/visibility` key.

**Spec types:**
```typescript
// From ext-apps spec.types.ts
type McpUiToolVisibility = "model" | "app";
interface McpUiToolMeta {
    resourceUri?: string;
    visibility?: McpUiToolVisibility[];  // Array, not single value
}
```

**Mapping:**
| Our SDK `ToolVisibility` | Spec equivalent |
|--------------------------|-----------------|
| `Public` (default) | `["model", "app"]` |
| `Private` | `["app"]` |
| (no equivalent) | `["model"]` -- model-only, not callable by app |

**Recommendation:** Add a `visibility` field to `ToolUIMetadata` or as a new builder method on `ToolInfo`. Emit it inside the nested `ui` object alongside `resourceUri`. Keep the existing `ChatGptToolMeta.visibility` (flat `openai/visibility`) for backward compat.

### Pattern 3: Nested CSP/Domain in `WidgetMeta::to_meta_map()`

**What:** `WidgetMeta::to_meta_map()` currently emits `prefersBorder` in both flat (`openai/widgetPrefersBorder`) and nested (`ui.prefersBorder`) formats, but `csp` and `domain` are flat-only. The ext-apps spec defines these as nested `ui.csp` and `ui.domain`.

**Fix:** Extend the dual-emit section of `to_meta_map()` to also emit `csp` and `domain` in the nested `ui` object. The `WidgetCSP` struct already has fields matching the spec (`connect_domains`, `resource_domains`, `frame_domains`). Need to also add `base_uri_domains` to `WidgetCSP` (present in spec, missing from our struct).

**Spec CSP fields vs our WidgetCSP:**
| Spec (`McpUiResourceCsp`) | Our `WidgetCSP` | Status |
|---------------------------|-----------------|--------|
| `connectDomains` | `connect_domains` | Present |
| `resourceDomains` | `resource_domains` | Present |
| `frameDomains` | `frame_domains` | Present |
| `baseUriDomains` | (missing) | Add |

**Spec fields vs our WidgetMeta for nested `ui`:**
| Spec (`McpUiResourceMeta`) | Our `WidgetMeta` | Status |
|---------------------------|------------------|--------|
| `ui.prefersBorder` | `prefers_border` | Dual-emit exists |
| `ui.domain` | `domain` | Flat-only, needs nested |
| `ui.csp` | `csp` | Flat-only, needs nested |
| `ui.permissions` | (missing) | New -- camera, microphone, etc. |

### Pattern 4: Capability Negotiation (Defer)

**What:** The ext-apps SDK defines `EXTENSION_ID = "io.modelcontextprotocol/ui"` and `getUiCapability()` to check if a client supports MCP Apps via the `extensions` field on `ClientCapabilities`. The MCP SDK itself hasn't added `extensions` yet (pending SEP-1724).

**Our `ClientCapabilities`:** Has `sampling`, `elicitation`, `roots`, `experimental` -- no `extensions` field.

**Recommendation:** Defer until the MCP spec adds `extensions`. Adding it now would be speculative and break forward compatibility. The ext-apps SDK itself notes this is "pending SEP-1724". For now, servers can check `experimental` as a workaround if needed.

### Anti-Patterns to Avoid
- **Breaking the test that asserts flat key is absent:** The test was correct at the time but the spec has settled on dual-emit. Update the test, don't skip it.
- **Removing `openai/outputTemplate`:** It's ChatGPT-specific but necessary for ChatGPT compatibility. Keep it.
- **Adding `extensions` to `ClientCapabilities` prematurely:** The field name/structure isn't finalized in the MCP spec.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CSP field serialization | Manual JSON construction | Extend `WidgetCSP` serde and map to nested | Existing struct already has correct field names |
| Visibility mapping | New enum | Extend existing `ToolVisibility` or add array-style | Spec uses string array, map from enum |

## Common Pitfalls

### Pitfall 1: Forgetting `from_metadata()` Symmetry
**What goes wrong:** Adding the flat key to `build_meta_map()` output but `from_metadata()` already reads it as fallback. No pitfall here -- `from_metadata()` already handles both formats.
**How to verify:** Existing test `test_tool_ui_metadata_from_legacy_flat_format` covers this.

### Pitfall 2: `WidgetCSP` Field Name Mismatch with Spec
**What goes wrong:** Our `WidgetCSP` uses `redirect_domains` which doesn't exist in the ext-apps spec's `McpUiResourceCsp`. The spec has `frameDomains`, `baseUriDomains`, `connectDomains`, `resourceDomains` only.
**How to avoid:** Only emit spec-defined fields in the nested `ui.csp` object. Keep `redirect_domains` in the flat `openai/widgetCSP` format only (it's ChatGPT-specific).

### Pitfall 3: Visibility Enum vs Array Mismatch
**What goes wrong:** Our `ToolVisibility` is a single enum (`Public`/`Private`). The spec uses an array `["model", "app"]`. A tool can be visible to model-only (`["model"]`), app-only (`["app"]`), or both (`["model", "app"]`).
**How to avoid:** When emitting nested `ui.visibility`, convert to the array format. When reading, accept both formats.

### Pitfall 4: `deep_merge` Interaction
**What goes wrong:** The `with_meta_entry` method uses `deep_merge`. If a user calls `.with_meta_entry("ui", ...)` after `.with_ui()`, the deep merge correctly preserves both. But if the flat key `ui/resourceUri` is a top-level key (not nested), it won't be affected by deep merge on the `ui` object. This is correct behavior.
**How to verify:** Add a test that chains `with_ui()` then `with_meta_entry("ui", json!({"prefersBorder": true}))` and verify all three keys exist (`ui.resourceUri`, `ui.prefersBorder`, `ui/resourceUri`).

## Code Examples

### Adding Legacy Flat Key (verified against ext-apps registerAppTool)

```rust
// In src/types/ui.rs - ToolUIMetadata::build_meta_map()
pub fn build_meta_map(uri: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut meta = serde_json::Map::with_capacity(3);
    let mut ui_obj = serde_json::Map::with_capacity(1);
    ui_obj.insert(
        "resourceUri".to_string(),
        serde_json::Value::String(uri.to_string()),
    );
    meta.insert("ui".to_string(), serde_json::Value::Object(ui_obj));
    // Legacy flat key (deprecated but emitted by official ext-apps SDK)
    meta.insert(
        "ui/resourceUri".to_string(),
        serde_json::Value::String(uri.to_string()),
    );
    // ChatGPT-specific alias
    meta.insert(
        "openai/outputTemplate".to_string(),
        serde_json::Value::String(uri.to_string()),
    );
    meta
}
```

### Nested CSP in `WidgetMeta::to_meta_map()` (dual-emit extension)

```rust
// In src/types/mcp_apps.rs - WidgetMeta::to_meta_map()
pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
    let mut map = match serde_json::to_value(self).ok() {
        Some(serde_json::Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    // Dual-emit: nested ui object for MCP standard fields
    let mut ui_obj = serde_json::Map::new();
    if let Some(prefers) = self.prefers_border {
        ui_obj.insert("prefersBorder".to_string(), serde_json::Value::Bool(prefers));
    }
    if let Some(domain) = &self.domain {
        ui_obj.insert("domain".to_string(), serde_json::Value::String(domain.clone()));
    }
    if let Some(csp) = &self.csp {
        let mut csp_obj = serde_json::Map::new();
        if !csp.connect_domains.is_empty() {
            csp_obj.insert("connectDomains".into(), serde_json::json!(csp.connect_domains));
        }
        if !csp.resource_domains.is_empty() {
            csp_obj.insert("resourceDomains".into(), serde_json::json!(csp.resource_domains));
        }
        if let Some(frames) = &csp.frame_domains {
            if !frames.is_empty() {
                csp_obj.insert("frameDomains".into(), serde_json::json!(frames));
            }
        }
        if !csp_obj.is_empty() {
            ui_obj.insert("csp".to_string(), serde_json::Value::Object(csp_obj));
        }
    }
    if !ui_obj.is_empty() {
        map.insert("ui".to_string(), serde_json::Value::Object(ui_obj));
    }
    map
}
```

### Visibility Array Emission

```rust
// Potential new ToolVisibility variants or mapping
// Spec: visibility?: ("model" | "app")[]
// Our SDK: ToolVisibility::Public -> ["model", "app"], Private -> ["app"]

/// Emit visibility as nested ui.visibility array
fn visibility_to_json(vis: &[&str]) -> serde_json::Value {
    serde_json::Value::Array(vis.iter().map(|s| serde_json::Value::String(s.to_string())).collect())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat `_meta["ui/resourceUri"]` only | Nested `_meta.ui.resourceUri` + flat for compat | ext-apps v1.x (2026-01) | Must emit both |
| Flat `openai/widgetCSP` only | Nested `ui.csp` with spec-defined fields | ext-apps v1.x (2026-01) | Must dual-emit |
| Single `visibility` enum | Array `["model", "app"]` | ext-apps v1.x (2026-01) | More granular control |
| No capability negotiation | `extensions["io.modelcontextprotocol/ui"]` | Pending SEP-1724 | Defer |

## Priority Ordering (Recommendation)

1. **Legacy flat key in `build_meta_map()`** -- smallest change, highest impact, fixes compatibility with older hosts
2. **Nested `ui.csp` and `ui.domain` in `WidgetMeta::to_meta_map()`** -- extends existing dual-emit pattern
3. **`ui.visibility` support** -- new field, moderate impact
4. **`baseUriDomains` on `WidgetCSP`** -- small addition to existing struct
5. **`ui.permissions`** -- new type, lowest priority (camera/mic/geo rarely needed)
6. **Capability negotiation** -- defer entirely (blocked on MCP spec SEP-1724)

## Open Questions

1. **`ui.permissions` scope**
   - What we know: The spec defines `McpUiResourcePermissions` with `camera`, `microphone`, `geolocation`, `clipboardWrite` fields
   - What's unclear: How many MCP Apps actually use these permissions in practice
   - Recommendation: Add the type but don't prioritize; include as a bonus if time permits

2. **`ToolVisibility` enum refactoring**
   - What we know: Our enum has `Public`/`Private`. The spec uses `["model"]`, `["app"]`, `["model", "app"]`.
   - What's unclear: Whether to add a third variant (`ModelOnly`) to our enum or switch to a different representation
   - Recommendation: Add `ModelOnly` variant and convert to array in the nested `ui.visibility` emission. Keep the flat `openai/visibility` using existing enum for backward compat.

## Sources

### Primary (HIGH confidence)
- Direct code inspection of `@modelcontextprotocol/ext-apps` repo (cloned to `/tmp/ext-apps-ref`)
  - `src/server/index.ts` -- `registerAppTool`, `registerAppResource`, `getUiCapability`, `EXTENSION_ID`
  - `src/spec.types.ts` -- `McpUiToolMeta`, `McpUiResourceMeta`, `McpUiResourceCsp`, `McpUiResourcePermissions`, `McpUiToolVisibility`, `McpUiClientCapabilities`
  - `src/app.ts` -- `RESOURCE_URI_META_KEY = "ui/resourceUri"`, `RESOURCE_MIME_TYPE = "text/html;profile=mcp-app"`

### Code Analysis (HIGH confidence)
- `src/types/ui.rs` -- `ToolUIMetadata::build_meta_map()`, `deep_merge()`, `build_ui_meta()`
- `src/types/mcp_apps.rs` -- `WidgetMeta`, `WidgetCSP`, `ChatGptToolMeta`, `ToolVisibility`
- `src/types/capabilities.rs` -- `ClientCapabilities` (no `extensions` field)
- `src/server/typed_tool.rs` -- all typed tool `with_ui()` implementations

## Metadata

**Confidence breakdown:**
- Gap analysis: HIGH -- verified by direct source code comparison
- Legacy flat key fix: HIGH -- exact behavior read from official `registerAppTool`
- CSP/domain nested format: HIGH -- exact types read from official `McpUiResourceMeta`
- Visibility mapping: HIGH -- exact types read from official `McpUiToolMeta`
- Capability negotiation deferral: HIGH -- ext-apps code itself says "pending SEP-1724"

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (30 days -- ext-apps spec is stable v1.x)
