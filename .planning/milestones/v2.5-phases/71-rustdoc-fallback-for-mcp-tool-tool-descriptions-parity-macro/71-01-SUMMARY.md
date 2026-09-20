---
phase: 71
plan: 01
subsystem: pmcp-macros-support
tags: [macros, support-crate, rustdoc, parity, tdd]
requires: []
provides:
  - "pmcp-macros-support crate (workspace member)"
  - "pmcp_macros_support::rustdoc::extract_doc_description"
  - "pmcp_macros_support::rustdoc::reference_normalize"
affects:
  - "workspace Cargo.toml (members list)"
tech_stack_added:
  - "proptest 1.6 (dev-dep for property tests)"
tech_stack_patterns:
  - "sibling non-proc-macro crate hosts pure helpers that proc-macro crate cannot expose (HIGH-1 Option A)"
key_files_created:
  - crates/pmcp-macros-support/Cargo.toml
  - crates/pmcp-macros-support/README.md
  - crates/pmcp-macros-support/src/lib.rs
  - crates/pmcp-macros-support/tests/property_tests.rs
key_files_modified:
  - Cargo.toml
decisions:
  - "HIGH-1 resolved via Option A: new sibling crate (not re-export, not duplication)"
  - "No __fuzz feature gate needed — pure helpers are unconditionally public in the non-proc-macro crate"
  - "reference_normalize exposed as public API so Plan 03 fuzz target can consume it without syn round-trip"
metrics:
  duration_min: 7
  completed: 2026-04-17
  commits: 3
  tasks: 3
  tests_added: 20
---

# Phase 71 Plan 01: pmcp-macros-support scaffolding + normalizer + property tests Summary

**One-liner:** New non-proc-macro sibling crate `pmcp-macros-support` hosts the pure rustdoc-harvest normalizer + reference oracle, exhaustively unit-tested against all 10 normalization vectors and verified by 4 property invariants at 1000 cases each — resolving Codex review HIGH-1 (proc-macro crates cannot export arbitrary public API).

## What Was Built

### Task 1 — Workspace scaffold (commit `671350ef`)

- Created `crates/pmcp-macros-support/` directory with `Cargo.toml`, `src/lib.rs` (stubs), and `README.md`.
- Cargo.toml declares `pmcp-macros-support 0.1.0`, edition 2021, MIT, `rust-version = "1.82.0"`, single `syn 2.0 + proc-macro2` prod-dep, `proptest 1.6` dev-dep.
- **NOT a proc-macro crate** (no `[lib] proc-macro = true`) — this is the whole point of HIGH-1 Option A.
- Appended `"crates/pmcp-macros-support"` to root `Cargo.toml` workspace members list (position: immediately after `"pmcp-macros"`).

### Task 2 — Real implementation + unit tests (commit `3e83e23a`)

- `pub fn extract_doc_description(attrs: &[syn::Attribute]) -> Option<String>` — harvests `#[doc = "..."]` attrs, trims each line, drops empty lines, joins with `"\n"`. Gracefully skips `Meta::List`, `Meta::Path`, `Expr::Macro` (i.e. `#[doc(hidden)]`, `#[doc(alias = ...)]`, `#[doc = include_str!("...")]`).
- `pub fn reference_normalize(lines: &[String]) -> Option<String>` — plain-Rust spec of the normalization algorithm, used both as a property-test oracle and (Plan 03) by the fuzz target.
- 16 unit tests:
  - 10 normalization vectors from 71-RESEARCH.md §"Test vectors" (single line, two lines, blank middle, leading/trailing whitespace, empty, whitespace-only, doc(hidden), embedded quotes, all-whitespace).
  - 3 unsupported-form tests (MEDIUM-3 from 71-REVIEWS.md): `include_str!`, `cfg_attr(..., doc=...)`, mixed with real docs.
  - 3 reference-oracle sanity checks (empty input, real-vs-reference equivalence, idempotence on normalized output).

### Task 3 — Property tests (commit `8fa744b8`)

- `tests/property_tests.rs` with 4 proptest invariants, all at **1000 cases each**:
  1. `prop_normalize_matches_reference` — `extract_doc_description` vs. `reference_normalize` equivalence.
  2. `prop_normalize_deterministic` — pure-function determinism.
  3. `prop_no_panic_on_arbitrary_utf8` — adversarial UTF-8 never panics; non-None outputs are non-empty.
  4. `prop_mixed_attr_shapes_robust` — mixed `#[doc = ...]` + `#[doc(hidden)]` + non-doc attrs (e.g. `#[allow(dead_code)]`) still match the oracle (LOW-3 from 71-REVIEWS.md).
