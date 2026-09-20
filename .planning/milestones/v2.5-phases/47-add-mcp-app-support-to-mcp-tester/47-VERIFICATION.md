---
phase: 47-add-mcp-app-support-to-mcp-tester
verified: 2026-03-11T00:00:00Z
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 47: Add MCP App Support to mcp-tester Verification Report

**Phase Goal:** Add MCP App protocol metadata validation to mcp-tester and cargo pmcp test, enabling CLI-based App compliance checks (metadata-only, no browser) with standard and host-specific modes
**Verified:** 2026-03-11
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | User can run `mcp-tester apps <url>` or `cargo pmcp test apps --url <url>` to validate App metadata on any MCP server | VERIFIED | `Commands::Apps` variant in `crates/mcp-tester/src/main.rs` (line 191); `TestCommand::Apps` in `cargo-pmcp/src/commands/test/mod.rs` (line 31); both wired to `run_apps_validation()` and `apps::execute()` respectively |
| 2 | Validation checks ui.resourceUri, MIME types, resource cross-references, and optionally ChatGPT-specific keys | VERIFIED | `AppValidator::validate_tool_meta()` checks uri presence/format; `validate_resource_match()` checks MIME type against `["text/html", "application/html+mcp-app", "text/html;profile=mcp-app"]`; `validate_chatgpt_keys()` checks `openai/*` keys in ChatGPT mode |
| 3 | `cargo pmcp test check` shows hint when App-capable tools are detected | VERIFIED | `cargo-pmcp/src/commands/test/check.rs` (lines 210-226): `AppValidator::is_app_capable()` filters tools, prints hint message suggesting `cargo pmcp test apps --url <url>` |
| 4 | --strict promotes warnings to failures, --tool filters to single tool, --mode selects host-specific checks | VERIFIED | `AppValidator.strict` field promotes `Warning` to `Failed` (app_validator.rs lines 81-87); `tool_filter` field filters by name (lines 51-55); mode enum drives ChatGPT branch (lines 70-74) |

