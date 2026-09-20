---
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
plan: 04
subsystem: docs-and-printer
tags: [mcp-apps, docs, guide-anchors, cli-help, claude-desktop, writer-seam]

# Dependency graph
requires:
  - plan: 78-01
    provides: AppValidator::validate_widgets emitting [guide:SLUG] tokens, three-element (tool_name, uri, html) tuple signature (REVISION HIGH-4)
provides:
  - "report::expand_guide_anchor — pure pub function turning [guide:SLUG] tokens into absolute GUIDE.md GitHub URLs at print time"
  - "TestReport::print_to_writer<W: Write> — writer-seam refactor enabling test capture into Vec<u8> (Codex MEDIUM remediation)"
  - "5 explicit HTML id anchors in src/server/mcp_apps/GUIDE.md (handlers-before-connect, do-not-pass-tools, csp-external-resources, vite-singlefile, common-failures-claude)"
  - "## App Validation section in cargo-pmcp/README.md with 3-mode comparison table, --tool filter example, MIME profile note, Vite singlefile note, Pitfall 1 disclosure"
  - "## App Validation section in crates/mcp-tester/README.md mirroring ## Protocol Conformance block style"
  - "Enriched ///-doc on TestCommand::Apps so `cargo pmcp test apps --help` long-text spells out all three modes including `--tool` honoring (REVISION HIGH-4)"
  - "3 integration tests in crates/mcp-tester/tests/error_messages_anchored.rs (end-to-end anchor expansion, no orphaned tokens, tool-name presence per REVISION HIGH-4)"
affects: [PHASE-78-AC-5 (README + --help document the new mode) — completed]

# Tech tracking
tech-stack:
  added: [no new runtime dependencies — std::io::Write only]
  patterns:
    - "Writer-seam refactor: extract `print_to_writer<W: Write>` from a stdout-bound `print` function so tests can capture output into Vec<u8> (Codex MEDIUM)"
    - "Token-based anchor expansion: validators emit opaque `[guide:SLUG]` tokens, printer expands at render time. Decouples error-message authoring from URL formatting"
    - "Explicit HTML `<a id=\"...\">` anchors immediately above `###` headings — stable link targets independent of heading text rewording"

key-files:
  created:
    - crates/mcp-tester/tests/error_messages_anchored.rs
  modified:
    - crates/mcp-tester/src/report.rs
    - crates/mcp-tester/src/lib.rs
    - src/server/mcp_apps/GUIDE.md
    - cargo-pmcp/README.md
    - crates/mcp-tester/README.md
    - cargo-pmcp/src/commands/test/mod.rs
    - .planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/deferred-items.md

key-decisions:
  - "Writer-seam refactor over stdout-capture (gag/os_pipe) — cleaner, no new deps, only minor invasiveness"
  - "Verbose mode also calls expand_guide_anchor for consistency (one extra line beyond plan's pretty-only requirement, but ergonomic — verbose users would otherwise see raw [guide:SLUG] tokens)"
  - "Pre-existing clippy errors in cargo-pmcp pentest/loadtest/deployment files NOT fixed inline — out of scope per executor SCOPE BOUNDARY rule, logged to deferred-items.md with reproduction proof"

patterns-established:
  - "Pattern: Writer-seam print_to_writer<W: Write> alongside thin stdout `print` — enables test capture and matches Rust ecosystem idiom (similar to slog/log writers, std::io::stdout())"
  - "Pattern: print_test_result_pretty calls expand_guide_anchor on details before colorizing — token-to-URL transformation happens at the very last possible moment"

requirements-completed: [PHASE-78-AC-5]

# Metrics
duration: 9min
completed: 2026-05-02
---

# Phase 78 Plan 04: Polish user-facing surface (READMEs, --help, GUIDE anchors, expander) Summary

**A pure `expand_guide_anchor` helper turns validator-emitted `[guide:SLUG]` tokens into absolute GUIDE.md GitHub URLs at print time, the printer is refactored to a writer-seam for test capture (Codex MEDIUM), GUIDE.md gains 5 stable HTML id anchors, both READMEs document `--mode claude-desktop` with the `--tool` filter (REVISION HIGH-4) and Gemini optional Vite/MIME notes, and 3 integration tests guard the end-to-end contract.**

## Performance

