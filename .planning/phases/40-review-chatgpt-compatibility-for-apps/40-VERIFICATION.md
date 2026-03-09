---
phase: 40-review-chatgpt-compatibility-for-apps
verified: 2026-03-06T00:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
gaps: []
---

# Phase 40: Review ChatGPT Compatibility for Apps — Verification Report

**Phase Goal:** Align SDK metadata emission with official ext-apps spec: add legacy flat key ui/resourceUri to build_meta_map, dual-emit nested ui.csp/ui.domain in WidgetMeta, add ui.visibility array format, and add ModelOnly visibility variant
**Verified:** 2026-03-06
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                                          | Status     | Evidence                                                                                                                           |
|----|----------------------------------------------------------------------------------------------------------------|------------|-------------------------------------------------------------------------------------------------------------------------------------|
| 1  | build_meta_map() emits nested ui.resourceUri, legacy flat ui/resourceUri, and openai/outputTemplate           | VERIFIED   | Lines 346-363 of src/types/ui.rs; all 3 keys inserted; map capacity 3; test_build_meta_map_emits_all_three_keys passes             |
| 2  | from_metadata() reads both nested and legacy flat formats with no regression                                  | VERIFIED   | Lines 369-406 of src/types/ui.rs; fallback chain preserved; test_tool_ui_metadata_from_legacy_flat_format + nested tests pass      |
| 3  | Existing test assertion inverted (flat key must be present)                                                   | VERIFIED   | test_tool_ui_metadata_to_nested_format at line 489 now asserts presence of ui/resourceUri; 18/18 ui tests pass                    |
| 4  | WidgetMeta::to_meta_map() emits nested ui.domain and ui.csp alongside flat openai/* keys                     | VERIFIED   | Lines 282-333 of src/types/mcp_apps.rs; ui_obj built with domain + csp fields; redirect_domains excluded from nested; tests pass  |
| 5  | ToolVisibility has ModelOnly variant; to_visibility_array() returns spec arrays; ui.visibility dual-emitted   | VERIFIED   | Lines 373-388 and 510-527 of src/types/mcp_apps.rs; 3-variant enum; ChatGptToolMeta::to_meta_map() inserts ui.visibility array    |

**Score:** 5/5 truths verified

---

## Required Artifacts

| Artifact                     | Expected                                                          | Status     | Details                                                                                   |
|------------------------------|-------------------------------------------------------------------|------------|-------------------------------------------------------------------------------------------|
| `src/types/ui.rs`            | Legacy flat key emission in build_meta_map()                      | VERIFIED   | Line 354-357: `meta.insert("ui/resourceUri".to_string(), ...)` present                   |
| `src/types/mcp_apps.rs`      | Nested ui.csp, ui.domain, ui.visibility emission; baseUriDomains on WidgetCSP; ModelOnly variant | VERIFIED   | base_uri_domains at line 109; ModelOnly at line 373; dual-emit in to_meta_map() at lines 282-333 and 510-527 |

Both artifacts: exist, substantive (real logic, no stubs), and wired (called by build_ui_meta() -> all tool type metadata() methods).

---

## Key Link Verification

| From                              | To                            | Via                        | Status  | Details                                                                                              |
|-----------------------------------|-------------------------------|----------------------------|---------|------------------------------------------------------------------------------------------------------|
| ToolUIMetadata::build_meta_map()  | ToolInfo::with_ui()           | build_ui_meta() call       | WIRED   | build_ui_meta() at line 316-321 calls build_meta_map(); all TypedTool variants call build_ui_meta() |
| ToolUIMetadata::build_meta_map()  | build_ui_meta()               | direct call                | WIRED   | Line 320: `Some(ToolUIMetadata::build_meta_map(uri))`                                               |
| WidgetMeta::to_meta_map()         | ui.csp nested object          | dual-emit pattern          | WIRED   | Lines 299-328: connectDomains, resourceDomains, frameDomains, baseUriDomains all emitted into ui_obj |
| ToolVisibility                    | ChatGptToolMeta::to_meta_map()| to_visibility_array()      | WIRED   | Lines 516-527: vis.to_visibility_array() called; result inserted as ui.visibility                   |

---

## Requirements Coverage

| Requirement | Source Plan | Description (inferred from phase goal)                                               | Status    | Evidence                                                              |
|-------------|-------------|--------------------------------------------------------------------------------------|-----------|-----------------------------------------------------------------------|
| COMPAT-01   | 40-01-PLAN  | Add legacy flat key ui/resourceUri to build_meta_map() for ext-apps backward compat | SATISFIED | build_meta_map() emits 3 keys including "ui/resourceUri"; tests pass  |
| COMPAT-02   | 40-02-PLAN  | Dual-emit nested ui.domain in WidgetMeta::to_meta_map()                             | SATISFIED | ui_obj.insert("domain", ...) at line 296; test_widget_meta_dual_emit_domain passes |
| COMPAT-03   | 40-02-PLAN  | Dual-emit nested ui.csp with spec field names in WidgetMeta::to_meta_map()          | SATISFIED | ui_obj.insert("csp", csp_obj) at line 328; baseUriDomains included; redirect_domains excluded; test_widget_meta_dual_emit_csp passes |
| COMPAT-04   | 40-02-PLAN  | Add ModelOnly variant to ToolVisibility; emit ui.visibility array in ChatGptToolMeta | SATISFIED | ToolVisibility::ModelOnly at line 373; to_visibility_array() returns ["model"]; ChatGptToolMeta::to_meta_map() emits ui.visibility |

**Note on COMPAT IDs:** These requirement IDs are defined exclusively in ROADMAP.md phase 40 description and the plan frontmatter. They do not appear in .planning/REQUIREMENTS.md (which covers v1.5 and v1.6 CLI requirements). This is a project documentation gap — not a code gap. The implementations fully satisfy the stated requirements.

**Orphaned requirements check:** No COMPAT-* IDs are defined in REQUIREMENTS.md, so there are no orphaned requirements in the formal requirements file. All four IDs are consistently cross-referenced between ROADMAP.md and the plan files.

---

## Anti-Patterns Found

| File                          | Pattern       | Severity | Impact |
|-------------------------------|---------------|----------|--------|
| No anti-patterns found        | —             | —        | —      |

Scanned both `src/types/ui.rs` and `src/types/mcp_apps.rs` for TODO/FIXME/PLACEHOLDER, empty implementations, and console.log-only handlers. Zero findings.

---

## Test Results

| Test Suite                     | Passed | Failed | Notes                                                              |
|-------------------------------|--------|--------|--------------------------------------------------------------------|
| types::ui::tests               | 18     | 0      | Includes test_build_meta_map_emits_all_three_keys, test_deep_merge_preserves_flat_key |
| types::mcp_apps                | 32     | 0      | Includes all dual-emit, ModelOnly, base_uri_domains tests          |

Clippy: zero warnings with `--features mcp-apps -- -D warnings`.

---

## Commits Verified

All four commits documented in the SUMMARY files were verified to exist in the git log:

| Hash      | Description                                                    |
|-----------|----------------------------------------------------------------|
| `0bbd4be` | test(40-01): add failing tests for legacy flat key emission    |
| `634ce30` | feat(40-01): add legacy flat key to build_meta_map             |
| `b59c930` | test(40-02): add baseUriDomains and ModelOnly tests + impl     |
| `6ca6ab9` | feat(40-02): dual-emit nested ui.csp, ui.domain, ui.visibility |

---

## Human Verification Required

None. All behaviors are verifiable programmatically through unit tests and code inspection.

---

## Summary

Phase 40 fully achieved its goal. All five observable truths are verified against the actual codebase:

1. `build_meta_map()` in `src/types/ui.rs` emits exactly 3 top-level keys: the nested `ui` object, the legacy flat `"ui/resourceUri"`, and `"openai/outputTemplate"`. The capacity was updated to 3 and the insertion is in the correct order matching the official ext-apps reference.

2. `WidgetMeta::to_meta_map()` in `src/types/mcp_apps.rs` dual-emits `domain` and `csp` into a nested `ui` object alongside the existing flat `openai/*` keys. The nested `ui.csp` uses spec camelCase field names (`connectDomains`, `resourceDomains`, `frameDomains`, `baseUriDomains`). The ChatGPT-specific `redirect_domains` is correctly excluded from the nested format.

3. `WidgetCSP` has the new `base_uri_domains` field with a builder method and `is_empty()` check, mapping to the spec's `McpUiResourceCsp.baseUriDomains`.

4. `ToolVisibility` has the new `ModelOnly` variant with `to_visibility_array()` returning `["model"]`.

5. `ChatGptToolMeta::to_meta_map()` dual-emits `ui.visibility` as a spec-compatible array alongside the flat `"openai/visibility"` key.

All 50 combined tests pass (18 in ui.rs + 32 in mcp_apps.rs). Zero clippy warnings. Zero anti-patterns found.

---

_Verified: 2026-03-06_
_Verifier: Claude (gsd-verifier)_