**Additional truths from plan 47-01 must_haves:**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 5 | App-capable tools detected by presence of ui.resourceUri in _meta (nested or flat) | VERIFIED | `extract_resource_uri()` checks both `_meta.ui.resourceUri` (nested) and `_meta["ui/resourceUri"]` (flat); 2 unit tests confirm both paths |
| 6 | Cross-reference with resources/list produces warnings (not failures) for missing resource URIs | VERIFIED | `validate_resource_match()` returns `TestStatus::Warning` (not `Failed`) when no matching resource found (line 214) |
| 7 | ChatGPT mode validates openai/* keys | VERIFIED | `validate_chatgpt_keys()` checks 4 keys: `openai/outputTemplate`, `openai/toolInvocation/invoking`, `openai/toolInvocation/invoked`, `openai/widgetAccessible`; missing = Warning |
| 8 | No App-capable tools found exits 0 with info message | VERIFIED | Both `run_apps_validation()` (main.rs lines 936-950) and `apps::execute()` (apps.rs lines 146-157) handle empty case with info message and return Ok(()) |

**Score:** 8/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/mcp-tester/src/app_validator.rs` | AppValidator struct with validate_tools() returning Vec<TestResult> | VERIFIED | 494 lines; full implementation with 6 methods and 8 unit tests |
| `crates/mcp-tester/src/report.rs` | TestCategory::Apps variant | VERIFIED | Line 33: `Apps,` in TestCategory enum |
| `crates/mcp-tester/src/lib.rs` | pub mod app_validator + re-exports | VERIFIED | Line 49: `pub mod app_validator;`; line 59: `pub use app_validator::{AppValidationMode, AppValidator};` |
| `crates/mcp-tester/src/main.rs` | Apps subcommand in Commands enum | VERIFIED | Lines 191-206: `Apps { url, mode, tool, strict }` variant; lines 437-456: match arm calling `run_apps_validation()` |
| `cargo-pmcp/src/commands/test/apps.rs` | cargo pmcp test apps handler | VERIFIED | 209 lines; full implementation with mode parsing, connectivity check, discovery, validation, report printing |
| `cargo-pmcp/src/commands/test/mod.rs` | Apps variant in TestCommand enum | VERIFIED | Lines 27-59: `Apps { url, mode, tool, strict, transport, verbose, timeout }` |
| `cargo-pmcp/src/commands/test/check.rs` | App-capable tools hint | VERIFIED | Lines 210-226: hint detection using `AppValidator::is_app_capable()` guarded by `should_output()` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/mcp-tester/src/main.rs` | `crates/mcp-tester/src/app_validator.rs` | `AppValidator::new()` called in `run_apps_validation()` | WIRED | Line 902: `use app_validator::{AppValidationMode, AppValidator};`; line 953: `AppValidator::new(...)` |
| `crates/mcp-tester/src/app_validator.rs` | `crates/mcp-tester/src/report.rs` | Returns `Vec<TestResult>` with `TestCategory::Apps` | WIRED | Line 7: `use crate::report::{TestCategory, TestResult, TestStatus};`; `TestCategory::Apps` used throughout |
| `cargo-pmcp/src/commands/test/apps.rs` | `crates/mcp-tester/src/app_validator.rs` | Imports AppValidator from mcp_tester crate | WIRED | Line 8: `use mcp_tester::{AppValidationMode, AppValidator, TestReport, TestStatus};`; line 171: `AppValidator::new(...)` |
| `cargo-pmcp/src/commands/test/check.rs` | `crates/mcp-tester/src/app_validator.rs` | `is_app_capable` for hint detection | WIRED | Line 12: `use mcp_tester::{AppValidator, ServerTester, TestStatus};`; line 215: `AppValidator::is_app_capable(t)` |

### Requirements Coverage

| Requirement | Source Plan | Description (derived from ROADMAP) | Status | Evidence |
|-------------|------------|-------------------------------------|--------|----------|
| APP-VAL-01 | 47-01, 47-02 | CLI command for App metadata validation | SATISFIED | `mcp-tester apps` and `cargo pmcp test apps` both implemented and wired |
| APP-VAL-02 | 47-01 | Detect App-capable tools via _meta.ui.resourceUri | SATISFIED | `is_app_capable()` + `extract_resource_uri()` handle nested and flat key forms |
| APP-VAL-03 | 47-01 | Cross-reference with resources/list (warnings not failures) | SATISFIED | `validate_resource_match()` issues Warning when resource URI not found |
| APP-VAL-04 | 47-01, 47-02 | check command hint when App-capable tools detected | SATISFIED | `check.rs` lines 210-226 detect and print hint |
| APP-VAL-05 | 47-01 | ChatGPT mode validates openai/* keys | SATISFIED | `validate_chatgpt_keys()` checks 4 openai/* keys plus flat ui/resourceUri |

**Note:** APP-VAL IDs are defined in ROADMAP.md but do not appear in `.planning/REQUIREMENTS.md`. The REQUIREMENTS.md covers v1.5/v1.6 CLI requirements and does not include Phase 47 App validation requirements. The IDs are self-consistent across ROADMAP and PLAN frontmatter and all 5 are covered by the implementation.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

No TODO/FIXME/placeholder comments, no stub returns, no empty handlers found in any modified files.

### Human Verification Required

None — all success criteria are verifiable programmatically via code inspection. The behavior (correct exit code on failures, output formatting, CI integration) follows directly from the code structure verified above.

### Build and Test Results

- `cargo build -p mcp-tester -p cargo-pmcp`: Finished with 0 errors
- `cargo clippy -p mcp-tester -p cargo-pmcp -- -D warnings`: Finished with 0 warnings
- `cargo test -p mcp-tester`: 10 tests passed (8 AppValidator unit tests + 2 scenario tests); 5 doctests passed
- All 4 commits verified in git history: `a4463ff`, `84f9c57`, `960e096`, `40b07b7`

### Gaps Summary

No gaps. All must-haves from both plan frontmatter and ROADMAP success criteria are verified at all three levels (exists, substantive, wired). Both entry points (`mcp-tester apps` and `cargo pmcp test apps`) are fully implemented, the validation engine covers all required checks, strict mode, tool filtering, and the check command hint are all wired.

---

_Verified: 2026-03-11_
_Verifier: Claude (gsd-verifier)_
