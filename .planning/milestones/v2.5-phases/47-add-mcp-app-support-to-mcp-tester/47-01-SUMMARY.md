---
phase: 47-add-mcp-app-support-to-mcp-tester
plan: 01
subsystem: testing
tags: [mcp-apps, mcp-tester, cli, validation, metadata]

requires:
  - phase: 46-mcp-bridge-review-and-fixes
    provides: stable MCP Apps bridge and tool result delivery
provides:
  - AppValidator module for MCP App metadata validation
  - TestCategory::Apps variant in report system
  - `mcp-tester apps <url>` CLI subcommand
affects: [47-02, cargo-pmcp-test-apps]

tech-stack:
  added: []
  patterns: [AppValidator struct with validate_tools() returning Vec<TestResult>]

key-files:
  created:
    - crates/mcp-tester/src/app_validator.rs
  modified:
    - crates/mcp-tester/src/report.rs
    - crates/mcp-tester/src/lib.rs
    - crates/mcp-tester/src/main.rs

key-decisions:
  - "Resource URI cross-reference mismatch produces Warning not Failure (per user decision in plan)"
  - "ChatGPT key absence produces Warning not Failure (advisory, not required)"
  - "AppValidator applies strict mode internally before returning results"

patterns-established:
  - "AppValidator::is_app_capable() static method for detecting App-capable tools via _meta"
  - "validate_tools() returns Vec<TestResult> with TestCategory::Apps for report integration"

requirements-completed: [APP-VAL-01, APP-VAL-02, APP-VAL-03, APP-VAL-05]

duration: 4min
completed: 2026-03-12
---

# Phase 47 Plan 01: AppValidator Module and Apps Subcommand Summary

**AppValidator engine validating MCP App _meta keys (nested/flat URI, ChatGPT openai/*, outputSchema) wired to `mcp-tester apps` CLI subcommand**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-12T00:25:47Z
- **Completed:** 2026-03-12T00:29:55Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- AppValidator module with 6 validation methods: validate_tools, is_app_capable, validate_tool_meta, validate_resource_match, validate_chatgpt_keys, validate_output_schema
- 8 unit tests covering nested/flat URI detection, resource cross-reference, ChatGPT mode, strict mode, and tool filtering
- `mcp-tester apps <url>` subcommand with --mode (standard|chatgpt|claude-desktop), --tool, --strict flags
- Zero clippy warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Create AppValidator module and add TestCategory::Apps** - `a4463ff` (feat)
2. **Task 2: Add `apps` subcommand to mcp-tester standalone binary** - `84f9c57` (feat)

## Files Created/Modified
- `crates/mcp-tester/src/app_validator.rs` - AppValidator with validate_tools(), is_app_capable(), ChatGPT/standard/claude-desktop modes, 8 unit tests
- `crates/mcp-tester/src/report.rs` - Added TestCategory::Apps variant
- `crates/mcp-tester/src/lib.rs` - Added pub mod app_validator and re-exports
- `crates/mcp-tester/src/main.rs` - Added Apps command variant, match arm, run_apps_validation() function

## Decisions Made
- Resource URI cross-reference mismatch produces Warning not Failure (per user decision in plan)
- ChatGPT key absence produces Warning not Failure (advisory, not required for all servers)
- AppValidator applies strict mode internally before returning results, and run_apps_validation also calls report.apply_strict_mode() for consistency with other subcommands

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed TestStatus move-after-use in chatgpt_keys validation**
- **Found during:** Task 1 (AppValidator module)
- **Issue:** TestStatus doesn't implement Copy; using it in both status field and condition caused borrow-after-move
- **Fix:** Replaced with boolean flag pattern (present/has_flat) to avoid move
- **Files modified:** crates/mcp-tester/src/app_validator.rs
- **Verification:** cargo build succeeds
- **Committed in:** a4463ff (Task 1 commit)

**2. [Rule 1 - Bug] Fixed non-exhaustive ToolInfo struct literal in tests**
- **Found during:** Task 1 (AppValidator module)
- **Issue:** ToolInfo is #[non_exhaustive], cannot use struct literal in tests
- **Fix:** Used ToolInfo::new() constructor with _meta field mutation
- **Files modified:** crates/mcp-tester/src/app_validator.rs
- **Verification:** cargo test passes (8/8 tests)
- **Committed in:** a4463ff (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both auto-fixes necessary for compilation. No scope creep.

## Issues Encountered
None beyond the auto-fixed compilation issues above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- AppValidator and AppValidationMode are re-exported from mcp_tester lib for use by cargo-pmcp
- Plan 47-02 can wire `cargo pmcp test apps` to this validator
- Zero clippy warnings, all tests passing

---
*Phase: 47-add-mcp-app-support-to-mcp-tester*
*Completed: 2026-03-12*
