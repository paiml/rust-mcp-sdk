---
phase: 71
plan: 02
subsystem: pmcp-macros
tags: [macros, proc-macro, resolver, shared-helper, parse-sites, integration-tests, PARITY-MACRO-01]
requires:
  - "pmcp-macros-support crate (from 71-01)"
  - "pmcp_macros_support::rustdoc::extract_doc_description (from 71-01)"
provides:
  - "pmcp-macros/src/mcp_common.rs::resolve_tool_args — SINGLE shared resolver used by both #[mcp_tool] parse sites"
  - "pmcp-macros/src/mcp_common.rs::MCP_TOOL_MISSING_DESCRIPTION_ERROR — canonical diagnostic const"
  - "pmcp-macros/src/mcp_common.rs::build_description_meta + has_description_meta (internal helpers routed through resolver only)"
  - "Standalone parse site delegation: mcp_tool.rs::expand_mcp_tool → resolve_tool_args (one-liner)"
  - "Impl-block parse site delegation: mcp_server.rs::parse_mcp_tool_attr → resolve_tool_args (one-liner, Meta::Path falls through with empty tokens)"
  - "Rustdoc-fallback behavior live end-to-end for standalone fn + impl-block method"
affects:
  - "pmcp-macros/Cargo.toml (new path dep on pmcp-macros-support)"
  - "pmcp-macros/src/mcp_tool.rs (hard-reject block replaced by resolver delegation)"
  - "pmcp-macros/src/mcp_server.rs (parse_mcp_tool_attr now delegates to resolver)"
  - "pmcp-macros/src/utils.rs (dead extract_doc_comment helper deleted as part of simplify pass)"
tech_stack_added: []
tech_stack_patterns:
  - "SINGLE shared resolver pattern — one function both parse sites call, no duplicated helper sequence (Codex review MEDIUM-1 resolution)"
  - "Synthetic NestedMeta construction via `syn::LitStr::new` + `parse_quote!` — correct quote/backslash/UTF-8 handling (supersedes string-formatting round-trip)"
  - "Same-module generated-struct direct instantiation in integration tests — `SquareToolHandler { server: Arc::new(...) }` to read `.metadata()` without a public accessor"
key_files_created:
  - .planning/phases/71-rustdoc-fallback-for-mcp-tool-tool-descriptions-parity-macro/71-02-SUMMARY.md
  - .planning/phases/71-rustdoc-fallback-for-mcp-tool-tool-descriptions-parity-macro/deferred-items.md
key_files_modified:
  - pmcp-macros/Cargo.toml
  - pmcp-macros/src/mcp_common.rs
  - pmcp-macros/src/mcp_tool.rs
  - pmcp-macros/src/mcp_server.rs
  - pmcp-macros/tests/mcp_tool_tests.rs
  - pmcp-macros/tests/mcp_server_tests.rs
decisions:
  - "MEDIUM-1 resolved via SINGLE shared resolver `resolve_tool_args` — both parse sites reduce to a one-line delegation, no symmetric call sequences to drift out of sync"
  - "MEDIUM-3 `description = \"\"` semantic locked: empty string is PRESENT, rustdoc fallback is NOT triggered, empty string passes through to darling"
  - "Meta::Path(_) branch in parse_mcp_tool_attr falls through with empty TokenStream (not early-return) so resolver handles the `#[mcp_tool]` no-parens form uniformly for rustdoc fallback"
  - "Meta::NameValue(_) branch keeps its orthogonal syntax-error early-return (wrong attribute shape, not a description issue)"
  - "UI stderr snapshot (mcp_tool_missing_description.stderr) working-tree diff left for Plan 03 per its files_modified list — not in 71-02 scope"
metrics:
  duration_min: 2
  completed: 2026-04-18
  commits: 3
  tasks: 3
  tests_added: 15
---

# Phase 71 Plan 02: Consume pmcp-macros-support + shared resolver + parse-site delegation Summary

**One-liner:** `#[mcp_tool]` now harvests rustdoc as a silent fallback for `description` via a single shared resolver `resolve_tool_args` in `pmcp-macros/src/mcp_common.rs` that both parse sites (standalone fn + impl-block method) call as a one-liner — eliminating Codex MEDIUM-1 drift risk and locking MEDIUM-3 (`description = ""`) semantics with a dedicated unit test.

## What Was Built

### Task 1 — Shared resolver + error const + internal helpers (commit `4d4b8fe1`)

