---
phase: 66-macros-documentation-rewrite
plan: 04-readme-and-rustdoc-rewrite
subsystem: docs
tags: [rust, proc-macro, rustdoc, include_str, doctest, pmcp-macros, mcp_tool, mcp_server, mcp_prompt, mcp_resource]

# Dependency graph
requires:
  - phase: 66-macros-documentation-rewrite
    plan: 01-poc-include-str-gate
    provides: "Verified include_str! + cargo test --doc cycle in pmcp-macros; POC_README.md as placeholder single-source-of-truth for crate-level rustdoc; ReadmeDoctests breadcrumb warning"
  - phase: 66-macros-documentation-rewrite
    plan: 02-delete-deprecated-macros
    provides: "Clean pmcp-macros source tree (202-line lib.rs, only real #[mcp_*] exports + Wave 0 wiring); no deprecated #[tool]/#[tool_router]/#[prompt]/#[resource] surface for the new README to document against"
provides:
  - "Authoritative 355-line pmcp-macros/README.md documenting all four #[mcp_*] macros with 5 compiling rust,no_run blocks"
  - "pmcp-macros/src/lib.rs crate-level include_str!(..) now points at ../README.md (not POC_README.md)"
  - "Rewritten per-macro /// doc comments on pub fn mcp_tool/mcp_server/mcp_prompt/mcp_resource — all 4 now use rust,no_run, reference the s23/s24 renamed examples, and compile under cargo test --doc"
  - "Empirical verification that `use pmcp_macros::mcp_resource;` compiles from inside pmcp-macros README/rustdoc doctests — Pitfall 4 option 1 fallback path validated in-situ (no Wave 0 Block B needed)"
  - "examples/s23_mcp_tool_macro.rs:14 and examples/s24_mcp_prompt_macro.rs:13 stale `63_`/`64_` cargo-run headers fixed (research addition #2)"
  - "POC_README.md deleted — no placeholder content left in the crate"
affects:
  - 66-05-changelog-version-bump
  - post-phase-66-mcp_resource-re-export-fix
  - docs.rs/pmcp-macros rendering (cutover goes live on next publish)

# Tech tracking
tech-stack:
  added: []  # pure docs rewrite, no new crates or dependencies
  patterns:
    - "Crate-level include_str!(../README.md) with rust,no_run doctests as the single source of truth for a proc-macro crate"
    - "Per-macro /// doc blocks use `use pmcp::{mcp_tool, mcp_server, mcp_prompt};` (re-export path) but fall back to `use pmcp_macros::mcp_resource;` with an inline note for the re-export-gap macro"
    - "README Note: paragraph above the mcp_resource section explaining the asymmetric import as a temporary gap, not a design choice"
    - "URI template variables sub-section as a prose-level feature callout with both `URI template` and `{variable_name}` literal tokens for grep-ability"

key-files:
  created: []
  modified:
    - "pmcp-macros/README.md"
    - "pmcp-macros/src/lib.rs"
    - "examples/s23_mcp_tool_macro.rs"
    - "examples/s24_mcp_prompt_macro.rs"
  deleted:
    - "pmcp-macros/POC_README.md"

key-decisions:
  - "Committed the README rewrite, include_str! cutover, and POC_README deletion as a single atomic commit rather than staging. The plan allowed either shape; a single commit keeps the crate in a consistent state at every hash and makes the cutover trivially revertible."
  - "Did not need the Pitfall 4 option 3 fallback for the #[mcp_resource] section. Wave 0 only validated `use pmcp::mcp_tool;` via POC_README.md; validation of `use pmcp_macros::mcp_resource;` happened live during Task 1 by running cargo test --doc against the new README. The `rust,no_run` block compiled on the first attempt — Pitfall 4 option 1 is verified working."
  - "Kept the ReadmeDoctests breadcrumb comment block in lib.rs but refreshed its body to reference README.md instead of POC_README.md, since POC_README.md no longer exists. The breadcrumb itself still serves its primary purpose: preventing future contributors from re-introducing `#[cfg(doctest)] pub struct ReadmeDoctests;` in a proc-macro crate."
  - "mcp_resource per-macro /// doc example imports from pmcp_macros directly (with an inline comment) rather than relying on a reverse-dep trick. This matches the README's approach and keeps the two doc surfaces symmetric."
  - "Used `use pmcp::mcp_prompt;` (re-export path) in the mcp_prompt /// doctest despite s24_mcp_prompt_macro.rs itself importing from `pmcp::{mcp_prompt, mcp_server, ...}` via pmcp::, not pmcp_macros::. Verified both routes work; the pmcp::-prefixed form is consistent with the README installation section which tells users to depend on pmcp (not pmcp-macros)."
  - "README line count landed at 355 instead of the plan's target of 200-300. Justified by (a) two code blocks in the #[mcp_tool] section (basic + State<T>) per the plan's `### Shared state (State<T>)` sub-section requirement, (b) the explicit `### URI template variables` prose section fold-in, (c) the `### Example (attributes)` blocks for each macro which each add ~5 lines for the attribute list. All sections fit the plan's required structure; the overflow is concise prose, not bloat."