- **Duration:** ~9 min (4 atomic task commits)
- **Started:** 2026-05-02T17:51:18Z
- **Completed:** 2026-05-02T17:59:56Z
- **Tasks:** 4 (Task 1 TDD, Task 2 docs-only, Task 3 docs-and-clap, Task 4 TDD)
- **Files modified:** 7 (1 created, 6 modified)

## Accomplishments

- Added `pub fn expand_guide_anchor(&str) -> String` in `crates/mcp-tester/src/report.rs` — pure, infallible string-replace over 5 known anchor slugs.
- Refactored `TestReport::print` into a writer-seam: new `pub fn print_to_writer<W: Write>(...)` helper accepts any writer; existing `print(OutputFormat)` is a thin wrapper. All four output-format paths (`print_pretty`, `print_json`, `print_minimal`, `print_verbose`) and four helpers (`print_test_result_pretty`, `print_summary_pretty`, `print_recommendations`, the per-test pretty row) converted from `println!` to `writeln!(w, ...)` with `std::io::Result<()>` return types.
- Wired `expand_guide_anchor` into both `print_test_result_pretty` and `print_verbose` so `details` strings auto-expand at render time.
- Re-exported `expand_guide_anchor` from `crates/mcp-tester/src/lib.rs` so the integration test can import it from the crate root.
- Added 5 unit tests covering known-slug expansion, no-token round-trip, unknown-slug preservation, multi-token expansion, and the `common-failures-claude` slug.
- Added 1 printer-level test `pretty_output_includes_expanded_url` (Codex MEDIUM remediation) capturing pretty output to `Vec<u8>` and asserting the canonical URL appears + the unexpanded token does NOT.
- Inserted 5 explicit HTML `<a id="...">` anchor markers in `src/server/mcp_apps/GUIDE.md` immediately before the matching `###` headings (csp-external-resources, handlers-before-connect, do-not-pass-tools, vite-singlefile, common-failures-claude).
- Enriched the `///`-doc on `cargo-pmcp::commands::test::TestCommand::Apps` so `cargo pmcp test apps --help` shows the 3-mode breakdown including the strict static widget inspection performed by `--mode claude-desktop` and the REVISION HIGH-4 note that the mode honors `--tool`.
- Added a new `## App Validation` section to `cargo-pmcp/README.md` (3-mode table + 4 invocation examples incl. `--tool` filter + Why-claude-desktop subsection + MIME profile note + Vite singlefile note + Pitfall 1 disclosure of legacy chess/dataviz/map examples failing the new mode).
- Added a shorter `## App Validation` section to `crates/mcp-tester/README.md` after `## Protocol Conformance` mirroring its block style.
- Created `crates/mcp-tester/tests/error_messages_anchored.rs` with 3 integration tests: `error_messages_anchored` (end-to-end token→URL), `no_orphaned_guide_tokens` (round-trip cleanliness, mitigates threat T-78-04-02), `error_messages_include_tool_name` (REVISION HIGH-4 actionable-error contract).

## Task Commits

Each task committed atomically (--no-verify per parallel-executor protocol; orchestrator validates hooks once after merge):

1. **Task 1 (TDD): expand_guide_anchor + writer-seam printer** — `8579d00d` (feat)
2. **Task 2: 5 explicit HTML id anchors in GUIDE.md** — `eec3d829` (docs)
3. **Task 3: enriched --help + READMEs (REVISION HIGH-4 + Gemini optional notes)** — `c8acf773` (docs)
4. **Task 4 (TDD): error_messages_anchored integration tests** — `e2c0d6e1` (test)

## Files Created/Modified

- **Created:** `crates/mcp-tester/tests/error_messages_anchored.rs` — 3 integration tests (`error_messages_anchored`, `no_orphaned_guide_tokens`, `error_messages_include_tool_name`).
- **Modified:**
    - `crates/mcp-tester/src/report.rs` — added `pub fn expand_guide_anchor`, `pub fn print_to_writer<W: Write>` writer-seam, converted four `print_*` helpers to take a writer + return `std::io::Result<()>`, wired `expand_guide_anchor` into `print_test_result_pretty` and `print_verbose`, added 5 unit tests + 1 printer-level test.
    - `crates/mcp-tester/src/lib.rs` — re-exported `expand_guide_anchor` from `report::`.
    - `src/server/mcp_apps/GUIDE.md` — 5 explicit HTML `<a id="...">` anchor markers.
    - `cargo-pmcp/src/commands/test/mod.rs` — replaced terse 2-line `///`-doc on `TestCommand::Apps` with enriched 18-line block listing all three modes and the `Honors --tool` REVISION HIGH-4 note.
    - `cargo-pmcp/README.md` — added new `## App Validation` section (~50 lines) between `## Commands` and `## Global Flags`.
    - `crates/mcp-tester/README.md` — added new `## App Validation` section (~17 lines) after `## Protocol Conformance`.
    - `.planning/phases/.../deferred-items.md` — appended a section logging 5 pre-existing clippy errors in `cargo-pmcp` pentest/loadtest/deployment code (verified pre-existing on base commit `a5fd2844` by stash + re-run).

