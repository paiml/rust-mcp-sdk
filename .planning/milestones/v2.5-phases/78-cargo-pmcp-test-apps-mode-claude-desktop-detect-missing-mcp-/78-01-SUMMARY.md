---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
plan: 01
subsystem: testing
tags: [mcp-apps, validator, claude-desktop, regex, widget-validation, mcp-tester]

# Dependency graph
requires:
  - phase: 14-19 (v1.3 MCP Apps Developer Experience)
    provides: AppValidator scaffold, AppValidationMode enum, AppValidationMode::ClaudeDesktop placeholder
provides:
  - "AppValidator::validate_widgets — pure public API taking [(tool_name, uri, html)] tuples and returning Vec<TestResult>"
  - "Three-way mode dispatch: ClaudeDesktop (per-signal Failed), Standard (one summary Warning), ChatGpt (zero rows)"
  - "AppValidator::extract_resource_uri promoted from private to pub for Plan 02 reuse"
  - "WidgetSignals pub(crate) struct + scan_widget + extract_inline_scripts + strip_js_comments private helpers"
  - "13 OnceLock<Regex> compile-once accessors (10 signal regexes + 3 comment-strip regexes)"
  - "20 new unit tests asserting per-mode emission shape, comment-stripper correctness, and SDK signal fallback"
affects: [78-02 (CLI plumbing), 78-03 (property/fuzz tests), 78-04 (docs + GUIDE anchors)]

# Tech tracking
tech-stack:
  added: [no new runtime dependencies — regex 1.x already a runtime dep at crates/mcp-tester/Cargo.toml:33]
  patterns: ["OnceLock<Regex> compile-once accessor (one fn per regex, cog 1 each)", "Best-effort comment stripper applied BEFORE signal regex sweeps (REVISION HIGH-3)", "Mode-driven three-way emission shape (per RESEARCH Q4 RESOLVED)", "Tool-name-bearing tuple input for actionable error reports (REVISION HIGH-4)"]

key-files:
  created: [.planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/deferred-items.md]
  modified: [crates/mcp-tester/src/app_validator.rs]

key-decisions:
  - "Three-way mode dispatch: ClaudeDesktop=per-signal Failed, Standard=summary Warning, ChatGpt=zero rows (preserves AC-78-4 'chatgpt mode unchanged')"
  - "Comment-stripping happens INSIDE extract_inline_scripts before signal regex sweeps so commented-out scaffolding cannot satisfy detection (REVISION HIGH-3 fixture hygiene + correctness gap closure)"
  - "Best-effort comment stripper is NOT string-literal aware — accepted simplification because signal detection is presence-based (over-stripping makes false-NEGATIVES, never false-positives) and Plan 03 property test catches false-negatives"
  - "ontoolresult stays Warning (soft) regardless of mode per RESEARCH Locked Decision 3 — some widgets render from getHostContext().toolOutput"
  - "validate_widgets is mounted as a separate public API; NOT yet wired into validate_tools — Plan 02 owns the CLI plumbing in cargo-pmcp's commands/test/apps.rs"
  - "validate_widgets signature changed mid-plan from &[(uri, html)] to &[(tool_name, uri, html)] (REVISION HIGH-4) so emitted TestResult.name strings identify which tool the widget belongs to — meaningful tool_filter at the report layer"
  - "ChatGpt early-return guard (matches!(self.mode, AppValidationMode::ChatGpt)) means the function does literally zero work in that mode — preserves the 'chatgpt mode unchanged' acceptance criterion"
  - "Transient #[allow(dead_code)] applied on validate_widgets + helpers because mcp-tester is a lib + bin crate and the bin (src/main.rs) doesn't call validate_widgets yet — Plan 02 wires it via `cargo pmcp test apps` and removes the allows then"

patterns-established:
  - "Pattern: OnceLock<Regex> compile-once accessor — each accessor is its own cog-1 function, regex literals static so .unwrap() safe at runtime"
  - "Pattern: Best-effort comment stripper applied BEFORE signal regex sweeps — protects fixture hygiene AND closes a real correctness gap (real widgets with commented-out scaffolding cannot falsely pass)"
  - "Pattern: Mode-driven three-way emission shape — per-signal Failed (strict), per-widget summary Warning (default), zero rows (silent) — chosen via match self.mode"
  - "Pattern: Tool-name in emitted TestResult.name strings — '[<tool>][<uri>] <label>' format gives actionable error reports"

