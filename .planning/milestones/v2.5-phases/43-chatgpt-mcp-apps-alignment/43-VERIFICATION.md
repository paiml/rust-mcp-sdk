---
phase: 43-chatgpt-mcp-apps-alignment
verified: 2026-03-08T16:45:00Z
status: passed
score: 7/7 must-haves verified
---

# Phase 43: ChatGPT MCP Apps Alignment Verification Report

**Phase Goal:** Fix 4 protocol gaps preventing ChatGPT from rendering MCP Apps widgets -- align _meta propagation across resources/list, resources/read, and tools/call with ChatGPT's expected protocol.
**Verified:** 2026-03-08T16:45:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | ResourceInfo includes _meta field serialized as JSON _meta key | VERIFIED | `src/types/protocol.rs:914-916` -- field `pub meta: Option<serde_json::Map<String, serde_json::Value>>` with `#[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]` |
| 2 | tools/call responses contain only openai/toolInvocation/* keys in _meta | VERIFIED | `src/types/protocol.rs:594-606` -- `with_widget_enrichment` filters with `starts_with("openai/toolInvocation/")` before calling `with_meta` |
| 3 | URI-to-tool-meta index is built at ServerCore construction time | VERIFIED | `src/server/core.rs:108-126` -- `build_uri_to_tool_meta()` function; line 227 calls it in `ServerCore::new()`; line 153 stores as field |
| 4 | All existing code compiles after ResourceInfo field addition | VERIFIED | `cargo check --all-targets` passes (only pre-existing unused import warning in unrelated test) |
| 5 | resources/list response includes openai/* _meta keys on widget ResourceInfo items | VERIFIED | `src/server/core.rs:457-465` -- post-process loop in `handle_list_resources` populates `resource.meta` from `uri_to_tool_meta` index |
| 6 | resources/read response merges tool descriptor keys into content _meta alongside display keys | VERIFIED | `src/server/core.rs:491-502` -- post-process loop in `handle_read_resource` uses `deep_merge` to combine descriptor keys into `Content::Resource` meta |
| 7 | Non-widget resources are unaffected by _meta propagation | VERIFIED | Both enrichment loops guard with `if let Some(tool_meta) = self.uri_to_tool_meta.get(...)` -- only URIs in the index are touched |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/types/protocol.rs` | ResourceInfo with _meta field, filtered with_widget_enrichment | VERIFIED | Field at line 916, filter at lines 594-606 |
| `src/server/core.rs` | uri_to_tool_meta field and builder on ServerCore | VERIFIED | Field at line 153, builder function at lines 108-126, called at line 227, used at lines 462 and 497 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| src/types/protocol.rs | src/server/core.rs | ResourceInfo struct used in handler results | WIRED | `ResourceInfo` is used in `ListResourcesResult.resources` which is mutated in `handle_list_resources` |
| src/server/core.rs | src/types/protocol.rs | with_widget_enrichment called with ToolInfo | WIRED | `with_widget_enrichment` called at tool-call handling; filters _meta to `openai/toolInvocation/*` |
| src/server/core.rs handle_list_resources | uri_to_tool_meta index | post-process ResourceInfo items with tool _meta | WIRED | Line 462: `self.uri_to_tool_meta.get(&resource.uri)` |
| src/server/core.rs handle_read_resource | uri_to_tool_meta index + deep_merge | merge descriptor keys into Content::Resource meta | WIRED | Line 497-499: `uri_to_tool_meta.get` then `deep_merge` call |

### Requirements Coverage

No requirement IDs declared for this phase (hotfix-style). No orphaned requirements found in REQUIREMENTS.md for Phase 43.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | - | - | - | No anti-patterns found in phase-modified files |

No TODO/FIXME/PLACEHOLDER/HACK comments found in modified core files. No stub implementations detected.

### Human Verification Required

### 1. ChatGPT Widget Rendering End-to-End

**Test:** Connect an MCP Apps server to ChatGPT and invoke a widget tool. Verify the widget renders.
**Expected:** ChatGPT discovers widget metadata from resources/list _meta, renders the widget iframe using resources/read content, and shows invocation state from tools/call _meta.
**Why human:** Requires live ChatGPT connection and visual confirmation of widget rendering.

### 2. Non-Widget Resource Isolation

**Test:** Connect a server with both widget and non-widget resources. Call resources/list and inspect JSON responses.
**Expected:** Widget resources have _meta with openai/* keys. Non-widget resources have no _meta field in JSON output.
**Why human:** Requires inspecting actual JSON protocol messages.

### Gaps Summary

No gaps found. All 7 observable truths verified against actual code. The implementation matches the phase goal:

- **Gap 1 (resources/list _meta):** Fixed -- `handle_list_resources` post-processes to populate `ResourceInfo._meta` from URI-to-tool-meta index
- **Gap 2 (resources/read _meta):** Fixed -- `handle_read_resource` post-processes to merge descriptor keys via `deep_merge`
- **Gap 3 (tools/call _meta filtering):** Fixed -- `with_widget_enrichment` filters to only `openai/toolInvocation/*` keys
- **Gap 4 (title field):** Intentionally deferred per user decision

All 4 phase commits verified present: `a995b61`, `9287455`, `eb7a7a9`, `4cb55a6`.

---

_Verified: 2026-03-08T16:45:00Z_
_Verifier: Claude (gsd-verifier)_
