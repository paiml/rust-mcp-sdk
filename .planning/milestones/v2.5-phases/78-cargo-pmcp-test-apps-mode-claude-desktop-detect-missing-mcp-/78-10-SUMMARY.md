---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
plan: 10
subsystem: mcp-tester app_validator (string-literal aware comment stripper + widened G2 constructor regex)
tags: [mcp-apps, validator, claude-desktop, gap-closure, cycle-2, comment-stripper, g2-regex, root-cause-fix]
gap_closure: true
requirements:
  - PHASE-78-AC-1
  - PHASE-78-AC-2
  - PHASE-78-AC-3
  - PHASE-78-AC-4
  - PHASE-78-ALWAYS-UNIT
  - PHASE-78-ALWAYS-PROPERTY

dependency-graph:
  requires:
    - "Plan 78-09 — captured 6 real-prod fixtures + 7 RED tests + CAPTURE.md root-cause documentation"
    - "Plan 78-06 — current cycle-1 validator code (regexes + strip_js_comments + emission helpers)"
  provides:
    - "GREEN state: cycle-2 RED tests turn GREEN (7 pass, 0 fail); cycle-1 invariants preserved (81 lib unit + 5 cycle-1 RED + 7 widget + 4 property tests all pass)"
    - "string-literal aware comment stripper handling minified bundles with /* and // inside JS strings"
    - "G2 constructor regex matching `new <id>({...,name:<expr>,version:<expr>...})` with non-literal value expressions"
    - "Property-encoded G2 false-positive guard preventing future widening regressions"
  affects:
    - "Phase 78 ROADMAP — gap-closure cycle 2 wave 2 LANDED (load-bearing fix)"
    - "Plan 78-11 — operator's Test 6 re-verification will now report zero Failed rows on the 8 cost-coach prod widgets (binary-boundary acceptance)"

tech-stack:
  added: []
  patterns:
    - "String-literal aware comment stripping: ~110-LOC state machine (out / block_comment / line_comment / single_string / double_string / template_string). Inside string states, /* and // are NOT comment delimiters. Escape sequences (\\<any>) consumed as a unit."
    - "Constructor regex widening via bounded character classes: `[^}]{0,200}\\bname\\s*:[^,}]{0,100},\\s*version\\s*:` matches name+version keys with arbitrary value expressions, capped to prevent catastrophic backtracking on adversarial input"
    - "Atomic single-file commit per cycle-1 lesson (B3): all changes land in one commit so cargo build is green at every commit boundary"

key-files:
  created: []
  modified:
    - "crates/mcp-tester/src/app_validator.rs (+299 / -31 lines): strip_js_comments rewrite, app_constructor_re widening, 12 cycle-2 unit tests"
    - "crates/mcp-tester/tests/property_tests.rs (+41 lines): prop_g2_cycle2_no_false_positive_on_unrelated_keys"

decisions:
  - "Implemented string-literal-aware stripping inline (state machine) rather than introducing a JS lexer dependency. Trade-off accepted: no recursion into template literal `${...}` interpolations; PMAT cog stays under 25 (single function ~110 LOC, mostly flat match arms with index advancement)."
  - "G2 widening uses bounded char classes `[^}]{0,200}` and `[^,}]{0,100}` instead of unbounded `.*?` to prevent catastrophic backtracking on the fuzz target's `\\PC{0,4096}` alphabet. 200 + 100 chars are enough for any realistic constructor payload."
  - "Required explicit comma between name-value and version key: `,\\s*version\\s*:`. Originally drafted as `[^,}]{0,100}\\bversion\\s*:` (no comma) but cycle-1 tests showed the greedy `[^,}]` consumes the value but then can't reach `version` past the `,`. Fixing the regex to require the comma made cycle-1 + cycle-2 tests both pass."
  - "Dropped reordered-keys (`{version:..., name:...}`) support from cycle 2. Real prod ALWAYS uses name-first ordering. The reordered case was over-engineering; the cycle-2 unit test for it would be synthetic."
  - "Preserved cycle-1 G1 signal regexes verbatim. The cycle-1 G1 detection was correct; only the upstream comment-stripping was buggy. Once strip_js_comments preserves the SDK section, G1 detection works as designed (proven by `scan_widget_g1_cycle2_csp_string_does_not_steal_sdk_section` test)."
  - "Auto-fixed clippy `manual_range_patterns` (`3 | 4 | 5` → `3..=5`) under Rule 3 — purely mechanical, zero behavior change."