patterns-established:
  - "Doctest-as-drift-detector for proc-macro crates: every public API surface gets a rust,no_run block in either the README or the per-macro /// doc; API drift breaks cargo test --doc automatically. 9 passing doctests now vs 1 baseline."
  - "Per-macro /// doc convention: one-line summary → attributes list → single rust,no_run example (not a full main()) → link to full runnable example in examples/s23 or s24. Keeps docs.rs individual macro pages scannable while the crate-level README carries the long form."
  - "Transparent gap documentation: when a known asymmetry exists (mcp_resource re-export gap), mention it in a Note: blockquote above the section rather than hiding it. Readers get working code and an explanation in the same place."

requirements-completed: [MACR-01, MACR-03]

# Metrics
duration: ~15min
completed: 2026-04-11
---

# Phase 66 Plan 04: README & Rustdoc Rewrite Summary

**Replaced the 252-line pmcp = "1.1" stale README with a 355-line authoritative document covering all four #[mcp_*] macros, flipped include_str! from POC_README.md to README.md, deleted the POC gate file, rewrote all four per-macro /// doc blocks to use rust,no_run fences referencing the s23/s24 renamed examples, and fixed two stale `63_`/`64_` cargo-run headers in the example files themselves — 9 passing doctests now vs 1 baseline, zero clippy warnings, all builds green.**

## Performance

- **Duration:** ~15 min (branch reset → final commit)
- **Started:** 2026-04-11T21:05:00Z (approx, branch base fix + context load)
- **Completed:** 2026-04-11T21:20:00Z
- **Tasks:** 2 (Task 1: README rewrite + cutover, Task 2: per-macro /// docs + example headers)
- **Files modified:** 4 (README.md, lib.rs, s23_mcp_tool_macro.rs, s24_mcp_prompt_macro.rs)
- **Files deleted:** 1 (POC_README.md)

## Accomplishments

- **Rewrote `pmcp-macros/README.md` from scratch** as a 355-line document that follows the exact section structure specified in the plan (one-paragraph intro → Installation → Overview table → `#[mcp_tool]` showcase → `#[mcp_server]` → `#[mcp_prompt]` → `#[mcp_resource]` with URI template variables sub-section → Feature flags → License). Every section matches the plan's skeleton headings verbatim, every example is `rust,no_run`-tagged, every GitHub link is an absolute URL to `https://github.com/paiml/pmcp/blob/main/examples/...`, and the installation section pins `pmcp = { version = "2.3", features = ["macros"] }` (not `pmcp-macros` as a direct dep).
- **Flipped the include_str! cutover** — `pmcp-macros/src/lib.rs` now has `#![doc = include_str!("../README.md")]` instead of `#![doc = include_str!("../POC_README.md")]`. The comment block above the attribute was updated to explain the include_str! → doctest chain in user-facing terms rather than its old "POC gate" framing.
- **Deleted `pmcp-macros/POC_README.md`** — the Wave 0 placeholder file is gone. No content duplication, no orphaned reference, git log preserves the full cutover history via Task 1's atomic commit (POC creation → real README rewrite → cutover in two commits across 66-01 and 66-04).
- **Rewrote all four per-macro `///` doc blocks** in `pmcp-macros/src/lib.rs` to use `rust,no_run` fences instead of `rust,ignore`. Each block now follows the convention: one-line summary → attributes list → a single concise `rust,no_run` block (not a full `main()`, leaving the full demo to s23/s24) → a "See examples/s23_mcp_tool_macro.rs for a complete runnable demo" footer line. The `mcp_resource` block uses `use pmcp_macros::mcp_resource;` with an inline `// Note: direct import until the re-export gap is closed` comment, consistent with the README.
- **Fixed stale example headers** — `examples/s23_mcp_tool_macro.rs:14` now says `cargo run --example s23_mcp_tool_macro --features full` (was `63_mcp_tool_macro`), and `examples/s24_mcp_prompt_macro.rs:13` now says `cargo run --example s24_mcp_prompt_macro --features full` (was `64_mcp_prompt_macro`). Research addition #2 fully addressed — zero `63_`/`64_` occurrences remain in either example file.
- **Preserved the ReadmeDoctests breadcrumb comment** in lib.rs but refreshed its body to reference `README.md` instead of `POC_README.md` (since the POC file no longer exists). The warning about `#[cfg(doctest)] pub struct ReadmeDoctests;` in proc-macro crates is intact — future contributors will still be prevented from re-introducing the forbidden pattern.
- **9 passing doctests** in `cargo test -p pmcp-macros --doc` (5 from the new README + 4 from the per-macro `///` blocks) — up from 1 passing in the Wave 0 baseline and 1 passing after 66-02. The 5 remaining "ignored" doctests are inside `pmcp-macros/src/mcp_*.rs` implementation files; those are out of Plan 04's scope (they're inner module docs, not user-facing API docs).
- **cargo build -p pmcp-macros, cargo clippy -p pmcp-macros, cargo build --example s23_mcp_tool_macro --features full, cargo build --example s24_mcp_prompt_macro --features full** all pass cleanly — zero warnings, zero errors.