## Decisions Made

- **Writer-seam refactor (Codex MEDIUM remediation).** Plan offered two paths: writer-seam refactor (cleaner) or stdout capture via `gag`/`os_pipe` (less invasive). Chose writer-seam: no new deps, smaller diff than expected, follows Rust idiom. Eight functions converted from `println!` to `writeln!(w, ...)`; the public surface retains the stdout-bound `print(OutputFormat)` wrapper for backward compatibility — bin callers and existing CLI users see no change.
- **Verbose mode also expands tokens.** Plan only required pretty-mode expansion. Verbose mode calls `print_pretty` first then dumps a "DETAILED TEST INFORMATION" block. Added `expand_guide_anchor(details)` to the verbose dump too — a verbose user encountering a raw `[guide:SLUG]` token would be confused. One extra `expand_guide_anchor` call, no downside.
- **`prettytable::Table::print(w)` instead of `printstd()`.** The summary table's only stdout sink was `table.printstd()`; `prettytable::Table::print(w: &mut W)` accepts any writer and is what `printstd` wraps internally. Direct port.
- **Wrapped print test in `colored::control::set_override(false)`.** ANSI escape sequences from `colored` would otherwise interfere with substring matching. Test sets the override on entry and resets on exit.
- **5 unit tests + 1 printer-level test.** Plan's behavior block specified 6 tests; mapped them 1:1.
- **Tool name in tuple uses `open_dashboard`.** Matches Plan 03's broken-fixture convention and the REVISION HIGH-4 contract from Plan 01's signature change.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Pre-existing clippy errors in `cargo-pmcp` (out-of-scope, NOT fixed inline)**

- **Found during:** Task 3 verification (`cargo clippy -p cargo-pmcp --all-targets -- -D warnings`).
- **Errors:** 5 errors in `pentest/attacks/data_exfiltration.rs` (3× `manual_contains`), `pentest/attacks/prompt_injection.rs` (2× `type_complexity`), `pentest/attacks/protocol_abuse.rs` (1× `unnecessary_cast`), `loadtest/summary.rs` (1× `vec_init_then_push`), `deployment/config.rs` (1× `collapsible_match`).
- **Verification this is pre-existing:** Stashed Plan 78-04 working-tree changes and re-ran `cargo clippy -p cargo-pmcp --all-targets -- -D warnings` against base commit `a5fd2844` — same errors reproduced.
- **Why not fixed inline:** Plan 78-04 only modifies markdown READMEs and a `///`-doc clap comment. None of the failing files are in scope. Per executor SCOPE BOUNDARY rule, only auto-fix issues directly caused by the current task's changes.
- **Action taken:** Logged to `.planning/phases/.../deferred-items.md` with full reproduction proof. Plan 78-04 verification scope narrowed to `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` (clean, exits 0) plus `cargo build -p cargo-pmcp` (exits 0) plus `cargo run -p cargo-pmcp -- test apps --help` content checks (all pass) plus `cargo fmt --all -- --check` (exits 0).
- **Committed in:** `c8acf773` (Task 3 — deferred-items.md update is part of the same commit).

**2. [fmt] Auto-format applied to report.rs after Task 1's initial Edit pass**

- **Found during:** Task 1 verification (`cargo fmt --all -- --check`).
- **Issue:** Two minor format violations in test code (line-length wrapping for `assert!` + `to_string()` placement on the printer-level test).
- **Fix:** Ran `cargo fmt --all`, then re-ran tests to confirm nothing broke. 9 tests still pass.
- **Files modified:** `crates/mcp-tester/src/report.rs` (whitespace/wrapping only).
- **Committed in:** `8579d00d` (Task 1 — same commit).

---

**Total deviations:** 2 — 1 SCOPE-BOUNDARY pre-existing-issue deferral (logged to deferred-items.md), 1 auto-fmt cleanup. No scope creep.

