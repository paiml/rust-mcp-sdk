---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
plan: 03
subsystem: testing
tags: [mcp-apps, testing, property-tests, fuzz, examples, fixtures]

# Dependency graph
requires:
  - phase: 78-01
    provides: "AppValidator::validate_widgets pure API + three-way mode dispatch + tool-name-bearing tuple"
  - phase: 78-02
    provides: "CLI plumbing exercising validate_widgets via cargo pmcp test apps"
provides:
  - "3 widget HTML fixtures (broken_no_sdk, broken_no_handlers, corrected_minimal) + README"
  - "2 proptest property tests (prop_scan_never_panics, prop_whitespace_idempotent)"
  - "7 integration tests in tests/app_validator_widgets.rs covering all three modes"
  - "1 fuzz target app_widget_scanner with three-element tuple input"
  - "1 working example validate_widget_pair demonstrating broken-vs-corrected pair"
  - "proptest = \"1\" added to mcp-tester [dev-dependencies]"
  - "[dependencies.mcp-tester] path entry in fuzz/Cargo.toml"
affects: [78-04 (docs + GUIDE anchors)]

# Tech tracking
tech-stack:
  added: ["proptest 1 (mcp-tester [dev-dependencies]) — property-based test framework"]
  patterns:
    - "Per-mode emission-shape integration tests (one fixture, three modes, three different result-vec shapes)"
    - "Fixture comment hygiene grep-loop (REVISION HIGH-3 belt-and-braces atop Plan 01's strip_js_comments)"
    - "Three-element tuple validator input throughout property/integration/fuzz/example call sites (REVISION HIGH-4)"
    - "ALWAYS-requirement quartet completed: PROPERTY (proptest) + UNIT (Plan 01) + FUZZ (libfuzzer harness) + EXAMPLE (cargo run --example)"

key-files:
  created:
    - "crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html (29 lines — Cost Coach reproducer)"
    - "crates/mcp-tester/tests/fixtures/widgets/broken_no_handlers.html (16 lines — SDK import + new App() but no handlers)"
    - "crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html (28 lines — minimal valid widget per GUIDE.md)"
    - "crates/mcp-tester/tests/fixtures/widgets/README.md (35 lines — per-fixture mode-emission table + REVISION HIGH-3 hygiene)"
    - "crates/mcp-tester/tests/property_tests.rs (39 lines — 2 proptests with 3-element tuple)"
    - "crates/mcp-tester/tests/app_validator_widgets.rs (151 lines — 7 integration tests)"
    - "crates/mcp-tester/examples/validate_widget_pair.rs (45 lines — ALWAYS-requirement working example)"
    - "fuzz/fuzz_targets/app_widget_scanner.rs (17 lines — libfuzzer harness with 3-element tuple)"
  modified:
    - "crates/mcp-tester/Cargo.toml (added proptest = \"1\" dev-dep)"
    - "fuzz/Cargo.toml (added [dependencies.mcp-tester] + [[bin]] entry for app_widget_scanner)"
    - ".planning/phases/78-.../deferred-items.md (logged worktree-only fuzz workspace-collision pre-existing issue)"

key-decisions:
  - "Three-element tuple is the canonical input shape EVERYWHERE — property tests, integration tests, fuzz harness, and the example all pass `(tool_name, uri, html)` per REVISION HIGH-4. This forces every consumer to supply a meaningful tool name, which the validator embeds in TestResult.name strings."
  - "REVISION HIGH-1 zero-results test for ChatGpt is asserted via results.len() == 0 (not just absence-of-Failed). Two separate tests cover broken AND corrected widgets to confirm ChatGpt mode is a no-op REGARDLESS of widget shape."
  - "REVISION HIGH-3 fixture-comment hygiene enforced by grep loop — comments describe absence in abstract terms only, never with literal signal strings. README.md publishes the grep recipe for future fixture authors."
  - "Proptest alphabet split into two regexes to test different invariants: \\PC{0,4096} (full unicode panic-freedom) for prop_scan_never_panics; [a-zA-Z<>/= .]{0,500} (constrained, whitespace-modifiable) for prop_whitespace_idempotent. The constrained alphabet is essential — random unicode would change result counts unpredictably under whitespace doubling."
  - "Example uses TestReport::print(OutputFormat::Pretty) rather than reimplementing report formatting — exercises the same code path the CLI uses, validates report rendering end-to-end."
  - "Worktree-only fuzz build failure logged to deferred-items.md (NOT a Plan 78-03 regression). The cargo workspace walker collides with the parent repository's Cargo.toml when invoked from inside the nested worktree path. ALL existing fuzz targets fail identically. CI and direct-repo invocations work normally because the parent repo's [workspace] excludes \"fuzz\" correctly."

