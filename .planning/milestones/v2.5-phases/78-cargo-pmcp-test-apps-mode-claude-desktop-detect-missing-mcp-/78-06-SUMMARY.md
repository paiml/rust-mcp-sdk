---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
plan: 06
subsystem: mcp-tester widget validator
tags: [mcp-apps, validator, claude-desktop, regex, widget-validation, mcp-tester, gap-closure, minification-resistant, cascade-elimination, green-phase]
gap_closure: true
requirements: [PHASE-78-AC-1, PHASE-78-AC-2, PHASE-78-AC-3, PHASE-78-AC-4, PHASE-78-ALWAYS-UNIT]

dependency-graph:
  requires:
    - "Plan 78-05 RED-phase fixtures + integration tests (binary regression contract)"
    - "AppValidator::validate_widgets API (Plan 78-01)"
    - "scan_widget + extract_inline_scripts + strip_js_comments helpers (Plan 78-01)"
  provides:
    - "Minification-resistant SDK-presence signal detection (4 independent signals OR'd into has_sdk)"
    - "Mangled-id-tolerant App constructor regex anchored on the unmangled {name, version} payload"
    - "Cascade-free per-signal emission in claude-desktop mode (each row reads exactly one field)"
    - "WidgetSignals public-shape with has_sdk, has_app_constructor, has_handlers derived fields"
    - "11 unit tests encoding the G1/G2/G3 invariants directly on scan_widget output"
  affects:
    - "Plan 78-07 — bundled --widgets-dir CLI work depends on the post-G1 scanner"
    - "Plan 78-08 — HUMAN-UAT re-run against cost-coach prod widgets must pass on the new validator"
    - "cost-coach v1 false-positive class (8 widgets, 33 false-positive failures) is structurally eliminated at the lib boundary"

tech-stack:
  added: []
  patterns:
    - "Compile-once OnceLock<Regex> accessor pattern extended with 4 new signals (3 SDK + 1 constructor)"
    - "Verbatim VERIFICATION.md regex literals copied byte-for-byte (no paraphrasing)"
    - "Atomic field rename (has_new_app -> has_app_constructor) across all 6 call sites in a single commit (B3 atomic-build invariant)"
    - "Cascade elimination by removing the `let sdk_present = ...` derived gate and reading s.has_sdk directly"

key-files:
  created: []
  modified:
    - "crates/mcp-tester/src/app_validator.rs"

decisions:
  - "Removed the existing test `sdk_signal_accepts_handler_count_fallback` and renamed to `sdk_signal_requires_independent_evidence_no_fallback` to encode the post-G3 contract — the original test asserted the EXACT bug Plan 06 eliminates (handler-count cascade satisfying the SDK row). Without this update, the G1+G2 task would build but the test suite would fail mid-plan, blocking the atomic commit."
  - "Kept `if !s.has_sdk { ... }` standalone-form INSIDE `emit_summary_warning_for_standard` (line 777) verbatim from the plan's Step 5 directive, despite the plan's Task-2 H1 grep flagging it. Semantic analysis: this is a per-signal missing-list builder (each `if !s.X { missing.push(\"X missing\") }` is independent), not a cascade. The plan H1 grep is over-broad relative to its own implementation prescription. The actual cascade — `let sdk_present = s.has_ext_apps_import || s.handlers_present.len() >= 3` — is removed."
  - "Both Task 1 and Task 2's intent (SDK-row independence from handler count) is structurally satisfied by Task 1's edit because the >=3-of-4 fallback was inside the cascade gate that Task 1 removes. All 5 Plan 05 RED tests turn green after Task 1's commit; Task 2 then adds the public-shape `has_handlers` field + 2 G3 invariant unit tests as documentation/regression-pinning."

metrics:
  duration: "~25 minutes wall-clock (read existing file → 2 atomic edits → fmt fix → quality-gate)"
  completed: "2026-05-03"
  tasks_completed: 2
  commits:
    - "75b03616 — feat(78-06): G1+G2 — minification-resistant SDK + constructor signals; rename has_new_app→has_app_constructor (atomic)"
    - "ce192152 — feat(78-06): G3 — has_handlers derived field; cascade-elimination unit tests"
---

# Phase 78 Plan 06: G1+G2+G3 Validator-Core Gap Closure (GREEN Phase) Summary

Closed three load-bearing validator-core gaps (G1 minification-resistant SDK detection, G2 mangled-id constructor regex, G3 cascade-free per-signal emission) in two atomic commits; the four Plan 05 RED-phase tests turn GREEN with no regressions on any pre-existing test, and the cost-coach v1 false-positive class (8 widgets, 33 false-positive failures) is structurally eliminated at the library boundary.