## Issues Encountered

- **Pre-existing clippy errors in cargo-pmcp.** Verified pre-existing on base commit `a5fd2844`. Documented above and in `deferred-items.md`. Does NOT block Plan 78-04 acceptance because Task 3 only changes markdown + clap doc-text and cannot have introduced new clippy violations in unrelated files.

## User Setup Required

None — no external service configuration required. All changes ship as documentation, tests, or pure-Rust additions.

## Next Phase Readiness

- **Phase 78 complete after this plan.** PHASE-78-AC-5 satisfied (README + `--help` document the new mode); the four AC-tracking items per `<must_haves>` (`--help` long-text, `## App Validation` sections in both READMEs, GUIDE.md anchors, anchor-expander wired into printer, 3 integration tests) are all delivered.
- **No blockers.** All Plan 78-04 verification commands exit 0; `cargo test -p mcp-tester --test error_messages_anchored` passes 3/3; `cargo test -p mcp-tester --lib` passes 58/58 (regression).
- **Future plans can reuse the writer-seam.** Any future verbose/pretty test that needs to capture output now has a stable `print_to_writer` API to call instead of resorting to stdout-capture hacks.

## Self-Check: PASSED

- **Files claimed created exist:**
    - `crates/mcp-tester/tests/error_messages_anchored.rs` — verified present.
- **Files claimed modified exist (with required content):**
    - `crates/mcp-tester/src/report.rs` — `pub fn expand_guide_anchor` and `pub fn print_to_writer<W: Write>` both present (grep verified).
    - `crates/mcp-tester/src/lib.rs` — `expand_guide_anchor` re-exported (grep verified).
    - `src/server/mcp_apps/GUIDE.md` — all 5 anchors present (`for slug in ...; do grep -q "id=\"$slug\""; done` verified).
    - `cargo-pmcp/src/commands/test/mod.rs` — clap doc contains `claude-desktop`, `statically inspects`, and ``Honors `--tool``` (verified by grep + `cargo run -p cargo-pmcp -- test apps --help` output capture).
    - `cargo-pmcp/README.md` — `## App Validation`, `claude-desktop`, `--tool`, `MIME profile`, `Vite singlefile`, `mcp-apps-chess` all grep-findable (verified).
    - `crates/mcp-tester/README.md` — `## App Validation` and `claude-desktop` grep-findable (verified).
    - `.planning/phases/.../deferred-items.md` — Plan 78-04 deferred-items section appended (verified).
- **Commits claimed exist:**
    - `8579d00d` — Task 1 (`feat(78-04): add expand_guide_anchor + writer-seam printer (Task 1)`).
    - `eec3d829` — Task 2 (`docs(78-04): add 5 explicit HTML id anchors to mcp_apps/GUIDE.md (Task 2)`).
    - `c8acf773` — Task 3 (`docs(78-04): document --mode claude-desktop in READMEs + --help (Task 3)`).
    - `e2c0d6e1` — Task 4 (`test(78-04): add error_messages_anchored integration tests (Task 4)`).
- **Acceptance criteria:**
    - `cargo test -p mcp-tester report` — 9 passed (5 unit + 1 printer + 3 pre-existing).
    - `cargo test -p mcp-tester --test error_messages_anchored` — 3 passed.
    - `cargo test -p mcp-tester --lib` — 58 passed (regression).
    - `cargo run -p cargo-pmcp -- test apps --help | grep -q claude-desktop` — exits 0.
    - `cargo run -p cargo-pmcp -- test apps --help | grep -qi static` — exits 0.
    - `grep -q "## App Validation" cargo-pmcp/README.md` — exits 0.
    - `grep -q "## App Validation" crates/mcp-tester/README.md` — exits 0.
    - `grep -q "MIME profile" cargo-pmcp/README.md` — exits 0.
    - `grep -q "Vite singlefile" cargo-pmcp/README.md` — exits 0.
    - `grep -q "mcp-apps-chess" cargo-pmcp/README.md` — exits 0 (Pitfall 1).
    - For-loop grep over the 5 GUIDE.md anchors — all exit 0.
    - `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` — exits 0 (Tasks 1+4 scope clean).
    - `cargo fmt --all -- --check` — exits 0.
    - `cargo build -p cargo-pmcp` — exits 0.

---
*Phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-*
*Plan: 04*
*Completed: 2026-05-02*
