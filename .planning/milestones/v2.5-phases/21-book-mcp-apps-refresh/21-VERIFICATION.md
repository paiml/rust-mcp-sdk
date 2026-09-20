---
phase: 21-book-mcp-apps-refresh
verified: 2026-02-27T00:00:00Z
status: passed
score: 4/4 must-haves verified
re_verification: false
---

# Phase 21: Book MCP Apps Refresh Verification Report

**Phase Goal:** Readers of pmcp-book Ch 12.5 can learn the current MCP Apps developer experience including WidgetDir, CLI scaffolding, and multi-platform adapters
**Verified:** 2026-02-27
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | Ch 12.5 documents WidgetDir file-based widget authoring with hot-reload development workflow so readers can author widgets from HTML files | VERIFIED | Lines 235–398: full WidgetDir section covering Convention, API (new/discover/read_widget/inject_bridge_script), Hot-Reload Development, and ResourceHandler Pattern. `discover()` and `read_widget()` match actual `src/server/mcp_apps/widget_fs.rs` signatures exactly. |
| 2  | Ch 12.5 walks through the `cargo pmcp app new/build/preview` developer workflow end-to-end | VERIFIED | Lines 524–710: Developer Workflow section covering all five stages (scaffold, author, run, preview, build). CLI flags match `cargo-pmcp/src/main.rs` (default port 8765 confirmed), `cargo-pmcp/src/commands/app.rs`, and `cargo-pmcp/src/commands/preview.rs`. |
| 3  | Ch 12.5 explains the multi-platform adapter pattern (ChatGPT, MCP Apps, MCP-UI) and bridge communication API | VERIFIED | Lines 712–965: Multi-Platform Adapter Pattern section. UIAdapter trait (5 methods), all three adapters (ChatGptAdapter, McpAppsAdapter, McpUiAdapter), MIME types, and bridge APIs documented in tables extracted from `src/server/mcp_apps/adapter.rs`. |
| 4  | Ch 12.5 references chess, map, and dataviz examples with architecture explanations readers can follow | VERIFIED | Lines 968–1252: Example Walkthroughs section. Tool names (`chess_new_game`, `chess_move`, `chess_valid_moves`, `search_cities`, `get_city_details`, `get_nearby_cities`, `execute_query`, `list_tables`, `describe_table`), ports (3000/3001/3002), and structs (`GameState`, `MapState`) match actual source files. |

**Score:** 4/4 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `pmcp-book/src/ch12-5-mcp-apps.md` | Rewritten chapter with WidgetDir, CLI workflow, adapter pattern, and example walkthroughs | VERIFIED | 1294 lines. Exceeds min_lines of 800 (Plan 21-02). Contains "WidgetDir" (54+ occurrences), "ChatGptAdapter" (57+ adapter-related hits), "discover", "read_widget", "mcpBridge.callTool", "cargo pmcp app new", "hot-reload" all present. No UIResourceBuilder references remain. |

---

### Key Link Verification

#### Plan 21-01 Key Links

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `pmcp-book/src/ch12-5-mcp-apps.md` | `src/server/mcp_apps/widget_fs.rs` | Documented WidgetDir API matches actual struct methods | WIRED | `WidgetDir::new` (line 61), `discover()` returns `std::io::Result<Vec<WidgetEntry>>` (line 79), `read_widget()` returns `String` (line 121), `inject_bridge_script()` (line 153) — all match chapter documentation exactly. |
| `pmcp-book/src/ch12-5-mcp-apps.md` | `cargo-pmcp/src/commands/app.rs` | Documented CLI subcommands match actual AppCommand enum variants | WIRED | `AppCommand` enum has `New`, `Manifest`, `Landing`, `Build` variants (lines 20-68 of app.rs). Chapter documents all four subcommands with correct flags (`--path`, `--url`, `--logo`, `--widget`, `--output`). |

