---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
plan: 05
subsystem: mcp-tester widget validator
tags: [mcp-apps, validator, claude-desktop, regex, widget-validation, mcp-tester, gap-closure, bundled-fixtures, red-phase, tdd]
gap_closure: true
requirements: [PHASE-78-AC-1, PHASE-78-AC-3, PHASE-78-AC-4, PHASE-78-ALWAYS-UNIT]

dependency-graph:
  requires:
    - "AppValidator::validate_widgets API (Plan 78-01)"
    - "AppValidationMode::ClaudeDesktop (Plan 78-01)"
    - "scan_widget + extract_inline_scripts + strip_js_comments helpers (Plan 78-01)"
  provides:
    - "Bundled fixture set encoding the cost-coach Vite singlefile false-positive class"
    - "RED-phase regression contract for G1 (log prefix + JSON-RPC method strings as SDK signals)"
    - "RED-phase regression contract for G2 (mangled-id constructor regex tolerance)"
    - "RED-phase regression contract for G3 (handler/connect/ontoolresult detection independent of SDK signal)"
    - "Per-mode emission shape documentation (claude-desktop / standard / chatgpt) for the bundled set"
  affects:
    - "Plan 78-06 (validator GREEN-phase fix) — must turn these 4 tests green without breaking app_validator_widgets.rs"
    - "Plan 78-07 / 78-08 (downstream gap-closure work) — depends on the corrected validator behavior"

tech-stack:
  added: []
  patterns:
    - "TDD RED phase: deliberate test-failure cluster encoding the regression contract before the fix lands"
    - "Synthesized minimal fixtures preserving load-bearing signals (mangled constructor, intact {name,version} payload, member-name handler assignments) without ~50 KB Vite runtime glue"
    - "Comment-hygiene grep belt-and-braces: fixtures contain none of the watched literals in HTML/JS comments"

key-files:
  created:
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/cost_summary_minified.html"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/cost_over_time_minified.html"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/synthetic_cascade_repro.html"
    - "crates/mcp-tester/tests/fixtures/widgets/bundled/README.md"
    - "crates/mcp-tester/tests/app_validator_widgets_bundled.rs"
  modified: []

decisions:
  - "Test 3 (synthetic cascade) tightened to exact `count_status(Failed) == 2`, not `>= 2` — Plan 06 must land SDK detection that is genuinely independent of the >=3-handler fallback so the cascade-repro fixture (which has all 5 handlers but no real SDK signals) reports SDK=Failed."
  - "Doc comments for #[test] functions kept terse to avoid Markdown list-item parsing tripping clippy `doc_lazy_continuation` (a `+` at line start in continuation lines becomes a bullet)."
  - "Doc-block at file top names the 4 expected-failing tests so a future reader running cargo test isn't surprised by the cluster of failures."
  - "No `#[ignore]` on any test — failures must be loud so the gap closure cannot silently regress."

metrics:
  duration: "~5 minutes wall-clock (fixture authoring + test scaffolding + clippy fix iteration)"
  completed: "2026-05-03"
  tasks_completed: 2
  commits:
    - "d2883d7e — test(78-05): add bundled widget fixtures for Vite singlefile false-positive class (G1+G2+G3 RED)"
    - "6f64086d — test(78-05): add RED-phase integration tests for bundled fixtures (G1+G2+G3 contract)"
---

# Phase 78 Plan 05: Bundled Widget Fixtures + RED-Phase Integration Tests Summary

Captured the cost-coach Vite singlefile false-positive class as durable regression fixtures and a 5-test integration file that fails today against v1 validator patterns; the failure cluster is the binary regression contract that Plan 06's GREEN phase must close.

## Objective Recap

Per the plan's `<objective>`: establish the RED-phase regression contract for gaps G1, G2, and G3 before the validator is fixed. Without this fixture set, the false-positive class would be invisible to the test suite (the existing synthetic fixtures `broken_no_sdk.html` / `corrected_minimal.html` don't resemble Vite singlefile output at all), so the v1 false-positive bug could ship a second time.

## What Landed

### Bundled fixtures (`crates/mcp-tester/tests/fixtures/widgets/bundled/`)

