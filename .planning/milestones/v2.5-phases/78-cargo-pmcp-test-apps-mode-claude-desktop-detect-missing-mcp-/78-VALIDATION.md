---
phase: 78
slug: cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-02
last_revised: 2026-05-02 (REVIEWS round 1 — incorporated HIGH-1, HIGH-2, HIGH-3, HIGH-4, MED-5, MED-6 + Gemini doc-only items)
---

# Phase 78 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (rust 1.x) + proptest 1.7 + cargo-fuzz (libfuzzer-sys 0.4) + assert_cmd 2 (REVISION HIGH-2 — CLI E2E) |
| **Config file** | Cargo.toml (workspace root + crates/mcp-tester/Cargo.toml + cargo-pmcp/Cargo.toml) |
| **Quick run command** | `cargo test -p mcp-tester app_validator` |
| **Full suite command** | `make quality-gate` (matches CI: fmt + clippy pedantic+nursery + build + test + audit) |
| **Estimated runtime** | quick: ~10s; full: ~120s |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p mcp-tester app_validator` (or scoped equivalent for the file under change)
- **After every plan wave:** Run `cargo test -p mcp-tester && cargo test -p cargo-pmcp`
- **Before `/gsd-verify-work`:** Full `make quality-gate` must be green
- **Max feedback latency:** 30 seconds for unit tests; 120 seconds for full quality-gate

---

## Per-Task Verification Map

> The planner fills concrete task IDs after PLAN.md generation. The matrix below
> captures the mandatory verification dimensions derived from RESEARCH.md and
> the REVIEWS-driven revision items.
>
> Note (post-REVIEWS revision): test command names below match the actual
> tests created in each plan after REVIEWS HIGH-1, HIGH-2, HIGH-3, HIGH-4,
> MED-5, MED-6 remediation.

| Dimension | Plan | Wave | Acceptance Criterion | Test Type | Automated Command | Status |
|-----------|------|------|----------------------|-----------|-------------------|--------|
| Strict mode WIRES UP behavior (no longer "same as Standard") | 01 | 1 | `AppValidationMode::ClaudeDesktop` produces ERROR-level issues for missing handlers/imports | unit | `cargo test -p mcp-tester app_validator::tests::claude_desktop_mode_emits_failed_for_missing_handlers` | ⬜ pending |
| Standard mode emits ONE summary WARN per widget (Q4 RESOLVED) | 01 | 1 | `AppValidationMode::Standard` emits exactly 1 Warning row per widget listing missing signals | unit | `cargo test -p mcp-tester app_validator::tests::standard_mode_emits_one_summary_warn_per_widget` | ⬜ pending |
| ChatGpt mode emits ZERO widget-related rows (REVISION HIGH-1) | 01 | 1 | `AppValidationMode::ChatGpt` returns an empty `Vec<TestResult>` for widget validation, regardless of widget shape | unit | `cargo test -p mcp-tester app_validator::tests::chatgpt_mode_emits_no_widget_results` | ⬜ pending |
| Comment stripper: signals inside JS/HTML comments do NOT match (REVISION HIGH-3) | 01 | 1 | `strip_js_comments` removes JS line/block + HTML comments before signal regexes; signals inside comments are NOT detected | unit | `cargo test -p mcp-tester app_validator::tests::scan_widget_ignores_signals_inside_comments` | ⬜ pending |
| Handler signal scanner (4 handlers + ontoolresult + connect + new App) | 01 | 1 | Each protocol-handler property assignment is detected; ≥3 of 4 fallback path covered | unit | `cargo test -p mcp-tester app_validator::tests::scan_widget_detects_handlers_via_property_assignment` | ⬜ pending |
| Scanner robustness (no panics on arbitrary HTML/JS) | 03 | 3 | Property test never panics; idempotent on whitespace normalization | property | `cargo test -p mcp-tester --test property_tests` | ⬜ pending |
| Fuzz target for regex/AST scan path | 03 | 3 | `cargo +nightly fuzz run app_widget_scanner` runs ≥60s without crash; compile-only check on stable | fuzz | `(cd fuzz && cargo build --bin app_widget_scanner)` (stable) / `cargo +nightly fuzz run app_widget_scanner -- -max_total_time=60` (nightly) | ⬜ pending |
| `resources/read` plumbing (Vec<(tool_name, uri, html)>) | 02 | 2 | apps.rs forwards widget bodies to validator with tool name; missing/large/non-text bodies handled | unit + integration | `cargo test -p cargo-pmcp --bin cargo-pmcp commands::test::apps && cargo test -p cargo-pmcp --test apps_helpers test_apps_resources_read_e2e` | ⬜ pending |
| `tool_filter` honored at widget read site (REVISION HIGH-4) | 02 | 2 | `--tool` flag restricts which widgets are read; validator output names the tool | unit + integration | `cargo test -p cargo-pmcp --test apps_helpers tool_filter_restricts_widget_validation` | ⬜ pending |
| Widget URIs deduplicated before reading (REVISION MED-6) | 02 | 2 | Multiple tools sharing one URI cause exactly one `read_resource` call; per-tool validator rows fan out from the cached body | unit + integration | `cargo test -p cargo-pmcp --bin cargo-pmcp commands::test::apps::tests::dedup_widget_uris_collapses_duplicates && cargo test -p cargo-pmcp --test apps_helpers widget_uris_deduplicated_at_read_time` | ⬜ pending |
| CLI E2E: broken widget FAILS `cargo pmcp test apps --mode claude-desktop` (REVISION HIGH-2) | 02 | 2 | Binary exit non-zero on broken fixture; stderr/stdout names a missing handler | acceptance (CLI) | `cargo test -p cargo-pmcp --test cli_acceptance cli_acceptance_broken_widget_fails_claude_desktop` | ⬜ pending |
| CLI E2E: corrected widget PASSES `cargo pmcp test apps --mode claude-desktop` (REVISION HIGH-2) | 02 | 2 | Binary exit zero on corrected fixture | acceptance (CLI) | `cargo test -p cargo-pmcp --test cli_acceptance cli_acceptance_corrected_widget_passes_claude_desktop` | ⬜ pending |
| CLI E2E: Standard mode passes both fixtures (REVISION HIGH-2 — AC-78-3) | 02 | 2 | Binary exit zero on both broken and corrected fixtures under default Standard mode | acceptance (CLI) | `cargo test -p cargo-pmcp --test cli_acceptance cli_acceptance_standard_mode_passes_both_fixtures` | ⬜ pending |
| CLI E2E: ChatGpt mode passes both fixtures + no handler-name output (REVISION HIGH-2 — AC-78-4) | 02 | 2 | Binary exit zero on both fixtures under `--mode chatgpt`; stderr/stdout does NOT contain any of the 4 protocol handler names | acceptance (CLI) | `cargo test -p cargo-pmcp --test cli_acceptance cli_acceptance_chatgpt_mode_passes_both_and_no_handler_messages` | ⬜ pending |
| Cost Coach reproducer FAILS in claude-desktop mode (library) | 03 | 3 | Broken widget produces ≥1 Failed row naming a missing handler AND tool name | integration (acceptance) | `cargo test -p mcp-tester --test app_validator_widgets test_broken_widget_fails_claude_desktop` | ⬜ pending |
| `cargo pmcp test apps` (Standard) emits ONE summary WARN on broken widget | 03 | 3 | No regression on permissive default; emission shape matches Q4 RESOLVED | integration | `cargo test -p mcp-tester --test app_validator_widgets test_standard_mode_one_summary_warn_for_broken` | ⬜ pending |
| `--mode chatgpt` zero widget rows (REVISION HIGH-1, library-side) | 03 | 3 | ChatGpt mode emits exactly 0 results for any widget shape | regression (tightened) | `cargo test -p mcp-tester --test app_validator_widgets test_chatgpt_mode_unchanged_zero_results` | ⬜ pending |
| README + `--help` document the mode (incl. `--tool` honoring per REVISION HIGH-4 + Gemini optional Vite/MIME notes) | 04 | 2 | grep finds claude-desktop section in README; `--help` shows the new mode; README mentions `--tool` filter, MIME profile, Vite singlefile note | docs check | `cargo run -p cargo-pmcp -- test apps --help \| grep claude-desktop && grep -q "claude-desktop" cargo-pmcp/README.md && grep -q "MIME profile" cargo-pmcp/README.md && grep -q "Vite singlefile" cargo-pmcp/README.md && grep -q -- "--tool" cargo-pmcp/README.md` | ⬜ pending |
| Error messages link to GUIDE anchor (library + tool name presence per REVISION HIGH-4) | 04 | 2 | Validator results include tool name in `name`; `[guide:handlers-before-connect]` token expands to canonical URL | integration | `cargo test -p mcp-tester --test error_messages_anchored error_messages_anchored && cargo test -p mcp-tester --test error_messages_anchored error_messages_include_tool_name` | ⬜ pending |
| Pretty printer renders expanded URL (REVISION Codex MEDIUM) | 04 | 2 | Captured pretty-print output contains the expanded GUIDE URL, not the raw `[guide:...]` token | unit (printer) | `cargo test -p mcp-tester report::tests::pretty_output_includes_expanded_url` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

The planner MUST emit task IDs that map onto every row above; no row may be left
without a `task_id` reference once plans are generated.

### Wave map (post-REVIEWS revision)

| Plan | Wave | depends_on | Tasks (post-revision) |
|------|------|------------|----------------------|
| 01 (validator core) | 1 | [] | 2 (Task 1: scanner + comment stripper; Task 2: validate_widgets w/ 3-way dispatch + tool-name tuple) |
| 02 (CLI plumbing + CLI E2E) | 2 | [78-01] | 4 (Task 1: helpers w/ dedup; Task 2: execute() wiring w/ tool_filter; Task 3: apps_helpers.rs; Task 4: cli_acceptance.rs — REVISION HIGH-2) |
| 03 (ALWAYS reqs + fixtures + tests) | 3 | [78-01, 78-02] | 3 (Task 1: fixtures w/ comment hygiene; Task 2: property + integration tests; Task 3: fuzz + example) |
| 04 (docs + GUIDE anchors + expander) | 2 | [78-01] | 4 (Task 1: expand_guide_anchor + writer-seam printer; Task 2: GUIDE anchors; Task 3: READMEs + Gemini optional notes; Task 4: error_messages_anchored.rs) |

---

## Wave 0 Requirements

- [ ] `crates/mcp-tester/Cargo.toml` — add `proptest = "1"` to `[dev-dependencies]`
- [ ] `cargo-pmcp/Cargo.toml` — add `assert_cmd = "2"` and `predicates = "3"` to `[dev-dependencies]` (REVISION HIGH-2)
- [ ] `crates/mcp-tester/fuzz/` — initialize cargo-fuzz workspace if absent (or attach to existing one)
- [ ] `crates/mcp-tester/tests/fixtures/widgets/` — directory will be created by Plan 03 Task 1
- [ ] `crates/mcp-tester/tests/` — directory exists for integration tests against the fixture pair

*Existing infrastructure covers ServerTester::read_resource, regex 1.x, and the cargo test harness.*

---

## Manual-Only Verifications

| Behavior | Acceptance Ref | Why Manual | Test Instructions |
|----------|----------------|------------|-------------------|
| Vite singlefile minification fidelity | RESEARCH §A1 (Q1 RESOLVED — deferred to follow-up) | Confirms `\.\s*onteardown\s*=` patterns survive `vite build --mode production` | Per RESEARCH Q1 RESOLVED, the fixture pair is hand-authored to mirror Vite output. Empirical Vite verification is moved to a follow-up phase if scanner false-negatives are observed in the wild. (REVISION Gemini optional: README mentions this in the Vite singlefile note.) |
| Cost Coach reproducer parity | Acceptance §1 (Q2 RESOLVED — synthetic fixture in-tree) | Phase 78 ships a synthetic minimal fixture; full Cost Coach bundle is not vendored | Per RESEARCH Q2 RESOLVED. |
| MIME profile (`;profile=mcp-app`) round-trip with a real Claude Desktop instance | Gemini optional (doc-only) | Requires Claude Desktop install + a published MCP server | Documented in cargo-pmcp/README.md `### MIME profile` subsection (REVISION optional Gemini doc-only). The validator does NOT check MIME profile parameter — that's a separate `validate_tools` concern. |

---

## Validation Sign-Off

- [ ] Every plan task has `<automated>` verify or Wave 0 dependency
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (proptest dep, assert_cmd dev-dep per REVISION HIGH-2, fuzz workspace, fixture dir)
- [ ] No watch-mode flags in any test command
- [ ] Feedback latency < 30s for unit; < 120s for full
- [ ] `nyquist_compliant: true` set in frontmatter once gate passes
- [ ] All ALWAYS requirements (unit + property + fuzz + example) mapped to plan tasks
- [ ] Every test command in the verification map maps to a real test name in the corresponding plan (Warning 5 fix)
- [ ] Wave numbering is consistent with depends_on (Wave 2 fix per Warning 3)
- [ ] All 4 REVIEWS HIGH findings addressed: HIGH-1 (chatgpt no-emit) + HIGH-2 (CLI E2E) + HIGH-3 (comment stripper + fixture hygiene) + HIGH-4 (tool_filter + tool-name in tuple)
- [ ] Both REVIEWS MEDIUM findings addressed: MED-5 (10MB cap reclassified as output hygiene) + MED-6 (URI dedup)
- [ ] Two doc-only Gemini optional items in Plan 04 README task: Vite singlefile note + MIME profile note

**Approval:** pending