- `pmcp-macros/Cargo.toml`: adds `pmcp-macros-support = { version = "0.1.0", path = "../crates/pmcp-macros-support" }` as a regular path dep.
- `pmcp-macros/src/mcp_common.rs` (+253 lines):
  - `pub(crate) const MCP_TOOL_MISSING_DESCRIPTION_ERROR: &str = "mcp_tool requires either a \`description = \"...\"\` attribute or a rustdoc comment on the function";` — canonical wording, one source of truth.
  - `pub(crate) fn resolve_tool_args(args_tokens, item_attrs, error_span_ident) -> syn::Result<Vec<NestedMeta>>` — the ONE function. Parses nested metas → if no `description = ...` present, consults `pmcp_macros_support::rustdoc::extract_doc_description` → if still absent, emits canonical error spanned at the identifier.
  - `build_description_meta(desc: &str)` — synthesizes `description = #lit` via `syn::LitStr::new` + `parse_quote!` (handles embedded quotes/backslashes/UTF-8 correctly).
  - `has_description_meta(metas)` — internal predicate; empty string `""` counts as PRESENT (MEDIUM-3 lock).
- 11 unit tests in the `rustdoc_fallback_tests` module cover: rustdoc-only synthesis, attribute precedence, missing-both error wording, `description = ""` + rustdoc (keeps empty), `description = ""` + no rustdoc (keeps empty), exact error wording constant, has-description true/false, synthesized meta with embedded quotes, and silent skip of `include_str!` / `cfg_attr(..., doc = ...)` unsupported forms.

### Task 2 — Parse-site delegation + simplify pass (commit `b0405fd0`)

- `pmcp-macros/src/mcp_tool.rs::expand_mcp_tool`: replaced the hard-reject block (`if args.is_empty() { return Err(...) }`) with a one-line delegation `let nested_metas = mcp_common::resolve_tool_args(args, &input.attrs, &input.sig.ident)?;` then feeds it straight to `McpToolArgs::from_list`.
- `pmcp-macros/src/mcp_server.rs::parse_mcp_tool_attr`: `Meta::List` tokens flow to the resolver; `Meta::Path(_)` no longer early-returns — it falls through with `TokenStream::new()` so the resolver can consult rustdoc uniformly; `Meta::NameValue(_)` keeps its orthogonal-syntax-error early-return.
- Simplify-pass deletions from a 3-agent code review:
  - `pmcp-macros/src/utils.rs::extract_doc_comment` — dead legacy helper deleted (name-clash risk with the new `pmcp_macros_support::rustdoc::extract_doc_description`).
  - `resolve_tool_args`: single-scan of `nested_metas` + local bool (halves work on the attribute-absent path).
  - `extract_doc_description` (in support crate): trim-before-allocate (saves one alloc per blank-after-trim line).
  - `reference_normalize` marked `#[doc(hidden)]` (test/fuzz oracle, not public API) + switched internal `Vec<String>` → `Vec<&str>`.
  - Stripped phase/task tags from source per the quality review (they belong in commit messages, not code).

### Task 3 — Integration tests (commit `346395bb`)

Four behavioral tests proving end-to-end rustdoc fallback works symmetrically from BOTH parse sites:

`pmcp-macros/tests/mcp_tool_tests.rs` (+60 lines):
- `test_rustdoc_only_description` — `/// Add two numbers together.` + no `description` attr → `metadata.description == Some("Add two numbers together.")`.
- `test_attribute_wins_over_rustdoc` — `#[mcp_tool(description = "WINS")]` + `/// IGNORED` rustdoc → `Some("WINS")` (silent precedence, no diagnostic).
- `test_multiline_rustdoc_normalization` — three-line rustdoc with blank middle → `Some("First line of the description.\nSecond line with more detail.\nThird line after a blank middle.")` (trim-joined, blank dropped).

`pmcp-macros/tests/mcp_server_tests.rs` (+42 lines):
- `test_impl_block_rustdoc_harvest` — `#[mcp_tool]` (no parens) on an impl method with `/// Compute the square of a number.` rustdoc. Constructs the macro-generated `SquareToolHandler` directly (same-module visibility) and asserts `metadata().description == Some("Compute the square of a number.")` + `metadata().name == "square"`. Proves byte-symmetry with the standalone path.

## Verification Evidence

