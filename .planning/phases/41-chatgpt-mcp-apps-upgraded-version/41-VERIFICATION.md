---
phase: 41-chatgpt-mcp-apps-upgraded-version
verified: 2026-03-07T06:26:27Z
status: passed
score: 11/11 must-haves verified
re_verification: false
---

# Phase 41: ChatGPT MCP Apps Upgraded Version — Verification Report

**Phase Goal:** Align SDK types, bridge protocol, and scaffold template with ChatGPT's official MCP Apps protocol -- add _meta to Content::Resource, fix MIME type, update bridge method names, fix scaffold
**Verified:** 2026-03-07T06:26:27Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | `Content::Resource` variant has an optional `_meta` field that serializes as `_meta` | VERIFIED | `src/types/protocol.rs:634` — `#[serde(rename = "_meta", skip_serializing_if = "Option::is_none")] meta: Option<...>` |
| 2  | `UIResourceContents` has an optional `_meta` field for resource-level widget metadata | VERIFIED | `src/types/ui.rs:205-206` — `#[serde(rename = "_meta", skip_serializing_if = "Option::is_none")] pub meta: Option<...>` |
| 3  | `ChatGptAdapter.preferred_mime_type()` returns `HtmlMcpApp` not `HtmlSkybridge` | VERIFIED | `src/types/mcp_apps.rs:1030` — `Self::ChatGpt => ExtendedUIMimeType::HtmlMcpApp` |
| 4  | `ChatGptAdapter.mime_type()` (UIAdapter impl) returns `HtmlMcpApp` not `HtmlSkybridge` | VERIFIED | `src/server/mcp_apps/adapter.rs:91-92` — `fn mime_type(&self) -> ExtendedUIMimeType { ExtendedUIMimeType::HtmlMcpApp }` |
| 5  | All existing `Content::Resource` construction sites compile with `meta: None` | VERIFIED | `simple_resources.rs:42,62`, `conversion.rs:63,103`, `lib.rs:203` all set `meta: None`; `mcp_client.rs` uses `..` in destructuring |
| 6  | Widget sends `ui/initialize` (not `ui/ready`) for handshake | VERIFIED | `widget-runtime.mjs:975` — `this._transport.send("ui/initialize", ...)` |
| 7  | Host sends `ui/notifications/tool-input` and `ui/notifications/tool-result` to widget | VERIFIED | `widget-runtime.mjs:1195,1207` — AppBridge uses official notification names |
| 8  | Widget sends `ui/message` (not `ui/sendMessage`) and `ui/open-link` (not `ui/openLink`) | VERIFIED | `widget-runtime.mjs:1026,1042` — App class outgoing methods use official names |
| 9  | Host sends `ui/resource-teardown` (not `ui/teardown`) to widget | VERIFIED | `widget-runtime.mjs:1229` — `this._transport.notify("ui/resource-teardown")` |
| 10 | AppBridge accepts both old and new method names for backward compat | VERIFIED | `widget-runtime.mjs:1117,1123,1128,1134,1144,1278,1282` — fall-through switch cases with `// backward compat` |
| 11 | Server-injected bridge script sends `ui/initialize` (not `ui/ready`) | VERIFIED | `adapter.rs:451` — `window.mcpBridge.notify('ui/initialize', {})` |
| 12 | Scaffold template uses `HtmlMcpApp` MIME type (not `HtmlSkybridge`) | VERIFIED | `cargo-pmcp/src/templates/mcp_app.rs:189,212` — both `read()` and `list()` use `HtmlMcpApp`; test at line 619-620 asserts this |
| 13 | Scaffold template calls `.with_ui()` on the tool | VERIFIED | `cargo-pmcp/src/templates/mcp_app.rs:235` — `.with_ui("ui://app/hello.html")`; test at line 622 asserts `.with_ui(` present |
| 14 | Scaffold template emits `_meta` on `Content::Resource` in resource read handler | VERIFIED | `cargo-pmcp/src/templates/mcp_app.rs:190` — `meta: Some(widget_meta.to_meta_map())`; test at line 625-626 asserts presence |
| 15 | `AppBridge` sends `ui/notifications/initialized` after `ui/initialize` handshake | VERIFIED | `widget-runtime.mjs:1264` — `this._transport.notify("ui/notifications/initialized", {})` via setTimeout(0) |

