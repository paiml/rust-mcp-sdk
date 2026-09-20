---
phase: 44-improving-mcp-preview-to-support-chatgpt-version
verified: 2026-03-28T05:00:00Z
status: passed
score: 14/14
re_verification: true
human_verified: 2026-03-28
human_verification:
  - test: "Start MCP server with widgets, run cargo pmcp preview --url http://localhost:3000 --mode chatgpt --open"
    expected: "Terminal banner shows ChatGPT Strict mode, browser badge shows red ChatGPT Strict"
    why_human: "Visual UI rendering and styling cannot be verified programmatically"
  - test: "Click Protocol tab in DevTools, call a tool, inspect validation results"
    expected: "Pass/fail checks for tools/list, resources/list, tools/call with expandable key diffs"
    why_human: "Interactive browser behavior and correct rendering of validation results"
  - test: "In ChatGPT mode, open browser console in widget iframe and check window.openai"
    expected: "window.openai exists with toolOutput, toolInput, theme, callTool properties"
    why_human: "Runtime JavaScript object injection requires live browser verification"
  - test: "Restart with --mode standard (or no --mode), verify standard mode works unchanged"
    expected: "Green Standard badge, Protocol tab shows informational checks, mcpBridge works normally"
    why_human: "Regression testing of standard mode behavior requires live interaction"
---

# Phase 44: Improving mcp-preview to Support ChatGPT Version Verification Report

**Phase Goal:** Add --mode chatgpt flag to mcp-preview enabling strict ChatGPT protocol validation, postMessage emulation with window.openai stub, and a Protocol diagnostics tab in DevTools
**Verified:** 2026-03-08T23:45:00Z
**Status:** human_needed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | PreviewMode enum exists with Standard and ChatGpt variants | VERIFIED | `crates/mcp-preview/src/server.rs` L21-27: enum with `#[default] Standard` and `ChatGpt` |
| 2 | PreviewConfig has a mode field that defaults to Standard | VERIFIED | `server.rs` L59: `pub mode: PreviewMode`, Default impl L71: `mode: PreviewMode::default()` |
| 3 | CLI accepts --mode standard or --mode chatgpt flag | VERIFIED | `cargo-pmcp/src/main.rs` L215-216: `#[arg(long, default_value = "standard")] mode: String` |
| 4 | GET /api/config returns mode, descriptor_keys, and invocation_keys | VERIFIED | `handlers/api.rs` L18-20: fields on ConfigResponse; L30-42: populated with 4 descriptor + 2 invocation keys |
| 5 | ResourceInfo and ResourceContentItem deserialize _meta from MCP server | VERIFIED | `proxy.rs` L114-115 (ResourceInfo), L128-129 (ResourceContentItem), L136-137 (ResourceReadResult) |
| 6 | Terminal banner displays active mode prominently | VERIFIED | `server.rs` L161-166: Mode line in banner with ChatGPT red / Standard green |
| 7 | Protocol tab appears in DevTools alongside State, Console, Network, Events tabs | VERIFIED | `index.html` L1015: Protocol tab button; L1048-1051: Protocol tab content section |
| 8 | Protocol tab validates tools/list, resources/list, tools/call, resources/read responses | VERIFIED | `index.html` L1177, L1202, L1308, L1612: validateProtocol calls after each MCP method |
| 9 | Protocol tab shows pass/fail per check with expandable details and key diffs | VERIFIED | `index.html` L2106-2170: validateProtocol + renderProtocolTab with expandable details, missing/extra key diffs |
| 10 | In ChatGPT mode, widget iframe receives postMessage with JSON-RPC ui/notifications/tool-result envelope | VERIFIED | `index.html` L1644-1649: postMessage with jsonrpc/method/params envelope |
| 11 | In ChatGPT mode, widget iframe has window.openai stub injected before widget-runtime.mjs | VERIFIED | `index.html` L1809-1821: openaiStub with toolOutput, toolInput, theme, callTool before module script |
| 12 | In Standard mode, existing mcpBridge pattern continues to work unchanged | VERIFIED | `index.html` L1633-1640: AppBridge delivery always active; ChatGPT postMessage is supplemental only |
| 13 | Mode badge is visible in the browser UI header area | VERIFIED | `index.html` L927: mode-badge span; L1142-1148: badge text/class set from config |
| 14 | Validation is warn-only -- never blocks tool execution or widget rendering | VERIFIED | `index.html` L2103: comment "warn-only, never blocks"; validateProtocol only pushes results, no throws/returns |