| Check | Command | Result |
|-------|---------|--------|
| Full pmcp-macros test suite | `cargo test -p pmcp-macros` | OK — 86 passed (82 pre-existing + 4 new integration + pre-existing lib tests including the 11 resolver units), 5 ignored |
| 3 new standalone integration tests | `cargo test -p pmcp-macros --test mcp_tool_tests -- test_rustdoc_only_description test_attribute_wins_over_rustdoc test_multiline_rustdoc_normalization` | OK — 3/3 passed |
| Impl-block integration test | `cargo test -p pmcp-macros --test mcp_server_tests -- test_impl_block_rustdoc_harvest` | OK — 1/1 passed |
| Workspace examples compile | `cargo check --workspace --examples --features full` | OK — all 25 `#[mcp_tool]` call sites unchanged-green |
| Clippy on plan-touched code | `cargo clippy -p pmcp-macros --lib --tests -- -D warnings -A clippy::useless_format` | OK — no issues found |
| Resolver call-site count | `grep -rc 'mcp_common::resolve_tool_args' pmcp-macros/src/` | OK — exactly 2 (one per parse site, no duplication) |
| Old error string gone | `grep -rc 'mcp_tool requires at least' pmcp-macros/src/` | OK — 0 matches |
| No stale `__fuzz` | `grep -rc '__fuzz' pmcp-macros/` | OK — 0 |

## Plan-Level Must-Haves (from frontmatter)

| Must-have | Evidence |
|-----------|----------|
| `pmcp-macros` depends on `pmcp-macros-support` as regular path dep | OK — Cargo.toml line added; no proc-macro API restriction violations |
| SINGLE shared resolver `resolve_tool_args` is the ONLY entry point both parse sites use | OK — `grep -rc 'mcp_common::resolve_tool_args' pmcp-macros/src/` = 2 (one per site), zero direct calls to `has_description_meta` / `extract_doc_description` / `build_description_meta` from parse sites |
| `#[mcp_tool]` + rustdoc + no description attr → description == rustdoc text | OK — `test_rustdoc_only_description` |
| `#[mcp_tool]` + BOTH rustdoc + description attr → attribute wins byte-for-byte | OK — `test_attribute_wins_over_rustdoc` |
| `#[mcp_tool]` + NEITHER → canonical error from `MCP_TOOL_MISSING_DESCRIPTION_ERROR` | OK — `resolve_neither_present_errors_with_canonical_wording` unit test + pre-existing trybuild compile_fail test |
| `#[mcp_tool(description = "")]` + no rustdoc → fails with missing-description error | DEVIATION — see "Deviations from Plan" below |
| Both parse sites call `resolve_tool_args` — one-liner invocation, no duplicated logic | OK — `grep` counts + inspection of `mcp_tool.rs::expand_mcp_tool` and `mcp_server.rs::parse_mcp_tool_attr` |
| All 17 pre-existing tests continue to pass — s23 coexistence sites byte-identical | OK — 86/86 test runs green including the previously-passing 17 + `cargo check --workspace --examples --features full` exit 0 |

## Deviations from Plan

### 1. [Rule 4 → documented] `description = ""` semantics deliberately diverges from one frontmatter bullet

- **Plan frontmatter said:** "`#[mcp_tool]` fn with `description = ""` (empty string) and no rustdoc **fails** with the missing-description error (explicit empty-string semantic, per MEDIUM-3)."
- **Plan body strategy note said:** "Explicit `description = \"\"` is treated as **PRESENT**. The `has_description_meta` check returns true, rustdoc fallback is NOT triggered, and the empty string passes through to `McpToolArgs::from_list` where darling accepts it. This is consistent with pre-Phase-71 behavior..."
- **Actual implementation (commit `4d4b8fe1`) follows the body text, not the frontmatter bullet:** `has_description_meta` returns `true` for `description = ""`, so no error is raised and the empty string passes through. The internal unit test `resolve_empty_string_description_no_rustdoc_keeps_empty` explicitly locks this semantic.
- **Why the body text wins:** The two are contradictory within the same plan. The detailed body strategy note is the more carefully-reasoned specification (explicitly references "pre-Phase-71 behavior", MEDIUM-3, and the darling round-trip). The one-line frontmatter bullet appears to be a summary miswrite. The body is what the implementing committer (`4d4b8fe1`) followed, and the code review that merged it (`b0405fd0`) did not flag it.
- **Impact:** Zero user-visible breakage — pre-Phase-71 behavior was "empty string is valid", and that is preserved. If a future requirement wants to fail-fast on empty descriptions, that is a separate change with its own requirement ID (documented in the plan body).

### 2. [Rule 2 - Scope boundary] Pre-existing clippy warning in `mcp_prompt_tests.rs:151` — DEFERRED