## Objective Recap

Per the plan's `<objective>`: turn the Plan 05 RED-phase regression tests GREEN by closing G1+G2+G3 in a single atomic plan because all three touch the same surface (`WidgetSignals` + `scan_widget` + emission helpers). Until G1+G2+G3 land together, partial fixes leak false-positives in different ways.

## What Landed

### Task 1 (commit `75b03616`) — G1 + G2 + atomic field rename

**4 new compile-once OnceLock<Regex> accessors** added in `app_validator.rs`:

| Accessor | Regex literal | Purpose |
|----------|---------------|---------|
| `ext_apps_log_prefix_re()` | `\[ext-apps\]` | G1 — survives Vite singlefile minification (bracketed string literal in console.log) |
| `ui_initialize_method_re()` | `ui/initialize` | G1 — JSON-RPC method literal (minifiers never rename quoted method strings) |
| `ui_tool_result_method_re()` | `ui/notifications/tool-result` | G1 — same rationale as ui/initialize |
| `app_constructor_re()` | `new [a-zA-Z_$][a-zA-Z0-9_$]{0,5}\(\s*\{\s*name\s*:\s*"[^"]+"\s*,\s*version\s*:\s*"[^"]+"\s*\}` | G2 — mangled-id-tolerant constructor anchored on the unmangled `{name, version}` payload |

The G2 regex literal is **verbatim from VERIFICATION.md** — no paraphrasing.

The legacy `new_app_call_re()` (`\bnew\s+App\s*\(\s*\{`) is **removed**.

**`WidgetSignals` reshape** — kept `has_ext_apps_import` for legacy diagnostic, added `has_log_prefix`, `has_method_initialize`, `has_method_tool_result`, derived `has_sdk` (4-signal OR), renamed `has_new_app` → `has_app_constructor`. All 6 call sites of the old name updated in the same commit (B3 atomic-build invariant — `cargo build -p mcp-tester --tests` is green at every commit boundary).

**Emission helpers** — `emit_results_for_claude_desktop` and `emit_summary_warning_for_standard` no longer compute the derived `let sdk_present = s.has_ext_apps_import || s.handlers_present.len() >= 3` gate. The SDK row reads `s.has_sdk` directly; the constructor row reads `s.has_app_constructor` directly. The chatgpt-only compound predicate now uses `s.has_sdk` instead of `s.has_ext_apps_import`.

**9 new unit tests** in `mod tests` (5 G1 + 4 G2):
- `scan_widget_g1_log_prefix_alone_satisfies_has_sdk`
- `scan_widget_g1_method_initialize_alone_satisfies_has_sdk`
- `scan_widget_g1_method_tool_result_alone_satisfies_has_sdk`
- `scan_widget_g1_legacy_import_still_satisfies_has_sdk`
- `scan_widget_g1_no_signals_means_no_sdk`
- `scan_widget_g2_mangled_yl_constructor_matches`
- `scan_widget_g2_mangled_gl_constructor_matches`
- `scan_widget_g2_unminified_app_constructor_still_matches`
- `scan_widget_g2_random_new_call_without_name_version_payload_does_not_match`

**Pre-existing tests adapted** in the same atomic commit:
- `scan_widget_detects_handlers_via_property_assignment` (line 1017 has_new_app → has_app_constructor) — passes against new G2 regex (App is a valid identifier under `[a-zA-Z_$][a-zA-Z0-9_$]{0,5}`).
- `scan_widget_ignores_signals_inside_comments` (line 1099 same rename) — passes (the only `new App({...})` is inside a `/* */` block which the comment-stripper removes).
- `regexes_compile` — extended to touch the 4 new accessors.

### Task 2 (commit `ce192152`) — G3 has_handlers + cascade-elimination invariant tests

- `WidgetSignals.has_handlers` derived field added (true when `!handlers_present.is_empty()`).
- `scan_widget` computes `let has_handlers = !handlers_present.is_empty()` and includes it in the struct literal.
- 2 new G3 unit tests:
  - `scan_widget_g3_handlers_detected_independently_of_has_sdk` — synthetic cascade shape (handlers + connect, no SDK signals): asserts `!has_sdk && !has_app_constructor && has_handlers && has_connect && handlers_present.len() == 4`.
  - `scan_widget_g3_chatgpt_only_diagnosis_requires_genuine_evidence_absence` — covers the chatgpt-only compound predicate's two branches.

The structural cascade-elimination already landed in Task 1 (removing `let sdk_present = ...`). Task 2's role is documentation/regression-pinning of the invariant.

## Verification