**Score:** 15/15 truths verified (11 unique plan must-haves, expanded to all plan truths across 3 plans)

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/types/protocol.rs` | `Content::Resource` with `_meta` field | VERIFIED | Line 634: `#[serde(rename = "_meta")] meta: Option<serde_json::Map<...>>` |
| `src/types/ui.rs` | `UIResourceContents` with `_meta` field | VERIFIED | Line 205-206: field `meta` with serde rename to `_meta` |
| `src/types/mcp_apps.rs` | `ChatGpt` preferred MIME type returns `HtmlMcpApp` | VERIFIED | Line 1030: match arm `Self::ChatGpt => ExtendedUIMimeType::HtmlMcpApp` |
| `src/server/mcp_apps/adapter.rs` | `ChatGptAdapter::mime_type()` returns `HtmlMcpApp`; inject_bridge sends `ui/initialize` | VERIFIED | Line 91-92: mime_type returns HtmlMcpApp; line 451: inject_bridge sends ui/initialize |
| `crates/mcp-preview/assets/widget-runtime.mjs` | Updated App class and AppBridge with correct protocol names | VERIFIED | Contains `ui/notifications/tool-input` (line 1116, 1195), `ui/initialize` (line 975), `ui/message` (1026), `ui/open-link` (1042), `ui/resource-teardown` (1229) |
| `crates/mcp-preview/assets/index.html` | Updated AppBridge host-side (actually a no-op — AppBridge lives in widget-runtime.mjs) | VERIFIED | Confirmed: index.html imports AppBridge from widget-runtime.mjs; no protocol strings duplicated in index.html |
| `cargo-pmcp/src/templates/mcp_app.rs` | Updated scaffold with `HtmlMcpApp`, `.with_ui()`, and `_meta` | VERIFIED | Lines 189-190, 212, 235: all three changes present; test assertions at lines 619-626 confirm |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/types/protocol.rs` | `src/server/simple_resources.rs` | `Content::Resource { ... meta: None }` | WIRED | `simple_resources.rs:42,62` set `meta: None`; line 315 passes `meta: contents.meta.clone()` for UI reads |
| `src/types/mcp_apps.rs` | `src/types/ui.rs` | `WidgetMeta.to_meta_map()` usable for resource `_meta` | WIRED | `to_meta_map()` exists at `mcp_apps.rs:341,549,636`; scaffold template uses `widget_meta.to_meta_map()` at line 190 |
| `src/server/mcp_apps/adapter.rs` | `src/types/mcp_apps.rs` | `ChatGptAdapter::mime_type()` returns `ExtendedUIMimeType::HtmlMcpApp` | WIRED | `adapter.rs:92` returns `HtmlMcpApp`; test at line 639 asserts `HtmlMcpApp` |
| `crates/mcp-preview/assets/widget-runtime.mjs` | `crates/mcp-preview/assets/index.html` | postMessage JSON-RPC bridge with `ui/initialize` | WIRED | `widget-runtime.mjs:975` App sends `ui/initialize`; `index.html:916-917` imports AppBridge from widget-runtime.mjs; `widget-runtime.mjs:1259` AppBridge handles `ui/initialize` |
| `src/server/mcp_apps/adapter.rs` | `crates/mcp-preview/assets/index.html` | `inject_bridge()` script sends `ui/initialize` which host handles | WIRED | `adapter.rs:451` inject_bridge sends `ui/initialize`; host AppBridge at `widget-runtime.mjs:1259` handles it |
| `cargo-pmcp/src/templates/mcp_app.rs` | `src/types/protocol.rs` | `Content::Resource` with `meta` field | WIRED | Template at line 190 uses `meta: Some(widget_meta.to_meta_map())`; test at line 625-626 asserts pattern |
| `cargo-pmcp/src/templates/mcp_app.rs` | `src/types/mcp_apps.rs` | `ExtendedUIMimeType::HtmlMcpApp` | WIRED | Template at lines 189, 212 reference `HtmlMcpApp`; test at line 619-620 asserts no `HtmlSkybridge` |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| P41-01 | 41-01-PLAN.md | Add `_meta` field to `Content::Resource` | SATISFIED | `protocol.rs:634` — `meta: Option<serde_json::Map<...>>` with serde rename `_meta`; 4 tests at lines 2120-2190 pass |
| P41-02 | 41-01-PLAN.md | Add `_meta` field to `UIResourceContents` | SATISFIED | `ui.rs:205-206` — `pub meta: Option<...>` with serde rename `_meta`; `html()` constructor sets `meta: None` |
| P41-03 | 41-01-PLAN.md | Fix `ChatGptAdapter` MIME type to `HtmlMcpApp` in both `mcp_apps.rs` and `adapter.rs` | SATISFIED | `mcp_apps.rs:1030` and `adapter.rs:91-92` both return `HtmlMcpApp`; `HtmlSkybridge` retained with deprecation doc; `supports_mime_type` accepts both |
| P41-04 | 41-02-PLAN.md | Align bridge protocol method names with official ChatGPT MCP Apps spec | SATISFIED | `widget-runtime.mjs` uses official names as primary, old names as fall-through backward compat; `adapter.rs:451` sends `ui/initialize` not `ui/ready`; `ui/notifications/initialized` sent post-handshake |
| P41-05 | 41-03-PLAN.md | Update `cargo pmcp new --mcp-app` scaffold template for ChatGPT compatibility | SATISFIED | `mcp_app.rs:189,212` use `HtmlMcpApp`; line 235 adds `.with_ui()`; line 190 emits `meta: Some(widget_meta.to_meta_map())`; all cargo-pmcp tests pass |

**Note:** Requirement IDs P41-01 through P41-05 are referenced in ROADMAP.md (line 294) but are NOT defined in REQUIREMENTS.md with full descriptions. The ROADMAP is the authoritative source of record for these IDs. This is a documentation gap (no descriptions in REQUIREMENTS.md) but does not affect goal achievement — all functionality specified in the plans has been implemented.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/server/mcp_apps/adapter.rs` | 219, 227 | `return null` | INFO | These are inside a JavaScript string literal (injected bridge script), not Rust stubs. Represent valid fallback behavior when `window.openai` API is absent. Not a code anti-pattern. |
| `cargo-pmcp/src/templates/mcp_app.rs` | 399 | `placeholder` | INFO | HTML `<input placeholder="...">` attribute in scaffold widget HTML. Not a code placeholder. Not an anti-pattern. |