## Task Commits

Each task was committed atomically with `--no-verify` per the parallel-executor protocol:

1. **Task 1: Rewrite README, flip include_str! cutover, delete POC_README** — `a9877876` (docs)
   - `docs(66): rewrite pmcp-macros/README.md for current macro API (MACR-01, MACR-03)`
   - 3 files changed, +288 / -220, 1 file deleted
   - Combines the README rewrite, lib.rs include_str! cutover, and POC_README.md deletion into a single atomic commit. The plan allowed either one atomic commit or staged cutover; atomic was chosen because it keeps the crate in a consistent state at every hash and is trivially revertible.

2. **Task 2: Per-macro /// doc rewrites + stale example header fixes** — `31c3fbf6` (docs)
   - `docs(66): rewrite per-macro /// docs + fix stale example headers (D-11)`
   - 3 files changed, +110 / -86
   - Covers all four `pub fn mcp_*` /// doc blocks plus the two example-file `//!` header fixes (research addition #2).

**Plan metadata commit:** pending — this SUMMARY.md commit follows.

## Files Created/Modified

### Modified (4)

| File | Delta | Change |
|---|---|---|
| `pmcp-macros/README.md` | 252 → 355 lines (+103) | Complete rewrite: pmcp 2.3 install pin, all 4 macros documented with rust,no_run examples, URI template variables sub-section, absolute GitHub URLs, zero migration/deprecated language, zero bare `rust` fences |
| `pmcp-macros/src/lib.rs` | 202 → 226 lines (+24) | include_str! path flipped POC_README.md → README.md; breadcrumb comment body refreshed; all four per-macro /// doc blocks rewritten (rust,ignore → rust,no_run, reference s23/s24 examples) |
| `examples/s23_mcp_tool_macro.rs` | line 14 edit | `cargo run --example 63_mcp_tool_macro` → `s23_mcp_tool_macro` |
| `examples/s24_mcp_prompt_macro.rs` | line 13 edit | `cargo run --example 64_mcp_prompt_macro` → `s24_mcp_prompt_macro` |

### Deleted (1)

| File | Lines | Purpose |
|---|---|---|
| `pmcp-macros/POC_README.md` | 35 | Wave 0 POC gate placeholder — replaced by the real README.md cutover in Task 1 |

## Decisions Made

