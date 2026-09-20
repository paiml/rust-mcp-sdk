---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
verified: 2026-05-02T00:00:00Z
updated: 2026-05-02T00:00:00Z
status: gaps_found
score: 9/9 must-haves verified at lib boundary; AC-78-1..3 fail at binary boundary against real prod (cost-coach false-positive evidence)
overrides_applied: 0
human_verification:
  - test: "Run `cargo pmcp test apps --mode claude-desktop` against a real MCP server with broken widget HTML"
    expected: "Process exits non-zero AND stdout/stderr names at least one missing handler (e.g. `onteardown`). This is AC-78-1 at the binary boundary."
    why_human: "All 4 CLI E2E tests in `cargo-pmcp/tests/cli_acceptance.rs` skip-gate via `skip_if_no_fixture()` because the fixture binary `mcp_widget_server` is not yet wired in as a `[[bin]]` target. Library-level AC verification passed (7/7 widget integration tests + 5/5 apps_helpers tests), but the binary-level CLI exit code path is unverified by automation. Plan 78-02's REVISION HIGH-2 explicitly documented this as an accepted skip-gate."
  - test: "Run `cargo pmcp test apps --mode claude-desktop` against a corrected widget"
    expected: "Process exits zero. AC-78-2 at the binary boundary."
    why_human: "Same skip-gate reason as above — `cli_acceptance_corrected_widget_passes_claude_desktop` returns early when fixture binary is absent."
  - test: "Run `cargo pmcp test apps` (no flag, Standard mode) against both broken and corrected widgets"
    expected: "Both invocations exit zero — no regression for the permissive default. AC-78-3."
    why_human: "Same skip-gate reason — `cli_acceptance_standard_mode_passes_both_fixtures` returns early when fixture binary is absent."
  - test: "Run `cargo pmcp test apps --mode chatgpt` against both fixtures"
    expected: "Zero exit on both AND stderr/stdout MUST NOT contain any of the four protocol handler names (chatgpt mode is a no-op for widget validation). AC-78-4."
    why_human: "Same skip-gate reason — `cli_acceptance_chatgpt_mode_passes_both_and_no_handler_messages` returns early when fixture binary is absent."
  - test: "Visual review of `cargo pmcp test apps --help` and both READMEs"
    expected: "User can understand what `--mode claude-desktop` checks and how it differs from the other modes. AC-78-5."
    why_human: "Help text is verified to contain `claude-desktop`, `static`, and the 3-mode breakdown; READMEs contain `## App Validation` sections. But UX quality (clarity, discoverability, prose flow) is subjective and benefits from human review."
---

# Phase 78: cargo pmcp test apps --mode claude-desktop Verification Report

