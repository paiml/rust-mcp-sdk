---
phase: 82-builder-dx-prerequisites
plan: 02
subsystem: testing
tags:
  - integration-test
  - handler-level-testing
  - regression-anchor
  - proptest
  - bldr-03
dependency_graph:
  requires:
    - phase: 82-builder-dx-prerequisites/plan-01
      provides: "ServerBuilder::tool_arc, ServerBuilder::prompt_arc, Server::get_tool"
  provides:
    - "tests/in_process_handler_pattern.rs (BLDR-03 regression anchor)"
    - "Property-test invariant: tool() and tool_arc() observationally equivalent on the public API surface"
  affects:
    - "Phase 83 pmcp-server-toolkit (the documented testing pattern these tests anchor)"
    - "Future refactors of ServerBuilder::tool_arc / get_tool / prompt_arc / get_prompt (the regression-anchor catches breakage)"
tech_stack:
  added: []
  patterns:
    - "Handler-level integration testing: builder().*_arc(...).build() → server.get_*(name) → handle(args, RequestHandlerExtra::default()).await"
    - "Arc-identity assertion via Arc::ptr_eq against a retained clone (proves no clone-on-insert)"
    - "Sync-proptest wrapping async via tokio::runtime::Builder::new_current_thread() + block_on (mirrors tests/property_tests.rs convention)"
    - "USAGE-narrowed negative grep for private-dispatch symbols (rejects .method_call( and use ...path statements; allows free-form prose)"
key_files:
  created:
    - tests/in_process_handler_pattern.rs
  modified: []
key_decisions:
  - "Property test asserts the public observable (handle output byte-equality + has_tool result) — not the private capabilities field; capability-shape equivalence lives in Plan 01 Task 3's crate-internal test"
  - "Negative-grep regex narrowed to USAGE patterns (method-call and import-statement forms) so the explanatory module-doc comment is allowed to mention the symbol name in prose"
  - "Prose deliberately avoids the literal `.handle_request(` token to keep the narrowed grep zero-match; the prose now refers to it abstractly as 'the private JSONRPC dispatch entry point'"
  - "Per-test EchoTool / EchoPrompt fixtures (not shared global) — keeps each test self-contained and matches the existing crate convention in tests/skills_integration.rs"
requirements_completed:
  - BLDR-01
  - BLDR-02
  - BLDR-03
metrics:
  duration_minutes: ~25
  tasks_completed: 2
  commits_created: 2
  files_modified: 1
  test_pass_count: "4 (3 #[tokio::test] + 1 proptest test); also clean under cargo build --tests, cargo clippy --tests -D warnings, and full make quality-gate"
completed: 2026-05-17
---

# Phase 82 Plan 02: BLDR-03 Handler-Level Testing Pattern Regression Anchor Summary

**Regression-anchor integration test exercising the documented handler-level testing pattern (CONTEXT.md D-02 part (b)) end-to-end via `tool_arc`+`get_tool` and `prompt_arc`+`get_prompt` round-trips, plus a 32-case proptest proving `tool()` and `tool_arc()` produce observationally equivalent post-build state on the public API.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-05-18T02:25:00Z (worktree handoff from orchestrator)
- **Completed:** 2026-05-18T02:48:00Z
- **Tasks:** 2 (both `type="auto" tdd="true"`)
- **Files modified:** 1 (created)

## Accomplishments

- **`tests/in_process_handler_pattern.rs` created** — the BLDR-03 regression anchor. If any future refactor changes the handler-level testing pattern that external toolkit authors (Phase 83+) rely on, this test fails.
- **Three `#[tokio::test]` round-trip tests** exercise the pattern's three load-bearing shapes: tool side, prompt side, and tool+prompt co-registration on a single builder.
- **One `proptest!` block (32 cases)** proves `builder.tool("echo", T)` and `builder.tool_arc("echo", Arc::new(T))` produce servers whose `get_tool("echo").handle(args, extra)` outputs are byte-equal `serde_json::Value`s for the same `args`. This is the public-API expression of the "two registration paths are equivalent" invariant.
- **All public surface only** — the test does not import or invoke `Server::handle_request` (verified by the USAGE-narrowed negative grep). No `pub(crate)` re-exports, no crate-private types.

## Task Commits

Each task was committed atomically:

1. **Task 82-02-01: Three round-trip integration tests (tool + prompt + compose)** — `1586da8d` (test/integration)
2. **Task 82-02-02: Equivalence property test (tool() vs tool_arc())** — `d722e5c4` (test/property)

_Note: This plan is `type="auto" tdd="true"` but the deliverable IS a test file. RED/GREEN collapse: the tests must pass against the already-shipped Plan 01 API (`tool_arc`, `prompt_arc`, `get_tool`, `get_prompt`). There is no separate implementation step._