requirements-completed: [PHASE-78-AC-1, PHASE-78-AC-2, PHASE-78-AC-3, PHASE-78-AC-4, PHASE-78-ALWAYS-UNIT]

# Metrics
duration: 15min
completed: 2026-05-02
---

# Phase 78 Plan 01: Promote AppValidationMode::ClaudeDesktop to a real strict mode Summary

**`AppValidator::validate_widgets` ships as a pure mode-driven public API that statically scans inline widget HTML for MCP Apps SDK wiring and emits per-signal Failed (ClaudeDesktop), one summary Warning (Standard), or zero rows (ChatGpt) — with a best-effort JS/HTML comment stripper applied before signal regexes so commented-out scaffolding cannot falsely pass.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-05-02T17:30:44Z
- **Completed:** 2026-05-02T17:45:36Z
- **Tasks:** 2 (Task 1 + Task 2, both atomic commits)
- **Files modified:** 2 (`crates/mcp-tester/src/app_validator.rs` + new `deferred-items.md`)

## Accomplishments

- Promoted `AppValidationMode::ClaudeDesktop` from a placeholder ("same as Standard for now") to a real strict mode driven by static script-block inspection.
- Added `AppValidator::validate_widgets` public API with three-way mode dispatch (REVISION HIGH-1 ChatGpt no-op + REVISION HIGH-4 tool-name-in-name).
- Added 13 compile-once `OnceLock<Regex>` accessors (10 signal regexes + 3 comment-strip regexes per REVISION HIGH-3).
- Added best-effort `strip_js_comments` helper run BEFORE signal sweeps inside `extract_inline_scripts` — closes the real correctness gap where commented-out scaffolding code could falsely satisfy detection.
- Added 20 new unit tests (11 in Task 1 covering scanner/comment-stripper, 9 in Task 2 covering per-mode emission shape) including the load-bearing `scan_widget_ignores_signals_inside_comments` (REVISION HIGH-3) and `chatgpt_mode_emits_no_widget_results` (REVISION HIGH-1).
- Promoted `AppValidator::extract_resource_uri` from private to pub so Plan 02's CLI plumbing (`commands/test/apps.rs`) can derive widget URIs from tool metadata.
- All eight pre-existing tests still pass (PHASE-78-AC-4 regression check satisfied).

## Task Commits

Each task was committed atomically:

1. **Task 1: WidgetSignals scanner + regex helpers + extract_inline_scripts + strip_js_comments** — `1c58b130` (feat)
2. **Task 2: validate_widgets with three-way mode dispatch + per-mode emission helpers + 9 unit tests** — `ddf78103` (feat)

## Files Created/Modified

- `crates/mcp-tester/src/app_validator.rs` — added `use regex::Regex` + `use std::sync::OnceLock` imports, 13 `OnceLock<Regex>` accessors (`script_block_re`, `ext_apps_import_re`, `new_app_call_re`, `handler_onteardown_re`, `handler_ontoolinput_re`, `handler_ontoolcancelled_re`, `handler_onerror_re`, `handler_ontoolresult_re`, `connect_call_re`, `chatgpt_only_channels_re`, `html_comment_re`, `js_block_comment_re`, `js_line_comment_re`), `pub(crate) struct WidgetSignals`, `strip_js_comments`, `extract_inline_scripts`, `scan_widget`, rewrote `AppValidationMode::ClaudeDesktop` docstring (deletes "same as Standard for now"), promoted `extract_resource_uri` to `pub`, added `pub fn validate_widgets`, added private impl helpers `emit_results_for_claude_desktop` / `emit_summary_warning_for_standard` / `widget_result_strict` / `widget_ontoolresult_result` / `widget_chatgpt_only_failed`, added 20 new unit tests (11 in Task 1 + 9 in Task 2), fixed two clippy::manual_contains hits via `.contains(&name)`.
- `.planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/deferred-items.md` — created to log a pre-existing clippy::for_kv_map error in `crates/mcp-tester/examples/render_ui.rs` that is out of scope for Plan 78-01.

## Decisions Made

All decisions track the plan's `<must_haves>` and the three relevant cross-AI revisions (HIGH-1, HIGH-3, HIGH-4):

