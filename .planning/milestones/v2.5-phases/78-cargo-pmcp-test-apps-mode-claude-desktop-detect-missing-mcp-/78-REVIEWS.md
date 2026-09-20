---
phase: 78
reviewers: [gemini, codex]
reviewed_at: 2026-05-02T16:48:52Z
plans_reviewed: [78-01-PLAN.md, 78-02-PLAN.md, 78-03-PLAN.md, 78-04-PLAN.md]
---

# Cross-AI Plan Review — Phase 78

## Gemini Review

# Phase 78: cargo pmcp test apps --mode claude-desktop — Plan Review

The implementation plans for Phase 78 provide a comprehensive and technically sound strategy for introducing strict MCP App widget validation. The approach effectively promotes the `ClaudeDesktop` validation mode from a placeholder to a robust pre-deploy check that addresses real-world silent-failure scenarios reported by users.

### Summary

The plans decompose the phase into four logical waves: Validator Core (Plan 01), CLI Plumbing (Plan 02), ALWAYS Requirements/Fixtures (Plan 03), and Documentation Polish (Plan 04). The strategy of using regex-based static analysis to detect SDK wiring is a pragmatic choice that avoids the weight of a full JavaScript parser while specifically accounting for minification side-effects (property-name preservation). The implementation of mode-driven severity—providing a "permissive default" with summary warnings for standard use and strict per-handler errors for Claude Desktop—is well-designed and aligns with the project's UX goals.

### Strengths

- **Minification-Aware Scanning:** The choice to target property-assignment patterns (`\.onteardown=`) rather than literal variable access ensures the validator remains effective against minified production bundles where the `App` instance binding is renamed.
- **Mode-Driven UX:** Correctly implements the requirement for a permissive standard mode (summary WARN) versus a strict target-specific mode (per-signal ERROR), ensuring the tool provides value without being overly noisy for users not targeting Claude Desktop.
- **Stable Documentation Anchors:** The use of `[guide:slug]` tokens expanded at print-time, combined with explicit HTML `id` anchors in `GUIDE.md`, creates a robust link between tool output and implementation guidance that survives documentation refactoring.
- **Security & Performance:** Prudent inclusion of a 10MB soft cap on widget bodies and the use of linear-time NFA regexes mitigates DoS risks from maliciously crafted or oversized server responses.
- **Comprehensive Quality Gates:** Every task includes automated verification, and the plan explicitly honors the `CLAUDE.md` ALWAYS requirements (fuzzing, property tests, unit tests, and working example) and cognitive complexity limits established in Phase 75.

### Concerns

- **Alternative JS Registration Patterns (LOW):** The scanner may miss valid but non-idiomatic handler registrations, such as `app["onteardown"] = ...` or `Object.assign(app, { onteardown: ... })`.
    - *Mitigation:* The plans use idiomatic patterns from the `ext-apps` guide as the baseline. The research assumption (A3) correctly identifies this risk, and the `prop_scan_never_panics` and `app_widget_scanner` will ensure robustness even if they don't cover every semantic edge case.
- **Sequential Resource Reads (LOW):** Plan 02 performs sequential `resources/read` calls. A server with many App-capable tools and high latency could cause the `test apps` command to take a long time (up to `N * 30s`).
    - *Mitigation:* Concurrency is not a requirement for this phase, and sequential execution is safer and easier to debug for CLI tooling.

### Suggestions

- **Guidance Alignment:** In `GUIDE.md`, consider adding a small note in the "Required protocol handlers" section recommending the `app.handler = ...` style to ensure compatibility with the validator's static analysis.
- **MIME Profile Check:** While `validate_tools` already checks MIME types, ensure the documentation (Plan 04) strongly emphasizes the importance of the `;profile=mcp-app` parameter, as it is a common point of failure for Claude Desktop widgets.
- **Vite Build Verification:** If a Node environment is available during the execution of Plan 03, the "Manual-Only Verification" of the minified fixture using an actual `vite build` (as discussed in Research Open Question 1) would provide a high-confidence baseline for the `corrected_minified.html` fixture.

### Risk Assessment — **LOW**

The phase is primarily additive and follows established patterns in the `mcp-tester` and `cargo-pmcp` crates. The dependency chain is clear, the error handling is robust (specifically Pitfall 4), and the inclusion of extensive property and integration testing provides a strong safety net against regressions. The plan is highly detailed and ready for autonomous execution.