- **Atomic cutover in one commit.** README rewrite + include_str! flip + POC_README deletion are three changes with no meaningful intermediate state. Committing them as one keeps `cargo test --doc` green at every commit hash and makes rollback a single `git revert`.
- **Pitfall 4 option 1 validated in-situ.** Wave 0 POC_README.md only proved `use pmcp::mcp_tool;` works; it did NOT test `use pmcp_macros::mcp_resource;`. Plan 04 was the first opportunity to validate the mcp_resource import path, which I did during Task 1 by writing the README's `#[mcp_resource]` section with the direct import and running `cargo test --doc`. It compiled on the first attempt — no Pitfall 4 option 3 prose-only fallback needed. This means the asymmetric import is documented honestly (in a Note: blockquote above the section) and the reader gets working code.
- **README line count (355) overruns the plan target (200-300).** The overrun is concentrated in three places: (a) the `#[mcp_tool]` section has two code blocks (basic + State<T>) per the plan's explicit `### Shared state (State<T>)` sub-section requirement; (b) the explicit URI template variables prose section per the fold-in from the external review adds ~15 lines; (c) each macro's Attributes list adds ~5-6 lines. The plan allowed "editorial freedom on prose and tone" — 355 lines is concise, not verbose, and every line directly supports a documented feature. No section was padded.
- **Per-macro /// docs use single-block example, not multi-block.** The pre-existing lib.rs had 3 `rust,ignore` examples per mcp_tool (basic + state + annotations) and 2 per mcp_prompt. I consolidated each to a single `rust,no_run` block, moving the advanced patterns to the README proper. Rationale: docs.rs's individual macro pages are "quick reference"; readers who want depth jump to the full README (included via include_str!) or the examples/s23/s24 files. Three rustdoc examples per macro crowded the page and forced a tradeoff between completeness and compilability — one clean example per macro plus a link to the showcase is cleaner.
- **mcp_resource /// doc uses the same direct-import pattern as the README.** This keeps the two doc surfaces (crate-level README + per-macro /// docs) symmetric. When the re-export gap closes, both locations need the same `use pmcp::mcp_resource;` update, but until then both honestly show the direct import path.
- **Kept the ReadmeDoctests breadcrumb.** It's a correctness invariant (warns against a pattern that doesn't compile in proc-macro crates), not just historical context. Refreshing the comment to reference `README.md` instead of `POC_README.md` preserves the warning while keeping it accurate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Worktree base commit was wrong**
- **Found during:** Initial worktree branch check (before Task 1)
- **Issue:** The worktree branch's HEAD was `47ee632b` (main with two unrelated fix commits on top) instead of the expected `1748048e` from the prompt's `<worktree_branch_check>` block. Without this reset, the `.planning/phases/66-macros-documentation-rewrite/` directory would not have had the 66-01 / 66-02 / 66-03 SUMMARY files, and the Wave 0 POC wiring in `pmcp-macros/src/lib.rs` would not have been present either.
- **Fix:** `git reset --hard 1748048e05a21f9ce64d41064fa12a95b46c13e6` per the explicit prompt protocol.
- **Files modified:** N/A (branch state only)
- **Verification:** `git log --oneline -3` after reset shows HEAD at `1748048e docs(66-03): complete downstream-markdown-fixup plan`, and `.planning/phases/66-macros-documentation-rewrite/` contains 66-01/02/03 SUMMARY files along with 66-04-PLAN.md.
- **Committed in:** N/A (pre-commit branch state fix — same root cause as Plan 01, 02, 03 all noted in their respective summaries)

---

**Total deviations:** 1 auto-fixed (blocking branch state error, identical to prior plans in the wave)
**Impact on plan:** The reset was necessary to find the plan files, Wave 0 POC artifacts, and Wave 1 deletion results. Plan 04 executed exactly as written after the reset — no scope creep, no unexpected fallbacks, no in-flight rescoping.

## Issues Encountered

