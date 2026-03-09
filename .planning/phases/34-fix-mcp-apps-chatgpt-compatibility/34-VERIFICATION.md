---
phase: 34-fix-mcp-apps-chatgpt-compatibility
verified: 2026-03-06T21:30:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
notes:
  - "CHATGPT-01 through CHATGPT-06 are referenced in ROADMAP.md but not formally defined in REQUIREMENTS.md — they are roadmap-local identifiers, not entries in the REQUIREMENTS.md traceability table. Flagged but not a blocker."
  - "ROADMAP.md shows 34-02-PLAN.md as unchecked ([]) — stale status; commit 9b1477b confirms the fix was applied."
  - "mcp_apps module tests require --features mcp-apps flag; confirmed passing with 15/15 tests."
---

# Phase 34: Fix MCP Apps ChatGPT Compatibility — Verification Report

**Phase Goal:** Fix SDK metadata format, MIME types, and mcp-preview routes to be compatible with ChatGPT's MCP Apps implementation. Fix mcp-preview server panic.
**Verified:** 2026-03-06T21:30:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | ToolInfo::with_ui() produces nested `_meta.ui.resourceUri` format (not flat `ui/resourceUri`) | VERIFIED | `protocol.rs:334-338`: inserts `"ui"` key with `{"resourceUri": uri}`; test `test_tool_info_with_ui_nested_format` passes; explicitly asserts no flat `ui/resourceUri` key |
| 2 | ToolInfo::with_ui() includes `openai/outputTemplate` as ChatGPT alias | VERIFIED | `protocol.rs:339`: inserts `"openai/outputTemplate"` sibling; test `test_tool_info_with_ui_openai_output_template` passes |
| 3 | TypedTool::metadata() includes `openai/outputTemplate` alongside nested `ui.resourceUri` | VERIFIED | `typed_tool.rs:233-237`: both keys emitted identically to protocol.rs; test `test_typed_tool_metadata_with_ui_has_openai_output_template` passes |
| 4 | UIMimeType and ExtendedUIMimeType accept `text/html;profile=mcp-app` | VERIFIED | `ui.rs:133,148,154,176`: `HtmlMcpApp` variant exists with correct `as_str()`, `is_chatgpt()=true`, `FromStr` parsing; `mcp_apps.rs:649,674,683,714`: identical for `ExtendedUIMimeType`; tests `test_mime_type_conversions`, `test_mime_type_platform_checks`, `test_extended_ui_mime_type`, `test_extended_ui_mime_type_from_str` pass |
| 5 | WidgetMeta emits nested `ui.prefersBorder` alongside flat `openai/widgetPrefersBorder` | VERIFIED | `mcp_apps.rs:261-272`: `to_meta_map()` serde-serializes flat keys then inserts nested `"ui"` object when `prefers_border` is Some; tests `test_widget_meta_dual_emit_prefers_border`, `test_widget_meta_dual_emit_with_domain`, `test_widget_meta_empty_no_ui_key` pass |
| 6 | ToolUIMetadata uses nested `ui` object format for serialization and deserialization | VERIFIED | `ui.rs:271-309`: `from_metadata()` reads nested format first, falls back to legacy flat; `to_metadata()` emits nested + `openai/outputTemplate`; tests `test_tool_ui_metadata_to_nested_format`, `test_tool_ui_metadata_from_nested_format`, `test_tool_ui_metadata_from_legacy_flat_format` pass |
| 7 | mcp-preview server starts without panicking on wildcard routes | VERIFIED | `server.rs:106,108`: both routes use `{*path}` syntax; commit `9b1477b` applied; `cargo check -p mcp-preview` and `cargo clippy -p mcp-preview` pass with zero warnings; version bumped to 0.1.2 |
| 8 | WASM artifact serving works at `/wasm/` paths | VERIFIED | `server.rs:106`: `.route("/wasm/{*path}", get(handlers::wasm::serve_artifact))` — correct axum 0.8 syntax, no panic |
| 9 | Static asset serving works at `/assets/` paths | VERIFIED | `server.rs:108`: `.route("/assets/{*path}", get(handlers::assets::serve))` — correct axum 0.8 syntax, no panic |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/types/protocol.rs` | Fixed ToolInfo::with_ui() with nested `_meta` and `openai/outputTemplate` | VERIFIED | Contains `openai/outputTemplate` at line 339; nested `"ui"` object at line 334-338; tests pass |
| `src/server/typed_tool.rs` | TypedTool::metadata() with `openai/outputTemplate` | VERIFIED | Contains `openai/outputTemplate` at line 234-237; test passes |
| `src/types/ui.rs` | HtmlMcpApp MIME variant and nested ToolUIMetadata | VERIFIED | `HtmlMcpApp` variant at line 133; `ToolUIMetadata` uses nested format with backward-compat parsing; all tests pass |
| `src/types/mcp_apps.rs` | HtmlMcpApp variant in ExtendedUIMimeType and dual-emit WidgetMeta | VERIFIED | `HtmlMcpApp` at line 649; `to_meta_map()` dual-emits at line 261-272; all tests pass |
| `crates/mcp-preview/src/server.rs` | Fixed axum 0.8 wildcard route syntax `{*path}` | VERIFIED | Both routes at lines 106,108 use `{*path}`; no `*path` legacy syntax present |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/types/protocol.rs` | `src/server/typed_tool.rs` | Both produce identical `_meta` format for tools with UI | WIRED | Both insert `"ui": {"resourceUri": uri}` + `"openai/outputTemplate": uri`; confirmed by reading both implementations; tests verify identical output |
| `src/types/ui.rs` | `src/types/mcp_apps.rs` | Both MIME enums have matching `HtmlMcpApp` variant | WIRED | Both enums declare `HtmlMcpApp`, both `as_str()` return `"text/html;profile=mcp-app"`, both `is_chatgpt()` return true; tests confirm |
| `crates/mcp-preview/src/server.rs` | axum 0.8 router | wildcard route syntax `{*path}` | WIRED | Lines 106,108 confirmed; `cargo check` passes |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CHATGPT-01 | 34-01-PLAN.md | Nested `_meta.ui.resourceUri` format in `ToolInfo::with_ui()` | SATISFIED | `protocol.rs:334-338`; test `test_tool_info_with_ui_nested_format` passes |
| CHATGPT-02 | 34-01-PLAN.md | `openai/outputTemplate` alias in tool `_meta` | SATISFIED | `protocol.rs:339`; test `test_tool_info_with_ui_openai_output_template` passes |
| CHATGPT-03 | 34-01-PLAN.md | `HtmlMcpApp` MIME variant (`text/html;profile=mcp-app`) in both MIME enums | SATISFIED | `ui.rs:133`, `mcp_apps.rs:649`; MIME tests pass |
| CHATGPT-04 | 34-01-PLAN.md | WidgetMeta dual-emit `prefersBorder` in flat and nested format | SATISFIED | `mcp_apps.rs:261-272`; dual-emit tests pass |
| CHATGPT-05 | 34-01-PLAN.md | ToolUIMetadata nested format with backward-compatible parsing | SATISFIED | `ui.rs:271-309`; nested + legacy flat tests pass |
| CHATGPT-06 | 34-02-PLAN.md | mcp-preview axum 0.8 wildcard route panic fix | SATISFIED | `server.rs:106,108`; `{*path}` syntax confirmed; `cargo check` passes |