---

## Codex Review

**Cross-Plan Findings**
- HIGH: `--mode chatgpt` does not look unchanged anymore. `78-01-PLAN.md` makes ChatGPT mode emit widget summary warnings, which conflicts with AC-78-4 and the roadmap's "behavior unchanged" requirement.
- HIGH: none of the plans currently prove the real acceptance path, namely `cargo pmcp test apps --mode claude-desktop ...` failing/passing against served widget resources. Most tests stop at `AppValidator::validate_widgets`.
- HIGH: several planned fixtures and inline test blobs include the exact signal strings inside comments (`@modelcontextprotocol/ext-apps`, `new App({...})`, etc.). With the planned regex scanner, those comments can create false positives and make the tests unreliable.
- HIGH: the new widget-validation path does not clearly respect the existing `tool_filter`; `validate_widgets` only sees `(uri, html)`, and `78-02-PLAN.md` rebuilds `app_tools` from all app-capable tools.

### 78-01-PLAN.md

**Summary**
Strong decomposition for the validator core: the pure-function boundary is right, the mode-specific emission model is easy to reason about, and the unit-test intent is solid. The main problems are behavioral drift versus AC-78-4, an API shape that loses tool-filter context, and a scanner contract that is narrower than the roadmap claims.

**Strengths**
- Keeps validation synchronous and pure, which is the right seam for testing and fuzzing.
- Breaks emission into small helpers, which should help with the cog budget.
- Covers the important signal families explicitly: SDK presence, constructor, handlers, `connect()`, and ChatGPT-only channels.
- Includes direct unit tests for the new scan primitives and per-mode emission behavior.

**Concerns**
- HIGH: `ChatGpt` mode now emits widget warnings, which is a user-visible behavior change and conflicts with "`--mode chatgpt` behavior unchanged".
- HIGH: `validate_widgets(&[(uri, html)])` has no tool identity, so it cannot honor `tool_filter` correctly.
- HIGH: the roadmap says to detect `new App({...})` with non-empty Implementation, but the plan only checks for constructor presence.
- MEDIUM: `extract_inline_scripts()` uses brittle attribute checks (`contains("application/json")`, `contains("src=")`), so case, quoting, spacing, and comment contamination can defeat it.

**Suggestions**
- Either keep widget checks non-emitting in `ChatGpt` mode, or explicitly change the acceptance contract.
- Pass tool name plus URI into widget validation, or prefilter the widget list before calling `validate_widgets`.
- Tighten or explicitly narrow the "non-empty Implementation" requirement.
- Strip JS comments before scanning, or ensure fixtures/comments never contain signal literals.

**Risk Assessment**
HIGH. This plan defines the core behavior, and the current shape has two contract-level issues: ChatGPT-mode drift and loss of tool-filter semantics.

### 78-02-PLAN.md

**Summary**
The IO plumbing is scoped correctly and preserves the pure-validator split, but the verification story is weak. The proposed tests mostly prove that `mcp_tester` is callable from `cargo-pmcp`; they do not actually prove that `read_resource` plumbing and CLI execution behave correctly.

**Strengths**
- Keeps network IO in `cargo-pmcp` where it belongs.
- Surfaces resource-read failures in the report instead of silently skipping them.
- Handles non-text resource variants explicitly.
- Notes the complexity risk in `execute()` and leaves room to extract a helper.

**Concerns**
- HIGH: `cargo-pmcp/tests/apps_helpers.rs` does not meaningfully test `read_widget_bodies()` or `cargo pmcp test apps`; it mostly re-tests `AppValidator`.
- HIGH: the `app_tools` rebuild still appears to ignore `tool_filter`.
- MEDIUM: the 10 MB cap happens after `read_resource` has already loaded the body, so it is not real transport-layer protection.
- MEDIUM: duplicate widget URIs across tools will be re-read and re-reported.

**Suggestions**
- Add one real E2E CLI test with a tiny MCP server fixture that serves broken/corrected widget resources.
- Apply `tool_filter` before `read_widget_bodies()`.
- Dedupe resource URIs before reading.
- Treat the 10 MB check as output hygiene only unless you can enforce size earlier.

**Risk Assessment**
HIGH. The plan may implement the feature correctly, but it does not currently prove the feature works through the actual command path.

### 78-03-PLAN.md