**Phase Goal:** Promote `AppValidationMode::ClaudeDesktop` from a placeholder to a real strict mode that statically inspects each App-capable widget HTML body (fetched via `resources/read`) for the `@modelcontextprotocol/ext-apps` import, the `new App({...})` constructor, the four required protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`), and the `app.connect()` call — emitting ERROR (vs WARN in Standard mode) on missing signals so widgets shipping to Claude Desktop / Claude.ai are caught before deploy.

**Verified:** 2026-05-02
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Acceptance Criteria from ROADMAP)

| #   | Truth                                                                                                                                          | Status              | Evidence                                                                                                                                                                                                                                                                                                                          |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | AC-78-1: Cost Coach reproducer (broken widget) FAILS `cargo pmcp test apps --mode claude-desktop` with errors that name the missing handler(s) | ✓ VERIFIED (lib) ⚠ NEEDS HUMAN (CLI) | Library-level test `test_broken_widget_fails_claude_desktop` passes — broken fixture under ClaudeDesktop emits ≥1 Failed rows whose `name` contains tool name `cost-coach` and includes handler names. CLI E2E test `cli_acceptance_broken_widget_fails_claude_desktop` skip-gates (no fixture binary). |
| 2   | AC-78-2: The corrected version PASSES `cargo pmcp test apps --mode claude-desktop`                                                              | ✓ VERIFIED (lib) ⚠ NEEDS HUMAN (CLI) | Library test `test_corrected_widget_passes_claude_desktop` passes — corrected fixture emits ZERO Failed rows. CLI E2E `cli_acceptance_corrected_widget_passes_claude_desktop` skip-gates.                                                                                                       |
| 3   | AC-78-3: `cargo pmcp test apps` (no flag, Standard mode) still passes for both — no regression for the permissive default                     | ✓ VERIFIED (lib) ⚠ NEEDS HUMAN (CLI) | Library tests `test_standard_mode_one_summary_warn_for_broken` (1 Warning, 0 Failed) and `test_corrected_widget_passes_standard_too` (0/0) pass. CLI E2E `cli_acceptance_standard_mode_passes_both_fixtures` skip-gates.                                                                                |
| 4   | AC-78-4: `--mode chatgpt` behavior unchanged                                                                                                   | ✓ VERIFIED (lib) ⚠ NEEDS HUMAN (CLI) | Library tests `test_chatgpt_mode_unchanged_zero_results` and `test_chatgpt_mode_zero_results_corrected_too` pass — both assert `results.len() == 0` for ChatGpt mode. CLI E2E `cli_acceptance_chatgpt_mode_passes_both_and_no_handler_messages` skip-gates.                                              |
| 5   | AC-78-5: README + `--help` document the new mode                                                                                                | ✓ VERIFIED          | `cargo run -p cargo-pmcp -- test apps --help` output contains `claude-desktop` (2 matches), `statically inspects`, all 4 handler names, `--tool` honoring note. `cargo-pmcp/README.md` line 141 has `## App Validation` section with 3-mode table + `--tool` example + MIME profile note + Vite singlefile note + Pitfall 1 disclosure. `crates/mcp-tester/README.md` line 104 has corresponding section.        |
| 6   | PHASE-78-ALWAYS-UNIT: Unit tests for each new check (positive and negative cases for each handler / SDK signal)                               | ✓ VERIFIED          | 28 unit tests in `crates/mcp-tester/src/app_validator.rs::tests` (8 pre-existing + 11 from Plan 78-01 Task 1 + 9 from Task 2). 8 unit tests in `cargo-pmcp/src/commands/test/apps.rs::tests`. `cargo test -p mcp-tester app_validator` reports 56 passed.                                                                                                |
| 7   | PHASE-78-ALWAYS-PROPERTY: Property tests for the script-block scanner                                                                          | ✓ VERIFIED          | `crates/mcp-tester/tests/property_tests.rs` contains `prop_scan_never_panics` (alphabet `\PC{0,4096}`) and `prop_whitespace_idempotent` (alphabet `[a-zA-Z<>/= .]{0,500}`). `cargo test -p mcp-tester --test property_tests` reports 2 passed.                                                                       |
| 8   | PHASE-78-ALWAYS-FUZZ: Fuzz target for the regex/AST scan path                                                                                  | ✓ VERIFIED          | `fuzz/fuzz_targets/app_widget_scanner.rs` exists with `#![no_main]` + `fuzz_target!` + UTF-8 guard + `mcp_tester::AppValidator::validate_widgets` call with three-element tuple. `fuzz/Cargo.toml` lines 30-31, 111-112 register `[dependencies.mcp-tester]` + `[[bin]] name = "app_widget_scanner"`. (Worktree-local build constrained — see deferred-items.md.) |
| 9   | PHASE-78-ALWAYS-EXAMPLE: A working `cargo run --example` showing a broken widget that fails and a corrected one that passes                  | ✓ VERIFIED          | `crates/mcp-tester/examples/validate_widget_pair.rs` exists. `cargo run -p mcp-tester --example validate_widget_pair` exits 0 and prints both reports — broken_no_sdk shows 8 Failed rows + 1 Warning, corrected_minimal shows 8 Passed.                                                                                       |

**Score:** 9/9 truths verified at the library level. CLI binary boundary verification deferred to human (5 human_verification items).

### Required Artifacts