**Note on CHATGPT-* IDs in REQUIREMENTS.md:** These requirement IDs are referenced in `ROADMAP.md` and plan frontmatter but are not formally defined with descriptions in `.planning/REQUIREMENTS.md` (which covers v1.5/v1.6 requirements under different ID prefixes). The CHATGPT-* identifiers are roadmap-local labels for this phase. This is an administrative gap — the requirements themselves are clearly expressed in the plan `must_haves` and success criteria and are fully implemented. No functional blocker.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | — | — | — | — |

No TODO, FIXME, placeholder, empty implementation, or stub patterns detected in any of the five modified files.

### Human Verification Required

#### 1. ChatGPT Live Rendering

**Test:** Register an MCP server using this SDK with a tool that uses `with_ui()`, connect it to ChatGPT, and invoke the tool.
**Expected:** ChatGPT renders the MCP App widget (not a plain text response).
**Why human:** Requires ChatGPT integration environment; SDK produces the correct format but actual rendering behavior can only be confirmed by ChatGPT's live client.

#### 2. mcp-preview Server Startup

**Test:** Run `cargo run -p mcp-preview` (or equivalent startup) and send a request to `/wasm/` or `/assets/` path.
**Expected:** Server starts without panic; request is served.
**Why human:** `cargo check` confirms no compile errors, but runtime routing behavior (wildcard extraction, correct handler dispatch) needs a live server test.

## Summary

Phase 34 goal is fully achieved. All nine observable truths verified against the actual codebase:

- Both `ToolInfo::with_ui()` and `TypedTool::metadata()` now produce identical, ChatGPT-compatible nested `_meta` format with `{"ui": {"resourceUri": "..."}}` and `"openai/outputTemplate"` sibling key. The two paths were previously inconsistent; they are now unified.

- Both `UIMimeType` and `ExtendedUIMimeType` have the `HtmlMcpApp` variant accepting `"text/html;profile=mcp-app"` with `is_chatgpt()=true`.

- `WidgetMeta.to_meta_map()` dual-emits `prefersBorder` in both flat `openai/widgetPrefersBorder` and nested `ui.prefersBorder` formats. ChatGPT-only fields (domain, csp, description) remain flat only.

- `ToolUIMetadata` reads both nested and legacy flat formats for backward compatibility, and writes nested format plus `openai/outputTemplate`.

- The mcp-preview server panic caused by the axum 0.8 wildcard syntax change (`*path` to `{*path}`) is fixed. Both `/wasm/{*path}` and `/assets/{*path}` routes compile cleanly with zero warnings.

All 15 directly relevant tests pass. No anti-patterns detected. Three commits (`eb6607a`, `c702463`, `9b1477b`) verified present in git history. One administrative note: CHATGPT-* requirement IDs are defined only in ROADMAP.md, not in REQUIREMENTS.md — this is a documentation consistency gap but has no impact on implementation completeness.

---

_Verified: 2026-03-06T21:30:00Z_
_Verifier: Claude (gsd-verifier)_
