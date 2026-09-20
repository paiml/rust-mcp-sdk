---
phase: 62-mcp-pen-test
plan: 02
subsystem: testing
tags: [pentest, security, prompt-injection, tool-poisoning, mcp-tester, attack-detection]

# Dependency graph
requires:
  - phase: 62-01
    provides: "Pentest module foundation: types, config, rate limiter, payloads, attack stubs, engine, CLI"
provides:
  - "Full prompt injection attack suite (PI-01 through PI-07) with marker-based echo detection, destructive mode gating, and rate-limited execution"
  - "Full tool poisoning detection suite (TP-01 through TP-06) with hidden instruction detection, script injection scanning, schema validation, and rug pull detection"
  - "Helper functions: extract_string_args, build_injected_args, check_response_for_markers, contains_hidden_instructions, contains_script_injection"
affects: [62-03-PLAN]

# Tech tracking
tech-stack:
  added: []
  patterns: [marker-based echo detection, destructive mode gating, static vs dynamic attack classification, recursive JSON marker search]

key-files:
  created: []
  modified:
    - cargo-pmcp/src/pentest/attacks/prompt_injection.rs
    - cargo-pmcp/src/pentest/attacks/tool_poisoning.rs

key-decisions:
  - "Skip content array in marker echo detection to reduce false positives from normal tool echo behavior"
  - "One finding per tool per attack type -- early return after first detection prevents noise"
  - "TP-01/02/03 are static analysis (no server requests) while TP-04/05/06 require rate-limited MCP calls"
  - "Known meta keys allowlist (ui, ui/resourceUri, pmcp:outputTypeName, annotations) for TP-02"
  - "Constructed ToolInfo via ::new() + field mutation in tests since ToolInfo is #[non_exhaustive]"

patterns-established:
  - "Marker-based echo detection: inject unique PMCP_INJECTION_MARKER_7a3f, check non-content response fields"
  - "Destructive mode gating: check config.destructive before calling mutation-based attacks"
  - "Static vs dynamic tool poisoning: TP-01..03 inspect tool definitions, TP-04..06 call tools"
  - "Rate-limited tool calls: limiter.wait().await before every send_custom_request"

requirements-completed: []

# Metrics
duration: 9min
completed: 2026-03-28
---

# Phase 62 Plan 02: Prompt Injection and Tool Poisoning Attack Runners Summary

**13 MCP attack implementations (PI-01..PI-07 prompt injection + TP-01..TP-06 tool poisoning) with marker-based detection, destructive gating, rate limiting, and 36 unit tests**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-28T14:30:35Z
- **Completed:** 2026-03-28T14:39:45Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Replaced prompt injection stub with full 7-attack suite: delimiter confusion (PI-01), instruction override (PI-02), system prompt extraction (PI-03), tool redirection (PI-04), marker echo detection (PI-05), unicode bypass (PI-06), oversized input (PI-07)
- Replaced tool poisoning stub with full 6-attack suite: hidden description instructions (TP-01), unexpected _meta keys (TP-02), script injection in resourceUri (TP-03), schema mismatch (TP-04), oversized response (TP-05), rug pull detection (TP-06)
- 36 unit tests covering all detection helpers, argument extraction, payload building, marker search, hidden instruction patterns, script injection patterns, meta key validation, and destructive mode gating

## Task Commits

Each task was committed atomically:

1. **Task 1: Prompt injection attack runner (PI-01 through PI-07)** - `9cd6f57` (feat)
2. **Task 2: Tool poisoning attack runner (TP-01 through TP-06)** - `7432dc6` (feat)
3. **Formatting fix** - `5f2ba1e` (style)

## Files Created/Modified

- `cargo-pmcp/src/pentest/attacks/prompt_injection.rs` - Full PI-01..PI-07 attack suite with helpers and 12 unit tests
- `cargo-pmcp/src/pentest/attacks/tool_poisoning.rs` - Full TP-01..TP-06 detection suite with helpers and 24 unit tests

## Decisions Made

- **Skip content array in marker search**: The `check_response_for_markers` function skips the top-level `content` key to avoid false positives from tools that legitimately echo input (per pitfall 5 in research). Only markers found in _meta, error messages, or structural fields trigger findings.
- **One finding per tool per attack**: Each attack function returns early after the first finding for a given tool. This prevents report noise while still flagging the vulnerability.
- **Static vs dynamic classification**: TP-01/02/03 are purely static analysis of tool definitions (no server requests needed, no rate limiter consumed). TP-04/05/06 require actual tool calls and go through the rate limiter.
- **Known meta keys allowlist**: TP-02 uses a curated list of known safe _meta keys (ui, ui/resourceUri, pmcp:outputTypeName, annotations). Any other root-level key triggers a finding.
- **ToolInfo construction in tests**: Since ToolInfo is `#[non_exhaustive]`, tests use `ToolInfo::new()` followed by direct field assignment, or a `tool_with_meta()` helper for tests needing custom `_meta`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] ToolInfo #[non_exhaustive] prevents struct literals in tests**
- **Found during:** Task 2 (tool poisoning tests)
- **Issue:** ToolInfo has `#[non_exhaustive]` attribute, preventing construction via struct literal syntax from outside the defining crate. Several tests needed ToolInfo instances with custom `_meta` fields.
- **Fix:** Created a `tool_with_meta()` helper function that uses `ToolInfo::new()` + direct field mutation for `_meta`. Applied the same pattern consistently across all tests.
- **Files modified:** cargo-pmcp/src/pentest/attacks/tool_poisoning.rs
- **Verification:** All 24 tool poisoning tests pass
- **Committed in:** 7432dc6

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor test construction adaptation. No scope creep. All planned attack implementations delivered.

## Issues Encountered

None beyond the auto-fixed deviation above.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. Both prompt injection and tool poisoning stubs have been fully replaced with real implementations.

The session security attack runner (`cargo-pmcp/src/pentest/attacks/session_security.rs`) remains a stub -- it is implemented in Plan 03.

## Next Phase Readiness

- Plan 03 (session security attacks SS-01..SS-06) can proceed immediately
- All attack runner patterns are established: rate-limited execution, finding creation, destructive mode gating
- The engine already calls `attacks::session_security::run()` with the correct signature

## Self-Check: PASSED

- [x] cargo-pmcp/src/pentest/attacks/prompt_injection.rs exists
- [x] cargo-pmcp/src/pentest/attacks/tool_poisoning.rs exists
- [x] Commit 9cd6f57 (Task 1: prompt injection) exists
- [x] Commit 7432dc6 (Task 2: tool poisoning) exists
- [x] Commit 5f2ba1e (formatting) exists
- [x] 36 unit tests pass (12 PI + 24 TP)
- [x] cargo check -p cargo-pmcp succeeds

---
*Phase: 62-mcp-pen-test*
*Completed: 2026-03-28*