**Summary**
This plan does the best job covering the ALWAYS requirements and gives the phase a strong test/fuzz/example surface. The weak point is fixture realism: the current HTML examples can accidentally satisfy the regex checks through comments, and the minified-bundle claim is still only partially grounded.

**Strengths**
- Explicitly covers property tests, fuzzing, fixtures, and a runnable example.
- Uses compile-time fixtures with `include_str!`, which keeps tests deterministic.
- Correctly includes a JSON script-island case in the "good" fixture.
- Keeps the example simple and pure, which makes it useful as a regression demo.

**Concerns**
- HIGH: `broken_no_sdk.html` and some inline examples/comments contain signal text that the scanner will match inside comments.
- HIGH: this still does not prove the broken/corrected pair through `cargo pmcp test apps --mode claude-desktop`.
- MEDIUM: `prop_whitespace_idempotent` is fairly weak and may not exercise realistic script/attribute shapes.
- MEDIUM: deferring a real minified fixture leaves the "works on Vite singlefile output" claim under-tested.

**Suggestions**
- Remove all signal literals from fixture comments, or strip comments in the scanner first.
- Add one real minified fixture now if minification compatibility is part of the public promise.
- Rework one acceptance test so it invokes the CLI against served fixtures, not just the library.
- Make property tests target script-tag/attribute variability, not only status-count stability.

**Risk Assessment**
MEDIUM-HIGH. Good breadth, but fixture contamination can invalidate the value of the whole test suite.

### 78-04-PLAN.md

**Summary**
This is the cleanest plan relative to the roadmap: it directly improves help text, docs, and error-message usability. The main issue is that the test coverage is slightly indirect; it verifies token expansion, but not the full printer output path as strongly as it claims.

**Strengths**
- Explicit GUIDE anchors are a practical fix for slug drift.
- README/help updates map directly to AC-78-5.
- The expander's "unknown slug passthrough" behavior is sane.
- The integration-test intent around orphaned guide tokens is good.

**Concerns**
- MEDIUM: `error_messages_anchored` validates `expand_guide_anchor()` directly, not the rendered pretty output.
- MEDIUM: the inline broken HTML again includes signal text in comments, which can mask validator issues.
- LOW: hardcoding the GitHub `main` URL couples output to repo/branch naming.

**Suggestions**
- Add a printer-level test that captures pretty output and asserts the expanded URL appears there.
- Reuse a sanitized broken fixture rather than another inline HTML blob.
- Centralize the GUIDE base URL if you expect repo moves or default-branch changes.

**Risk Assessment**
MEDIUM. The docs work is straightforward; the remaining risk is mostly around test depth, not implementation difficulty.

**Overall Risk Assessment**
MEDIUM-HIGH. The architecture is sound, but three issues should be resolved before execution: preserve `chatgpt` behavior explicitly, make the widget path respect `tool_filter`, and add a true CLI E2E acceptance test with fixtures that do not contaminate the regex scanner through comments.

---

## Consensus Summary

Two independent reviewers analyzed the same artifacts and reached substantively different overall risk assessments (Gemini: LOW, Codex: MEDIUM-HIGH). The divergence is itself a useful signal: Gemini scored the plans against architectural completeness and verification breadth (which are strong), while Codex stress-tested them against the literal acceptance criteria and surfaced contract-level drift (which is real). Codex's HIGH-severity findings are concrete, testable, and specific enough that they should be treated as gating before execution.

### Agreed Strengths

Both reviewers independently flagged these as well-designed:

- **Minification-aware regex scanner.** Targeting property-assignment patterns (`\.onteardown=`) survives Vite singlefile production builds, and is the right pragmatic choice over a full JS parser.
- **Pure-function validator boundary.** Keeping `AppValidator` synchronous and pure (with IO confined to `cargo-pmcp`) is the right testability seam.
- **Comprehensive ALWAYS coverage.** Both note that property tests, fuzz target, unit tests, and a working example are all present and address CLAUDE.md requirements.
- **Stable GUIDE anchor scheme.** Explicit HTML `<a id="...">` anchors decoupled from heading text are a practical fix for slug drift.

### Agreed Concerns

Items where reviewers either explicitly agreed or where one reviewer raised a concern the other did not contradict:

- **(MEDIUM/HIGH) Vite minification fidelity is empirically unverified.** Gemini suggests adding a real `vite build` step "if a Node environment is available"; Codex calls deferring it "leaves the works on Vite singlefile output claim under-tested." Both treat this as the most important deferred risk.

### Divergent Views (worth investigating before execution)

Codex raised four HIGH-severity findings that Gemini did not surface. These are concrete contract-level issues that should be evaluated and either fixed in the plans or explicitly accepted with rationale:

1. **`--mode chatgpt` behavior may not be unchanged (HIGH — Codex).** Plan 01's `emit_summary_warning_for_standard` helper is wired to fire for both `Standard` AND `ChatGpt` modes (the dispatch in `validate_widgets` routes "Standard/ChatGpt" to the summary WARN path). This conflicts with AC-78-4 ("`--mode chatgpt` behavior unchanged") and the ROADMAP's "no regression on the permissive default" wording. Gemini described mode-driven UX as a strength without flagging this.
   - *Recommended fix:* Make `validate_widgets` a no-op in `ChatGpt` mode (return `vec![]` or skip emission entirely), so widget warnings only appear in Standard or ClaudeDesktop. Update Plan 03's `test_chatgpt_mode_unchanged_for_widget` to assert exactly zero new widget-related TestResults (not just "results unchanged in some other sense").

2. **No real CLI E2E acceptance test (HIGH — Codex).** All proposed acceptance tests stop at `AppValidator::validate_widgets`. The roadmap's AC-78-1/AC-78-2 explicitly require `cargo pmcp test apps --mode claude-desktop` (i.e., the actual CLI binary) to fail/pass against served widget resources — not the library function. Plan 02's `apps_helpers.rs` and Plan 03's example both bypass the CLI entry point.
   - *Recommended fix:* Add at least one CLI-level acceptance test that runs `cargo pmcp test apps --mode claude-desktop` against an in-process MCP server fixture (or via `assert_cmd::Command`) and asserts exit code + stderr substrings. Place it in `cargo-pmcp/tests/` or `examples/`. Without this, AC-78-1 and AC-78-2 are not provably met.

3. **Fixture comments contain scanner signal literals (HIGH — Codex).** Hand-authored fixtures (`broken_no_sdk.html` and inline test HTML strings) include `@modelcontextprotocol/ext-apps`, `new App({...})`, and handler names inside HTML/JS comments. The current regex strategy does not strip comments before scanning, so the scanner will match these comment-only literals and the "broken" fixtures will pass when they should fail.
   - *Recommended fix (pick one):* (a) Remove all signal literals from fixture comments — write fixtures with only structural markup, no hint text. (b) Strip JS line/block comments and HTML comments before scanning (preferred — protects real-world widgets too). Option (b) closes a real correctness gap, not just a test-fixture gap.

4. **`tool_filter` is bypassed in the widget-validation path (HIGH — Codex).** Plan 02 rebuilds `app_tools` from all App-capable tools in the server, ignoring whatever `--tool` filter the user passed to `cargo pmcp test apps`. Plan 01's `validate_widgets(&[(uri, html)])` API also has no tool identity, so even if the filter were applied, the validator could not associate widget results with the original tool name in error reports.
   - *Recommended fix:* Either (a) prefilter `app_tools` against `tool_filter` in `cargo-pmcp/src/commands/test/apps.rs` before reading widget bodies, OR (b) change the `validate_widgets` signature to `&[(tool_name, uri, html)]` so error reports can name the tool. Option (a) is the minimal fix; option (b) also resolves report quality.

Two MEDIUM Codex concerns are also worth addressing:

5. **(MEDIUM — Codex)** The 10MB cap is enforced after `read_resource` already loaded the body, so it is not real transport-layer protection — it is output hygiene only. Either document it as such in Plan 02's `<threat_model>` (downgrading T-78-02-01's mitigation strength) or apply size enforcement in `ServerTester::read_resource` itself.

6. **(MEDIUM — Codex)** Duplicate widget URIs across tools will be re-read and re-reported. Dedupe URIs before calling `read_widget_bodies` to avoid noise in reports and unnecessary network calls.

### Recommendation

Run `/gsd-plan-phase 78 --reviews` to incorporate these findings. The four HIGH-severity items (ChatGPT-mode drift, missing CLI E2E test, fixture comment contamination, `tool_filter` bypass) should be treated as plan-blocking and resolved in the revised plans before execution begins.