- **Three-way mode dispatch (REVISION HIGH-1, RESEARCH Q4 RESOLVED).** ClaudeDesktop emits one Failed row per missing signal (full breakdown, pre-deploy gate). Standard emits ONE summary Warning row per widget with missing signals listed in `details`. ChatGpt returns an empty `Vec<TestResult>` via early-return — preserves AC-78-4 "chatgpt mode unchanged" because the widget-validation surface did not exist before this phase, so the only correct preservation is no new rows.
- **Tool-name in tuple signature (REVISION HIGH-4).** `validate_widgets` takes `&[(String, String, String)]` (tool_name, uri, html). Every emitted `TestResult.name` includes the tool name, making error reports actionable when a server has many widgets and giving meaningful semantics to the existing `tool_filter` at the report layer.
- **Comment stripper applied BEFORE signal regex sweeps (REVISION HIGH-3).** `strip_js_comments` runs inside `extract_inline_scripts` so the signal regexes never see commented-out scaffolding code. Best-effort: not string-literal aware. Accepted simplification documented in the docstring — signal detection is presence-based, so over-stripping yields false-NEGATIVES (never false-positives), and Plan 03's property test catches false-negatives via the `\PC{0,4096}` alphabet.
- **`ontoolresult` stays Warning (soft) regardless of mode** per RESEARCH Locked Decision 3 — some widgets render from `getHostContext().toolOutput` rather than the handler. `widget_ontoolresult_result` emits `TestStatus::Warning` even under ClaudeDesktop.
- **SDK-presence signal accepts EITHER the literal `@modelcontextprotocol/ext-apps` import OR ≥3 of the 4 protocol-handler property assignments** so minified bundles where the App binding is renamed (e.g. `n.onteardown=`) still pass.
- **`<script type="application/json">` blocks excluded** from signal scanning so JSON data islands containing the import string do not produce false positives (Pitfall 3, threat T-78-05).
- **`extract_resource_uri` promoted to `pub`** (was private) so Plan 02 can derive widget URIs from tool metadata.
- **Transient `#[allow(dead_code)]` applied** on `validate_widgets` + helpers because `mcp-tester` is a lib + bin crate and the bin (`src/main.rs`) does not call `validate_widgets` yet (Plan 02 wires it via `cargo pmcp test apps` and removes the allows then). Documented in a multi-line comment block at the top of the regex section.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `#[allow(dead_code)]` to new `validate_widgets` + helpers + free functions + struct**

- **Found during:** Task 1 verification (`cargo clippy -p mcp-tester --all-targets -- -D warnings`)
- **Issue:** `mcp-tester` is a lib + bin crate where `src/main.rs` includes `mod app_validator;` directly. The bin's reachable graph from `main` does NOT call `validate_widgets` (Plan 02 wires it). Rust's dead-code lint flagged the entire chain (regex helpers → `scan_widget` → `validate_widgets`) as unused in the bin target. Lib + tests had no warnings because the test mod uses every helper.
- **Fix:** Applied `#[allow(dead_code)]` to each new free function (`script_block_re` … `js_line_comment_re`), the `WidgetSignals` struct, and the new methods on `AppValidator` (`validate_widgets`, `emit_results_for_claude_desktop`, `emit_summary_warning_for_standard`, `widget_result_strict`, `widget_ontoolresult_result`, `widget_chatgpt_only_failed`). Added a multi-line comment block at the top of the regex section explaining why and noting the allows are removed when Plan 02 lands.
- **Files modified:** `crates/mcp-tester/src/app_validator.rs`
- **Verification:** `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` exits 0.
- **Committed in:** `1c58b130` (Task 1 — initial application) and refined in `ddf78103` (Task 2 — re-applied after Task 2's body briefly removed them, then needed to be re-added because the bin still doesn't reach the new method).

**2. [Rule 1 - Bug] Fixed two `clippy::manual_contains` hits introduced in Task 2**

- **Found during:** Task 2 verification (`cargo clippy -p mcp-tester --lib --tests -- -D warnings`)
- **Issue:** The new `emit_results_for_claude_desktop` and `emit_summary_warning_for_standard` helpers used `s.handlers_present.iter().any(|h| *h == name)` which clippy 1.95 flags as inefficient vs `Vec::contains`.
- **Fix:** Replaced both occurrences with `s.handlers_present.contains(&name)`.
- **Files modified:** `crates/mcp-tester/src/app_validator.rs`
- **Verification:** Clippy clean on lib + tests + bins.
- **Committed in:** `ddf78103` (Task 2 commit).

