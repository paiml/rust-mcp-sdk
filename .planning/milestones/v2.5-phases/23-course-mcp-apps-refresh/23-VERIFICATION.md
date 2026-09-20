---
phase: 23-course-mcp-apps-refresh
verified: 2026-02-27T00:00:00Z
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 23: Course MCP Apps Refresh — Verification Report

**Phase Goal:** Course learners can follow updated Ch 20 sub-chapters that teach the current MCP Apps workflow with hands-on examples
**Verified:** 2026-02-27
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | Learner can scaffold a new MCP Apps project using `cargo pmcp app new` and run it with `cargo pmcp preview` | VERIFIED | ch20-mcp-apps.md Quick Start (steps 1-3); ch20-01-ui-resources.md full scaffolding walkthrough with 4 steps including run/preview commands |
| 2  | Learner understands WidgetDir file-based convention: `.html` files in `widgets/` map to `ui://app/{name}` URIs | VERIFIED | ch20-01-ui-resources.md "The WidgetDir Convention" section with file-to-URI mapping table and API walkthrough (construction, discover, read_widget, inject_bridge_script) |
| 3  | Learner can write a widget using `window.mcpBridge.callTool()` as the universal bridge API | VERIFIED | ch20-02-tool-ui-association.md "The Bridge API" section with hands-on code examples; ch20-03-postmessage.md shows all three examples using mcpBridge throughout |
| 4  | Learner understands bridge communication flow: widget calls mcpBridge -> bridge script -> host -> MCP server -> response | VERIFIED | ch20-02-tool-ui-association.md "Communication Flow" section with full ASCII diagram tracing each step; platform differences (window.openai vs postMessage) explained |
| 5  | Learner knows the three adapters (ChatGptAdapter, McpAppsAdapter, McpUiAdapter) and when to use each | VERIFIED | ch20-02-tool-ui-association.md "The Adapter Pattern" section with individual adapter sections, comparison table, and "Choosing an Adapter" guidance |
| 6  | Learner can implement the ResourceHandler pattern connecting WidgetDir to ChatGptAdapter | VERIFIED | ch20-01-ui-resources.md "The ResourceHandler Pattern" section with complete read() and list() implementations and 3-step table; ch20-03-postmessage.md "The Common Pattern" reinforces it as the 4-step reusable architecture |
| 7  | Ch 20 parent chapter introduces MCP Apps with WidgetDir paradigm, not the old UIResourceBuilder paradigm | VERIFIED | ch20-mcp-apps.md contains no UIResourceBuilder references; introduces WidgetDir, mcpBridge, and adapters in Quick Start; grep for old APIs returns 0 matches |
| 8  | Learner can run the chess, map, and dataviz examples locally following walkthroughs | VERIFIED | ch20-03-postmessage.md has run instructions for all three examples with `cd examples/mcp-apps-chess`, `cd examples/mcp-apps-map`, and `cd examples/mcp-apps-dataviz`; tool tables, architecture diagrams, and "Try this" exercises for each |

**Score:** 8/8 truths verified

---

### Required Artifacts

