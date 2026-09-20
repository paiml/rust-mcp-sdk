---
phase: 40-review-chatgpt-compatibility-for-apps
plan: 02
subsystem: ui
tags: [mcp-apps, chatgpt, csp, dual-emit, visibility, ext-apps-spec]

requires:
  - phase: 39-add-deep-merge-for-ui-meta-key-to-prevent-collision
    provides: deep_merge pattern for ui meta objects
provides:
  - Nested ui.csp emission in WidgetMeta::to_meta_map()
  - Nested ui.domain emission in WidgetMeta::to_meta_map()
  - Nested ui.visibility array emission in ChatGptToolMeta::to_meta_map()
  - WidgetCSP.base_uri_domains field (spec baseUriDomains)
  - ToolVisibility::ModelOnly variant with to_visibility_array() helper
affects: [mcp-apps, chatgpt-compatibility]

tech-stack:
  added: []
  patterns: [dual-emit flat+nested for spec compatibility, spec-field-name mapping in nested objects]

key-files:
  created: []
  modified:
    - src/types/mcp_apps.rs

key-decisions:
  - "redirect_domains excluded from nested ui.csp -- ChatGPT-specific field not in ext-apps spec"
  - "ToolVisibility::ModelOnly serializes as 'modelonly' for flat openai/visibility key; to_visibility_array() returns ['model'] for nested ui.visibility"
  - "Nested ui.csp uses spec camelCase field names (connectDomains, resourceDomains, frameDomains, baseUriDomains)"

patterns-established:
  - "Dual-emit pattern extended to csp/domain/visibility: flat openai/* keys for ChatGPT backward compat, nested ui.* for standard MCP hosts"

requirements-completed: [COMPAT-02, COMPAT-03, COMPAT-04]

duration: 8min
completed: 2026-03-07
---

# Phase 40 Plan 02: Dual-emit CSP, Domain, and Visibility Summary

**Dual-emit nested ui.csp, ui.domain, ui.visibility alongside flat openai/* keys for ext-apps spec compatibility; add baseUriDomains and ModelOnly variant**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-07T03:15:54Z
- **Completed:** 2026-03-07T03:24:04Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- WidgetMeta::to_meta_map() now dual-emits domain and csp into nested ui object with spec field names
- ChatGptToolMeta::to_meta_map() dual-emits visibility as nested ui.visibility array (e.g., ["model", "app"])
- WidgetCSP extended with base_uri_domains field matching spec McpUiResourceCsp.baseUriDomains
- ToolVisibility extended with ModelOnly variant and to_visibility_array() helper
- redirect_domains correctly excluded from nested ui.csp (ChatGPT-specific only)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add baseUriDomains to WidgetCSP and ModelOnly to ToolVisibility** - `b59c930` (test+feat)
2. **Task 2: Dual-emit nested ui.csp, ui.domain, ui.visibility** - `6ca6ab9` (feat)

_Note: TDD tasks had tests written first (RED), then implementation (GREEN)._

## Files Created/Modified
- `src/types/mcp_apps.rs` - Added base_uri_domains, ModelOnly, dual-emit for csp/domain/visibility in to_meta_map()

## Decisions Made
- redirect_domains excluded from nested ui.csp: it is ChatGPT-specific and not part of the ext-apps spec
- Nested ui.csp uses camelCase spec field names (connectDomains, resourceDomains, frameDomains, baseUriDomains)
- ToolVisibility::ModelOnly serializes as "modelonly" via serde rename_all; to_visibility_array() returns ["model"] for nested format

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated existing test assertion for domain dual-emit**
- **Found during:** Task 2 (Dual-emit implementation)
- **Issue:** Existing test_widget_meta_dual_emit_with_domain asserted domain was NOT in nested ui (correct before this task, incorrect after)
- **Fix:** Updated assertion to verify domain IS now in nested ui object
- **Files modified:** src/types/mcp_apps.rs (test section)
- **Verification:** All 32 mcp_apps tests pass
- **Committed in:** 6ca6ab9 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix in test assertion)
**Impact on plan:** Necessary update to reflect new dual-emit behavior. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All ChatGPT compatibility dual-emit patterns complete
- WidgetMeta, ChatGptToolMeta both emit flat openai/* and nested ui.* formats
- Ready for any remaining compatibility review items

---
*Phase: 40-review-chatgpt-compatibility-for-apps*
*Completed: 2026-03-07*

## Self-Check: PASSED