Three HTML fixtures plus a `README.md` per-mode-emission table:

1. **`cost_summary_minified.html`** — mirrors cost-coach prod output verbatim:
   - Mangled `yl` constructor: `new yl({name:"cost-coach-cost-summary",version:"1.0.0"})` with **unquoted JSON keys** (matches the actual Vite/Rollup output shape).
   - `[ext-apps]` log prefix in `console.log`.
   - JSON-RPC method strings `ui/initialize` and `ui/notifications/tool-result`.
   - All 5 member-name handler assignments (`.onteardown=`, `.ontoolinput=`, `.ontoolcancelled=`, `.onerror=`, `.ontoolresult=`).
   - `connect()` invocation.
   - **Intentionally absent**: literal `@modelcontextprotocol/ext-apps` (Vite inlined it), literal `new App(` (minifier mangled it).

2. **`cost_over_time_minified.html`** — same skeleton, mangled id is `gl` (different from `yl`) and tool name is `cost-coach-cost-over-time`. Proves the post-fix regex must tolerate id variance, not lock onto a single mangled identifier.

3. **`synthetic_cascade_repro.html`** — the G3 spec fixture. Handlers (all 5, including `ontoolresult`) and `connect()` are present, but **no SDK presence signals** (no log prefix, no method strings, no `@modelcontextprotocol/ext-apps`) and **no constructor** (no `new App(` and no mangled-id `new <X>({name,version})`). Proves the cascade-elimination contract: handlers and connect must be detected independently of `has_sdk`.

4. **`README.md`** — per-fixture mode-emission table, comment-hygiene rule restatement, rationale for synthesizing rather than copying ~50 KB Vite-bundled bytes verbatim.

### RED-phase integration tests (`crates/mcp-tester/tests/app_validator_widgets_bundled.rs`)

Five `#[test]` functions; all five named verbatim per plan:

| # | Name | Today's status | Post-Plan-06 status |
|---|------|---------------|---------------------|
| 1 | `test_cost_summary_minified_passes_claude_desktop` | FAIL (1 Failed row: `App constructor`) | PASS (0 Failed) |
| 2 | `test_cost_over_time_minified_passes_claude_desktop` | FAIL (1 Failed row: `App constructor`) | PASS (0 Failed) |
| 3 | `test_synthetic_cascade_no_handler_cascade_when_sdk_absent` | FAIL (1 Failed; SDK currently passes via the >=3-handler fallback, so test expecting exactly 2 Failed mismatches) | PASS (2 Failed: SDK + constructor; 6 Passed: 5 handlers + connect) |
| 4 | `test_bundled_fixtures_pass_standard_mode` | FAIL (1 Warning summary on cost-summary; same on cost-over-time) | PASS (0 Warning on the 2 fully-wired fixtures, 1 Warning on synthetic-cascade) |
| 5 | `test_bundled_fixtures_zero_results_chatgpt_mode` | PASS (chatgpt mode early-returns) | PASS unchanged |

Test 3 is the load-bearing G3 cascade-elimination assertion — it pairs `count_status(Failed) == 2` (exact, not `>=`) with name-contains assertions for `MCP Apps SDK wiring`, `App constructor`, `handler: onteardown`, `handler: ontoolinput`, `handler: ontoolcancelled`, `handler: onerror`, `handler: ontoolresult`, and `connect() call`. The fixture body has all 5 handlers + connect, so post-fix the validator must detect each independently.

## Verification