**Score:** 14/14 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/mcp-preview/src/server.rs` | PreviewMode enum, mode field, banner | VERIFIED | 190 lines, enum + config + banner all present |
| `crates/mcp-preview/src/proxy.rs` | _meta fields on resource structs | VERIFIED | 447 lines, _meta on ResourceInfo, ResourceContentItem, ResourceReadResult |
| `crates/mcp-preview/src/handlers/api.rs` | ConfigResponse with mode/keys, _meta in handlers | VERIFIED | 320 lines, ConfigResponse fields + _meta in list_resources json |
| `crates/mcp-preview/src/lib.rs` | Export PreviewMode | VERIFIED | L26: `pub use server::{PreviewConfig, PreviewMode, PreviewServer}` |
| `cargo-pmcp/src/main.rs` | --mode CLI argument | VERIFIED | L215-216: mode arg, L386-397: wired through to execute() |
| `cargo-pmcp/src/commands/preview.rs` | Mode threading to PreviewConfig | VERIFIED | L18-22: parse mode string, L57: set on config |
| `crates/mcp-preview/assets/index.html` | Protocol tab, mode badge, ChatGPT postMessage, validation | VERIFIED | 2200 lines (>= 1900 min), all features present |
| `crates/mcp-preview/assets/widget-runtime.mjs` | ChatGPT mode wrapWidgetHtml | VERIFIED | 1524 lines (>= 1524 min) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `cargo-pmcp/src/main.rs` | `commands/preview.rs` | mode string passed to execute() | WIRED | L397: `mode` passed as arg; L15: `mode: String` param |
| `commands/preview.rs` | `server.rs` | PreviewConfig.mode field | WIRED | L57: `mode: preview_mode` on config struct |
| `server.rs` | `handlers/api.rs` | state.config.mode in get_config | WIRED | L30: `mode: state.config.mode.to_string()` |
| `index.html init()` | `/api/config` | fetch on startup | WIRED | L1117: `fetch('/api/config')`, L1137-1139: stores mode and keys |
| `index.html handleToolResponse` | widget iframe | postMessage with ui/notifications/tool-result | WIRED | L1644-1649: conditional on `this.mode === 'chatgpt'` |
| `index.html Protocol tab` | validateProtocol | called after tools/list, resources/list, tools/call, resources/read | WIRED | L1177, L1202, L1308, L1612: calls validateProtocol at each point |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| P44-MODE | 44-01 | PreviewMode enum and --mode CLI flag | VERIFIED | Enum in server.rs, CLI arg in main.rs, threading in preview.rs |
| P44-CONFIG | 44-01 | ConfigResponse with mode and ChatGPT keys | VERIFIED | api.rs ConfigResponse with mode, descriptor_keys, invocation_keys |
| P44-RESOURCEMETA | 44-01 | _meta passthrough on proxy resource structs | VERIFIED | proxy.rs _meta on ResourceInfo, ResourceContentItem, ResourceReadResult |
| P44-PROTOCOL-TAB | 44-02 | Protocol diagnostics tab in DevTools | VERIFIED | index.html Protocol tab with validation, pass/fail, expandable diffs |
| P44-CHATGPT-EMULATION | 44-02 | ChatGPT postMessage emulation + window.openai stub | VERIFIED | index.html postMessage + openaiStub injection |
| P44-BADGE | 44-02 | Mode badge in browser UI header | VERIFIED | index.html mode-badge element with green/red styling |

Note: P44 requirement IDs are not listed in REQUIREMENTS.md (no entries found). They are declared in ROADMAP.md and plan frontmatter only.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No TODO/FIXME/PLACEHOLDER/HACK comments found in any modified file |

### Human Verification Required

### 1. Protocol Tab Visual and Interactive Behavior

**Test:** Start an MCP server with widgets, run `cargo pmcp preview --url http://localhost:3000 --mode chatgpt --open`. Click Protocol tab, call a tool, inspect results.
**Expected:** Protocol tab shows pass/fail checks with expandable details showing expected vs actual keys, missing/extra key diffs. Red severity in ChatGPT mode.
**Why human:** Interactive browser UI behavior, click-to-expand, styling, and real-time validation rendering.

### 2. ChatGPT Mode postMessage and window.openai Stub

**Test:** In ChatGPT mode, open browser DevTools console in the widget iframe. Check `window.openai` object.
**Expected:** `window.openai` exists with `toolOutput`, `toolInput`, `theme`, `callTool` properties. Widget receives data via postMessage.
**Why human:** Runtime JavaScript object injection and postMessage delivery require live browser verification.

### 3. Standard Mode Regression

**Test:** Restart with `--mode standard` (or no --mode flag). Verify existing widget behavior.
**Expected:** Green "Standard" badge, Protocol tab shows informational (yellow) checks, mcpBridge/AppBridge works normally, no behavior change from pre-phase-44.
**Why human:** Regression testing of default mode requires live interaction with widgets.

### 4. Terminal Banner Mode Display

**Test:** Start preview in both modes and observe terminal output.
**Expected:** Banner shows "ChatGPT Strict" in red or "Standard" in green depending on mode.
**Why human:** Terminal ANSI color rendering varies by terminal emulator.

### Gaps Summary

No gaps found. All 14 observable truths are verified at the code level. All 6 requirement IDs (P44-MODE, P44-CONFIG, P44-RESOURCEMETA, P44-PROTOCOL-TAB, P44-CHATGPT-EMULATION, P44-BADGE) are satisfied by substantive implementations that are properly wired end-to-end.

The phase requires human verification to confirm visual/interactive behavior in the browser, but all automated checks pass.

---

_Verified: 2026-03-08T23:45:00Z_
_Verifier: Claude (gsd-verifier)_