Two unrelated commits (`8a99ac28` and `13834d45`) from Plan 82-03 landed between my Task 1 and Task 2 commits via parallel orchestration on the same branch. They modify `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, and `pmcp-book/src/ch15-testing.md` — none of which overlap with `tests/in_process_handler_pattern.rs`.

## Files Created/Modified

- `tests/in_process_handler_pattern.rs` — **created**. 228 lines. Contains:
  - Module-level `//!` docs describing the BLDR-03 regression-anchor purpose.
  - `#![cfg(not(target_arch = "wasm32"))]` file-level gate (matches `tests/skills_integration.rs:27` shape).
  - Imports from `pmcp::types::{Content, GetPromptResult, PromptMessage, Role}` and `pmcp::{PromptHandler, RequestHandlerExtra, Server, ToolHandler}` (all root-level public).
  - `EchoTool` struct + `#[async_trait] impl ToolHandler` (returns `{"echoed": args}`).
  - `EchoPrompt` struct + `#[async_trait] impl PromptHandler` (returns one `Role::User` message with `Content::text("hello")`).
  - Three `#[tokio::test]` round-trip tests with `Arc::ptr_eq` identity assertions.
  - One `proptest!` block with `cases: 32` and the equivalence property test wrapped in `tokio::runtime::Builder::new_current_thread().block_on(...)`.

## Decisions Made

### D-01: Property test narrows scope to publicly observable state

The plan's `must_haves` was revised post-cross-AI-review to drop `capabilities`-shape assertion from this integration test, since `Server::capabilities` is a private field (`src/server/mod.rs:317`). The integration test asserts on the two observable signals: byte-equal `handle(args, extra)` outputs and identical `has_tool(name)` results. Capability-shape equivalence between the two registration paths is asserted crate-internally by Plan 01 Task 3's `#[cfg(test)]` unit test (which has private-field access).

### D-02: Prose comment carefully avoids the literal `.handle_request(` token