| Artifact                                                                | Expected                                                                                                                              | Status     | Details                                                                                                                                                                                  |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/mcp-tester/src/app_validator.rs`                                | `validate_widgets` + `WidgetSignals` + `scan_widget` + `extract_inline_scripts` + `strip_js_comments` + emit helpers + 20 new tests | ✓ VERIFIED | 1396 lines. Contains all required symbols (verified via grep): `pub fn validate_widgets` (line 601), `pub(crate) struct WidgetSignals` (line 128), `fn scan_widget` (191), `fn extract_inline_scripts` (171), `fn strip_js_comments` (157), `pub fn extract_resource_uri` (331), `if matches!(self.mode, AppValidationMode::ChatGpt)` (604), `[guide:handlers-before-connect]` token. Placeholder `same as Standard for now` removed. |
| `cargo-pmcp/src/commands/test/apps.rs`                                  | `read_widget_bodies` + `first_text_body` + `dedup_widget_uris` + `make_read_failure_result` + execute() wiring                       | ✓ VERIFIED | 507 lines. `MAX_WIDGET_BODY_BYTES` (29), `dedup_widget_uris` (300), `read_widget_bodies` (323), `first_text_body` (383), `make_read_failure_result` (398). `validator.validate_widgets(&widget_bodies)` called at line 146; `tool_filter` applied at read site (line 134). |
| `crates/mcp-tester/tests/property_tests.rs`                             | 2 proptests with three-element tuple                                                                                                  | ✓ VERIFIED | 38 lines. Both `prop_scan_never_panics` and `prop_whitespace_idempotent` present.                                                                                                       |
| `crates/mcp-tester/tests/app_validator_widgets.rs`                      | 7 integration tests covering all three modes                                                                                          | ✓ VERIFIED | 150 lines, 7 `#[test]` functions. All pass.                                                                                                                                              |
| `crates/mcp-tester/tests/error_messages_anchored.rs`                    | 3 integration tests (anchor expansion + tool-name presence)                                                                           | ✓ VERIFIED | 87 lines, 3 `#[test]` functions. All pass.                                                                                                                                               |
| `crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html`           | Cost Coach reproducer; uses `window.openai`; no SDK, no handlers; comments contain no signal literals                                 | ✓ VERIFIED | Present. `window.openai` literal present.                                                                                                                                                |
| `crates/mcp-tester/tests/fixtures/widgets/broken_no_handlers.html`      | SDK import + `new App()` but no handlers                                                                                              | ✓ VERIFIED | Present. `@modelcontextprotocol/ext-apps` literal present.                                                                                                                              |
| `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html`       | Minimal valid widget per GUIDE.md                                                                                                     | ✓ VERIFIED | Present. `onteardown` literal present.                                                                                                                                                  |
| `crates/mcp-tester/tests/fixtures/widgets/README.md`                    | Per-fixture mode-emission table                                                                                                       | ✓ VERIFIED | Present.                                                                                                                                                                                 |
| `crates/mcp-tester/examples/validate_widget_pair.rs`                    | ALWAYS-requirement working example                                                                                                    | ✓ VERIFIED | 49 lines. `cargo run -p mcp-tester --example validate_widget_pair` exits 0 with both reports printed.                                                                                  |
| `fuzz/fuzz_targets/app_widget_scanner.rs`                               | libfuzzer harness with three-element tuple                                                                                            | ✓ VERIFIED | 17 lines. `#![no_main]`, `fuzz_target!`, `mcp_tester::AppValidator::validate_widgets`, `"fuzz-tool"` literal — all present.                                                          |
| `fuzz/Cargo.toml`                                                       | `[[bin]]` entry + `[dependencies.mcp-tester]`                                                                                         | ✓ VERIFIED | Lines 30-31, 111-112.                                                                                                                                                                   |
| `cargo-pmcp/tests/apps_helpers.rs`                                      | 5 integration tests covering wiring                                                                                                   | ✓ VERIFIED | 190 lines. 5 `#[test]` functions. `cargo test -p cargo-pmcp --test apps_helpers` reports 5 passed.                                                                                       |
| `cargo-pmcp/tests/cli_acceptance.rs`                                    | 4 CLI E2E acceptance tests (REVISION HIGH-2)                                                                                          | ⚠️ ORPHANED | 146 lines, 4 `#[test]` functions present. Tests skip-gate via `skip_if_no_fixture()` because fixture binary is not built — they pass-by-skipping rather than asserting CLI behavior. **The TEST FILE is wired and reviewable, but the assertions are not exercised** until the fixture binary lands as a `[[bin]]` target. |
| `cargo-pmcp/README.md`                                                  | `## App Validation` section with 3-mode comparison + `--tool` note                                                                    | ✓ VERIFIED | Line 141. Contains `claude-desktop`, `--tool`, `MIME profile`, `Vite singlefile`, `mcp-apps-chess` (Pitfall 1).                                                                          |
| `crates/mcp-tester/README.md`                                           | `## App Validation` section                                                                                                           | ✓ VERIFIED | Line 104. Contains `claude-desktop`.                                                                                                                                                    |
| `cargo-pmcp/src/commands/test/mod.rs`                                   | Enriched `///`-doc on `TestCommand::Apps`                                                                                             | ✓ VERIFIED | `cargo run -p cargo-pmcp -- test apps --help` shows 3-mode breakdown, `statically inspects`, `Honors `--tool``.                                                                          |
| `src/server/mcp_apps/GUIDE.md`                                          | 5 explicit HTML id anchors                                                                                                            | ✓ VERIFIED | Lines 135, 184, 198, 335, 428: `id="csp-external-resources"`, `handlers-before-connect`, `do-not-pass-tools`, `vite-singlefile`, `common-failures-claude` — all present.        |
| `crates/mcp-tester/src/report.rs`                                       | `expand_guide_anchor` helper + `print_to_writer` writer-seam                                                                          | ✓ VERIFIED | Line 19 (`pub fn expand_guide_anchor`), line 244 (`pub fn print_to_writer<W: Write>`). Wired into `print_test_result_pretty` (line 326) and `print_verbose` (line 503).             |