- **No feature gate** — the previously planned `__fuzz` feature and `__fuzz_support` re-export are now obsolete. Plan 03's fuzz target will depend on `pmcp-macros-support` directly.

## Verification Evidence

| Check | Command | Result |
|-------|---------|--------|
| Workspace builds | `cargo check --workspace` | ✅ exit 0 (only unrelated pre-existing cargo-pmcp warnings) |
| Unit + property tests | `cargo test -p pmcp-macros-support` | ✅ **20/20** passed (16 lib + 4 property; doctests: 0) |
| Unit tests only | `cargo test -p pmcp-macros-support --lib` | ✅ 16/16 passed |
| Property tests only | `cargo test -p pmcp-macros-support --test property_tests` | ✅ 4/4 passed at 1000 cases each |
| Clippy | `cargo clippy -p pmcp-macros-support --all-targets -- -D warnings` | ✅ No issues found |
| Rustdoc | `cargo doc -p pmcp-macros-support --no-deps` | ✅ Zero warnings; generated index.html |
| Feature-gate absence | `grep -rn __fuzz crates/pmcp-macros-support/` | ✅ No files found (0 matches) |
| Feature-gate absence in pmcp-macros | `grep -rn __fuzz pmcp-macros/` | ✅ No files found (0 matches — pmcp-macros untouched by Plan 01 as expected) |

## Plan-Level Must-Haves (from frontmatter)

| Must-have | Evidence |
|-----------|----------|
| New non-proc-macro crate exists and is a workspace member | ✅ `crates/pmcp-macros-support/` + Cargo.toml workspace.members updated |
| `extract_doc_description` implemented and exhaustively unit-tested against all 10 normalization vectors | ✅ 10 `vec1`…`vec10` tests present |
| Reference implementation exposed for property-test oracle + fuzz consumer | ✅ `pub fn reference_normalize` in `rustdoc` module |
| ≥3 property invariants at 1000 cases each, no feature flag | ✅ 4 invariants; `cases: 1000` configured; no feature gate |
| Normalization is deterministic and idempotent | ✅ `prop_normalize_deterministic` + `ref_idempotent_on_normalized_output` |
| Support crate has zero public API other than the pure helper + types | ✅ Only `pub mod rustdoc { pub fn extract_doc_description, pub fn reference_normalize }` |

All 6 must-have truths satisfied; all 4 artifacts present with expected content.

## Deviations from Plan

**None.** Plan executed exactly as written. Minor implementation detail: used `String::new()` instead of `"".to_string()` in one idempotence test per clippy-pedantic's `needless_borrows_for_generic_args` and stylistic preferences — purely cosmetic, behavior unchanged.

## Handoff to Plan 02

Plan 02 (Wave 2) can now:

1. Add `pmcp-macros-support = { path = "../crates/pmcp-macros-support" }` to `pmcp-macros/Cargo.toml` as a regular path dependency.
2. Call `pmcp_macros_support::rustdoc::extract_doc_description` from the shared `resolve_tool_args` resolver in `pmcp-macros/src/mcp_common.rs`.
3. Remove any leftover references to `__fuzz_support` or `__fuzz` feature (none exist — pmcp-macros was untouched in Plan 01).

The public API `pmcp_macros_support::rustdoc::{extract_doc_description, reference_normalize}` is stable per the plan's `<interfaces>` contract.

## Self-Check: PASSED

**Files claimed to exist — verified:**
- ✅ `crates/pmcp-macros-support/Cargo.toml`
- ✅ `crates/pmcp-macros-support/README.md`
- ✅ `crates/pmcp-macros-support/src/lib.rs`
- ✅ `crates/pmcp-macros-support/tests/property_tests.rs`

**Commits claimed to exist — verified:**
- ✅ `671350ef` — feat(71-01): scaffold pmcp-macros-support sibling crate (HIGH-1 Option A)
- ✅ `3e83e23a` — feat(71-01): implement extract_doc_description + reference_normalize
- ✅ `8fa744b8` — test(71-01): add 4 proptest invariants at 1000 cases each
