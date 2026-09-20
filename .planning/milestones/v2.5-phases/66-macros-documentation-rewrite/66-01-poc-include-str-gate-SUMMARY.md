---
phase: 66-macros-documentation-rewrite
plan: 01-poc-include-str-gate
subsystem: docs
tags: [rust, proc-macro, rustdoc, include_str, doctest, pmcp-macros]

# Dependency graph
requires:
  - phase: 65-examples-cleanup-protocol-accuracy
    provides: "stable s23_mcp_tool_macro example paths, `pmcp = { path = '..', features = ['full'] }` dev-dep on pmcp-macros, workspace-wide `pmcp::mcp_tool` re-export under the `macros` feature"
provides:
  - "Verified include_str! + `cargo test --doc -p pmcp-macros` doctest cycle"
  - "Verified `use pmcp::mcp_tool;` cross-dep import works inside a same-crate proc-macro doctest"
  - "Pitfall 6 discovered and documented: `#[cfg(doctest)] pub struct ReadmeDoctests;` cannot compile in a `proc-macro = true` crate"
  - "Working `pmcp-macros/POC_README.md` as a reference minimum viable doctest shape"
  - "Breadcrumb comment in `pmcp-macros/src/lib.rs` warning future contributors not to re-introduce the `ReadmeDoctests` struct"
affects:
  - 66-02+
  - all-wave-1-macros-readme-rewrite-plans
  - future-phases-attempting-include_str-on-other-proc-macro-crates

# Tech tracking
tech-stack:
  added: []  # no new crates, no new dependencies
  patterns:
    - "Crate-level `#![doc = include_str!(\"../FILE.md\")]` for proc-macro crates (without companion struct)"
    - "`rust,no_run` + `use pmcp::mcp_tool;` as the canonical doctest form for pmcp-macros"
    - "Minimal zero-arg `#[mcp_tool(description = \"...\")] async fn name() -> pmcp::Result<serde_json::Value>` form"

key-files:
  created:
    - "pmcp-macros/POC_README.md"
  modified:
    - "pmcp-macros/src/lib.rs"

key-decisions:
  - "Use a dedicated POC_README.md rather than wiring the real README.md — the existing README contains stale `rust` code blocks that would immediately break cargo test --doc if picked up by include_str!"
  - "Skip the `#[cfg(doctest)] pub struct ReadmeDoctests;` belt-and-suspenders pattern — rustc rejects it in proc-macro crates"
  - "Preserve existing `//!` block, all `pub fn` exports, and the current README.md entirely untouched — the POC is scoped strictly to prove the mechanism, not to start the rewrite"

patterns-established:
  - "Crate-level include_str doc pattern for proc-macro crates: ONLY `#![doc = include_str!(\"../FILE.md\")]` — no struct"
  - "Minimum-viable doctest shape for pmcp-macros: zero-arg tool, untyped Value return, uses `pmcp::mcp_tool` re-export"

requirements-completed: []  # POC gate plan — no requirements satisfied yet; it de-risks MACR-01 and MACR-03

# Metrics
duration: ~12min
completed: 2026-04-11
---

# Phase 66 Plan 01: POC include_str! Gate Summary

**Proved `#![doc = include_str!(...)]` + `use pmcp::mcp_tool;` doctest cycle works in pmcp-macros, discovered that the rustdoc-book `ReadmeDoctests` struct pattern is incompatible with proc-macro crates, and unblocked all Wave 1+ README-rewrite plans.**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-04-11T20:36:00Z (approx, branch base fix + context load)
- **Completed:** 2026-04-11T20:48:11Z
- **Tasks:** 1 (atomic POC gate)
- **Files modified:** 2 (`pmcp-macros/POC_README.md` new, `pmcp-macros/src/lib.rs` modified)

## Accomplishments

- **POC gate passed.** `cargo test --doc -p pmcp-macros` moved from baseline `0 passed; 0 failed; 19 ignored` to `1 passed; 0 failed; 19 ignored` — the passing test is the `rust,no_run` block from `POC_README.md` attached to the crate root via `#![doc = include_str!(...)]`.
- **Cross-crate proc-macro doctest verified.** The passing block uses `use pmcp::mcp_tool;` (pulled via the `pmcp` dev-dependency under `features = ["full"]`), proving that `pmcp-macros` doctests can invoke the crate's own proc-macros transitively without cyclic-dependency errors.
- **Pitfall discovered and documented (not in 66-RESEARCH.md).** The rustdoc book's canonical `#[cfg(doctest)] pub struct ReadmeDoctests;` pattern does NOT compile in `proc-macro = true` crates — rustc 1.94 rejects it with `error: proc-macro crate types currently cannot export any items other than functions tagged with #[proc_macro]...`. A breadcrumb comment was left in `lib.rs` so Wave 1+ executors and any future contributor do not re-introduce it.
- **`cargo doc -p pmcp-macros --no-deps` builds cleanly** — the POC content renders as crate-level documentation on the rustdoc landing page, confirming docs.rs would surface it after publish.
- **`cargo clippy -p pmcp-macros` is clean** — zero warnings from the POC changes.
- **Zero touch to deletion targets.** The existing `//!` block, all `pub fn tool` / `pub fn mcp_tool` / etc. exports, and the real `pmcp-macros/README.md` are entirely untouched. Wave 1+ deletion plans start from an identical baseline.