- **Found during:** Task 3 clippy run (`cargo clippy -p pmcp-macros --all-targets -- -D warnings`)
- **Issue:** `clippy::useless_format` in `pmcp-macros/tests/mcp_prompt_tests.rs:151` (pre-existing, reproduced under `git stash`)
- **Disposition:** Out of scope for 71-02 (file not in `files_modified`, not caused by this plan's changes). Logged to `.planning/phases/71-rustdoc-fallback-for-mcp-tool-tool-descriptions-parity-macro/deferred-items.md`.
- **Commit gate:** The two prior 71-02 commits (`4d4b8fe1`, `b0405fd0`) landed despite this pre-existing warning — the repo's pre-commit hook is not installed. Task 3 commit (`346395bb`) preserved the same status quo; `cargo clippy -p pmcp-macros --lib --tests -- -A clippy::useless_format` is clean.

### 3. [Documented] Working-tree diff to `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` — LEFT FOR 71-03

- A one-line change updating the expected trybuild error text from `"mcp_tool requires at least..."` → `"mcp_tool requires either..."` was staged in the working tree before this agent started.
- Per Plan 71-02 frontmatter `files_modified` the stderr file is NOT owned by 71-02. Plan 71-03's `files_modified` lists it for snapshot regeneration.
- Per the resumption-context guidance ("If unclear, leave it alone for 71-03"), the working-tree change was left unstaged and uncommitted. 71-03 will pick it up when it regenerates trybuild snapshots.
- Net effect on 71-02: none — `compile_fail_tests` currently passes because the working-tree stderr already matches the new error string, but the change is not attributed to 71-02's commits.

## TDD Gate Compliance

Plan 71-02 frontmatter is `type: execute` (not `type: tdd`), so the plan-level RED/GREEN/REFACTOR gate ordering is not enforced. Individual tasks do include `tdd="true"`:

- Task 1: resolver was landed with 11 internal unit tests in the SAME commit (`4d4b8fe1`) — the unit-test RED was not split from GREEN, but the `test(...)` vs `feat(...)` gate is advisory for non-TDD plans. Feature-level behavior gate validated end-to-end via Task 3 integration tests.
- Task 2: refactor-style — no new behavior, only delegation change. Gated by the pre-existing 17 integration tests (all green).
- Task 3: pure `test(71-02): add 4 integration tests...` commit — correctly typed.

## Handoff to Plan 03

Plan 03 (Wave 3) can now:

1. Regenerate the trybuild `.stderr` snapshots (`mcp_tool_missing_description.stderr` working-tree diff is already in place, and `mcp_server_tests` may want its own missing-description snapshot).
2. Land the cargo-fuzz target consuming `pmcp_macros_support::rustdoc::reference_normalize` directly (no feature gate needed — `pmcp-macros-support` is a plain dep).
3. Update READMEs + doctests to document the rustdoc-fallback behavior (both `pmcp-macros` and `pmcp`).
4. Address the deferred `clippy::useless_format` in `mcp_prompt_tests.rs:151` at its convenience.

The resolver contract in `pmcp-macros/src/mcp_common.rs` is stable:
- `pub(crate) fn resolve_tool_args(args: TokenStream, item_attrs: &[Attribute], error_span_ident: &Ident) -> syn::Result<Vec<NestedMeta>>`
- `pub(crate) const MCP_TOOL_MISSING_DESCRIPTION_ERROR: &str`

## Self-Check: PASSED

**Files claimed to exist or be modified — verified:**
- OK `pmcp-macros/Cargo.toml` (pmcp-macros-support path dep present)
- OK `pmcp-macros/src/mcp_common.rs` (resolve_tool_args + MCP_TOOL_MISSING_DESCRIPTION_ERROR + helpers present)
- OK `pmcp-macros/src/mcp_tool.rs` (mcp_common::resolve_tool_args call present)
- OK `pmcp-macros/src/mcp_server.rs` (mcp_common::resolve_tool_args call present)
- OK `pmcp-macros/tests/mcp_tool_tests.rs` (3 new test fns present)
- OK `pmcp-macros/tests/mcp_server_tests.rs` (test_impl_block_rustdoc_harvest present)
- OK `.planning/phases/71-rustdoc-fallback-for-mcp-tool-tool-descriptions-parity-macro/deferred-items.md`

**Commits claimed to exist — verified (git log):**
- OK `4d4b8fe1` — feat(71-02): wire pmcp-macros-support dep + shared resolve_tool_args resolver
- OK `b0405fd0` — refactor(71-02): wire parse sites + simplify pass from 3-agent code review
- OK `346395bb` — test(71-02): add 4 integration tests for rustdoc-fallback (PARITY-MACRO-01)
