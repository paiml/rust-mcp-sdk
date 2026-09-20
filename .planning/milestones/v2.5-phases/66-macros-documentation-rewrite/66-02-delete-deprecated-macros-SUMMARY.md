---
phase: 66-macros-documentation-rewrite
plan: 02-delete-deprecated-macros
subsystem: macros
tags: [rust, proc-macro, deletion, cleanup, pmcp-macros, breaking-change]

# Dependency graph
requires:
  - phase: 66-macros-documentation-rewrite
    plan: 01-poc-include-str-gate
    provides: "Verified include_str! + breadcrumb comment; POC_README.md as single-source-of-truth for crate-level rustdoc"
provides:
  - "Trimmed pmcp-macros/src/lib.rs (202 lines, down from 374) containing only real mcp_* exports + Wave 0 wiring"
  - "Cleaned pmcp-macros/Cargo.toml features table (default + debug only; tool_router_dev feature removed)"
  - "Unblocks Wave 2 plans 66-03 (downstream markdown fixup) and 66-04 (README/rustdoc rewrite) — the source tree is now consistent with the new-API-only documentation they will write"
affects:
  - 66-03-downstream-markdown-fixup
  - 66-04-readme-and-rustdoc-rewrite
  - 66-05-changelog-version-bump
  - pmcp-macros v0.5.0 breaking minor bump (pre-1.0 semver-legal per D-20)

# Tech tracking
tech-stack:
  added: []  # pure deletion plan, no new dependencies
  patterns:
    - "Two-commit refactor with deliberate intermediate broken-build window (rm files, then fix references)"
    - "Emergency --no-verify override justified for staged multi-commit refactor per CLAUDE.md"

key-files:
  created: []
  modified:
    - "pmcp-macros/src/lib.rs"
    - "pmcp-macros/Cargo.toml"
  deleted:
    - "pmcp-macros/src/tool.rs (426 lines)"
    - "pmcp-macros/src/tool_router.rs (257 lines)"
    - "pmcp-macros/tests/tool_tests.rs (129 lines)"
    - "pmcp-macros/tests/tool_router_tests.rs (71 lines)"
    - "pmcp-macros/tests/ui/tool_missing_description.rs (10 lines)"
    - "pmcp-macros/tests/ui/tool_missing_description.stderr (5 lines)"

key-decisions:
  - "Used --no-verify on both task commits per plan-authorized override protocol: Task 1 deliberately leaves build broken (files removed but lib.rs still declares mod tool/tool_router), Task 2 restores. Parallel-executor safety override also in play (orchestrator validates hooks once after all wave agents complete)."
  - "Combined Region 4+5+6 of lib.rs edit (pub fn tool_router / pub fn prompt / pub fn resource) into a single contiguous Edit replacement rather than three separate edits — they were contiguous in the file and the replacement delta is smaller that way. All six plan-specified regions were in fact cleaned."
  - "Preserved syn::ItemImpl import in the use statement despite deleting pub fn tool_router — ItemImpl is still needed by pub fn mcp_server (the real router macro) at line 123."

patterns-established:
  - "Safe deletion of proc-macro source files requires two commits: (1) git rm the .rs files, (2) remove mod declarations and pub fn exports. The intermediate state is broken-by-design; document the window in plan <notes> so executors don't panic."
  - "Wave 0 preservation checks as a SUMMARY section — explicit verification that earlier-wave artifacts survived this wave's deletions."

requirements-completed: [MACR-01]

# Metrics
duration: ~4min
completed: 2026-04-11
---

# Phase 66 Plan 02: Delete Deprecated Macros Summary

**Surgically removed 1,070 lines of deprecated and stub proc-macro surface from `pmcp-macros` (683 lines of `#[tool]`/`#[tool_router]` source + 215 lines of their tests + 172 lines of `lib.rs` cleanup + the `tool_router_dev` Cargo feature) in two atomic commits, leaving only the four real `pub fn mcp_*` exports plus Wave 0 `include_str!` wiring intact.**

## Performance

- **Duration:** ~4 min (from branch-base reset to final commit)
- **Completed:** 2026-04-11
- **Tasks:** 2 (Task 1: file deletion, Task 2: lib.rs + Cargo.toml cleanup)
- **Files deleted:** 6 (898 lines removed via `git rm`)
- **Files modified:** 2 (lib.rs: 374→202 lines, Cargo.toml: -1 line)

## Accomplishments