patterns-established:
  - "Pattern: Three-element tuple as canonical validator input (consistent across property tests, integration tests, fuzz harness, and example)"
  - "Pattern: ALWAYS-requirement quartet (PROPERTY + UNIT + FUZZ + EXAMPLE) for every new public API in mcp-tester"
  - "Pattern: Fixture comment hygiene grep-loop as a CI-runnable quality gate (belt-and-braces hygiene atop scanner-side correctness fixes)"
  - "Pattern: Per-mode emission-shape integration tests — same fixture body, three modes, three different assertion shapes — exercises the validator's match self.mode dispatch comprehensively"

requirements-completed: [PHASE-78-AC-1, PHASE-78-AC-2, PHASE-78-AC-3, PHASE-78-AC-4, PHASE-78-ALWAYS-PROPERTY, PHASE-78-ALWAYS-FUZZ, PHASE-78-ALWAYS-EXAMPLE]

# Metrics
duration: ~18 min
completed: 2026-05-02
---

# Phase 78 Plan 03: Property tests + integration tests + fuzz target + example Summary

**The CLAUDE.md ALWAYS-requirement quartet (PROPERTY + UNIT + FUZZ + EXAMPLE) is now complete for Phase 78: 4 widget HTML fixtures land alongside 2 proptests, 7 integration tests, 1 libfuzzer harness, and 1 working example — all using the three-element tuple shape (REVISION HIGH-4) and asserting tight zero-results for ChatGpt mode (REVISION HIGH-1).**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-05-02T18:18:33Z
- **Completed:** 2026-05-02T18:36:42Z
- **Tasks:** 3 (each atomic commit)
- **Files created:** 8 (4 fixtures + 2 test files + 1 example + 1 fuzz target)
- **Files modified:** 3 (`crates/mcp-tester/Cargo.toml`, `fuzz/Cargo.toml`, `deferred-items.md`)

## Accomplishments

- **3 HTML fixtures + README landed** at `crates/mcp-tester/tests/fixtures/widgets/`. Fixtures match the plan's load-bearing shapes: `broken_no_sdk.html` (Cost Coach reproducer — uses `window.openai`, no SDK import), `broken_no_handlers.html` (SDK import + `new App({...})` but no handlers, no `connect()`), and `corrected_minimal.html` (minimal valid widget per `src/server/mcp_apps/GUIDE.md` §"Minimal widget example", lines 207-263). README.md publishes the per-fixture mode-emission table and the REVISION HIGH-3 grep recipe.
- **REVISION HIGH-3 fixture hygiene verified** by grep loop: `grep -E '(//|/\*|<!--).*(@modelcontextprotocol/ext-apps|new App\(|onteardown|ontoolinput|ontoolcancelled|onerror)'` exits zero across all three HTML fixtures.
- **2 proptest property tests passing** (`prop_scan_never_panics`, `prop_whitespace_idempotent`) — both invoke `validate_widgets` with the three-element tuple `("prop-tool", "ui://prop-test", html)` per REVISION HIGH-4. The whitespace-idempotent test asserts that doubling spaces and newlines does NOT change the count of `TestStatus::Failed` or `TestStatus::Warning` rows.
- **7 integration tests passing** in `crates/mcp-tester/tests/app_validator_widgets.rs`:
    1. `test_broken_widget_fails_claude_desktop` — broken_no_sdk under ClaudeDesktop emits >=1 Failed; every Failed row's `name` contains the tool name `cost-coach` (REVISION HIGH-4 assertion).
    2. `test_broken_widget_fails_claude_desktop_no_handlers` — broken_no_handlers emits >=4 Failed rows.
    3. `test_corrected_widget_passes_claude_desktop` — corrected_minimal emits ZERO Failed.
    4. `test_standard_mode_one_summary_warn_for_broken` — broken under Standard emits ZERO Failed and EXACTLY 1 Warning.
    5. `test_corrected_widget_passes_standard_too` — corrected under Standard emits ZERO Failed and ZERO Warning.
    6. `test_chatgpt_mode_unchanged_zero_results` — broken under ChatGpt emits `len() == 0` (REVISION HIGH-1 LOAD-BEARING).
    7. `test_chatgpt_mode_zero_results_corrected_too` — corrected under ChatGpt also emits `len() == 0`.