Library acceptance (all green after both commits):
- `cargo test -p mcp-tester --test app_validator_widgets_bundled` — **5 passed; 0 failed** (was 1 passed; 4 failed at the start of Plan 06; the binary regression contract is fully honored).
- `cargo test -p mcp-tester --test app_validator_widgets` — **7 passed; 0 failed** (no regression on synthetic fixtures).
- `cargo test -p mcp-tester --lib app_validator` — **39 passed; 0 failed** (was 28 before Plan 06; +11 = 9 G1+G2 + 2 G3).
- `cargo test -p mcp-tester --test error_messages_anchored` — **3 passed; 0 failed**.
- `cargo test -p mcp-tester --test property_tests` — **2 passed; 0 failed**.

Quality gates:
- `cargo build -p mcp-tester --tests` — exits 0 at every commit boundary (B3 atomic-build invariant honored).
- `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` — exits 0.
- `cargo fmt --all -- --check` — exits 0.
- `make quality-gate` — exits 0 (CLAUDE.md mandatory check).

Grep-based acceptance:
- `grep -q 'fn ext_apps_log_prefix_re'` — match (OK).
- `grep -q 'fn ui_initialize_method_re'` — match (OK).
- `grep -q 'fn ui_tool_result_method_re'` — match (OK).
- `grep -q 'fn app_constructor_re'` — match (OK).
- `! grep -q 'fn new_app_call_re'` — no match (OK).
- `! grep -q 'has_new_app'` — no match (OK).
- `grep -q 'has_log_prefix: bool'` — match (OK).
- `grep -q 'has_method_initialize: bool'` — match (OK).
- `grep -q 'has_method_tool_result: bool'` — match (OK).
- `grep -q 'has_sdk: bool'` — match (OK).
- `grep -q 'has_app_constructor: bool'` — match (OK).
- `grep -q 'has_handlers: bool'` — match (OK).
- `grep -q 'let has_handlers = !handlers_present\.is_empty()'` — match (OK).
- `grep -q 's\.has_sdk'` — match (OK, used by emission helpers).
- `! grep -nE 'sdk[_a-z]*present\s*='` — no match (OK; H1 cascade-free property — the derived gate is gone).
- `! grep -nE 'if\s*!\s*s?\.?has_sdk\s*\{'` — **1 match** (line 777). See Deviations §1.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated existing `sdk_signal_accepts_handler_count_fallback` test to encode the post-G3 contract**

- **Found during:** Task 1 implementation (when removing the `>=3 of 4 handlers` fallback that was the structural source of the G3 cascade).
- **Issue:** The pre-existing test (now in commit `75b03616`) at the location previously around line 1248 of `app_validator.rs` asserted `sdk_row.status == TestStatus::Passed` for a widget that has 3 handlers + `connect` + `new App` but no SDK signals. This is **exactly the bug** Plan 06 G3 eliminates — handlers alone must NOT imply SDK presence. Leaving the test unchanged would break `cargo test -p mcp-tester --lib app_validator` mid-plan, blocking the atomic commit and the subsequent Plan 05 RED-phase tests.
- **Fix:** Renamed to `sdk_signal_requires_independent_evidence_no_fallback` and inverted the assertion: SDK row MUST be `Failed` for a widget with handlers but no SDK signals; handler row MUST be `Passed` independently. Comment block documents the rename and the inversion as the post-G3 contract.
- **Files modified:** `crates/mcp-tester/src/app_validator.rs` (in the same Task 1 commit `75b03616`).
- **Why this counts as Rule 3:** The plan's `must_haves.truths` and Plan 05's binary regression contract require the cascade to be eliminated. The previous test encoded the cascade. Without this rename, the G1+G2 atomic commit would not pass the project pre-commit hook (CLAUDE.md "Zero tolerance for defects" + `make quality-gate` enforces test green). The plan's body did not list this test in its <read_first> "exact has_new_app line numbers" section, so it was an unforeseen but trivial-to-fix consequence.

**2. [Rule 1 - Bug-fix-friction] Plan H1 grep `! grep -nE 'if\s*!\s*s?\.?has_sdk\s*\{'` is over-broad relative to its own Step 5 prescription**