- **Deleted 6 deprecated files via `git rm`** totaling 898 lines:
  - `pmcp-macros/src/tool.rs` — 426 lines (the `#[tool]` attribute macro impl + output-schema codegen)
  - `pmcp-macros/src/tool_router.rs` — 257 lines (the `#[tool_router]` impl-block router impl)
  - `pmcp-macros/tests/tool_tests.rs` — 129 lines (integration tests for `#[tool]`)
  - `pmcp-macros/tests/tool_router_tests.rs` — 71 lines (integration tests for `#[tool_router]`, gated behind the `tool_router_dev` feature)
  - `pmcp-macros/tests/ui/tool_missing_description.rs` — 10 lines (trybuild UI test for the `#[tool]` missing-description compile error — Pitfall 6 from 66-RESEARCH.md)
  - `pmcp-macros/tests/ui/tool_missing_description.stderr` — 5 lines (its paired `.stderr` snapshot)
- **Trimmed `pmcp-macros/src/lib.rs` from 374 to 202 lines (46% reduction)** with six surgical deletions:
  - Region 1: 53-line `//!` module-doc block advertising the now-deleted macros and showing an obsolete calculator example
  - Region 2: `mod tool;` and `mod tool_router;` declarations
  - Region 3: `pub fn tool` block (doc comment + `#[deprecated(since = "0.3.0", ...)]` + `#[proc_macro_attribute]` + function body)
  - Region 4: `pub fn tool_router` block (same structure)
  - Region 5: `pub fn prompt` stub (identity function with "deferred to future release" comment)
  - Region 6: `pub fn resource` stub (identity function with "deferred to future release" comment)
- **Cleaned `pmcp-macros/Cargo.toml`** — removed the `tool_router_dev = []  # Enable WIP tool_router tests` feature. `[features]` table is now just `default = []` and `debug = []`.
- **Preserved all Wave 0 artifacts** — the `#![doc = include_str!("../POC_README.md")]` attribute at line 6, the 23-line `ReadmeDoctests` incompatibility breadcrumb comment block (lines 19-41), and the `pmcp-macros/POC_README.md` file are untouched.
- **Preserved all real `mcp_*` surface** — `pub fn mcp_tool`, `pub fn mcp_server`, `pub fn mcp_prompt`, `pub fn mcp_resource` and their `///` doc comments are untouched (Plan 04 will rewrite those docs). The 5 real `mod mcp_*;` declarations and the `mod utils;` declaration also remain.
- **Build, unit-test, and doctest gates all pass.** `cargo build -p pmcp-macros` compiles in 5s. `cargo test -p pmcp-macros` runs 11 unit tests (mcp_tool_tests) + 2 trybuild UI tests (mcp_tool_missing_description, mcp_tool_multiple_args) + 1 compile_fail_tests — all pass, 0 failures. `cargo test -p pmcp-macros --doc` runs 13 doctests: 1 passed (the Wave 0 POC_README.md block at lib.rs line 24), 12 ignored, 0 failures.

## Task Commits

Each task was committed atomically with `--no-verify` per the plan's Emergency Override protocol (see "Deviations" below):

1. **Task 1: Delete 6 deprecated files via `git rm`** — `33adc963`
   - `refactor(66): delete 6 deprecated macro files (WIP — lib.rs cleanup follows in next commit)`
   - 898 deletions across 6 files
   - Deliberately leaves the workspace in a broken-build state (lib.rs still references `mod tool;` and `mod tool_router;` pointing at now-missing files). Restored by Task 2.

2. **Task 2: Clean up lib.rs + Cargo.toml** — `f240ec89`
   - `refactor(66): delete deprecated #[tool]/#[tool_router] and stub #[prompt]/#[resource]`
   - 172 deletions across 2 files (lib.rs: -172 lines; Cargo.toml: -1 line replaced with nothing)
   - Restores the build; all three cargo gates green.

## Cargo Gate Evidence

### `cargo build -p pmcp-macros`

```
   Compiling pmcp-macros v0.4.1 (/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/worktrees/agent-a1cfd17f/pmcp-macros)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.01s
```

Exit 0. No warnings on pmcp-macros itself.

### `cargo test -p pmcp-macros`

```
test test_custom_name ... ok
test test_annotations ... ok
test test_untyped_no_output_schema ... ok
test test_echo_tool_metadata ... ok
test test_no_arg_tool ... ok
test test_sync_tool ... ok
test test_tool_with_extra ... ok
test test_echo_tool_handle ... ok
test test_tool_with_state ... ok
...
test tests/ui/mcp_tool_missing_description.rs ... ok
test tests/ui/mcp_tool_multiple_args.rs ... ok

test compile_fail_tests ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s
```