No blocker or warning-level anti-patterns found.

---

### Commit Verification

All three plan commits exist in git history:

| Commit | Plan | Description |
|--------|------|-------------|
| `5ce9db9` | 41-01 | feat: add `_meta` field to `Content::Resource` and `UIResourceContents` |
| `63815ea` | 41-01 | feat: fix `ChatGptAdapter` MIME type from `HtmlSkybridge` to `HtmlMcpApp` |
| `ea0f766` | 41-02 | feat: update bridge protocol method names in `widget-runtime.mjs` and `adapter.rs` |
| `3aecf2f` | 41-03 | feat: update MCP App scaffold template for ChatGPT compatibility |

---

### Quality Gate Results

| Check | Result |
|-------|--------|
| `cargo clippy --features full -- -D warnings` | PASSED — zero warnings |
| `cargo build -p mcp-preview` | PASSED — builds successfully |
| `cargo test --features full --lib types::protocol::tests -- content_resource_meta` | PASSED — 4 tests (serialization, no-meta serialization, deserialization, backward compat) |
| `cargo test -p cargo-pmcp` | PASSED — 7 tests + 2 doctests |

---

### Human Verification Required

None required. All must-haves are verifiable via static analysis and automated tests.

The following behaviors are observable but low-risk given the grep and test evidence:

1. **Visual widget rendering in mcp-preview:** The updated AppBridge/App protocol alignment works end-to-end in a browser. Evidence is strong (protocol strings verified, fall-through switch confirmed, initialized notification wired), but browser runtime behavior is not automatically testable.
   - **Test:** Launch `cargo pmcp preview`, open a ChatGPT-compatible MCP App, verify the widget initializes and tool calls flow correctly.
   - **Expected:** Widget sends `ui/initialize`, host responds, `ui/notifications/initialized` received by widget, tool calls dispatch `ui/notifications/tool-input` and `ui/notifications/tool-result`.
   - **Why human:** Browser postMessage bridge cannot be tested headlessly without additional test harness.

---

### Gaps Summary

No gaps found. All 15 observable truths verified. All 5 requirement IDs satisfied. All artifacts exist and are substantive (not stubs). All key links are wired. Zero clippy warnings. Zero blocker anti-patterns.

The only documentation gap is that P41-01 through P41-05 are referenced in ROADMAP.md but lack full descriptions in REQUIREMENTS.md. This does not affect goal achievement and is pre-existing.

---

_Verified: 2026-03-07T06:26:27Z_
_Verifier: Claude (gsd-verifier)_
