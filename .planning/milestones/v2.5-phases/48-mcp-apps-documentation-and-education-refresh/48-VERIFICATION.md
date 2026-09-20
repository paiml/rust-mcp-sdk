---
phase: 48-mcp-apps-documentation-and-education-refresh
verified: 2026-03-12T21:00:00Z
status: passed
score: 11/11 must-haves verified
human_verification:
  - test: "Toggle light/dark theme in mcp-preview with an ext-apps widget"
    expected: "Widget visually changes colors when theme toggle is pressed"
    why_human: "Requires browser interaction — CSS variable propagation to iframe cannot be verified programmatically"
  - test: "Run mcp-tester apps against a live MCP App server"
    expected: "Validation output shows pass/warn/fail per check with readable summary"
    why_human: "Requires a running server process"
---

# Phase 48: MCP Apps Documentation and Education Refresh Verification Report

**Phase Goal:** Update all documentation, tooling READMEs, book chapters, and course materials to reflect the current MCP Apps capabilities including multi-host support (ChatGPT, Claude Desktop), mcp-tester apps validation, mcp-preview improvements, and the developer guide. Also fix mcp-preview theme support by sending CSS variable palettes in host context.
**Verified:** 2026-03-12T21:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | mcp-tester README documents the `apps` subcommand with usage examples and validation modes | VERIFIED | 7 occurrences of "mcp-tester apps" in README; --mode, --tool, --strict flags documented; all three modes shown |
| 2 | mcp-preview README describes current capabilities including multi-host preview, widget runtime, and DevTools | VERIFIED | README expanded to 171 lines (was ~40); 4 occurrences of `--mode chatgpt`; Installation, DevTools Tabs, Bridge Modes, Architecture sections present |
| 3 | pmcp-book MCP Apps chapters are updated with current tooling, host layer system, and developer guide content | VERIFIED | ch12-5 (561 lines): 4× `with_host_layer`, 5× `ToolInfo::with_ui`, 12× `ext-apps`, 4× `onteardown`; no deprecated `ChatGptAdapter`; ch15 has App Metadata Testing subsection with cross-reference to ch12-5 |
| 4 | pmcp-course materials are aligned with book updates | VERIFIED | All 6 course files updated; ch20-mcp-apps: 2× `with_host_layer`; ch20-01: 9× `html_mcp_app`; ch20-02: 8× `ToolInfo::with_ui`; ch20-03: 13× `ext-apps`, 9× `onteardown`; ch11-02: 8× `mcp-tester apps`, 3× `cargo pmcp test apps` |
| 5 | mcp-preview sends `styles.variables` CSS custom properties in host context so widgets respond to theme changes | VERIFIED | `THEME_PALETTES` constant at line 1379 (before PreviewRuntime class); `styles.variables` wired at line 2417-2419 (init) and line 2669-2671 (theme toggle via `sendHostContextChanged`) |

**Score:** 5/5 success criteria verified

### Required Artifacts (from Plan must_haves)

**Plan 48-01 Artifacts:**

| Artifact | Status | Evidence |
|----------|--------|----------|
| `crates/mcp-tester/README.md` | VERIFIED | Exists; 7× "mcp-tester apps"; Installation section at line 33; --strict, --tool, --mode documented |
| `crates/mcp-preview/README.md` | VERIFIED | Exists; 171 lines (>100 required); 4× "--mode chatgpt"; Installation at line 21 |
| `pmcp-book/src/ch12-5-mcp-apps.md` | VERIFIED | Exists; 561 lines; 4× "with_host_layer"; standard-first ordering confirmed (ChatGptAdapter absent) |
| `pmcp-book/src/ch15-testing.md` | VERIFIED | Exists; 1497 lines; 2× "cargo pmcp test apps"; "App Metadata" subsection present |
| `.github/workflows/release-preview.yml` | VERIFIED | Exists; 7× "mcp-preview"; triggers on release/workflow_dispatch; matrix: linux-x86_64, macos-x86_64, windows-x86_64; correct package name `mcp-preview` |

**Plan 48-02 Artifacts:**

| Artifact | Status | Evidence |
|----------|--------|----------|
| `pmcp-course/src/part8-advanced/ch20-mcp-apps.md` | VERIFIED | Exists; 168 lines; 2× "with_host_layer"; standard-first framing |
| `pmcp-course/src/part8-advanced/ch20-01-ui-resources.md` | VERIFIED | Exists; 239 lines; 9× "html_mcp_app"; WidgetCSP, UIResourceContents covered |
| `pmcp-course/src/part8-advanced/ch20-02-tool-ui-association.md` | VERIFIED | Exists; 237 lines; 8× "ToolInfo::with_ui"; with_structured_content, with_output_schema present |
| `pmcp-course/src/part8-advanced/ch20-03-postmessage.md` | VERIFIED | Exists; 384 lines; 13× "ext-apps"; 9× "onteardown"; required handler warning block present |
| `pmcp-course/src/part4-testing/ch11-02-mcp-tester.md` | VERIFIED | Exists; 1210 lines; App Metadata Validation section at line 1145; 8× "mcp-tester apps", 3× "cargo pmcp test apps" |
| `pmcp-course/src/part8-advanced/ch20-exercises.md` | VERIFIED | Exists; 40 lines; 2× "mcp-tester apps" |

