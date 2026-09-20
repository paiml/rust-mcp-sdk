---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 05
subsystem: pmcp-workbook-compiler/formula tokenizer
tags: [refactor, pmat-gate, cognitive-complexity, tokenizer, behavior-preserving]
requires: []
provides:
  - "formula/token.rs cleared from PMAT complexity gate (tokenize, lex_quoted_sheet_ref, scan_atom_run all <= 23)"
affects:
  - "PR #279 PMAT quality-gate (3 of the workbook violations closed)"
tech-stack:
  added: []
  patterns:
    - "match-to-helper dispatch (tokenize -> lex_next -> lex_delimited)"
    - "early-return guard clauses (read_quoted_sheet_name)"
    - "step-descriptor classifier (AtomStep + classify_atom_char) replacing an if/else ladder"
key-files:
  created: []
  modified:
    - crates/pmcp-workbook-compiler/src/formula/token.rs
decisions:
  - "Decomposed the main lex loop into lex_next (single-token dispatch) + lex_delimited (delimiter->scan_* map) instead of inlining; tokenize is now a 3-line while loop."
  - "scan_atom_run's character-class accept ladder became a classify_atom_char returning an AtomStep enum carrying step/dollar/breaks_numeric, preserving the exact flag semantics."
  - "No #[allow(clippy::cognitive_complexity)] used (PMAT ignores it per CONTEXT D-10-B); only genuine structural decomposition."
metrics:
  duration: ~25m
  completed: 2026-06-16
---

# Phase 99 Plan 05: Formula Tokenizer Cognitive-Complexity Reduction Summary

Behavior-preserving decomposition of the Excel-formula tokenizer's three over-cap
functions (`tokenize` cog 52, `lex_quoted_sheet_ref` cog 33, `scan_atom_run` cog 30)
to below the PMAT gate's effective ceiling (<= 23 each), with the exact token stream
and lex-error positions preserved.

## What Changed

All work in `crates/pmcp-workbook-compiler/src/formula/token.rs`.

### Task 1 — Decompose `tokenize` (commit a6b5ee5c)
- `tokenize` (cog 52 -> 6): the body is now a thin `while i < chars.len() { i = lex_next(...) }` loop.
- New `lex_next` (cog 15): dispatches the lead char — whitespace skip, delimiter scan, atom run, operator fallthrough.
- New `lex_delimited` (cog 1): maps a delimiter char (`"`, `[`, `'`, `#`) to its existing `lex_*` scan helper, returning `Option<Result<(Token, usize), LexError>>` (`None` => fall through to atom/operator).

### Task 2 — Flatten `lex_quoted_sheet_ref` and `scan_atom_run` (commit ee01ed52)
- `lex_quoted_sheet_ref` (cog 33 -> below gate): extracted `read_quoted_sheet_name` (the `'…'` name loop with `''` escaping, using a guard-clause early return on close) and `scan_addr_run` (the trailing `!addr` run).
- `scan_atom_run` (cog 30 -> below gate): the long `if / else if` character-class ladder is replaced by `classify_atom_char` returning an `AtomStep` enum (`Stop` | `Advance { step, dollar, breaks_numeric }`); the loop folds the flags via `has_dollar |= dollar` and `all_numeric_shape &= !breaks_numeric`, reproducing the original `$` / digit / scientific-`E` / alnum accept semantics exactly (including the 2-index step for a signed exponent).

### Task 3 — Verify gate + clippy
- PMAT oracle empty for token.rs; clippy clean; tests green (verification only, folded into the Task 1/2 commits since no further source change was required).

## Verification

- **PMAT oracle (`pmat analyze complexity --max-cognitive 25 --format json`):** `[.files[] | select(.path | test("formula/token.rs")) | .functions[] | select(.metrics.cognitive > 23)]` => `[]` (empty). token.rs no longer appears in the over-threshold file list at all.
- **Post-refactor cog (from the threshold-25 run before the file dropped off):** `tokenize` 6, `lex_next` 15, `lex_delimited` 1; `lex_quoted_sheet_ref`, `read_quoted_sheet_name`, `scan_addr_run`, `scan_atom_run`, `classify_atom_char` all <= 23 (none flagged by the gate).
- **Tests:** `cargo test -p pmcp-workbook-compiler` => 315 passed, 1 ignored. `cargo test -p pmcp-workbook-dialect` (linter goldens) => 6 passed.
- **Clippy:** `cargo clippy -p pmcp-workbook-compiler --all-features -- -D warnings` => no issues (the pedantic/nursery merge bar; `unnecessary_wraps` does NOT fire on `lex_delimited` because it genuinely returns `None`).
- **No `#[allow(clippy::cognitive_complexity)]`, no `.pmatignore` edit.** `git diff --name-only` against the base shows ONLY `token.rs` touched.

## Behavior-Preservation Notes

Zero behavior change was the hard constraint. The token stream and `LexError` positions
are identical:
- `lex_delimited` returns `Some(Ok(lex_error_literal(...)))` (infallible) vs `Some(lex_string/external/quoted(...))` (fallible) — the `?` in `lex_next` propagates errors at the same point as the original inline `?`.
- `read_quoted_sheet_name` returns `Err(UnterminatedQuotedSheet)` only on falling off the end without a closing quote — same as the original `closed` flag check.
- `classify_atom_char` reproduces the original accept ladder arm-for-arm; the `$` arm still sets both `has_dollar` and breaks numeric shape, the plain-alnum arm breaks numeric shape only, and the scientific-`E` arm consumes the optional sign.
- The full tokenizer regression net (digit-leading sheet names, `""`-escaping, sheet-qualified ranges, anchored quoted ranges, scientific notation, external refs, error literals, comparison operators) stays green.

## Deviations from Plan

The plan's task narrative assumed `tokenize` still inlined every per-char branch and
that the helpers needed first-time extraction. In reality the file had already been
partially decomposed (pre-existing `lex_string`, `lex_external_ref`, `lex_operator`,
`lex_atom`, `scan_atom_run`, etc.). The three named functions were still over-cap, so
the refactor targeted exactly them with the techniques the plan specified (match-to-helper
dispatch, guard clauses, classifier extraction). No scope change, no behavior change —
this is a more-surgical application of the same plan, not a deviation requiring a rule.

None of Rules 1–4 triggered. No auth gates.

## Known Stubs

None.

## Self-Check: PASSED
- `crates/pmcp-workbook-compiler/src/formula/token.rs` exists and is the only modified file.
- Commit a6b5ee5c (Task 1) present in git log.
- Commit ee01ed52 (Task 2) present in git log.
- PMAT oracle empty for token.rs; clippy clean; 315 + 6 tests green.