- **1 fuzz target landed** at `fuzz/fuzz_targets/app_widget_scanner.rs` (libfuzzer harness feeding arbitrary UTF-8 bytes into `validate_widgets` with the three-element tuple). Registered in `fuzz/Cargo.toml` `[[bin]]` block; `[dependencies.mcp-tester]` path entry added.
- **1 working example landed** at `crates/mcp-tester/examples/validate_widget_pair.rs` (ALWAYS-requirement EXAMPLE). Runs `validate_widgets` on broken + corrected fixtures under ClaudeDesktop, prints both reports via `TestReport::print(OutputFormat::Pretty)`, exits 0. Output snapshot below.
- **proptest = "1" added** to `crates/mcp-tester/Cargo.toml` `[dev-dependencies]`.
- All 138 mcp-tester tests pass (lib + 4 integration suites + main + doctests). Touched files clippy-clean and fmt-clean.

## Task Commits

Each task was committed atomically (`--no-verify` per parallel-executor protocol):

1. **Task 1: Widget HTML fixtures (3 HTML + README) with REVISION HIGH-3 hygiene** — `c4080351` (test)
2. **Task 2: proptest dev-dep + 2 proptests + 7 integration tests** — `e6f4612e` (test)
3. **Task 3: app_widget_scanner fuzz target + validate_widget_pair example + deferred-items log** — `178600b6` (feat)

## Files Created/Modified

- `crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html` (created, 29 lines): Cost Coach reproducer. Uses `window.openai`. Comments describe absence abstractly — no signal literals.
- `crates/mcp-tester/tests/fixtures/widgets/broken_no_handlers.html` (created, 16 lines): `import { App } from "@modelcontextprotocol/ext-apps"` + `new App({...})` real code lines, but no handlers, no `connect()`.
- `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html` (created, 28 lines): Minimal valid widget — SDK import, `new App({...})`, all 4 handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`), `app.connect()`. Includes a `<script type="application/json">` data island (exercises Pitfall 3 exclusion).
- `crates/mcp-tester/tests/fixtures/widgets/README.md` (created, 35 lines): Per-fixture mode-emission table; REVISION HIGH-3 grep recipe; deferred `corrected_minified.html` note.
- `crates/mcp-tester/tests/property_tests.rs` (created, 39 lines): 2 proptests using three-element tuple. `\PC{0,4096}` alphabet for panic-freedom; `[a-zA-Z<>/= .]{0,500}` constrained alphabet for whitespace-doubling idempotence.
- `crates/mcp-tester/tests/app_validator_widgets.rs` (created, 151 lines): 7 integration tests covering all three `AppValidationMode` shapes against all three fixtures.
- `crates/mcp-tester/examples/validate_widget_pair.rs` (created, 45 lines): Working example that calls `validate_widgets` on broken + corrected fixtures, prints reports via `TestReport::print(OutputFormat::Pretty)`.
- `fuzz/fuzz_targets/app_widget_scanner.rs` (created, 17 lines): libfuzzer harness — `#![no_main]` + `fuzz_target!` + UTF-8 decode guard + `mcp_tester::AppValidator::validate_widgets` with three-element tuple.
- `crates/mcp-tester/Cargo.toml` (modified): Added `proptest = "1"` to `[dev-dependencies]`.
- `fuzz/Cargo.toml` (modified): Added `[dependencies.mcp-tester] path = "../crates/mcp-tester"` + `[[bin]] name = "app_widget_scanner"` block.
- `.planning/phases/78-.../deferred-items.md` (modified): Appended worktree-only fuzz workspace-collision section with full reproduction details, verification that the issue is pre-existing (affects all fuzz targets), and explanation of why CI/direct-repo invocations are unaffected.