### Key Link Verification

| From                                                            | To                                                | Via                                  | Status     | Details                                                                           |
| --------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------ | ---------- | --------------------------------------------------------------------------------- |
| `cargo-pmcp/src/commands/test/apps.rs::execute`                 | `mcp_tester::AppValidator::validate_widgets`      | `validator.validate_widgets(...)`    | ✓ WIRED    | Line 146: `results.extend(validator.validate_widgets(&widget_bodies));`           |
| `apps.rs::read_widget_bodies`                                   | `mcp_tester::ServerTester::read_resource`         | best-effort async per-URI read       | ✓ WIRED    | Line 333 area: `tester.read_resource(uri).await`. Confirmed via grep.            |
| `apps.rs::read_widget_bodies`                                   | `AppValidator::extract_resource_uri`              | derives URI from tool _meta          | ✓ WIRED    | `extract_resource_uri` is now `pub` (line 331 in app_validator.rs).               |
| `app_validator.rs::validate_widgets`                            | `app_validator.rs::scan_widget`                   | per-widget call                      | ✓ WIRED    | scan_widget is private but called from validate_widgets path.                     |
| `app_validator.rs::scan_widget`                                 | `app_validator.rs::extract_inline_scripts`        | concatenates `<script>` bodies        | ✓ WIRED    |                                                                                   |
| `app_validator.rs::extract_inline_scripts`                      | `app_validator.rs::strip_js_comments`              | strips comments before signal sweep   | ✓ WIRED    |                                                                                   |
| `tests/app_validator_widgets.rs`                                | `tests/fixtures/widgets/broken_no_sdk.html`        | `include_str!`                       | ✓ WIRED    | Plan 78-03 verified.                                                              |
| `fuzz/fuzz_targets/app_widget_scanner.rs`                       | `mcp_tester::AppValidator::validate_widgets`      | three-element tuple                  | ✓ WIRED    | Line 12 of fuzz target.                                                          |
| `examples/validate_widget_pair.rs`                              | `tests/fixtures/widgets/corrected_minimal.html`    | `include_str!`                       | ✓ WIRED    | Example exits 0 — confirms data flows.                                          |
| `cargo-pmcp/tests/cli_acceptance.rs`                            | the cargo-pmcp binary                             | `assert_cmd::Command::cargo_bin`     | ⚠️ NOT_WIRED | Tests skip-gate when fixture binary missing — present in source but functionally orphaned. See deferred items. |
| `report.rs::expand_guide_anchor`                                | `GUIDE.md#handlers-before-connect`                | string replace                       | ✓ WIRED    | KNOWN_SLUGS array contains all 5 slugs; matching anchors exist in GUIDE.md.        |
| `report.rs::print_test_result_pretty` / `print_verbose`         | `expand_guide_anchor`                              | called at render time                | ✓ WIRED    | Lines 326, 503.                                                                  |

### Data-Flow Trace (Level 4)

