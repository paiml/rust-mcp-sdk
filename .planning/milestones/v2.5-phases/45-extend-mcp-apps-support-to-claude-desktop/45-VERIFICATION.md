---
phase: 45-extend-mcp-apps-support-to-claude-desktop
verified: 2026-03-28T05:00:00Z
status: passed
score: 7/7 must-haves verified
human_verified: 2026-03-28
human_verification:
  - test: "Start chess example in standard preview mode and verify rendering"
    expected: "Chess board widget renders, pieces interactive, no console errors"
    why_human: "Visual rendering and interactive behavior cannot be verified programmatically"
  - test: "Start map example in standard preview mode and verify rendering"
    expected: "Map widget renders, no console errors"
    why_human: "Visual rendering cannot be verified programmatically"
  - test: "Start chess example in ChatGPT mode (--mode chatgpt) and verify rendering"
    expected: "ChatGPT mode banner shown, widget renders, ChatGPT emulation active"
    why_human: "End-to-end ChatGPT emulation behavior requires visual confirmation"
---

# Phase 45: Extend MCP Apps Support to Claude Desktop -- Verification Report

**Phase Goal:** Extend MCP Apps support to Claude Desktop by making SDK metadata emission standard-only by default, normalizing the widget-runtime bridge to be host-agnostic, and updating mcp-preview to use standard mode as default.
**Verified:** 2026-03-09T20:00:00Z
**Status:** human_needed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | build_meta_map emits only nested ui.resourceUri -- no ui/resourceUri flat key, no openai/outputTemplate | VERIFIED | `src/types/ui.rs` L402-408: build_meta_map creates Map with capacity 1, inserts only "ui" key. Test `test_build_meta_map_emits_standard_key_only` (L656-677) asserts exactly 1 key, absence of flat/openai keys. |
| 2 | ServerCoreBuilder.with_host_layer(HostType::ChatGpt) causes openai/* keys to appear in tool _meta | VERIFIED | `src/server/builder.rs` L550-554: `with_host_layer` method exists with dedup. L809-817: build() iterates host_layers and calls `enrich_meta_for_host`. `src/server/core.rs` L107-131: enrich_meta_for_host extracts ui.resourceUri, inserts openai/outputTemplate and openai/widgetAccessible. Tests at builder.rs L1019-1037. |
| 3 | build_uri_to_tool_meta indexes by ui.resourceUri (standard key) by default, not openai/outputTemplate | VERIFIED | `src/server/core.rs` L151-170: build_uri_to_tool_meta extracts URI via `meta.get("ui").and_then(v.get("resourceUri"))`. Tests: `test_build_uri_to_tool_meta_indexes_by_standard_key` (L1304), `test_build_uri_to_tool_meta_standard_only_no_openai` (L1369). |
| 4 | window.mcpBridge is the canonical developer-facing API with standard methods only at root level | VERIFIED | `packages/widget-runtime/src/types.ts` L420-440: McpBridge interface has callTool, readResource, getPrompt, openExternal, notify, sendIntent, openLink at root. extensions is optional McpBridgeExtensions. |
| 5 | ChatGPT-specific capabilities are available under mcpBridge.extensions.chatgpt namespace | VERIFIED | `packages/widget-runtime/src/types.ts` L354-406: ChatGptExtensions interface with all ChatGPT methods. McpBridgeExtensions has optional chatgpt field. `src/compat.ts` L56-109: buildChatGptExtensions() creates the object. `src/runtime.ts`: _getChatGptExtensions() accessor used by all ChatGPT-specific methods. |
| 6 | mcp-preview --mode standard is the default (no flag needed) | VERIFIED | `crates/mcp-preview/src/server.rs` L21-27: PreviewMode enum with `#[default]` on Standard variant. L62-74: PreviewConfig::default() uses PreviewMode::default(). Banner shows "Standard MCP Apps" for Standard mode (L162-165). |
| 7 | mcp-preview --mode chatgpt still works with ChatGPT strict validation | VERIFIED | `crates/mcp-preview/src/handlers/api.rs`: Mode-aware enrichment via enrich_meta_for_chatgpt() applied to list_tools (L84-87), call_tool (L120-122), list_resources (L215-218). Config response differentiates modes (L29-34). |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/types/ui.rs` | Standard-only emit_resource_uri_keys and build_meta_map | VERIFIED | emit_resource_uri_keys (L358-365) only inserts into ui_obj. build_meta_map (L402-408) returns 1-key map. 15+ tests covering standard-only behavior. |
| `src/server/builder.rs` | with_host_layer method on ServerCoreBuilder | VERIFIED | host_layers field (L79), with_host_layer method (L550-554), build-time enrichment loop (L809-817). Tests at L1019-1037. |
| `src/server/core.rs` | Standard-keyed uri_to_tool_meta index + host-layer enrichment pipeline | VERIFIED | enrich_meta_for_host (L107-131), RESOURCE_PROPAGATION_PREFIXES (L139-143), build_uri_to_tool_meta (L151+) uses ui.resourceUri. 3 dedicated tests. |
| `packages/widget-runtime/src/types.ts` | McpBridge interface with extensions namespace | VERIFIED | ChatGptExtensions (L354-393), McpBridgeExtensions (L401-406), McpBridge with extensions field (L420-440). |
| `packages/widget-runtime/src/compat.ts` | Host-agnostic compat layer with ChatGPT extensions | VERIFIED | buildChatGptExtensions (L56-109), installCompat builds bridge with standard root + extensions namespace (L138-236). |
| `crates/mcp-preview/assets/widget-runtime.mjs` | Built widget-runtime with extensions namespace | VERIFIED | Contains "extensions" references (grep confirmed 4 matches including extensions.chatgpt). |
| `crates/mcp-preview/src/server.rs` | PreviewMode with Standard as default | VERIFIED | L21-27: enum with #[default] Standard. |
| `examples/mcp-apps-chess/preview.html` | Chess example verified in standard mode | VERIFIED (existence) | File exists. Human verification needed for rendering. |
| `examples/mcp-apps-map/preview.html` | Map example verified in standard mode | VERIFIED (existence) | File exists. Human verification needed for rendering. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| builder.rs | core.rs | host_layers passed to enrichment | WIRED | builder.rs L812 iterates host_layers, calls core::enrich_meta_for_host. core.rs L107 implements enrichment. |
| ui.rs | core.rs | build_meta_map produces standard-only; core enriches per host | WIRED | build_meta_map returns 1-key standard map. enrich_meta_for_host reads ui.resourceUri and adds openai/* keys. |
| types.ts | compat.ts | McpBridge interface implemented by compat layer | WIRED | compat.ts imports ChatGptExtensions, McpBridgeExtensions from types.ts (L13). Builds bridge conforming to McpBridge interface. |
| compat.ts | widget-runtime.mjs | Built output from TypeScript sources | WIRED | grep confirms "extensions" in built .mjs output. |
| server.rs (preview) | api.rs handlers | PreviewMode determines enrichment | WIRED | api.rs checks state.config.mode == PreviewMode::ChatGpt before enriching with openai/* keys (L84, L120, L215). |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| P45-STANDARD-DEFAULT | 45-01 | Standard-only metadata emission by default | SATISFIED | build_meta_map returns 1 key. emit_resource_uri_keys standard-only. Tests pass. |
| P45-HOST-LAYER | 45-01 | Host layer system for opt-in host-specific keys | SATISFIED | with_host_layer method, enrich_meta_for_host pipeline, build-time enrichment loop. |
| P45-URI-INDEX | 45-01 | URI index uses standard key | SATISFIED | build_uri_to_tool_meta indexes by ui.resourceUri nested key. |
| P45-BRIDGE-NORMALIZE | 45-02 | Host-agnostic bridge with standard root | SATISFIED | McpBridge has standard methods at root. ChatGPT methods under extensions.chatgpt. |
| P45-EXTENSIONS-NS | 45-02 | Extensions namespace for host-specific capabilities | SATISFIED | ChatGptExtensions interface, McpBridgeExtensions with chatgpt? field, buildChatGptExtensions(). |
| P45-PREVIEW-STANDARD | 45-03 | mcp-preview defaults to standard mode | SATISFIED | PreviewMode::Standard is #[default]. Config response mode-aware. |
| P45-EXAMPLES-VERIFY | 45-03 | Examples verified in standard mode | NEEDS HUMAN | Files exist. Human verification of rendering required. |

**Note:** No P45-* requirement IDs found in REQUIREMENTS.md. These requirements are defined only in the plan frontmatter. No orphaned requirements detected.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No TODO, FIXME, PLACEHOLDER, or stub patterns found in any modified files. |

### Human Verification Required

### 1. Chess Example Standard Mode Rendering

**Test:** Run `cd examples/mcp-apps-chess && cargo pmcp preview` and open browser
**Expected:** Chess board widget renders with interactive pieces, no console errors
**Why human:** Visual rendering and widget interaction cannot be verified programmatically

### 2. Map Example Standard Mode Rendering

**Test:** Run `cd examples/mcp-apps-map && cargo pmcp preview` and open browser
**Expected:** Map widget renders, no console errors
**Why human:** Visual rendering cannot be verified programmatically

### 3. Chess Example ChatGPT Mode

**Test:** Run `cd examples/mcp-apps-chess && cargo pmcp preview --mode chatgpt` and open browser
**Expected:** ChatGPT Strict mode banner shows, widget renders with ChatGPT emulation active
**Why human:** End-to-end ChatGPT emulation behavior needs visual confirmation

### Gaps Summary

No automated gaps found. All 7 observable truths are verified through code inspection. All 7 requirement IDs are satisfied with implementation evidence. All key links are wired. No anti-patterns detected. The commits (8ca158b through 066f406) form a coherent TDD progression.

The only remaining verification is human confirmation that examples render correctly in both standard and ChatGPT preview modes (P45-EXAMPLES-VERIFY).

---

_Verified: 2026-03-09T20:00:00Z_
_Verifier: Claude (gsd-verifier)_