- **Found during:** Task 2 final verification (running the `! grep` H1 check from the plan's `<verify><automated>` block).
- **Issue:** The plan's Step 5 explicitly directs me to write:
  ```rust
  if !s.has_sdk {
      missing.push(
          "MCP Apps SDK presence (any of: @modelcontextprotocol/ext-apps import, [ext-apps] log prefix, ui/initialize method, or ui/notifications/tool-result method)"
              .to_string(),
      );
  }
  ```
  inside `emit_summary_warning_for_standard`. The plan's Task-2 H1 grep then asserts this exact pattern must NOT exist in the file.
- **Semantic analysis:** This is **not a cascade**. A cascade is "field A's value gates field B's emission" (e.g., the removed `let sdk_present = s.has_ext_apps_import || s.handlers_present.len() >= 3` was a cascade because it conflated SDK and handler evidence into a single SDK verdict). The standard-mode summary's `if !s.has_sdk { missing.push("SDK missing") }` is the orthogonal pattern: each missing signal independently appends its own message. The handlers, connect, and constructor each have their own independent `if !s.X { missing.push(...) }` check immediately below. No cross-field gating.
- **Fix:** Keep the standard-summary `if !s.has_sdk { ... }` as the plan's Step 5 directive instructs. The H1 intent (no derived sdk_present gate, no cross-field cascade in claude-desktop emission) is fully satisfied — the actual cascade has been removed. The 1 remaining grep hit is a per-signal independence check (the opposite of a cascade).
- **Files modified:** None (the implementation matches the plan's prescription byte-for-byte; this deviation is purely a plan-internal grep contradiction documented for the verifier).
- **Why this counts as Rule 1:** Without this note, the verifier might flag a non-bug as a violation. The H1 invariant the plan cares about — "SDK row's claude-desktop emission does not gate handler/connect rows" — IS satisfied: `emit_results_for_claude_desktop` no longer contains `let sdk_present = ...` and each row reads its own field independently. The standard-mode summary list-builder is independent of this invariant.

### Other Deviations

None — fixtures untouched (Plan 05 contract honored), regex literals byte-verbatim from VERIFICATION.md, all 11 new unit tests + 5 RED-phase integration tests + 7 synthetic-fixture integration tests + 2 property tests + 3 error-anchored tests pass.

## Auth Gates

None — fully autonomous.

## Notes for Plan 07 / Plan 08

- The library boundary is now structurally cascade-free. Any future work that re-introduces a `let sdk_present = ...` derived gate or a `if !s.has_sdk { /* gates other rows */ }` block in `emit_results_for_claude_desktop` will regress G3 silently — the per-row independence is preserved by structural pattern, not by a runtime invariant check.
- Plan 78-08 (HUMAN-UAT re-run against cost-coach prod) should now show 0 false-positive Failed/Warning rows on the 8 cost-coach widgets that were producing 33 false-positive failures under v1.
- The G2 regex anchors on the unmangled `{name, version}` payload. If a future Vite version starts mangling the property keys (e.g., minifying `{name: "x"}` to `{n: "x"}`), G2 will silently regress. A property test on the regex against a corpus of real Vite singlefile outputs would catch this — deferred to Plan 79+.

## Self-Check: PASSED

- [x] `crates/mcp-tester/src/app_validator.rs` exists and is modified (verified via `git diff --stat`).
- [x] Commit `75b03616` (Task 1) present in `git log` (verified).
- [x] Commit `ce192152` (Task 2) present in `git log` (verified).
- [x] All 4 grep-based "function exists" checks pass: `ext_apps_log_prefix_re`, `ui_initialize_method_re`, `ui_tool_result_method_re`, `app_constructor_re`.
- [x] Both grep-based "removed" checks pass: `! grep -q 'fn new_app_call_re'`, `! grep -q 'has_new_app'`.
- [x] All 6 grep-based "field exists" checks pass: `has_log_prefix`, `has_method_initialize`, `has_method_tool_result`, `has_sdk`, `has_app_constructor`, `has_handlers`.
- [x] H1 cascade-free grep `! grep -nE 'sdk[_a-z]*present\s*='` exits non-zero (no matches — the derived gate is gone).
- [x] H1 second grep `! grep -nE 'if\s*!\s*s?\.?has_sdk\s*\{'` has 1 match at line 777 (standard-summary missing-list builder, NOT a cascade — see Deviations §2).
- [x] `cargo build -p mcp-tester --tests` exits 0.
- [x] `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` exits 0.
- [x] `cargo fmt --all -- --check` exits 0.
- [x] `cargo test -p mcp-tester --test app_validator_widgets_bundled` reports `5 passed; 0 failed`.
- [x] `cargo test -p mcp-tester --test app_validator_widgets` reports `7 passed; 0 failed`.
- [x] `cargo test -p mcp-tester --lib app_validator` reports `39 passed; 0 failed`.
- [x] `cargo test -p mcp-tester --test error_messages_anchored` reports `3 passed; 0 failed`.
- [x] `cargo test -p mcp-tester --test property_tests` reports `2 passed; 0 failed`.
- [x] `make quality-gate` exits 0.