- **PreToolUse read-before-edit reminders fired repeatedly.** The hook warned seven times during Task 2 that I was about to edit files already read earlier in the session. All edits succeeded (the harness accepted them based on the session's earlier Read calls), but each warning added some chatter to the output. Standard harness invariant, not a bug.
- **Zero compile errors on first attempt.** Both the README rewrite and the per-macro /// doc rewrites compiled on first `cargo test --doc` — no iteration needed. Pitfall 5 (rustc diagnostic spans pointing at `lib.rs:1` for README errors) was prepared for but not exercised.
- **`rust,no_run` block count landed at 5 in README, not 4.** The plan's acceptance criterion required ≥4, actual is 5 because the `#[mcp_tool]` section has two blocks (basic + State<T>) as the plan's own skeleton required. Verified this is within spec: `[ $(grep -c '^```rust,no_run' pmcp-macros/README.md) -ge 4 ]` passes.

## User Setup Required

None — pure documentation and Rust source changes. No environment variables, no external services, no manual dashboard configuration. All changes will surface on docs.rs/pmcp-macros automatically on the next `pmcp-macros` publish.

## Next Phase Readiness

- **Plan 66-05 (changelog + version bump) is unblocked.** pmcp-macros v0.4.1 → v0.5.0 has all the semver-legal justification it needs: the breaking deletions happened in 66-02, the new public surface is now fully documented in 66-04, and `cargo test -p pmcp-macros --doc` serves as the drift detector to prove the new API docs match the code. Plan 05 just needs to add the CHANGELOG.md entry and bump the version pins.
- **The mcp_resource re-export gap is still open.** Intentionally — it's documented in the README's Note: blockquote, in the mcp_resource /// doc's inline `// Note: direct import ...` comment, and in CONTEXT.md's Deferred Ideas list. A future phase can close it by adding `mcp_resource` to `pmcp/src/lib.rs:147`'s `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};` line. At that point, the Note: blockquote and the inline comment should be removed and the mcp_resource imports flipped from `use pmcp_macros::mcp_resource;` to `use pmcp::mcp_resource;`. Two-line fix.
- **No blockers for downstream consumers.** pmcp-course and docs/advanced/migration-from-typescript.md were already updated in 66-03, so they point at the current API. The README they link to (pmcp-macros/README.md) is now also current.
- **docs.rs cutover is ready.** On the next publish (pmcp-macros v0.5.0), docs.rs will render the new 355-line README as the crate-level page, and each individual macro page will render the new per-macro `///` doc blocks. The two doc surfaces are intentionally symmetric (same examples, same import conventions, same example-file references), so readers landing from either entry point see consistent documentation.

## README Content Summary

Per the plan's explicit output spec:

- **Final README line count:** 355 lines (plan target: 200-300; overrun explained in "Decisions Made")
- **Total `rust,no_run` block count in README:** 5 (plan acceptance criterion: ≥4)
  - `#[mcp_tool]` basic example (lib.rs:61 doctest)
  - `#[mcp_tool]` State<T> example (lib.rs:108 doctest)
  - `#[mcp_server]` impl-block example (lib.rs:170 doctest)
  - `#[mcp_prompt]` standalone example (lib.rs:239 doctest)
  - `#[mcp_resource]` URI template example (lib.rs:330 doctest)
- **Wave 0 POC Block B fallback used:** No. The `#[mcp_resource]` section uses Pitfall 4 option 1 (direct import from `pmcp_macros::` with a Note: blockquote explanation) and compiled successfully on the first `cargo test --doc` run during Task 1. No option 3 prose-only downgrade needed.
- **`cargo test --doc` output confirmation:**
  ```
  test result: ok. 9 passed; 0 failed; 5 ignored; 0 measured; 0 filtered out; finished in 0.32s
  ```
  Baseline (start of 66-04): 1 passed, 12 ignored. Delta: +8 passing doctests.
- **Deviations from skeleton:**
  - No section order changes — plan skeleton order preserved exactly (`# pmcp-macros` → `## Installation` → `## Overview` → `## #[mcp_tool]` → `## #[mcp_server]` → `## #[mcp_prompt]` → `## #[mcp_resource]` → `## Feature flags` → `## License`).
  - No additional top-level sections added.
  - One additional sub-heading within `#[mcp_tool]`: `### Purpose` (the plan's skeleton had it as a free-standing paragraph, I promoted it to a `###` sub-heading for scan-ability and consistency with the other sections which all have `### Purpose` / `### Attributes` / `### Example` / `### Full runnable example` sub-structure).
  - The Wave 0 POC fallback path was NOT invoked for `#[mcp_resource]` (Pitfall 4 option 1 worked on first attempt, as noted above).

## Known Stubs

None. The README documents features that exist in shipped code; every `rust,no_run` block exercises real compile-time behavior. No placeholder text like "coming soon" or "TODO" appears anywhere in the new README, per the plan's explicit acceptance criteria and the stub-tracking scan.

## Self-Check: PASSED

- `pmcp-macros/README.md` exists: FOUND (355 lines)
- `pmcp-macros/POC_README.md` deleted: MISSING (correctly deleted)
- `pmcp-macros/src/lib.rs` contains `#![doc = include_str!("../README.md")]`: VERIFIED
- `pmcp-macros/src/lib.rs` does NOT contain `POC_README.md` reference: VERIFIED (grep returns 0)
- `pmcp-macros/src/lib.rs` breadcrumb comment preserved: VERIFIED (lines 19-41)
- `pmcp-macros/src/lib.rs` has zero `rust,ignore` fences in /// blocks: VERIFIED (grep returns 0)
- `pmcp-macros/src/lib.rs` has ≥4 `rust,no_run` fences: VERIFIED (4 in /// blocks, plus 2 in breadcrumb text → 6 total matches)
- `pmcp-macros/src/lib.rs` references `s23_mcp_tool_macro`: VERIFIED (3 occurrences: mcp_tool /// doc, mcp_server /// doc, mcp_resource /// doc)
- `pmcp-macros/src/lib.rs` references `s24_mcp_prompt_macro`: VERIFIED (2 occurrences: mcp_prompt /// doc, mcp_resource /// doc)
- `pmcp-macros/src/lib.rs` has zero `63_mcp_tool_macro` / `64_mcp_prompt_macro` references: VERIFIED
- `examples/s23_mcp_tool_macro.rs` header says `s23_mcp_tool_macro`: VERIFIED (line 14)
- `examples/s23_mcp_tool_macro.rs` has zero `63_mcp_tool_macro` references: VERIFIED
- `examples/s24_mcp_prompt_macro.rs` header says `s24_mcp_prompt_macro`: VERIFIED (line 13)
- `examples/s24_mcp_prompt_macro.rs` has zero `64_mcp_prompt_macro` references: VERIFIED
- `pmcp-macros/README.md` line 1 is `# pmcp-macros`: VERIFIED
- `pmcp-macros/README.md` contains `## Installation`: VERIFIED
- `pmcp-macros/README.md` contains `## `#[mcp_tool]``: VERIFIED
- `pmcp-macros/README.md` contains `## `#[mcp_server]``: VERIFIED
- `pmcp-macros/README.md` contains `## `#[mcp_prompt]``: VERIFIED
- `pmcp-macros/README.md` contains `## `#[mcp_resource]``: VERIFIED
- `pmcp-macros/README.md` has 5 `rust,no_run` blocks (≥4 required): VERIFIED
- `pmcp-macros/README.md` has zero `rust,ignore` blocks: VERIFIED
- `pmcp-macros/README.md` has zero bare `rust` blocks: VERIFIED
- `pmcp-macros/README.md` contains `pmcp = { version = "2.3"` install pin: VERIFIED
- `pmcp-macros/README.md` has zero `pmcp = "1.*"` references: VERIFIED
- `pmcp-macros/README.md` contains `use pmcp::{` import: VERIFIED (4 occurrences across mcp_tool/mcp_server/mcp_prompt sections)
- `pmcp-macros/README.md` has zero `#[tool(` / `#[tool_router` / `#[prompt(` / `#[resource(` references: VERIFIED
- `pmcp-macros/README.md` has zero `migration` / `deprecated` occurrences: VERIFIED (grep -ci returns 0)
- `pmcp-macros/README.md` contains `github.com/paiml/pmcp/blob/main/examples/s23_mcp_tool_macro.rs`: VERIFIED (2 occurrences)
- `pmcp-macros/README.md` contains `github.com/paiml/pmcp/blob/main/examples/s24_mcp_prompt_macro.rs`: VERIFIED (1 occurrence)
- `pmcp-macros/README.md` contains literal `URI template` text: VERIFIED (4 occurrences)
- `pmcp-macros/README.md` contains `{variable_name}`-shape token: VERIFIED (7 `{...}` matches including `{topic}`, `{variable_name}`)
- Commit `a9877876` (Task 1) exists in git log: FOUND
- Commit `31c3fbf6` (Task 2) exists in git log: FOUND
- `cargo test -p pmcp-macros --doc` exit 0: VERIFIED (9 passed, 0 failed, 5 ignored — delta +8 from baseline)
- `cargo test -p pmcp-macros` unit/trybuild tests pass: VERIFIED
- `cargo build -p pmcp-macros`: VERIFIED (clean)
- `cargo build --example s23_mcp_tool_macro --features full`: VERIFIED (clean)
- `cargo build --example s24_mcp_prompt_macro --features full`: VERIFIED (clean)
- `cargo clippy -p pmcp-macros`: VERIFIED (zero warnings)

---
*Phase: 66-macros-documentation-rewrite*
*Plan: 04-readme-and-rustdoc-rewrite*
*Completed: 2026-04-11*