All three test binaries (`mcp_tool_tests`, `mcp_prompt_tests`, `mcp_server_tests` plus the trybuild suite) pass.

### `cargo test -p pmcp-macros --doc`

```
running 13 tests
test pmcp-macros/src/lib.rs - mcp_prompt (line 142) ... ignored
test pmcp-macros/src/lib.rs - mcp_prompt (line 156) ... ignored
test pmcp-macros/src/lib.rs - mcp_resource (line 187) ... ignored
test pmcp-macros/src/lib.rs - mcp_server (line 108) ... ignored
test pmcp-macros/src/lib.rs - mcp_tool (line 60) ... ignored
test pmcp-macros/src/lib.rs - mcp_tool (line 71) ... ignored
test pmcp-macros/src/lib.rs - mcp_tool (line 83) ... ignored
test pmcp-macros/src/mcp_prompt.rs - mcp_prompt (line 16) ... ignored
test pmcp-macros/src/mcp_resource.rs - mcp_resource (line 17) ... ignored
test pmcp-macros/src/mcp_resource.rs - mcp_resource (line 25) ... ignored
test pmcp-macros/src/mcp_server.rs - mcp_server (line 13) ... ignored
test pmcp-macros/src/mcp_tool.rs - mcp_tool (line 16) ... ignored
test pmcp-macros/src/lib.rs - (line 24) - compile ... ok

test result: ok. 1 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

**Wave 0 POC_README.md doctest continues to pass at `lib.rs - (line 24)`.** This is the critical cross-wave invariant — the include_str! wiring survived the Task 2 cleanup. (The 12 ignored tests are `rust,ignore` blocks in the per-macro `///` doc comments; Plan 04 will flip those to `rust,no_run`.)

## Files Created/Modified

### Deleted (6)

| File | Lines | Purpose |
|---|---|---|
| `pmcp-macros/src/tool.rs` | 426 | `#[tool]` attribute macro — the OG attribute form, superseded by `#[mcp_tool]` (which has mandatory description, State<T> injection, async auto-detection, MCP annotations) |
| `pmcp-macros/src/tool_router.rs` | 257 | `#[tool_router]` router macro for impl blocks, superseded by `#[mcp_server]` |
| `pmcp-macros/tests/tool_tests.rs` | 129 | Integration tests for `#[tool]` |
| `pmcp-macros/tests/tool_router_tests.rs` | 71 | Integration tests for `#[tool_router]`, gated behind the deleted `tool_router_dev` Cargo feature |
| `pmcp-macros/tests/ui/tool_missing_description.rs` | 10 | Trybuild UI test: `#[tool]` without description should fail to compile |
| `pmcp-macros/tests/ui/tool_missing_description.stderr` | 5 | Expected error snapshot for the above |

**Total deleted:** 898 lines across 6 files.

### Modified (2)

| File | Delta | Change |
|---|---|---|
| `pmcp-macros/src/lib.rs` | 374 → 202 lines (-172) | Removed `//!` block, `mod tool;`/`mod tool_router;` declarations, `pub fn tool`, `pub fn tool_router`, `pub fn prompt` stub, `pub fn resource` stub |
| `pmcp-macros/Cargo.toml` | -1 line | Removed `tool_router_dev = []  # Enable WIP tool_router tests` from `[features]` |

## Decisions Made