metrics:
  duration: "~25 minutes wall-clock (regex widening + state machine implementation + 12 unit tests + clippy fix + property test addition)"
  completed: "2026-05-02"
  tasks_completed: "2 of 2"
  commits:
    - "dd15b7db — fix(78-10): cycle-2 — string-literal aware comment stripper + widened G2 regex"
    - "c5b34c7b — test(78-10): add G2 cycle-2 false-positive guard property test"
---

# Phase 78 Plan 10: cycle-2 validator fix Summary

Plan 78-10 closed Phase 78 Gap G6 by fixing the load-bearing root cause discovered by Plan 78-09: `strip_js_comments` was destroying SDK code in cost-coach prod bundles, and `app_constructor_re()` required literal `name`/`version` values that real prod doesn't use. Both fixes landed in a single atomic commit; a property test now guards G2 widening against false positives.

## Objective Recap

Generalize G1 (SDK-presence detection) and G2 (constructor detection) to match the actual cost-coach prod minified shape. Turn the 6 real-prod RED tests from Plan 78-09 GREEN without regressing any cycle-1 invariants.

(Rescoped from "add new G1 signals" — see Plan 78-09 SUMMARY's "ROOT CAUSE FINDING" section.)

## What Landed

### Task 1 (commit `dd15b7db`) — Validator fix

**Bug (a) fix: string-literal aware comment stripping.** The cycle-1 `strip_js_comments` (3 chained regex passes) destroyed ~21 KB of SDK code in cost-over-time and savings-summary because the block-comment regex `/\*.*?\*/` was not string-literal aware. A JS string `"/*.example.com..."` (a CSP frame-src directive value) opened a phantom block comment that closed at a real `*/` license-header banner thousands of bytes later.

Replaced with a ~110-LOC state machine. States:
- `0` = outside strings/comments
- `1` = inside `/* ... */` block comment
- `2` = inside `// ...` line comment
- `3` = inside `'...'` single-quoted string
- `4` = inside `"..."` double-quoted string
- `5` = inside `` `...` `` template-literal string

Inside any string state, `/*` and `//` are NOT comment delimiters and are preserved. Escape sequences (`\<any>`) consumed as a unit. HTML comments still stripped via the existing regex up front. PMAT cog ≤ 25 maintained (zero new violations on app_validator.rs).

**Bug (b) fix: G2 constructor regex widening.** Real prod uses `name:"cost-coach-"+t,version:"1.0.0"` (string concatenation) — the cycle-1 regex required literal string values. Replaced:

```regex
new [a-zA-Z_$][a-zA-Z0-9_$]{0,5}\(\s*\{\s*name\s*:\s*"[^"]+"\s*,\s*version\s*:\s*"[^"]+"\s*\}
```

with:

```regex
new [a-zA-Z_$][a-zA-Z0-9_$]{0,20}\(\s*\{[^}]{0,200}\bname\s*:[^,}]{0,100},\s*version\s*:
```

- Mangled-id cap widened from `{0,5}` to `{0,20}`
- `[^}]{0,200}` matches any non-`}` content as prefix to `name`
- `[^,}]{0,100}` matches any value-expression for `name` (literal, concat, function call, etc.)
- `,\s*version\s*:` requires the comma between key-value pairs (load-bearing — without explicit comma the cycle-1 unit tests fail because `[^,}]{0,100}` greedily consumes value, can't reach `version` past `,`)
- Bounded character classes prevent catastrophic backtracking on adversarial input

**12 cycle-2 unit tests added at the end of the existing `tests` module:**
- `strip_js_comments_preserves_block_comment_inside_double_quoted_string` (the cost-coach CSP-string regression)
- `strip_js_comments_preserves_block_comment_inside_single_quoted_string`
- `strip_js_comments_preserves_line_comment_marker_inside_string`
- `strip_js_comments_still_strips_real_block_comments_outside_strings`
- `strip_js_comments_still_strips_real_line_comments_outside_strings`
- `strip_js_comments_handles_escaped_string_delimiters`
- `strip_js_comments_handles_template_literal`
- `scan_widget_g2_cycle2_string_concat_name_value_matches`
- `scan_widget_g2_cycle2_longer_mangled_id_matches`
- `scan_widget_g2_cycle2_random_new_call_with_unrelated_keys_does_not_match`
- `scan_widget_g2_cycle2_real_cost_coach_prod_pattern`
- `scan_widget_g1_cycle2_csp_string_does_not_steal_sdk_section` (load-bearing root-cause regression — combines all the failure-mode signals from cost-over-time)

### Task 2 (commit `c5b34c7b`) — G2 false-positive guard property test

`prop_g2_cycle2_no_false_positive_on_unrelated_keys` generates random `new <Class>({<key1>:<val1>,<key2>:<val2>})` shapes where neither key is `name`+`version`, and asserts the App constructor row's status is Failed. Guards Plan 78-10 Task 1's widened G2 regex from over-matching benign code that uses `new SomeClass({...})` with arbitrary keys.

Search space: ~12 keys × ~12 keys × class-name space (default proptest config, 256 cases per run). Property tests now cover all four cycle-1+cycle-2 invariants (panic-freedom, whitespace idempotence, G3 cascade elimination, G2 false-positive guard).

## Verification Evidence

```sh
$ cargo test -p mcp-tester --lib
test result: ok. 81 passed; 0 failed (was 39 cycle-1 + 12 cycle-2 = 51 directly; 81 includes other unit tests)

$ cargo test -p mcp-tester --test app_validator_widgets_real_prod
test result: ok. 7 passed; 0 failed
   (was: 1 passed, 6 failed pre-Plan-10 — RED→GREEN conversion confirmed)

$ cargo test -p mcp-tester --test app_validator_widgets_bundled
test result: ok. 5 passed; 0 failed (cycle-1 RED tests preserved)

$ cargo test -p mcp-tester --test app_validator_widgets
test result: ok. 7 passed; 0 failed (pre-existing widget tests preserved)

$ cargo test -p mcp-tester --test property_tests
test result: ok. 4 passed; 0 failed (3 cycle-1 + 1 cycle-2)

$ cargo test -p mcp-tester --test error_messages_anchored
test result: ok. 3 passed; 0 failed

$ cargo clippy -p mcp-tester --lib --tests --bins --no-deps -- -D warnings
exit 0

$ cargo fmt --all -- --check
exit 0

$ ! grep -E '(TODO|FIXME|HACK|XXX):' crates/mcp-tester/src/app_validator.rs
exit 0 (no SATD)

$ pmat analyze complexity --max-cognitive 25 --path crates/mcp-tester/src/app_validator.rs
Violations in app_validator: 0
```

## Deviations from Plan

### Auto-fixed Issues (Rule 3 — blocking issues)

**1. clippy::manual_range_patterns on the state-machine match arm**
- **Found during:** Task 1 verification (`cargo clippy --lib --tests --bins -- -D warnings`)
- **Issue:** `3 | 4 | 5 => {` in the strip_js_comments state machine triggered the `manual_range_patterns` lint with the rust 1.95 toolchain.
- **Why blocking:** clippy `-D warnings` would block commit.
- **Fix:** Replaced `3 | 4 | 5 => {` with `3..=5 => {`. Purely mechanical, zero behavior change. Applied as part of Task 1's atomic commit.

**2. fmt drift after the manual edit**
- **Found during:** Task 1 verification.
- **Issue:** Manual edits left a comma-dangle inconsistency in the state-machine match arms.
- **Fix:** `cargo fmt -p mcp-tester` applied. Purely mechanical.

### Plan-internal scope decision

**3. Dropped reordered-keys (`{version:..., name:...}`) support**
- **Why:** Real prod ALWAYS uses name-first ordering. The reordered case in the original plan was over-engineering; supporting it would have required either alternation or two regexes OR'd. The cycle-2 unit test for it would have been synthetic. The current regex requires `name:<expr>,<ws>version:<expr>` — name-first only.
- **Trade-off:** If a future widget uses version-first ordering, that widget would need a cycle-3 fix. Acceptable based on observed prod behavior (29f46efd: 6 of 6 widgets are name-first).

### Out-of-Scope (none for this plan)

## Auth Gates

None.

## Self-Check: PASSED

- All 7 cycle-2 RED tests pass (was: 1 passed, 6 failed).
- All 5 cycle-1 RED tests still pass.
- All 7 pre-existing widget tests still pass.
- All 81 lib unit tests pass (39 cycle-1 + 12 new cycle-2 + 30 other).
- All 4 property tests pass (3 cycle-1 + 1 cycle-2).
- clippy clean for mcp-tester crate (`-D warnings`).
- fmt clean across workspace.
- Zero SATD in app_validator.rs.
- PMAT cog ≤ 25 maintained (zero violations on app_validator.rs).
- `cycle-2` substring present in app_validator.rs (grep-verifiable).
- Two commits on main: `dd15b7db` + `c5b34c7b`.