| Artifact                                                        | Data Variable           | Source                                                      | Produces Real Data | Status      |
| --------------------------------------------------------------- | ----------------------- | ----------------------------------------------------------- | ------------------ | ----------- |
| `apps.rs::execute` → results                                    | `widget_bodies`         | `read_widget_bodies` reads via `tester.read_resource(uri)`  | Yes                | ✓ FLOWING   |
| `validate_widgets` → Vec<TestResult>                            | `signals`               | `scan_widget(html)` reads from real widget HTML body        | Yes                | ✓ FLOWING   |
| `validate_widget_pair` example → reports                        | `results`               | `validate_widgets` invoked on `include_str!` fixture HTML   | Yes (verified by demo run) | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior                                                                                | Command                                                                                            | Result                                                                                  | Status     |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ---------- |
| Lib-level: AppValidator emits errors for broken widget under ClaudeDesktop              | `cargo test -p mcp-tester --test app_validator_widgets`                                            | 7 passed                                                                                | ✓ PASS     |
| Property tests don't panic on arbitrary input                                           | `cargo test -p mcp-tester --test property_tests`                                                    | 2 passed                                                                                | ✓ PASS     |
| Anchor expansion works end-to-end                                                       | `cargo test -p mcp-tester --test error_messages_anchored`                                          | 3 passed                                                                                | ✓ PASS     |
| CLI plumbing tests pass                                                                 | `cargo test -p cargo-pmcp --test apps_helpers --test cli_acceptance`                               | 9 passed (5 + 4-skip-gated)                                                             | ✓ PASS     |
| Working example exits 0                                                                 | `cargo run -p mcp-tester --example validate_widget_pair`                                           | Exit 0; both reports printed                                                            | ✓ PASS     |
| `--help` long-text mentions claude-desktop and "static"                                 | `cargo run -p cargo-pmcp -- test apps --help`                                                      | 2 mentions of `claude-desktop`, 1 mention of `static`                                   | ✓ PASS     |
| All app_validator unit tests pass (regression check)                                    | `cargo test -p mcp-tester app_validator`                                                           | 56 passed                                                                               | ✓ PASS     |
| CLI binary AC-78-1 (broken fails)                                                       | (Skip-gated `cli_acceptance_broken_widget_fails_claude_desktop`)                                   | Test skips (no fixture binary)                                                          | ? SKIP     |
| CLI binary AC-78-2 (corrected passes)                                                    | (Skip-gated `cli_acceptance_corrected_widget_passes_claude_desktop`)                              | Test skips                                                                              | ? SKIP     |
| CLI binary AC-78-3 (Standard mode passes both)                                          | (Skip-gated `cli_acceptance_standard_mode_passes_both_fixtures`)                                  | Test skips                                                                              | ? SKIP     |
| CLI binary AC-78-4 (chatgpt mode no handler output)                                     | (Skip-gated `cli_acceptance_chatgpt_mode_passes_both_and_no_handler_messages`)                    | Test skips                                                                              | ? SKIP     |

### Requirements Coverage

| Requirement              | Source Plan          | Description                                                              | Status      | Evidence                                                                                     |
| ------------------------ | -------------------- | ------------------------------------------------------------------------ | ----------- | -------------------------------------------------------------------------------------------- |
| PHASE-78-AC-1            | 78-01, 78-02, 78-03   | Cost Coach reproducer FAILS `--mode claude-desktop` with named handlers   | ✓ SATISFIED (lib) ⚠ NEEDS HUMAN (CLI) | `test_broken_widget_fails_claude_desktop` passes — assertion that Failed row's name contains `cost-coach`. CLI binary boundary needs human verification. |
| PHASE-78-AC-2            | 78-01, 78-02, 78-03   | Corrected version PASSES                                                  | ✓ SATISFIED (lib) ⚠ NEEDS HUMAN (CLI) | `test_corrected_widget_passes_claude_desktop` passes (0 Failed). CLI binary boundary needs human verification. |
| PHASE-78-AC-3            | 78-01, 78-02, 78-03   | `cargo pmcp test apps` (Standard) still passes both                       | ✓ SATISFIED (lib) ⚠ NEEDS HUMAN (CLI) | `test_standard_mode_one_summary_warn_for_broken` and `test_corrected_widget_passes_standard_too` pass. CLI binary boundary needs human verification. |
| PHASE-78-AC-4            | 78-01, 78-02, 78-03   | `--mode chatgpt` behavior unchanged                                       | ✓ SATISFIED (lib) ⚠ NEEDS HUMAN (CLI) | Two integration tests assert `results.len() == 0` for ChatGpt mode. 8 pre-existing tests still green. CLI binary boundary needs human verification. |
| PHASE-78-AC-5            | 78-04                | README + `--help` document the new mode                                  | ✓ SATISFIED | Both READMEs have `## App Validation`; `--help` long-text covers all 3 modes incl. `--tool` honoring; `claude-desktop` and `static` both grep-findable in help output. |
| PHASE-78-ALWAYS-UNIT     | 78-01, 78-02         | Unit tests for each new check                                             | ✓ SATISFIED | 28 unit tests in `app_validator.rs::tests` (8 pre-existing + 20 new) + 8 in `apps.rs::tests`. |
| PHASE-78-ALWAYS-PROPERTY | 78-03                | Property tests for the script-block scanner                               | ✓ SATISFIED | 2 proptests in `property_tests.rs` covering panic-freedom + whitespace idempotence. |
| PHASE-78-ALWAYS-FUZZ     | 78-03                | Fuzz target for the regex/AST scan path                                   | ✓ SATISFIED | `fuzz/fuzz_targets/app_widget_scanner.rs` + `[[bin]]` registration in `fuzz/Cargo.toml`. (Worktree-local build constraint documented in deferred-items.md is environmental, not a regression.) |
| PHASE-78-ALWAYS-EXAMPLE  | 78-03                | A working `cargo run --example` showing broken/corrected pair             | ✓ SATISFIED | `examples/validate_widget_pair.rs` runs cleanly and prints both reports. |