- **Two-commit refactor with `--no-verify` override.** Task 1 deliberately leaves the workspace in a broken state (files removed from disk, `lib.rs` still declares `mod tool;` and `mod tool_router;`). The pre-commit hook would reject Task 1 if run normally. Per the plan's explicit protocol and CLAUDE.md's Emergency Override clause for staged multi-commit refactors where the next commit immediately restores quality, both commits used `git commit --no-verify`. Task 2's commit passes all three cargo gates at the tip, so quality is restored before any other work can run. (Parallel-executor safety mode also authorizes `--no-verify` for this wave; the orchestrator validates hooks once after all wave agents complete.)
- **Combined Region 4/5/6 into a single Edit operation.** The plan listed three separate lib.rs regions (delete `pub fn tool_router`, delete `pub fn prompt` stub, delete `pub fn resource` stub) but they were contiguous in the file. A single Edit replacing the combined 74-line block with `""` is smaller, less error-prone, and produced the same diff as three sequential edits would have. All six plan-specified regions were in fact cleaned; this is a cosmetic executor optimization, not a scope deviation.
- **Preserved `syn::ItemImpl` import.** Deleting `pub fn tool_router` eliminated one consumer of `syn::ItemImpl`, but `pub fn mcp_server` at line 123 still parses its input as `ItemImpl` (it's the real impl-block router macro). The `use syn::{parse_macro_input, ItemFn, ItemImpl};` import remains necessary — no dead-import cleanup needed.
- **Did NOT touch `mcp_server.rs` implementation.** Plan 02 is pure deletion; the real `#[mcp_server]` router macro is out of scope and untouched.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Worktree base commit was wrong**
- **Found during:** Initial worktree branch check (before Task 1)
- **Issue:** The worktree branch's HEAD was `87eea42b` (main with two unrelated fix commits on top) instead of the expected base `fbb208009c0b63ef3cc9b5d72fb9da4aca9c234a`. Without this reset, the `.planning/phases/66-macros-documentation-rewrite/` directory would not have existed at the worktree's working tree (it's created in a commit later than `87eea42b`), and the Wave 0 POC wiring in `pmcp-macros/src/lib.rs` would not have been present.
- **Fix:** `git reset --hard fbb208009c0b63ef3cc9b5d72fb9da4aca9c234a` (per the explicit `<worktree_branch_check>` protocol in the prompt).
- **Files modified:** N/A (branch state only)
- **Verification:** `git log --oneline -5` showed HEAD at `fbb20800 docs(66): commit phase 66 PLAN files for worktree access` with `a9962168 docs(66-01): complete POC include_str gate plan` and `3b9b6630 feat(66-01): POC gate — wire include_str! Markdown into pmcp-macros doctests` directly underneath. The `.planning/phases/66-macros-documentation-rewrite/` directory was then present.
- **Committed in:** N/A (pre-commit branch state fix — identical to Plan 01's same deviation)

---

**Total deviations:** 1 auto-fixed (blocking branch state error — same root cause as Plan 01, and noted in Wave 0 preservation block of this prompt as a known worktree-setup issue)
**Impact on plan:** The fix was necessary to find the plan files and Wave 0 artifacts. Plan executed exactly as written after the reset.

## Wave 0 Preservation Verification

Per the critical Wave 0 preservation requirements in the prompt, all three artifacts survived this plan's deletions:

- **`#![doc = include_str!("../POC_README.md")]`** at `pmcp-macros/src/lib.rs:6` — **PRESERVED**
- **`ReadmeDoctests` breadcrumb comment block** at `pmcp-macros/src/lib.rs:19-41` (23 lines) — **PRESERVED**. This documents why `#[cfg(doctest)] pub struct ReadmeDoctests;` does NOT compile in proc-macro crates and prevents regression.
- **`pmcp-macros/POC_README.md`** file — **PRESERVED** (untouched by this plan; Wave 2 Plan 04 will replace it with the real README.md rewrite or redirect the `include_str!` path).

Doctest evidence: `cargo test -p pmcp-macros --doc` still reports `test pmcp-macros/src/lib.rs - (line 24) - compile ... ok` — the same line the Wave 0 SUMMARY documents as the POC-gate-passing block. Line 24 refers to the content line inside `POC_README.md`, not `lib.rs:24`; this is a known rustc edition-2021 span quirk (edition 2024 fixes it) noted in Plan 01's SUMMARY.

## Issues Encountered

- **PreToolUse:Edit hook repeatedly requested re-read of lib.rs between edits.** The harness's read-before-edit guard fired six times during Task 2, once after each successful Edit operation on `pmcp-macros/src/lib.rs` (which I had read once at the start). Each re-read confirmed the previous edit had landed and the next edit's anchor string was still unique, so no work was lost — but it added some churn to the task. Not a bug; a standard harness invariant.
- **No other unexpected references to deleted symbols.** A workspace-wide grep for `pmcp_macros::tool` and `pmcp_macros::tool_router` found only one hit, in `.planning/phases/58-mcp-tool-proc-macro/58-RESEARCH.md:605` — a historical research document from Phase 58 that predates phase 66. No code references; no additional cleanup needed. The other downstream consumers (pmcp-course chapters, `docs/advanced/migration-from-typescript.md`) are cleaned up by Plan 03.

## User Setup Required

None — pure Rust source + Cargo.toml deletion. No environment variables, no external services, no manual dashboard configuration.

## Next Phase Readiness

- **Plan 66-03 (Wave 2 — downstream markdown fixup) is unblocked.** pmcp-macros no longer exports `#[tool]` / `#[tool_router]` / stub `#[prompt]` / stub `#[resource]`, so the course chapters and migration doc that still reference them can now be updated against a clean source tree. The course changes won't create dangling cross-links because no macro surface disappears for the mcp_* replacements.
- **Plan 66-04 (Wave 2 — README / rustdoc rewrite) is unblocked.** The `//!` module-doc block is gone, so plan 04 can wire the real `README.md` via `include_str!` (replacing the POC path) without conflicting with any pre-existing module docs. The four real `pub fn mcp_*` exports still carry their old `///` docs — plan 04 rewrites those to reference the Phase 65 `s23_mcp_tool_macro` / `s24_mcp_prompt_macro` examples and flip `rust,ignore` → `rust,no_run`.
- **Plan 66-05 (changelog / version bump) has all the raw material** — this plan provides the concrete deletion inventory (6 files, 898 lines) and the semver-legal pre-1.0 breaking-minor trigger (deleted public proc-macro exports). `pmcp-macros` is ready for v0.4.1 → v0.5.0.

## Known Stubs

None. Every `pub fn` in `pmcp-macros/src/lib.rs` is now a real, working proc-macro export with non-trivial codegen. No placeholder identity functions remain.

## Final lib.rs Line Count

- **Before Plan 02:** 374 lines (Wave 0 POC wiring added 5 lines on top of pre-existing 369)
- **After Plan 02:** 202 lines
- **Delta:** −172 lines (−46%)
- **Plan target:** "roughly 200-220 lines after trimming" — **within target (202 is at the low end, which is the desired outcome).**

## Self-Check: PASSED

- Commit `33adc963` (Task 1) exists in git log: FOUND
- Commit `f240ec89` (Task 2) exists in git log: FOUND
- `pmcp-macros/src/tool.rs`: MISSING (correctly deleted)
- `pmcp-macros/src/tool_router.rs`: MISSING (correctly deleted)
- `pmcp-macros/tests/tool_tests.rs`: MISSING (correctly deleted)
- `pmcp-macros/tests/tool_router_tests.rs`: MISSING (correctly deleted)
- `pmcp-macros/tests/ui/tool_missing_description.rs`: MISSING (correctly deleted)
- `pmcp-macros/tests/ui/tool_missing_description.stderr`: MISSING (correctly deleted)
- `pmcp-macros/src/mcp_tool.rs`: FOUND (real macro, preserved)
- `pmcp-macros/src/mcp_server.rs`: FOUND
- `pmcp-macros/src/mcp_prompt.rs`: FOUND
- `pmcp-macros/src/mcp_resource.rs`: FOUND
- `pmcp-macros/tests/mcp_tool_tests.rs`: FOUND
- `pmcp-macros/tests/ui/mcp_tool_missing_description.rs`: FOUND
- `pmcp-macros/POC_README.md` (Wave 0 artifact): FOUND
- `pmcp-macros/src/lib.rs` contains `#![doc = include_str!("../POC_README.md")]`: VERIFIED (Wave 0 preserved)
- `pmcp-macros/src/lib.rs` contains `ReadmeDoctests` breadcrumb comment: VERIFIED (Wave 0 preserved)
- `pmcp-macros/src/lib.rs` has zero `//! ` lines: VERIFIED
- `pmcp-macros/src/lib.rs` has zero `#[deprecated` attributes: VERIFIED
- `pmcp-macros/src/lib.rs` has zero `pub fn tool(`, `pub fn tool_router(`, `pub fn prompt(`, `pub fn resource(`: VERIFIED
- `pmcp-macros/src/lib.rs` retains all four `pub fn mcp_*(` exports: VERIFIED
- `pmcp-macros/Cargo.toml` has zero `tool_router_dev` references: VERIFIED
- `pmcp-macros/Cargo.toml` retains `default = []` and `debug = []`: VERIFIED
- `cargo build -p pmcp-macros`: exit 0, compiled in 5.01s
- `cargo test -p pmcp-macros`: 11 passed, 0 failed across unit + trybuild + compile_fail
- `cargo test -p pmcp-macros --doc`: 1 passed, 12 ignored, 0 failed (Wave 0 POC doctest still green)

---
*Phase: 66-macros-documentation-rewrite*
*Plan: 02-delete-deprecated-macros*
*Completed: 2026-04-11*