---

**Total deviations:** 2 auto-fixed (1 blocking dead-code-chain suppression, 1 clippy::manual_contains bugfix).
**Impact on plan:** Both auto-fixes essential for completing the plan's `cargo clippy ... -- -D warnings` acceptance criterion. No scope creep.

## Issues Encountered

- **Pre-existing `clippy::for_kv_map` error in `crates/mcp-tester/examples/render_ui.rs:88`** surfaced when running `cargo clippy -p mcp-tester --all-targets -- -D warnings`. Verified pre-existing by stashing Plan 78-01 changes and re-running clippy against the base commit (`78a844e8`) — the error reproduced. Out of scope per executor SCOPE BOUNDARY rule (only auto-fix issues directly caused by the current task's changes). Logged to `.planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/deferred-items.md` with the suggested fix for a follow-up commit. Plan 78-01's clippy verification was scoped to `--lib --tests --bins` (which is clean) instead of the full `--all-targets`.

## User Setup Required

None — no external service configuration required. This plan ships pure Rust changes inside `crates/mcp-tester/src/app_validator.rs`.

## Next Phase Readiness

- **Plan 02 (CLI plumbing)** can now invoke `AppValidator::validate_widgets` from `cargo-pmcp/src/commands/test/apps.rs` and use `AppValidator::extract_resource_uri` (now `pub`) to derive widget URIs from tool metadata. Plan 02 will:
    1. Read widget bodies via `resources/read` for each App-capable tool's `ui.resourceUri` (with the 10 MB streaming cap per threat T-78-02).
    2. Apply the existing `tool_filter` at the read site so only the requested widgets are validated.
    3. Pass `&[(tool_name, uri, html)]` to `AppValidator::validate_widgets`.
    4. Remove the `#[allow(dead_code)]` annotations on `validate_widgets` and the impl helpers added in Plan 78-01.
- **Plan 03 (property + fuzz)** can now exercise `scan_widget` and `strip_js_comments` against arbitrary HTML/JS via the `\PC{0,4096}` alphabet to confirm panic-freedom and to catch any false-negatives introduced by the best-effort comment stripper.
- **Plan 04 (docs + GUIDE anchors)** can replace the `[guide:handlers-before-connect]` and `[guide:common-failures-claude]` anchor tokens in error details with real GUIDE.md links.
- **No blockers or concerns.** All 28 `app_validator` unit tests pass (8 pre-existing + 11 from Task 1 + 9 from Task 2). `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` and `cargo fmt --all -- --check` both exit 0.

## Self-Check: PASSED

- **Files claimed created exist:**
    - `.planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/deferred-items.md` — verified present.
- **Files claimed modified exist:**
    - `crates/mcp-tester/src/app_validator.rs` — verified present, contains all new symbols (`validate_widgets`, `WidgetSignals`, `scan_widget`, `extract_inline_scripts`, `strip_js_comments`, `emit_results_for_claude_desktop`, `emit_summary_warning_for_standard`, `widget_result_strict`, `widget_ontoolresult_result`, `widget_chatgpt_only_failed`).
- **Commits claimed exist:**
    - `1c58b130` — verified present in `git log --oneline -3`.
    - `ddf78103` — verified present in `git log --oneline -3`.
- **Acceptance criteria:**
    - `cargo test -p mcp-tester app_validator` — 56 passed, 0 failed (28 in lib + 28 in bin = same module instantiated from both crate targets).
    - `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` — exits 0.
    - `cargo fmt --all -- --check` — exits 0.
    - File contains `pub fn validate_widgets(&self, widget_bodies: &[(String, String, String)]) -> Vec<TestResult>` — verified.
    - File contains `if matches!(self.mode, AppValidationMode::ChatGpt) {` (REVISION HIGH-1 early-return) — verified.
    - File no longer contains `same as Standard for now` — verified (`grep -c` returned 0).
    - File contains `Claude Desktop mode: strictly validates widget HTML` — verified.
    - File contains `THREE-WAY split` — verified (2 matches).
    - File contains `[guide:handlers-before-connect]` literal — verified (4 matches in error messages).
    - File contains `MCP Apps widget wiring (summary)` — verified.
    - File contains `pub fn extract_resource_uri` — verified.

---
*Phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-*
*Plan: 01*
*Completed: 2026-05-02*
