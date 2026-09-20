---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 02
subsystem: pmcp-workbook-runtime / sheet_ir evaluator
tags: [refactor, pmat, cognitive-complexity, sheet-ir, executor, semantics]
requires:
  - "PMAT 3.15.0 (cog gate oracle)"
  - "pmcp-workbook-runtime existing scalar_eval + quirk/reconcile test net"
provides:
  - "eval_expr decomposed to a thin match dispatcher (cog 58 -> cleared)"
  - "f_index flattened via one_based_index guard (cog 24 -> cleared)"
  - "f_search flattened via search_start_position + search_char_position (cog 31 -> cleared)"
affects:
  - "crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs"
  - "crates/pmcp-workbook-runtime/src/sheet_ir/semantics.rs"
tech-stack:
  added: []
  patterns:
    - "Per-variant match-arm extraction (eval_call/eval_binary_op/eval_unary_op)"
    - "Borrowed evaluation-context struct (Ctx) to keep helpers under clippy arg-count bar"
    - "Early-return guard-clause extraction (one_based_index, search_start_position)"
key-files:
  created: []
  modified:
    - "crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs"
    - "crates/pmcp-workbook-runtime/src/sheet_ir/semantics.rs"
decisions:
  - "Bundle env/errs/current_sheet/trace into a borrowed Ctx struct rather than thread 8 positional args (clippy too_many_arguments bar is 7)"
  - "Used the `pmat quality-gate --fail-on-violation --checks complexity` command as the oracle (not `analyze complexity` JSON, whose summary.violations only surfaces cog>25; the gate also flags the recommended-23 tier that f_index/f_search sit in)"
metrics:
  duration: "~20m"
  completed: "2026-06-16"
  tasks: 3
  files: 2
---

# Phase 99 Plan 02: sheet_ir Evaluator Cognitive-Complexity Reduction Summary

Behavior-preserving decomposition of the three flagged `sheet_ir` evaluator functions in `pmcp-workbook-runtime` — `executor.rs::eval_expr` (cog 58, over the hard-50 cap) plus `semantics.rs::f_index` (cog 24) and `f_search` (cog 31) — all now clear the org-required PMAT complexity gate with the existing scalar_eval + quirk-corpus + reconcile test net green and clippy clean.

## What Was Done

### Task 1 — Decompose `eval_expr` via per-variant helper dispatch
`eval_expr` was a single large `match` over `Expr` variants whose cog (58) was dominated by the inline `Call`/`BinaryOp`/`UnaryOp` arms (the Call arm had a triple-nested operand-flattening loop; the binary/unary arms each had a `Pow`/`Percent` special case plus a leaf-lowering fast path plus a recursive fallback). Extracted:
- `eval_call` — argument materialization + dispatched-fn/operand trace recording + `semantics::apply`.
- `record_operand_values` — the scalar/range trace-flattening loop (lifted out of the Call arm).
- `eval_binary_op` — `Pow`/leaf-lower/recursive-fallback for binary ops.
- `eval_unary_op` — `Percent`/leaf-lower/recursive-fallback for unary ops.

`eval_expr` is now a thin match that delegates. Recursion structure (helpers call back into `eval_expr`) and every error/coercion path preserved exactly.

### Task 2 — Flatten `f_index` and `f_search`
- `f_index`: extracted `one_based_index` — the repeated "positive integer or `#VALUE!`" guard that gated both the 1-D `n` and the 2-D `col` arg. Both call sites now share it, flattening the nesting.
- `f_search`: extracted `search_start_position` (optional 1-based `start_num` → 0-based index, `#VALUE!` on a non-positive/fractional value) and `search_char_position` (the char-slice scan with the needle-longer-than-haystack pre-guard that prevents the T-09-13 slice overrun). INDEX/SEARCH semantics (1-based positions, case-insensitive SEARCH, `#REF!`/`#VALUE!` error values) unchanged.

### Task 3 — Keep clippy clean + final gate verification
The Task-1 split initially tripped `clippy::too_many_arguments` on `eval_binary_op` (8 params > 7 bar). Resolved by bundling the four threaded context refs (`env`, `errs`, `current_sheet`, `trace`) into a borrowed `Ctx<'a>` struct with a `Ctx::eval` convenience method; `eval_binary_op`/`eval_unary_op` now take `&mut Ctx`. Behavior identical.

## Verification

- **PMAT gate (oracle):** `pmat quality-gate --fail-on-violation --checks complexity` — **no `sheet_ir` function listed** (eval_expr, f_index, f_search all cleared; every new helper is below the recommended-23 tier).
- **Tests:** `cargo test -p pmcp-workbook-runtime` — **157 passed, 0 failed** (scalar_eval unit tests, quirk corpus, reconcile fixtures, render goldens, doctest).
- **Clippy:** `cargo clippy -p pmcp-workbook-runtime --all-features -- -D warnings` — **No issues found** (pedantic-clean; `too_many_arguments` resolved).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `clippy::too_many_arguments` on `eval_binary_op`**
- **Found during:** Task 3 (final clippy gate)
- **Issue:** The per-variant split passed 8 positional args (`e, left, op, right, env, errs, current_sheet, trace`), exceeding clippy's 7-arg bar — a hard error under `-D warnings`, which would block the merge bar.
- **Fix:** Introduced a borrowed `Ctx { env, errs, current_sheet, trace }` struct with a `Ctx::eval` method; `eval_binary_op`/`eval_unary_op` take `&mut Ctx`. No behavior change.
- **Files modified:** crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs
- **Commit:** bd225aef

### Note on the oracle command
The plan's `<verify>` blocks used `pmat analyze complexity --max-cognitive 25 --format json | jq '... select(.path | test(...))'`. Empirically that JSON exposes violations under `.summary.violations[]` keyed by `file` (not `path`), and its `summary.violations` array only surfaces cog>25 — so `f_index` (24) and `f_search` (31-against-recommended-23) were invisible there. The authoritative `pmat quality-gate --fail-on-violation --checks complexity` command (the exact CI gate) was used as the oracle instead; it correctly listed all three at baseline and confirms all three cleared after the refactor.

## Self-Check: PASSED

- FOUND: crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs (modified)
- FOUND: crates/pmcp-workbook-runtime/src/sheet_ir/semantics.rs (modified)
- FOUND commit 4bf5540a (eval_expr decomposition)
- FOUND commit 623fa015 (f_index/f_search flatten)
- FOUND commit bd225aef (Ctx bundling)
- PMAT gate: no sheet_ir violations
- Tests: 157 passed
- Clippy: clean