## Task Commits

Each task was committed atomically:

1. **Task 1: POC gate — wire include_str Markdown into pmcp-macros doctests** — `3b9b6630` (feat)
   - Added `pmcp-macros/POC_README.md` (new, ~40 lines, single `rust,no_run` block + explanation)
   - Added `#![doc = include_str!("../POC_README.md")]` at the top of `pmcp-macros/src/lib.rs`
   - Added a detailed breadcrumb comment about the `ReadmeDoctests` struct incompatibility (discovered during this task)

**Plan metadata commit:** pending — this SUMMARY.md commit follows.

## Files Created/Modified

- `pmcp-macros/POC_README.md` (created) — Dedicated POC Markdown file containing the single `rust,no_run` code block that exercises `use pmcp::mcp_tool;`. Intentionally not user-facing documentation; will be deleted or replaced when the real README rewrite lands.
- `pmcp-macros/src/lib.rs` (modified) — Added crate-level `#![doc = include_str!("../POC_README.md")]` attribute at the top (before the existing `//!` block), plus a breadcrumb comment block after the `mod` declarations documenting why the `ReadmeDoctests` struct pattern is forbidden here.

## Decisions Made

- **Use POC_README.md, not README.md, for the gate.** The existing `pmcp-macros/README.md` contains multiple bare ```rust code blocks (no `ignore` tag) that reference the soon-to-be-deleted `#[tool]`/`#[tool_router]` macros and a stale `pmcp = "1.1"` version pin. Wiring the real README via `include_str!` at this stage would immediately fail `cargo test --doc` before the Wave 1+ rewrite can land. A dedicated POC file sidesteps this and scopes the gate to mechanism-only.
- **Do not add `#[cfg(doctest)] pub struct ReadmeDoctests;`.** The rustdoc book recommends this pattern but rustc forbids it in proc-macro crates. The crate-level `#![doc = include_str!(...)]` on its own is sufficient on modern rustc (1.54+), which matches what `tracing-attributes` and `rmcp-macros` actually do (contrary to the research doc's line 101 which suggested both should be used together).
- **Leave all deletion targets untouched.** D-01, D-02, D-10 from 66-CONTEXT.md call for deleting 683+ lines of deprecated macros and rewriting `lib.rs` lines 1–53. None of that happens in 66-01. The POC is a pure-addition plan.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed `#[cfg(doctest)] pub struct ReadmeDoctests;` after rustc rejected it**
- **Found during:** Task 1 (POC gate wiring)
- **Issue:** The first iteration of `pmcp-macros/src/lib.rs` included the rustdoc-book's recommended belt-and-suspenders pattern:
  ```rust
  #[cfg(doctest)]
  #[doc = include_str!("../POC_README.md")]
  #[allow(dead_code)]
  pub struct ReadmeDoctests;
  ```
  `cargo test --doc -p pmcp-macros` failed with:
  ```
  error: `proc-macro` crate types currently cannot export any items
  other than functions tagged with `#[proc_macro]`,
  `#[proc_macro_derive]`, or `#[proc_macro_attribute]`
    --> pmcp-macros/src/lib.rs:88:1
  ```
  This is a Rust compiler hard constraint on `proc-macro = true` crates, not a bug in the pattern. The rustdoc book does not call this out.
- **Fix:** Replaced the struct with a 20-line breadcrumb comment explaining the incompatibility, why the struct is forbidden, and that the crate-level `#![doc = include_str!(...)]` attribute is the correct idiom for proc-macro crates on rustc 1.54+.
- **Files modified:** `pmcp-macros/src/lib.rs`
- **Verification:** After removing the struct, `cargo test --doc -p pmcp-macros` reported `1 passed; 0 failed; 19 ignored` — the POC gate passed.
- **Committed in:** `3b9b6630` (part of the atomic Task 1 commit — the struct addition and removal happened before any commit was created)

**2. [Rule 3 - Blocking] Worktree branch base was wrong**
- **Found during:** Initial worktree branch check (before Task 1)
- **Issue:** The worktree branch's merge-base was `47ee632b` (main, with two unrelated fix commits `edc16b17` and `87eea42b` on top) instead of the expected `cf21eb63`. The `.planning/phases/66-macros-documentation-rewrite/` directory was missing entirely from the working tree because it was created in a commit later than the actual worktree base.
- **Fix:** `git reset --hard cf21eb636c9f70d9082fe0831cdddb3b519d1af4` to discard the unrelated commits and restore the working tree to match the phase-66 base.
- **Files modified:** N/A (branch state only)
- **Verification:** `git log --oneline -3` shows `cf21eb63` as HEAD; `ls .planning/phases/66-macros-documentation-rewrite/` now shows `66-CONTEXT.md`, `66-RESEARCH.md`, `66-REVIEWS.md`, `66-VALIDATION.md`.
- **Committed in:** N/A (pre-commit branch state fix)

---

**Total deviations:** 2 auto-fixed (1 bug discovered by compiler, 1 blocking branch base error)
**Impact on plan:** Both auto-fixes were necessary to get to a passing POC gate. The ReadmeDoctests discovery is genuinely new information that the research missed and directly de-risks all Wave 1+ plans — without this gate, Wave 1+ executors would have copied the rustdoc-book pattern and hit the same error. No scope creep; the plan still executed as a single atomic POC task.

## Issues Encountered

- **No `66-01-poc-include-str-gate-PLAN.md` file existed.** The orchestrator spawned this agent to execute the plan but no PLAN.md was present in the phase directory — only 66-CONTEXT.md, 66-RESEARCH.md, 66-REVIEWS.md, and 66-VALIDATION.md. The task list was synthesized from the Wave 0 requirements in `66-VALIDATION.md` ("Proof-of-concept: verify `#![doc = include_str!("../README.md")]` + same-crate proc-macro doctest imports compile") plus the mechanics documented in `66-RESEARCH.md` (Pattern 1, Pattern 2). Scope was kept intentionally minimal: prove the mechanism, commit atomically, document the pitfall.

## User Setup Required

None — pure Rust-doc-only changes. No environment variables, no external services, no manual dashboard configuration.

## Next Phase Readiness

- **Wave 1+ macros-documentation-rewrite plans are unblocked.** Specifically:
  - Any plan that wires `#![doc = include_str!("../README.md")]` knows the mechanism works and knows NOT to add a `ReadmeDoctests` struct.
  - Any plan that writes new README code blocks knows to use `rust,no_run` and `use pmcp::mcp_tool;` (not `rust,ignore` and not `use pmcp_macros::mcp_tool;`).
  - Any plan that eventually deletes `POC_README.md` and points `#![doc]` at the real `README.md` has a clear cutover path: rewrite the README first (all code blocks `rust,no_run`-tagged), then flip the `include_str!` path, then delete `POC_README.md`. Or do it atomically.
- **No blockers for Wave 1+.** Baseline is clean: clippy zero-warnings, doctest cycle green, cargo doc builds.
- **One open question for the next plan (informational, not a blocker):** The research's Pitfall 5 notes that under edition 2021, rustdoc diagnostic spans for include_str'd doctests point at the `lib.rs` include site rather than the README line. Observed in the POC run — the passing test is reported as `pmcp-macros/src/lib.rs - (line 24) - compile ... ok`, where "line 24" refers to the POC_README.md content line, not the lib.rs line. If a future rewrite breaks a block, error messages will land in lib.rs, and the executor will need to grep the README for the failing symbol. This is a known rustc quirk (edition 2024 fixes it) and is acceptable for this phase.

## POC Gate Evidence

```
$ cargo test --doc -p pmcp-macros 2>&1 | grep -E "test result|line 24"
test pmcp-macros/src/lib.rs - (line 24) - compile ... ok
test result: ok. 1 passed; 0 failed; 19 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

Baseline (before this plan):
```
test result: ok. 0 passed; 0 failed; 19 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Delta: **+1 passing doctest** — the `rust,no_run` block from `POC_README.md` at line 24 compiles successfully under `cargo test --doc -p pmcp-macros`. **Gate: PASSED.**

## Self-Check: PASSED

- `pmcp-macros/POC_README.md` exists: FOUND
- `pmcp-macros/src/lib.rs` modified with `#![doc = include_str!("../POC_README.md")]`: FOUND
- `pmcp-macros/src/lib.rs` does NOT contain `pub struct ReadmeDoctests`: VERIFIED (replaced with breadcrumb comment)
- Commit `3b9b6630` exists in `git log`: FOUND
- `cargo test --doc -p pmcp-macros`: 1 passed, 0 failed, 19 ignored (baseline: 0 passed, 0 failed, 19 ignored)
- `cargo clippy -p pmcp-macros`: zero warnings
- `cargo doc -p pmcp-macros --no-deps`: builds cleanly

---
*Phase: 66-macros-documentation-rewrite*
*Plan: 01-poc-include-str-gate*
*Completed: 2026-04-11*