**No orphaned requirements** — every PLAN frontmatter requirement maps to evidence; every ROADMAP success criterion (AC-78-1..5) is exercised at the library level.

### Anti-Patterns Found

| File                                  | Line | Pattern                                                | Severity | Impact                                                                              |
| ------------------------------------- | ---- | ------------------------------------------------------ | -------- | ----------------------------------------------------------------------------------- |
| `cargo-pmcp/tests/cli_acceptance.rs`  | 38-50 | `skip_if_no_fixture()` returns early when binary absent | ⚠️ Warning | The 4 CLI E2E acceptance tests pass-by-skip rather than asserting CLI behavior. Documented in plan as REVISION HIGH-2 + Plan 02 SUMMARY explicitly accepts this gating. The TEST FILE is the load-bearing artifact (assertions are written and reviewable); the fixture binary `mcp_widget_server` not yet wired as `[[bin]]`. AC-78-1..4 are NOT verified at the CLI binary boundary by automation. |
| `crates/mcp-tester/src/app_validator.rs` | (multiple) | `#[allow(dead_code)]` annotations on validator helpers | ℹ️ Info | Plan 78-01 SUMMARY documents these as transient annotations; Plan 78-02 Task 2 was supposed to remove some but `dead_code` allows persist (per 78-REVIEW WARN). Not a goal blocker; cosmetic cleanup. |

No TODO/FIXME/placeholder comments in any modified file (verified via grep). The placeholder `same as Standard for now` is removed from `app_validator.rs`.

### Human Verification Required

See `human_verification` block in frontmatter. Five items:

1. **AC-78-1 at CLI binary boundary** — broken widget fails with named handler in stderr/stdout
2. **AC-78-2 at CLI binary boundary** — corrected widget exits zero
3. **AC-78-3 at CLI binary boundary** — Standard mode passes both fixtures
4. **AC-78-4 at CLI binary boundary** — chatgpt mode emits no handler messages
5. **UX review of READMEs and `--help` text** — discoverability and clarity

The CLI binary boundary verification is gated on a fixture binary that the planner explicitly deferred to a follow-up phase (REVISION HIGH-2 in Plan 78-02; documented at `cargo-pmcp/tests/fixtures/mcp_widget_server.rs.todo`). Until that fixture binary lands as a `[[bin]]` target, the only path to verifying AC-78-1..4 at the binary boundary is manual: the developer points `cargo pmcp test apps --mode claude-desktop` at a real running MCP server with broken/corrected widgets.

### Gaps Summary (UPDATED 2026-05-02 — cost-coach prod feedback)

Status escalated from `human_needed` → `gaps_found`. Human UAT against the cost-coach production server (`https://cost-coach.us-west.pmcp.run/mcp`, 8 widgets, 97 tests, 33 failures, 4 warnings) revealed that **all 33 failures are false positives**. Every cost-coach widget has the SDK correctly wired and renders correctly in Claude Desktop / claude.ai. The validator's static-analysis patterns are defeated by Vite singlefile minification on bundled prod widgets. AC-78-1..3 fail at the binary boundary against real prod input.