| Artifact | Expected | Lines | Min | Status | Details |
|----------|----------|-------|-----|--------|---------|
| `pmcp-course/src/part8-advanced/ch20-mcp-apps.md` | Parent chapter with WidgetDir quick-start and chapter overview | 163 | 150 | VERIFIED | Contains "WidgetDir" (11 pattern matches including Learning Objectives, Quick Start, Architecture, Feature Flag, Chapter Contents, quiz embed, cross-reference to book ch12-5) |
| `pmcp-course/src/part8-advanced/ch20-01-ui-resources.md` | Hands-on tutorial for WidgetDir authoring and cargo pmcp app workflow | 499 | 350 | VERIFIED | Contains "cargo pmcp app new"; 49 key-content pattern matches; covers scaffolding, WidgetDir API, ResourceHandler, hot-reload, build, preview deep dive |
| `pmcp-course/src/part8-advanced/ch20-02-tool-ui-association.md` | Hands-on tutorial for bridge communication, mcpBridge API, and adapter pattern | 409 | 350 | VERIFIED | Contains "window.mcpBridge"; 81 key-content pattern matches; covers all four core methods, error handling, bridge init, ChatGPT extras, communication flow, three adapters, MultiPlatformResource |
| `pmcp-course/src/part8-advanced/ch20-03-postmessage.md` | Hands-on walkthroughs of chess, map, and dataviz examples | 575 | 400 | VERIFIED | Contains "chess", "mcp-apps-chess", "mcp-apps-map"; 80 key-content pattern matches; all three examples with architecture diagrams, run instructions, tool tables, "Try this" exercises, common 4-step pattern, comparison table, best practices |
| `pmcp-course/src/SUMMARY.md` | Updated Ch 20 sub-chapter titles and file references | — | — | VERIFIED | All 4 new file paths present at lines 156-159; correct titles: "MCP Apps: Interactive Widgets", "Widget Authoring and Developer Workflow", "Bridge Communication and Adapters", "Example Walkthroughs" |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `ch20-mcp-apps.md` | `pmcp-book/src/ch12-5-mcp-apps.md` | Cross-reference link for full reference documentation | WIRED | Line 155: `[Chapter 12.5: MCP Apps Extension](../../pmcp-book/src/ch12-5-mcp-apps.md)` present |
| `ch20-03-postmessage.md` | `examples/mcp-apps-chess/` | References chess example source with `cd examples/mcp-apps-chess` | WIRED | Line 46: `cd examples/mcp-apps-chess` present in run instructions |
| `ch20-03-postmessage.md` | `examples/mcp-apps-map/` | References map example source with `cd examples/mcp-apps-map` | WIRED | Line 221: `cd examples/mcp-apps-map` present in run instructions |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CRAP-01 | 23-01-PLAN.md | Ch 20 sub-chapters updated with WidgetDir, `cargo pmcp app` workflow, and adapter pattern APIs | SATISFIED | ch20-01-ui-resources.md (499 lines) covers WidgetDir convention, `cargo pmcp app new/build/preview`, ResourceHandler pattern; ch20-02-tool-ui-association.md (409 lines) covers all three adapters; REQUIREMENTS.md marks CRAP-01 Complete |
| CRAP-02 | 23-01-PLAN.md | Ch 20 updated with bridge communication patterns (postMessage, window.mcpBridge, window.openai) | SATISFIED | ch20-02-tool-ui-association.md "Communication Flow" explains how widget -> bridge script -> host -> server works; ChatGPT uses `window.openai`, MCP Apps uses `postMessage` JSON-RPC; all abstracted behind `window.mcpBridge`; REQUIREMENTS.md marks CRAP-02 Complete |
| CRAP-03 | 23-02-PLAN.md | Ch 20 references current examples (chess, map, dataviz) with hands-on walkthrough style | SATISFIED | ch20-03-postmessage.md (575 lines) has full walkthroughs for all three; architecture ASCII diagrams; `cargo run` + `cargo pmcp preview` run instructions; tool tables; "Try this" exercises; common 4-step pattern summary; REQUIREMENTS.md marks CRAP-03 Complete |

No orphaned requirements — all three CRAP-* IDs are claimed by plans and covered by implementation.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `ch20-03-postmessage.md` | 520 | `window.parent.postMessage` | Info | This is inside a Best Practices bullet explicitly telling learners NOT to use `window.parent.postMessage` directly. It is correct teaching content, not an old-API reference. No action needed. |

No stubs, placeholders, empty implementations, or TODO comments found across any of the four Ch 20 files.

---

### Human Verification Required

#### 1. Teaching Style Quality

**Test:** Open ch20-01-ui-resources.md and read the "Hands-On: Scaffold Your First MCP App" section end-to-end, following the steps in a real terminal.
**Expected:** Commands work, generated code matches what the chapter describes, the counter widget exercise in Step 4 can be completed without ambiguity.
**Why human:** Cannot verify that the `cargo pmcp app new` scaffolded output exactly matches the code shown in the tutorial (Step 2 code walkthrough) without running the command.

#### 2. Quiz Embed Validity

**Test:** Build the course with `mdbook build` and verify `{{#quiz ../quizzes/ch20-mcp-apps.toml}}` in ch20-mcp-apps.md renders without error.
**Expected:** Quiz renders in the browser-based course reader.
**Why human:** Cannot verify the quiz TOML file exists and is compatible with the embed without building the course.

#### 3. Cross-Link Navigation

**Test:** Open the built course and follow the navigation chain: ch20-mcp-apps.md -> ch20-01-ui-resources.md -> ch20-02-tool-ui-association.md -> ch20-03-postmessage.md and back to ch20-mcp-apps.md.
**Expected:** All "Continue to" and "Back to" navigation links resolve to correct pages in the built course.
**Why human:** Links are correct relative paths in markdown but mdBook link resolution requires a build to fully verify.

---

### Gaps Summary

None. All eight observable truths are verified. All five artifacts exist, exceed minimum line counts, and contain required content patterns. All three key links are wired. All three requirements are satisfied with zero old-API contamination (UIResourceBuilder, .with_ui(), ToolUIMetadata, raw jsonrpc postMessage).

The one flagged "anti-pattern" (line 520, `window.parent.postMessage`) is correctly used in a "never do this" Best Practices warning and is not a defect.

---

_Verified: 2026-02-27_
_Verifier: Claude (gsd-verifier)_