## Property Test Invariants Chosen

**Invariant 1 — `prop_scan_never_panics`:** For ANY string of up to 4096 non-control unicode codepoints (`\PC{0,4096}`), `validate_widgets` returns a `Vec<TestResult>` without panicking. This guards the regex-and-comment-stripper pipeline against pathological inputs that might trigger panics in future refactors.

**Invariant 2 — `prop_whitespace_idempotent`:** For any string drawn from `[a-zA-Z<>/= .]{0,500}` (a constrained alphabet that includes HTML structural characters but excludes regex-meaningful chars), doubling all single spaces (` ` → `  `) and all single newlines (`\n` → `\n\n`) MUST NOT change:
- The total length of the result vec.
- The count of `TestStatus::Failed` rows.
- The count of `TestStatus::Warning` rows.

This invariant catches a class of bugs where minified vs. unminified HTML would produce different signal-detection results — a real concern because users feed both human-formatted and Vite-bundled widget HTML into the validator.

The constrained alphabet is essential: random unicode would change result counts unpredictably under whitespace doubling because expanded characters can disrupt regex anchors. The chosen alphabet preserves the regex semantics of signal detection while still varying the input shape enough to exercise the whitespace-handling code path.

## Fuzz Target Shape

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;
use mcp_tester::AppValidationMode;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return; };
    let validator = mcp_tester::AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    let _ = validator.validate_widgets(&[(
        "fuzz-tool".to_string(),
        "ui://fuzz".to_string(),
        s.to_string(),
    )]);
});
```

- Skips non-UTF-8 inputs (early return) so the validator only sees well-formed strings — matches real-world widget HTML which is always UTF-8.
- Uses ClaudeDesktop mode (the strictest mode with the most signal-detection branches) for maximum coverage per byte fuzzed.
- Three-element tuple (REVISION HIGH-4) — fuzzes the `(tool_name, uri, html)` shape that the CLI feeds the validator in production.

## Example Output Snapshot

`cargo run -p mcp-tester --example validate_widget_pair` produces (excerpted):

```
MCP Apps widget validator — broken/corrected demo
==================================================

=== broken_no_sdk (mode = claude-desktop) ===