#### Plan 21-02 Key Links

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `pmcp-book/src/ch12-5-mcp-apps.md` | `src/server/mcp_apps/adapter.rs` | Documented adapter trait and implementations match actual Rust source | WIRED | `UIAdapter` trait (5 methods), `ChatGptAdapter`, `McpAppsAdapter`, `McpUiAdapter`, `McpUiFormat` enum all present and match chapter. MIME types `text/html+skybridge`, `text/html+mcp`, `text/html` verified against adapter.rs. |
| `pmcp-book/src/ch12-5-mcp-apps.md` | `examples/mcp-apps-chess/src/main.rs` | Chess example walkthrough references actual source structure | WIRED | `chess_new_game`, `chess_move`, `chess_valid_moves` found in main.rs (lines 513-523). `GameState` struct with board/turn/history/castling/en_passant/status fields matches chapter exactly. Port 3000 confirmed. |
| `pmcp-book/src/ch12-5-mcp-apps.md` | `examples/mcp-apps-map/src/main.rs` | Map example walkthrough references actual source structure | WIRED | `search_cities_handler`, `get_city_details_handler`, `get_nearby_handler` found. Registered as `search_cities`, `get_city_details`, `get_nearby_cities` (lines 419-431). Port 3001 confirmed. `MapState` struct documented. |
| `pmcp-book/src/ch12-5-mcp-apps.md` | `examples/mcp-apps-dataviz/src/main.rs` | Dataviz example walkthrough references actual source structure | WIRED | `execute_query_handler`, `list_tables_handler`, `describe_table_handler` found. Registered as `execute_query`, `list_tables`, `describe_table` (lines 320-332). Port 3002 confirmed. SQL injection prevention code matches exactly. |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| BKAP-01 | 21-01 | Ch 12.5 updated with WidgetDir file-based authoring pattern and hot-reload development workflow | SATISFIED | Lines 235-398: WidgetDir section (Convention, API, Hot-Reload, ResourceHandler). API details match `widget_fs.rs`. REQUIREMENTS.md marks as `[x]`. |
| BKAP-02 | 21-01 | Ch 12.5 updated with `cargo pmcp app new/build/preview` developer workflow | SATISFIED | Lines 524-710: Developer Workflow section (scaffold, preview, build, project detection). All flags verified against cargo-pmcp source. REQUIREMENTS.md marks as `[x]`. |
| BKAP-03 | 21-02 | Ch 12.5 updated with multi-platform adapter pattern (ChatGPT, MCP Apps, MCP-UI) and bridge API | SATISFIED | Lines 712-965: Multi-Platform Adapter Pattern section. UIAdapter trait, all three adapters, bridge API tables, MultiPlatformResource. REQUIREMENTS.md marks as `[x]`. |
| BKAP-04 | 21-02 | Ch 12.5 references current examples (chess, map, dataviz) with architecture explanations | SATISFIED | Lines 968-1252: Example Walkthroughs section. All three examples with tool names, port numbers, key types, code excerpts, running instructions, and Common Architecture Pattern. REQUIREMENTS.md marks as `[x]`. |

**Orphaned requirements:** None. All BKAP-01 through BKAP-04 are claimed by Phase 21 plans and implemented.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | None found | — | — |

No placeholder text, TODO/FIXME comments, empty implementations, or stub sections detected. The continuation marker `<!-- CONTINUED IN PLAN 21-02 -->` was correctly removed by Plan 21-02 before appending the adapter and example sections.

---

### Human Verification Required

The following items cannot be verified programmatically and may warrant human review:

#### 1. Bridge API Accuracy vs. Injected JavaScript

**Test:** Read `src/server/mcp_apps/adapter.rs` `inject_bridge()` methods and compare the full JavaScript source against the mcpBridge API tables in the chapter (lines 409-450 and 810-834).
**Expected:** All method names, property names, and descriptions in the tables exactly match the JavaScript in the injected bridge scripts.
**Why human:** The bridge script is a large JavaScript string embedded in Rust. Automated grep can confirm names are present but cannot validate the full semantic accuracy of every table row.

#### 2. Chapter Reading Flow

**Test:** Read Ch 12.5 end-to-end as a new developer would, following the Quick Start in Section 2.
**Expected:** A developer with basic Rust knowledge can scaffold a project, write a widget, run the preview, and understand the output without consulting any other documentation.
**Why human:** Prose clarity, conceptual ordering, and pedagogical effectiveness cannot be assessed with file scanning.

#### 3. mdbook Build

**Test:** Run `mdbook build pmcp-book` from the repository root.
**Expected:** Build completes without broken links or syntax errors. Ch 12.5 renders correctly in the browser.
**Why human:** Requires the mdbook binary and browser inspection. The SUMMARY.md claims `mdbook builds successfully` but this was not verified in the automated check.

---

### Gaps Summary

No gaps. All four observable truths are verified. All key links between the chapter and actual source code are confirmed wired. All four BKAP requirements are satisfied with substantive, source-faithful content. The chapter is 1294 lines with 9 major sections and 26 subsections covering the complete MCP Apps developer experience.

---

_Verified: 2026-02-27_
_Verifier: Claude (gsd-verifier)_