- `cargo test -p mcp-tester --test app_validator_widgets_bundled` exits 1 with `1 passed; 4 failed` (RED contract intact).
- `cargo build -p mcp-tester --tests` exits 0 (test file compiles cleanly).
- `cargo clippy -p mcp-tester --tests -- -D warnings` exits 0 (no lint errors).
- `cargo fmt --all -- --check` exits 0 (formatted).
- Comment-hygiene grep across all 3 fixtures: 0 matches.
- All 8 grep-based fixture assertions in the plan's `<verify><automated>` block: pass.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Doc comments tripped clippy `doc_lazy_continuation`**
- **Found during:** Task 2 verification (running `cargo clippy -p mcp-tester --tests -- -D warnings` per the plan's verify block)
- **Issue:** The initial `///` doc comment on `test_cost_summary_minified_passes_claude_desktop` had a continuation line beginning with `+ ` followed by indented continuation lines, which rustdoc's CommonMark parser interpreted as a Markdown bullet item without indentation, triggering `error: doc list item without indentation` (3 occurrences).
- **Fix:** Rewrote the doc comment to avoid `+` at continuation line starts; replaced the bullet-style enumeration with a flowing sentence enumerating the fixture signals.
- **Files modified:** `crates/mcp-tester/tests/app_validator_widgets_bundled.rs` (committed in the same Task 2 commit)
- **Why this counts as Rule 3:** The plan's success criteria require `cargo clippy -p mcp-tester --tests -- -D warnings` to exit 0. Without the doc fix the test file would compile but fail the lint gate, blocking the commit (project pre-commit hook + CLAUDE.md "Zero tolerance for defects" enforce clippy zero-warnings).

### Other Deviations

None — fixtures, integration test scaffolding, comment hygiene, and emission-shape table all match the plan's specification byte-for-byte.

## Auth Gates

None — fully autonomous.

## Notes for Plan 06 (the GREEN phase)

The 4 failing tests in `app_validator_widgets_bundled.rs` are the binary regression contract. Plan 06 cannot land without them turning green. Concretely, Plan 06 must:

1. **G1**: Add SDK presence detection that recognizes `[ext-apps]` log prefix, `ui/initialize`, and `ui/notifications/tool-result` method-string literals — independent of (or alongside) the existing import-literal check at `crates/mcp-tester/src/app_validator.rs:47`.

2. **G2**: Replace or extend the constructor regex at `crates/mcp-tester/src/app_validator.rs:53` (`r"\bnew\s+App\s*\(\s*\{"`) with one tolerant of mangled identifiers — e.g. `r#"new\s+[a-zA-Z_$][a-zA-Z0-9_$]{0,5}\s*\(\s*\{\s*name\s*:\s*"[^"]+"\s*,\s*version\s*:\s*"[^"]+"\s*\}"#` (note: matches **unquoted** JSON keys per cost-coach prod evidence).

3. **G3**: Decouple SDK detection from the `>=3 of 4 handlers` fallback so a fixture with handlers but no real SDK signals reports `has_sdk = false`. Concretely, the fallback should require at least one robust SDK signal (log prefix or method string) plus the handler count, OR drop the handler-count fallback entirely now that G1 provides robust signals.

The existing `app_validator_widgets.rs` test suite (5 tests using `broken_no_sdk.html` / `broken_no_handlers.html` / `corrected_minimal.html`) MUST stay green throughout — Plan 06 cannot regress them.

## Self-Check: PASSED

- [x] `crates/mcp-tester/tests/fixtures/widgets/bundled/cost_summary_minified.html` exists (`d2883d7e`).
- [x] `crates/mcp-tester/tests/fixtures/widgets/bundled/cost_over_time_minified.html` exists (`d2883d7e`).
- [x] `crates/mcp-tester/tests/fixtures/widgets/bundled/synthetic_cascade_repro.html` exists (`d2883d7e`).
- [x] `crates/mcp-tester/tests/fixtures/widgets/bundled/README.md` exists (`d2883d7e`).
- [x] `crates/mcp-tester/tests/app_validator_widgets_bundled.rs` exists (`6f64086d`).
- [x] Commit `d2883d7e` present in `git log` (verified).
- [x] Commit `6f64086d` present in `git log` (verified).
- [x] All 8 grep-based fixture assertions from plan's verify block pass.
- [x] Comment-hygiene grep across all 3 fixtures: 0 matches.
- [x] `cargo build -p mcp-tester --tests` exits 0.
- [x] `cargo clippy -p mcp-tester --tests -- -D warnings` exits 0.
- [x] `cargo fmt --all -- --check` exits 0.
- [x] `cargo test -p mcp-tester --test app_validator_widgets_bundled` produces `1 passed; 4 failed` (RED-phase contract).