**Plan 48-03 Artifacts:**

| Artifact | Status | Evidence |
|----------|--------|----------|
| `crates/mcp-preview/assets/index.html` | VERIFIED | `THEME_PALETTES` at line 1379 (3 total references: 1 definition, 2 usages); `styles.variables` at both init (line 2417) and theme toggle (line 2669); light palette: background-primary=#ffffff, text-primary=#1a1a1a; dark palette: background-primary=#1a1a1a, text-primary=#e0e0e0 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `pmcp-book/src/ch12-5-mcp-apps.md` | `src/server/mcp_apps/GUIDE.md` | content alignment | VERIFIED | GUIDE.md has 22× key API terms; ch12-5 covers all: with_host_layer, ToolInfo::with_ui, html_mcp_app, structuredContent |
| `pmcp-book/src/ch15-testing.md` | `pmcp-book/src/ch12-5-mcp-apps.md` | cross-reference | VERIFIED | Line 1464: explicit link `[Chapter 12.5: MCP Apps Extension](ch12-5-mcp-apps.md)` |
| `pmcp-course/src/part8-advanced/ch20-mcp-apps.md` | `pmcp-book/src/ch12-5-mcp-apps.md` | content alignment | VERIFIED | 12× `with_host_layer|ToolInfo::with_ui|ext-apps` in course ch20 files |
| `pmcp-course/src/part4-testing/ch11-02-mcp-tester.md` | `pmcp-book/src/ch15-testing.md` | content alignment | VERIFIED | Both have mcp-tester apps and cargo pmcp test apps with matching validation modes |
| `crates/mcp-preview/assets/index.html` | `ext-apps McpUiStyleVariableKey` | CSS variable names | VERIFIED | Variable names match spec: `--color-background-primary`, `--color-text-primary`, `--font-sans`, `--border-radius-md` etc. present in THEME_PALETTES |
| `sendHostContextChanged` | `THEME_PALETTES` | styles.variables lookup | VERIFIED | `THEME_PALETTES[this.theme] || {}` at both init (line 2418) and theme toggle (line 2670) |

### Requirements Coverage

The requirement IDs DOCS-01, DOCS-02, DOCS-03, DOCS-04, PREVIEW-01 appear in ROADMAP.md but are **not registered** in `.planning/REQUIREMENTS.md` (which tracks v1.5/v1.6 CLI requirements only). These are phase-internal labels, not cross-project tracked requirements. No orphaned requirements exist.

| Requirement | Source Plan | Status | Evidence |
|-------------|-------------|--------|----------|
| DOCS-01 (mcp-tester README) | 48-01 | SATISFIED | README has Installation + apps subcommand section |
| DOCS-02 (mcp-preview README) | 48-01 | SATISFIED | README expanded to 171 lines with multi-host and DevTools |
| DOCS-03 (book ch12-5 + ch15) | 48-01 | SATISFIED | ch12-5 rewritten standard-first; ch15 has App Metadata Testing |
| DOCS-04 (course ch20 + ch11-02) | 48-02 | SATISFIED | All 6 course files updated with current APIs |
| PREVIEW-01 (theme CSS variables) | 48-03 | SATISFIED | THEME_PALETTES wired into init and theme-toggle host context |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/mcp-tester/README.md` | 424, 436, 458, 467 | "TODO:", "Placeholder" | Info | These are intentional teaching content explaining the scenario generator's auto-generated test values — not implementation stubs |
| `pmcp-book/src/ch15-testing.md` | 551, 562 | "TODO:" | Info | Same — illustrating scenario file format with placeholder replacement instructions |

No blocker anti-patterns found.

### Human Verification Required

#### 1. Theme Toggle Visual Test

**Test:** Open mcp-preview pointing at an MCP App server (e.g., open-images), click the light/dark toggle button in the preview chrome
**Expected:** Widget colors visually update — background switches from white to dark, text from dark to light, matching the THEME_PALETTES values
**Why human:** CSS variable propagation into iframe requires browser execution; cannot verify postMessage delivery or widget CSS rendering programmatically

#### 2. mcp-tester apps Live Validation

**Test:** Run `mcp-tester apps http://localhost:3000` against an MCP App server, then again with `--mode chatgpt --strict`
**Expected:** Validation output shows per-check pass/warn/fail with a summary line; chatgpt mode additionally checks openai/* descriptor keys
**Why human:** Requires a running MCP App server process

### Gaps Summary

No gaps. All automated checks pass. All 5 roadmap success criteria verified. All 11 plan artifacts confirmed to exist, be substantive, and be wired. Commits ab11c93, b7bce92, 06f47b7, 86a9611, bf314a8, e7ebe18 all confirmed present in git history.

---

_Verified: 2026-03-12T21:00:00Z_
_Verifier: Claude (gsd-verifier)_