The plan's USAGE-narrowed negative grep (`! grep -E '\.handle_request\(|use .*handle_request'`) was intended to allow explanatory prose. But two prose forms accidentally tripped it on the first draft:

1. The literal token `.handle_request(` inside backticks tripped the first alternation.
2. The English phrase "does NOT use \`Server::handle_request\`" tripped the second alternation (the regex matches the word "use" — including English usage — followed by anything ending in `handle_request`).

Resolution: rewrote the prose to refer to the symbol abstractly as "the private JSONRPC dispatch entry point on `Server`" so neither pattern matches. The symbol is still named in the module-level prose (search-discoverable for future maintainers), just not in a form the grep treats as USAGE.

### D-03: Negative-grep `fallback` token similarly avoided

The plan's `! grep -qE 'fallback|if proptest|hard-coded loop'` was intended to verify the property test has no commented-out fallback path. The first draft included the prose "no fallback branch" in a comment, which tripped the literal `fallback` token. Resolution: rephrased to "depends on it as a hard precondition (no alternative path)".

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Module-doc prose accidentally triggered the USAGE-narrowed negative grep**

- **Found during:** Task 1 verification (running the acceptance-criteria grep)
- **Issue:** Module docs included the literal sequence `.handle_request(` inside a backtick-quoted code reference (when explaining "the negative grep targets actual USAGE — `.handle_request(` calls and `use ...handle_request` imports — not the literal token"). The negative grep pattern `\.handle_request\(` matched the docstring even though it was prose, not Rust code. Subsequently the rephrase "does NOT use `Server::handle_request`" tripped `use .*handle_request`.
- **Fix:** Rewrote the prose to refer to the symbol abstractly as "the private JSONRPC dispatch entry point on `Server`" — the symbol name no longer appears in the prose in either USAGE form.
- **Verification:** `grep -nE '\.handle_request\(|use .*handle_request' tests/in_process_handler_pattern.rs` → 0 matches; `cargo test --test in_process_handler_pattern --features full` → 4 passed.
- **Committed in:** `1586da8d` (Task 1 commit — fix was inline before commit, not a retry).

**2. [Rule 1 — Bug] Property-test comment triggered the `fallback` negative grep**

- **Found during:** Task 2 verification
- **Issue:** The comment "this test relies on it directly with no fallback branch" tripped the negative grep `! grep -qE 'fallback|if proptest|hard-coded loop'`. The grep was intended to catch commented-out fallback code, not the word "fallback" in prose; but the grep is literal, so any occurrence trips it.
- **Fix:** Rephrased the comment to "this test depends on it as a hard precondition (no alternative path)".
- **Verification:** `grep -cE 'fallback|if proptest|hard-coded loop' tests/in_process_handler_pattern.rs` → 0; `cargo test --test in_process_handler_pattern --features full -- tool_and_tool_arc` → 1 passed.
- **Committed in:** `d722e5c4` (Task 2 commit — fix was inline before commit).

---

**Total deviations:** 2 auto-fixed (both Rule 1 — prose-vs-grep collisions in the test's own comments).
**Impact on plan:** Both fixes are pure documentation rewordings inside the new test file. They do not change test behavior, do not change the test's coverage, and do not affect any other file. The plan's verification grep contract is honored verbatim.

## Issues Encountered

- The `rg` binary on this machine is shimmed through a Claude Code wrapper that uses POSIX-grep regex semantics rather than ripgrep semantics, which made the plan's `rg --quiet '\.handle_request\(|use .*handle_request'` exit with code 2 (parse error) rather than 0/1. Worked around by using `grep -E` directly (equivalent regex semantics for this pattern); the substantive grep result is identical.

## Verification Results

| Step | Command | Result |
|------|---------|--------|
| 1 | `cargo test --test in_process_handler_pattern --features full` | exit 0 — **4 passed, 0 failed** |
| 2 | `cargo test --test in_process_handler_pattern --features full -- --nocapture` | exit 0 — all 4 test names visible |
| 3 | `cargo test --test in_process_handler_pattern --features full -- tool_and_tool_arc` | exit 0 — **1 passed** (property test alone) |
| 4 | `PROPTEST_CASES=128 cargo test --test in_process_handler_pattern --features full -- tool_and_tool_arc` | exit 0 — finished in <30s as the plan required |
| 5 | `cargo build --tests -p pmcp --features full` | exit 0 |
| 6 | `cargo clippy --tests -p pmcp --features full -- -D warnings` | exit 0 — **no issues** |
| 7 | `grep -c '#\[tokio::test\]' tests/in_process_handler_pattern.rs` | **3** (≥3 ✓) |
| 8 | `grep -cE 'tool_arc\|prompt_arc' tests/in_process_handler_pattern.rs` | **16** (≥3 ✓) |
| 9 | `grep -E '\.handle_request\(\|use .*handle_request' tests/in_process_handler_pattern.rs` | **0 matches** (negative ✓) |
| 10 | `grep -cE 'fallback\|if proptest\|hard-coded loop' tests/in_process_handler_pattern.rs` | **0** (negative ✓) |
| 11 | `grep -cE '// TODO\|// FIXME\|// HACK\|// XXX' tests/in_process_handler_pattern.rs` | **0** (zero SATD ✓) |
| 12 | `grep -c 'server\.capabilities\.\|server_a\.capabilities\|server_b\.capabilities' tests/in_process_handler_pattern.rs` | **0** (no private-field access ✓) |
| 13 | `grep -c 'Arc::ptr_eq' tests/in_process_handler_pattern.rs` | **2** (Arc-identity assertion in both round-trip tests) |
| 14 | `grep -c 'use pmcp::types::.*Role' tests/in_process_handler_pattern.rs` | **1** (Role imported via `pmcp::types::*` convention) |
| 15 | **`make quality-gate`** | exit 0 — **`✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`** |

## Requirements Closed

- **BLDR-01** (regression-anchor portion) — `tool_arc` on public `ServerBuilder` is exercised by `tool_arc_get_tool_handle_round_trip` and by the property test's `server_b` branch.
- **BLDR-02** (regression-anchor portion) — `prompt_arc` on public `ServerBuilder` is exercised by `prompt_arc_get_prompt_handle_round_trip`.
- **BLDR-03** (regression-anchor portion completed) — The documented handler-level testing pattern now has its regression anchor. The reference test exists at `tests/in_process_handler_pattern.rs` and exercises both the tool and prompt sides. (The doctest documentation portion of BLDR-03 was completed in Plan 82-01 Task 4; the book-section narrative portion is completed in Plan 82-03 Task 3.)

(Plan 82-02 does not contribute to BLDR-04 — that was fully closed by Plan 82-01 Task 3.)

## Threat Flags

None. Per the plan's `<threat_model>`, this is a test-only file:

- No new runtime code, no new public API.
- The integration test compiles into a separate test binary (per Cargo's `tests/*.rs` convention) and is never shipped to crates.io.
- It exercises only the public API that any external consumer can already exercise.

T-82-04 (the threat register's only entry for this file) holds: STRIDE does not apply.

## Next Phase Readiness

- Plan 82-03 (book chapter narrative + ROADMAP + REQUIREMENTS doc-only updates) has been running in parallel and has landed two of its three commits (`122e2080`, `8a99ac28`, `13834d45`) on the same branch. The orchestrator's Plan 82-04 (final docs + state) is unblocked.
- Phase 83 (`pmcp-server-toolkit`) can now consume the handler-level testing pattern with full confidence that it will be caught if regressed.

## Self-Check: PASSED

- ✅ `tests/in_process_handler_pattern.rs` exists (228 lines, contains `#![cfg(not(target_arch = "wasm32"))]`, three `#[tokio::test]` functions, one `proptest!` block).
- ✅ Commit `1586da8d` exists in `git log` (Task 1).
- ✅ Commit `d722e5c4` exists in `git log` (Task 2).
- ✅ `cargo test --test in_process_handler_pattern --features full` → 4 passed, 0 failed.
- ✅ `make quality-gate` → exit 0 with `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`.
- ✅ All 15 verification steps in this summary exit 0 / produce the expected count.
- ✅ No files outside `tests/in_process_handler_pattern.rs` were modified by this plan (verified via `git show --name-only 1586da8d d722e5c4`).
- ✅ No SATD introduced (zero `TODO`/`FIXME`/`HACK`/`XXX` comments).
- ✅ No private-API access (zero `server.capabilities.*` accesses; zero `.handle_request(` USAGE; zero `use ... handle_request` imports).

---
*Phase: 82-builder-dx-prerequisites*
*Plan: 02*
*Completed: 2026-05-17*