Source: `/Users/guy/projects/mcp/cost-coach/drafts/feedback-pmcp-test-apps-v1-false-positives.md` (8.4 KB, dated 2026-05-02). Three concrete root causes:

#### Gap G1 (HIGH, blocks AC-78-3): SDK-presence regex `import.*ext-apps` matches zero bundled widgets

Vite singlefile inlines `import { App } from '@modelcontextprotocol/ext-apps'` — the literal import string is gone from prod bundles. The package name only survives as a log-prefix string `[ext-apps]` inside the inlined SDK runtime (e.g. `[ext-apps] App.${e}() called before connect`).

**Evidence:** Cost-coach reports `grep '[ext-apps]' widget/dist/*.html` finds the log prefix in all 8 bundles, but `import.*ext-apps` matches zero.

**Fix:** Replace the `import.*ext-apps` literal with three minification-resistant signals (any one is sufficient SDK evidence): `[ext-apps]` log prefix OR `ui/initialize` method string OR `ui/notifications/tool-result` method string. These are protocol-level JSON-RPC method names — minifiers cannot rename them.

**Acceptance:** `app_validator::scan_widget` returns `signals.has_sdk = true` for `widget/dist/cost-summary.html` (Cost-Coach's actual prod bundle, captured as a fixture).

#### Gap G2 (HIGH, blocks AC-78-1, AC-78-3): Constructor regex `new App\(` matches zero bundled widgets

Vite minification renames the imported `App` class to a 2-letter identifier (different per bundle hash): `App` → `yl` / `gl` / `ol` / etc. The current regex `new App\(` finds nothing on minified bundles. The user-controlled payload `{name: "...", version: "..."}` survives intact because that's the data the cost-coach team passes in.

**Evidence:** Cost-coach reports concrete construction sites: `new yl({name:"cost-coach-cost-summary",version:"1.0.0"})`, `new gl({name:"cost-coach-cost-over-time",version:"1.0.0"})`, `new ol({name:"cost-coach-savings-summary",version:"1.0.0"})`.

**Fix:** Replace `new App\(` with a regex that tolerates a mangled constructor identifier paired with the unmangled payload: `new [a-zA-Z_$][a-zA-Z0-9_$]{0,5}\(\s*\{\s*name\s*:\s*"[^"]+"\s*,\s*version\s*:\s*"[^"]+"\s*\}`. The `{name, version}` literal is what user code controls; minifiers don't touch string keys or quoted values.

**Acceptance:** `app_validator::scan_widget` returns `signals.has_app_constructor = true` for all three captured fixtures (`cost-summary.html`, `cost-over-time.html`, `savings-summary.html`).

#### Gap G3 (HIGH, blocks AC-78-1): SDK-detection failure cascades to all 8 handler/connect checks (single FN → 8× FN)

Output across cost-coach widgets: 5 widgets where SDK was somehow detected → 1 failure each (only `new App({...})`); 3 widget URIs where SDK was not detected → **8 failures each** (every check). This pattern is the classic cascade: when the SDK-presence signal is missing, the validator treats the entire widget as un-wired and fails every downstream check, ignoring the per-handler evidence that the bundle does contain `\.onteardown\s*=`, `\.connect\(\)`, etc.

**Evidence:** From feedback report — "5 widgets where SDK import was somehow detected: 1 failure each (only `new App({...})`); 3 widget URIs where SDK import wasn't detected: 8 failures each (everything). Single false negative → 8× false negatives."

**Fix:** Make every per-signal check independent of `has_sdk`. If `\.onteardown\s*=` is in the bundle, that's positive handler evidence regardless of whether the SDK-presence heuristic fired. The cascade should be reserved for cases where independent evidence is genuinely absent. Member names (`.onteardown`, `.ontoolinput`, `.ontoolcancelled`, `.onerror`, `.ontoolresult`) and method names (`.connect()`) are protocol-level identifiers the SDK exposes by string name — they survive minification untouched.

**Acceptance:** A widget with all 5 handler member-assignments and `.connect()` PRESENT but `[ext-apps]` log prefix ABSENT must report `has_handlers = true`, `has_connect = true`, `has_sdk = false` — three independent verdicts, no cascade.

#### Gap G4 (MEDIUM, follow-up to G1–G3): No `--widgets-dir <path>` source-scan mode

The validator currently fetches `resources/read` and scans the served HTML — the bundle. Bundles are the worst surface to static-analyze: minified, inlined, intentionally compact. Source HTML (e.g. `widget/*.html` in cost-coach's repo) has unmangled identifiers, intact import statements, clear handler assignments. Pre-deploy this is the higher-confidence path.

**Fix:** Add `--widgets-dir <path>` flag to `cargo pmcp test apps`. When provided, the command scans `<path>/*.html` instead of fetching via `resources/read`. `cargo pmcp preview` already has a similar flag — same name, same path-resolution semantics, different scanner.

**Acceptance:** `cargo pmcp test apps --widgets-dir cost-coach/widget --mode claude-desktop` exits zero and reports zero failures; `cargo pmcp test apps https://cost-coach.us-west.pmcp.run/mcp --mode claude-desktop` (bundle scan) also exits zero after G1+G2+G3 land.

#### Gap G5 (MEDIUM): Captured cost-coach bundle fixtures missing from regression suite

We have synthetic fixtures (`broken_no_sdk.html`, `broken_no_handlers.html`, `corrected_minimal.html`) but no fixtures that resemble real Vite singlefile bundle output. Without these, the false-positive class is invisible to the test suite — the test fixtures look nothing like the prod bundles that broke.

**Fix:** Add three captured-from-prod fixtures (or convincing synthesizations) under `crates/mcp-tester/tests/fixtures/widgets/bundled/`:
- `cost_summary_minified.html` (mangled `new yl({name:"...",version:"..."})` constructor; all 5 handlers; `.connect()`; `[ext-apps]` log prefix; no `import` literal)
- `cost_over_time_minified.html` (same shape, different mangled id)
- `synthetic_cascade_repro.html` (minimal: handlers + connect present; SDK signals absent — must report `has_handlers=true`, `has_sdk=false`, NO cascade)

Plus integration tests asserting the exact verdict shape per fixture × mode (Standard/ClaudeDesktop/ChatGpt). Use the cost-coach evidence as the spec: the test must FAIL today (exposing G1+G2+G3) and PASS after fixes.

#### Cross-references

- Source feedback: `/Users/guy/projects/mcp/cost-coach/drafts/feedback-pmcp-test-apps-v1-false-positives.md`
- Cost-coach prod endpoint: `https://cost-coach.us-west.pmcp.run/mcp`
- v1 result on cost-coach: 97 tests, 60 passed, 33 failed, 4 warnings — overall FAILED
- All 33 failures are confirmed false-positives (cost-coach team verified each widget renders correctly in Claude Desktop AND that `widget/dist/*.html` contains all 5 handlers, mangled constructor, `.connect()`, `[ext-apps]` log prefix)
- Original 5 human_verification items (CLI binary boundary tests) are NO LONGER addressable in the current state — they were already executed against prod and found the false-positive class. Those items remain `pending` in `78-HUMAN-UAT.md` until the gap closure lands and re-verification is run.

#### Out of scope (defer to a later phase)

- Constructor-side detection of `Implementation` shape with non-empty `name` — already implicit in G2's regex. Don't deepen it.
- AST parsing of bundled JS via `swc` or similar — complexity exceeds value given regex patterns above are sufficient.
- `PreviewMode::ClaudeDesktop` host emulator — already deferred per phase 78 ROADMAP scope notes.
- Replacing the deferred `cli_acceptance.rs` fixture binary — orthogonal concern; keep as documented in `mcp_widget_server.rs.todo`.

#### Why `gaps_found` not `human_needed`

The previous `human_needed` flag assumed CLI verification was the only outstanding work. The cost-coach feedback proves that even a successful CLI run is currently misleading — the validator produces false-positive output on correctly-wired prod bundles. This is a detection-correctness gap, not a verification-rigor gap. Closing it requires code changes (G1–G5), not human attention. After G1–G3 land and re-verification against cost-coach prod returns zero failures, the original 5 human_verification items can be re-evaluated as part of `human_needed` resolution.

**Risk if NOT closed:** A 100% false-positive rate on correctly-wired widgets trains users to ignore the validator output. Then on the day they ship a real bug, the same red CI doesn't surface as a signal — it's been trained out as background noise. This second-order risk converts a useful pre-deploy check into anti-signal.

---
*Verified: 2026-05-02*
*Verifier: Claude (gsd-verifier)*