TEST RESULTS
════════════════════════════════════════════════════════════
Apps:
  ✗ [example-broken_no_sdk][ui://example-...   Widget does not import @modelcontextprotocol/ext-apps...
  ✗ [example-broken_no_sdk][ui://example-...   Widget does not call `new App({...})`...
  ✗ [example-broken_no_sdk][ui://example-...   Widget does not register `app.onteardown` before `connect()`...
  ✗ [example-broken_no_sdk][ui://example-...   Widget does not register `app.ontoolinput` before `connect()`...
  ✗ [example-broken_no_sdk][ui://example-...   Widget does not register `app.ontoolcancelled`...
  ✗ [example-broken_no_sdk][ui://example-...   Widget does not register `app.onerror`...
  ⚠ [example-broken_no_sdk][ui://example-...   Widget does not register `app.ontoolresult` (soft warning)...
  ✗ [example-broken_no_sdk][ui://example-...   Widget does not call `app.connect()`...
  ✗ [example-broken_no_sdk][ui://example-...   Widget uses `window.openai`/`window.mcpBridge` channels...
SUMMARY: Total 9, Passed 0, Failed 8, Warnings 1. Overall Status: FAILED
Summary for broken_no_sdk: 0 passed, 1 warnings, 8 failed.

=== corrected_minimal (mode = claude-desktop) ===

TEST RESULTS
Apps:
  ✓ [example-corrected_minimal][ui://exam...   (8 passing rows)
SUMMARY: Total 8, Passed 8. Overall Status: PASSED
Summary for corrected_minimal: 8 passed, 0 warnings, 0 failed.

Done. The broken widget produced Failed rows; the corrected one did not.
```

The example exits 0 even though the broken widget produces Failed rows — it's a DEMO, not a CI gate. The contrast between the two reports demonstrates the silent-fail bug Cost Coach hit (broken widget under ChatGpt mode would render fine; under ClaudeDesktop the validator now catches the issue pre-deploy).

## Decisions Made

All decisions track the plan's `<must_haves>` and the three relevant cross-AI revisions (HIGH-1, HIGH-3, HIGH-4):

- **Three-element tuple is canonical EVERYWHERE (REVISION HIGH-4).** Property tests, integration tests, fuzz harness, and the example all pass `(tool_name, uri, html)` to `validate_widgets`. This forces every consumer to supply a meaningful tool name, which the validator embeds in `TestResult.name` strings (`[<tool>][<uri>] <label>` format). The `test_broken_widget_fails_claude_desktop` integration test explicitly asserts that every Failed row's `name` contains `cost-coach`.
- **REVISION HIGH-1 zero-results assertion is `results.len() == 0`** — not "no Failed rows" by some weaker measure. Two separate tests confirm ChatGpt mode is a no-op regardless of widget shape: `test_chatgpt_mode_unchanged_zero_results` (against the broken widget that under ClaudeDesktop emits 9 rows) and `test_chatgpt_mode_zero_results_corrected_too` (against the corrected widget). Both assert `results.len() == 0`.
- **REVISION HIGH-3 fixture comment hygiene enforced by grep loop.** Comments describe widget absence abstractly (e.g., "no protocol handlers are wired up") without using literal signal strings (`@modelcontextprotocol/ext-apps`, `new App(`, handler names, `connect()`). The grep loop `for f in *.html; do grep -E '(//|/\*|<!--).*(<signals>)' "$f"; done` exits zero across all three fixtures — verified during Task 1 verification.
- **Proptest alphabet split into two regexes** to test different invariants. `\PC{0,4096}` (any non-control unicode, up to 4096 chars) drives `prop_scan_never_panics`. `[a-zA-Z<>/= .]{0,500}` (constrained alphabet) drives `prop_whitespace_idempotent` because random unicode would unpredictably change result counts under whitespace doubling.
- **Example uses `TestReport::print(OutputFormat::Pretty)`** rather than reimplementing report formatting — exercises the same code path the CLI uses, validates report rendering end-to-end.
- **Fuzz target uses `mcp_tester::AppValidator::` explicit path** in the call site (rather than `use mcp_tester::AppValidator` and bare `AppValidator::new`). This satisfies the plan's literal acceptance criterion that the file contains the substring `mcp_tester::AppValidator`. Functionally equivalent to the import-and-bare-name form.
- **proptest 1 added to `[dev-dependencies]`** (not `[dependencies]`) — proptest is only used by the test suite, never by the production library or binary. This avoids inflating the runtime dependency surface.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] cargo fmt formatting required after writing test files**

- **Found during:** Task 2 verification (`cargo fmt --all -- --check`)
- **Issue:** The test helper function `validate(mode, tool_name, html)` was being called with three short string-literal arguments on a single line, but `rustfmt` prefers multi-line function-call formatting when the closing paren-and-indented-block exceeds the column budget for assertion macros. `--check` flagged the formatting drift.
- **Fix:** Ran `cargo fmt --all`; rustfmt rewrote the call sites into vertical-arg-list format. Re-verified `cargo fmt --all -- --check` exits 0 and all 7 integration tests still pass.
- **Files modified:** `crates/mcp-tester/tests/app_validator_widgets.rs`
- **Committed in:** `e6f4612e` (Task 2 commit, after fmt was applied)

### Out-of-scope discoveries (NOT fixed inline)

**Worktree-only fuzz workspace-collision pre-existing issue.** Discovered during Task 3 verification when `cd fuzz && cargo build --bin app_widget_scanner` failed with cargo's "current package believes it's in a workspace when it's not" error. Verified the failure reproduces for ALL existing fuzz targets (`protocol_parsing`, `auth_flows`, etc.) when invoked from inside this worktree. Root cause: cargo's workspace walker resolves UPWARD past the worktree root and finds the parent repository's `Cargo.toml` (`/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml`) before stopping. The parent repo's `[workspace] exclude` does NOT include the worktree-specific `.claude/worktrees/agent-.../fuzz` path. This is a worktree-environment artifact, not a Plan 78-03 regression. Logged to `.planning/phases/78-.../deferred-items.md` with full reproduction details and verification.

**Pre-existing clippy error in `crates/mcp-tester/examples/render_ui.rs`** surfaces when running `cargo clippy -p mcp-tester --examples -- -D warnings`. This was already documented in `deferred-items.md` by Plan 78-01. Plan 78-03 keeps the verification scope to `--lib --tests --bins --example validate_widget_pair` (which is clippy-clean).

---

**Total deviations:** 1 auto-fixed (cargo fmt formatting drift, caught and fixed during Task 2 verification). 1 pre-existing worktree-environment issue logged. 1 pre-existing clippy error already logged by Plan 78-01.

**Impact on plan:** The fmt formatting fix did not change test logic — all 7 integration tests passed before and after. The worktree fuzz issue did not block the plan because: (a) the fuzz target file is syntactically valid, (b) `cargo fuzz list --fuzz-dir fuzz` succeeds and shows `app_widget_scanner` in the registered binaries, (c) the fuzz harness API surface is identical to the passing integration/property tests, and (d) CI and direct-repo invocations will work normally because the upstream Cargo.toml correctly excludes the upstream `fuzz/` directory.

## Issues Encountered

- **Worktree-only fuzz build limitation** (documented above and in `deferred-items.md`). All existing fuzz targets fail to build from inside this worktree because cargo's workspace walker collides with the parent repository's `Cargo.toml`. This is environmental and resolves automatically post-merge.
- **rustfmt vertical-arg-list reformatting** of multi-arg test helper calls — caught at `cargo fmt --all -- --check` and fixed by running `cargo fmt --all`. Standard rustfmt behavior; no semantic impact.

## User Setup Required

None — pure Rust changes inside `crates/mcp-tester/tests/`, `crates/mcp-tester/examples/`, `fuzz/fuzz_targets/`, plus dev-dep additions to `crates/mcp-tester/Cargo.toml` and `fuzz/Cargo.toml`. No external service configuration. No GUIDE/docs changes (Plan 78-04's responsibility).

## Threat Surface Compliance

Plan 78-03 introduces no new transport surface, no new auth path, and no new schema changes. The fuzz target operates on the existing `AppValidator::validate_widgets` API surface (Plan 78-01's trust boundary) and exercises the existing regex pipeline. The fuzz target's `std::str::from_utf8` early-return guard prevents non-UTF-8 inputs from reaching the validator (matches real-world conditions — widget HTML is always UTF-8).

No new `threat_flag` markers introduced.

## Next Phase Readiness

- **Plan 04 (docs + GUIDE anchors)** can now reference the fixture pair as a canonical broken-vs-corrected example in GUIDE.md updates. The example output snapshot in this SUMMARY can be lifted into GUIDE.md verbatim to demonstrate the validator's diagnostic output.
- **Phase 78 ALWAYS-requirements quartet is COMPLETE:**
    - PROPERTY: 2 proptests in `crates/mcp-tester/tests/property_tests.rs` (this plan).
    - UNIT: 28 unit tests in `crates/mcp-tester/src/app_validator.rs::tests` (Plan 78-01) + 8 in `cargo-pmcp/src/commands/test/apps.rs::tests` (Plan 78-02).
    - FUZZ: 1 fuzz target in `fuzz/fuzz_targets/app_widget_scanner.rs` (this plan).
    - EXAMPLE: 1 working example in `crates/mcp-tester/examples/validate_widget_pair.rs` (this plan).
- **Plan 02's CLI E2E tests** (`cargo-pmcp/tests/cli_acceptance.rs`) remain skip-gated until a fixture binary is wired in. Plan 03's fixtures (broken/corrected HTML) are now available for that follow-up.
- **No blockers.** All 138 mcp-tester tests pass; touched files clippy-clean and fmt-clean; example exits 0; fuzz target syntactically valid (worktree-environmental build failure is logged and not regressing).

## Self-Check: PASSED

- **Files claimed created exist:**
    - `crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html` — verified present.
    - `crates/mcp-tester/tests/fixtures/widgets/broken_no_handlers.html` — verified present.
    - `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html` — verified present.
    - `crates/mcp-tester/tests/fixtures/widgets/README.md` — verified present.
    - `crates/mcp-tester/tests/property_tests.rs` — verified present.
    - `crates/mcp-tester/tests/app_validator_widgets.rs` — verified present.
    - `crates/mcp-tester/examples/validate_widget_pair.rs` — verified present.
    - `fuzz/fuzz_targets/app_widget_scanner.rs` — verified present.
- **Files claimed modified exist:**
    - `crates/mcp-tester/Cargo.toml` — verified present, `[dev-dependencies]` contains `proptest = "1"`.
    - `fuzz/Cargo.toml` — verified present, contains `[dependencies.mcp-tester]` + `[[bin]] name = "app_widget_scanner"`.
    - `.planning/phases/78-.../deferred-items.md` — verified present, appended with `## Worktree-only:` section.
- **Commits claimed exist:**
    - `c4080351` — verified present (`test(78-03): add widget HTML fixtures...`).
    - `e6f4612e` — verified present (`test(78-03): add proptest dev-dep + property + integration tests...`).
    - `178600b6` — verified present (`feat(78-03): add app_widget_scanner fuzz target + validate_widget_pair example`).
- **Acceptance criteria:**
    - `cargo test -p mcp-tester --test property_tests` — 2 passed.
    - `cargo test -p mcp-tester --test app_validator_widgets` — 7 passed.
    - `cargo test -p mcp-tester` — 138 passed (full mcp-tester regression).
    - `cargo run -p mcp-tester --example validate_widget_pair` — exit 0; output contains `broken_no_sdk` and `corrected_minimal`.
    - `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` — exits 0 (Plan 78-01/02 precedent scope).
    - `cargo clippy -p mcp-tester --example validate_widget_pair -- -D warnings` — exits 0.
    - `cargo fmt --all -- --check` — exits 0.
    - File `fuzz/fuzz_targets/app_widget_scanner.rs` contains `#![no_main]`, `fuzz_target!`, `mcp_tester::AppValidator`, `validate_widgets`, `fuzz-tool` — all verified.
    - File `fuzz/Cargo.toml` contains `name = "app_widget_scanner"` and `[dependencies.mcp-tester]` — both verified.
    - REVISION HIGH-3 fixture-comment hygiene grep loop exits 0 — verified during Task 1.
    - REVISION HIGH-1 `test_chatgpt_mode_unchanged_zero_results` asserts `results.len() == 0` — verified.
    - REVISION HIGH-4 tool name in tuple + assertion in `name` field — verified by `test_broken_widget_fails_claude_desktop`'s `cost-coach` substring check.

## REVISION Remediation Traces

- **REVISION HIGH-1 (ChatGpt zero-results):** Two tightened tests in `app_validator_widgets.rs` — `test_chatgpt_mode_unchanged_zero_results` and `test_chatgpt_mode_zero_results_corrected_too` — both asserting `results.len() == 0` (not just absence-of-Failed). Tests cover broken AND corrected widgets to confirm ChatGpt mode is a no-op regardless of widget shape.
- **REVISION HIGH-3 (fixture comment hygiene):** All three HTML fixtures pass the grep loop `grep -E '(//|/\*|<!--).*(@modelcontextprotocol/ext-apps|new App\(|onteardown|ontoolinput|ontoolcancelled|onerror)'` (zero matches). README.md publishes the recipe for future fixture authors.
- **REVISION HIGH-4 (tool name in tuple):** Three-element tuple `(tool_name, uri, html)` used consistently in property tests (`"prop-tool"`), integration tests (`"cost-coach"`), fuzz harness (`"fuzz-tool"`), and example (`format!("example-{label}")`). Integration test `test_broken_widget_fails_claude_desktop` asserts every Failed row's `name` contains `cost-coach`.

## Deferred Items

- **`corrected_minified.html`** — A Vite-built minified version of `corrected_minimal.html` is deferred to a follow-up phase. Per RESEARCH Open Question 1 RESOLVED: empirical Vite-build verification will land if scanner false-negatives are observed in the wild. The README.md `## Deferred:` section documents this.
- **CLI E2E fixture binary** — Plan 02's `cargo-pmcp/tests/cli_acceptance.rs` E2E tests remain skip-gated until a fixture-server binary is wired in as a `[[bin]]` target. Plan 03's fixtures (broken/corrected HTML) are now available for that follow-up.

---
*Phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-*
*Plan: 03*
*Completed: 2026-05-02*
