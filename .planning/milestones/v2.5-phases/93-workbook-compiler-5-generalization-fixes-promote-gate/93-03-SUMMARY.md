---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
plan: 03
subsystem: workbook-compiler (parse-and-analyze middle: dialect linter + formula parser + DAG)
tags: [workbook, dialect, linter, formula-parser, pratt, dag, kahn-toposort, fuzz, security]
requires:
  - "pmcp-workbook-dialect: WHITELIST / DialectRules / CandidateRole contract (Phase 91)"
  - "pmcp-workbook-runtime: Expr/BinOp/UnOp, Dag/toposort, RangeRef, ExcelError, LintFinding/LintReport/Severity, resolve primitives (Phase 91)"
  - "pmcp-workbook-compiler Wave-1 stubs: dialect/ formula/ dag/ (Plan 01)"
provides:
  - "dialect::linter — WBDL-03 collect-all located linter over a synthetic CellSource"
  - "dialect::{CellSource, CellView, SheetView, DefinedName} — the reader-free cell-iteration seam"
  - "formula::parser — whitelist-AT-PARSE-time Pratt parser building runtime Expr, typed ParseError, MAX_PARSE_DEPTH"
  - "formula::token — Excel tokenizer (MAX_FORMULA_LEN DoS guard)"
  - "formula::rebase — within-block-range row-offset templating"
  - "dag::topo::build_dag — Kahn toposort over synthetic ParsedCell + DefinedName, typed DagBuildError (cycle enumerated)"
  - "fuzz_formula_parser — cargo-fuzz target over the untrusted parser"
affects:
  - "Plan 04 (manifest/reconcile) wires the real owned cell model into CellSource + drives the parser/DAG"
  - "Plan 07 (driver) composes ingest→lint→parse→DAG"
tech-stack:
  added: []
  patterns:
    - "Synthetic trait seam (CellSource) to keep parallel plans decoupled from an upstream type"
    - "Whitelist-at-parse-time as a typed rejection (parse-error vs lint-finding boundary kept crisp)"
    - "Reuse runtime IR/container types; never re-declare (re-export keystone)"
    - "DoS guard enforced in code (MAX_PARSE_DEPTH) before fuzzing"
key-files:
  created:
    - crates/pmcp-workbook-compiler/src/dialect/linter.rs
    - crates/pmcp-workbook-compiler/src/formula/token.rs
    - crates/pmcp-workbook-compiler/src/formula/parser.rs
    - crates/pmcp-workbook-compiler/src/formula/rebase.rs
    - crates/pmcp-workbook-compiler/src/dag/graph.rs
    - crates/pmcp-workbook-compiler/src/dag/resolve.rs
    - crates/pmcp-workbook-compiler/src/dag/topo.rs
    - crates/pmcp-workbook-compiler/fuzz/Cargo.toml
    - crates/pmcp-workbook-compiler/fuzz/fuzz_targets/fuzz_formula_parser.rs
    - crates/pmcp-workbook-compiler/fuzz/.gitignore
  modified:
    - crates/pmcp-workbook-compiler/src/dialect/mod.rs
    - crates/pmcp-workbook-compiler/src/formula/mod.rs
    - crates/pmcp-workbook-compiler/src/dag/mod.rs
decisions:
  - "Parser returns a typed ParseError for whitelist misses (NOT a LintFinding) — keeps the parser-error vs lint-finding boundary crisp (Codex MEDIUM); the linter, not the parser, owns dialect findings"
  - "CellSource trait (CellView/SheetView/DefinedName) is the synthetic seam — the linter/parser/DAG read it, never 93-02's owned cell model, keeping 93-02/93-03 parallel; the real model implements it in Plan 04"
  - "DAG build returns a typed DagBuildError (range-too-large/malformed/unknown-name/cycle) — same crisp transform-vs-reporting boundary as the parser"
  - "Renamed lighthouse MAX_DEPTH → MAX_PARSE_DEPTH for an unambiguous public const (acceptance grep keys on `MAX_PARSE_DEPTH|depth`)"
  - "Created the fuzz crate myself (Plan 02 owns it but runs in a parallel wave); modelled on the root fuzz/Cargo.toml, standalone [workspace] so it resolves independently"
metrics:
  duration: 12min
  completed: 2026-06-11
  tasks: 3
  files: 13
  tests: 63
---

# Phase 93 Plan 03: Dialect Linter + Formula Parser + DAG Compile Summary

WBDL-03 collect-all dialect linter, the whitelist-AT-PARSE-time Pratt formula parser building the runtime `Expr`, and the Kahn DAG compile (no petgraph) — all lifted from the lighthouse, generalized onto a synthetic `CellSource` seam so they stay parallel with 93-02, with the untrusted formula surface fuzzed behind an in-code recursion-depth limit.

## What Was Built

- **Task 1 — WBDL-03 dialect linter** (`dialect/linter.rs`, `dialect/mod.rs`): a collect-all, cell-addressed linter over a new reader-free `CellSource` trait (`CellView`/`SheetView`/`DefinedName`). It reports macros, external links, out-of-bounds named ranges, hidden sheets/rows, array formulas, and out-of-whitelist functions in ONE pass (D-01, never fail-fast). The whitelist, palette, and layering come from `pmcp_workbook_dialect` (re-exported, never re-declared); findings are the runtime's `LintFinding`/`LintReport`/`Severity`. Colour evidence is surfaced as advisory `Info` only (colour proposes, never decides). 11 tests over a hand-built `TestCells` double.
- **Task 2 — formula parser + DAG** (`formula/{token,parser,rebase,mod}.rs`, `dag/{graph,resolve,topo,mod}.rs`): the Excel tokenizer (digit-leading sheet names, `""`-doubled strings, sheet-qualified ranges with the sheet recorded once, scientific/percent/error lexemes, `MAX_FORMULA_LEN` guard); the Pratt parser that builds the re-exported runtime `Expr`, enforcing the WHITELIST **at parse time** as a typed `ParseError::UnsupportedFunction` (an out-of-whitelist function never reaches the IR), with distinct typed errors for external refs, array braces, and `MAX_PARSE_DEPTH` over-nesting, plus a documented supported-formula matrix at the parser head; `rebase` (within-block-range row templating); and the DAG build reusing the runtime `Dag` + Kahn `toposort` (no petgraph) over a synthetic `ParsedCell` + `DefinedName` slice, with cycles surfaced as a typed `DagBuildError::Cycle` enumerating the cells. 52 tests, including all six mandated behaviors.
- **Task 3 — fuzz target** (`fuzz/fuzz_formula_parser.rs` + standalone `fuzz/Cargo.toml`): a cargo-fuzz target feeding arbitrary UTF-8 bytes into `formula::parse`; invariant is Ok(Expr)-or-typed-error, never panic/hang. Build gate green; smoke run (`-runs=10000`) clean, no crash.

## Verification

- `cargo test -p pmcp-workbook-compiler` → 63 passed, 0 failed (dialect 11, formula 41, dag 11).
- Acceptance grep gates all pass: no re-declared `WHITELIST` in `dialect/`; no `WorkbookMap` in `dialect/`/`formula/`/`dag/`; no local `Expr` in `formula/`; `MAX_PARSE_DEPTH` present; supported-formula matrix documented; no `petgraph` in `Cargo.toml`.
- `cargo clippy -p pmcp-workbook-compiler --all-targets -- -D warnings` → clean.
- `cargo fmt -p pmcp-workbook-compiler -- --check` → clean.
- PMAT cognitive-complexity (CI gate, `--max-cognitive 25`) → 0 violations in the compiler.
- `cargo test -p pmcp-workbook-dialect` → green (the consumed spec-binding WHITELIST drift test still holds).
- `cargo +nightly fuzz build fuzz_formula_parser` → exit 0; `cargo +nightly fuzz run fuzz_formula_parser -- -runs=10000` → no panic/crash.

## Threat Model Outcomes

- **T-93-03-INJ (whitelist bypass)** — mitigated: an out-of-whitelist function is a parse-time `ParseError::UnsupportedFunction`, returned BEFORE the `Expr::Call` node is built (`parse_call` rejects first). Test `whitelist_at_parse_rejects` + the matrix test assert it.
- **T-93-03-DOS (deep nesting / cycle)** — mitigated: `MAX_PARSE_DEPTH` (256) enforced in `parser.rs` returns `ParseError::TooDeep` (test `depth_limit_rejects`); the DAG detects cycles as `DagBuildError::Cycle` (test `cycle_detected`); `MAX_FORMULA_LEN` bounds input; the fuzz target exercises the surface (no crash over 10k runs); crate is `#![deny(clippy::panic)]`.
- **T-93-03-DRIFT (re-declared WHITELIST)** — mitigated: `WHITELIST`/`DialectRules`/`CandidateRole` are re-exported from `pmcp_workbook_dialect`; the grep gate confirms no local copy.

## Deviations from Plan

### Auto-fixed / generalization decisions (Rule 2/3)

**1. [Rule 2 - generalization] Parser returns a typed `ParseError`, not a `LintFinding`.**
- **Found during:** Task 2. The lighthouse parser pushed `whitelist/unsupported-fn` LintFindings into a `LintReport` (collect-all). The plan's Task 2 behavior + Codex MEDIUM explicitly require a crisp parser-error-vs-lint-finding boundary: the parser returns a typed error, the linter owns findings.
- **Fix:** Designed `ParseError` (UnsupportedFunction/ExternalRef/ArrayFormula/TooDeep/Lex/Malformed) and made `parse(...) -> Result<Expr, ParseError>`. The out-of-whitelist function is rejected BEFORE building the node (stronger than the lighthouse's build-then-flag). The DAG build path got the same treatment (`DagBuildError`).
- **Files:** `formula/parser.rs`, `dag/resolve.rs`, `dag/topo.rs`.

**2. [Rule 3 - blocking] Created the compiler `fuzz/` crate (Plan 02 nominally owns it).**
- **Found during:** Task 3. The plan says "reuse the fuzz Cargo.toml from Plan 02", but 93-02 runs in a parallel wave and the dir does not exist in this worktree. Auto-substituting / waiting is not viable for a parallel plan.
- **Fix:** Created `fuzz/Cargo.toml` (standalone `[workspace]`, modelled on the repo's root `fuzz/Cargo.toml` `cargo-fuzz` pattern) + the target + `.gitignore`. When 93-02 merges, if it also adds a fuzz crate the two targets coexist (different bin names: `fuzz_provenance_*` vs `fuzz_formula_parser`); a follow-up may consolidate into one fuzz crate.
- **Files:** `fuzz/Cargo.toml`, `fuzz/fuzz_targets/fuzz_formula_parser.rs`, `fuzz/.gitignore`.

**3. [Rule 1 - type] `DagBuildError::RangeTooLarge.cells` is `u64` (matches the runtime).**
- **Found during:** Task 2 build. The runtime `ResolveError::RangeTooLarge.cells` is `u64`; my initial `usize` mismatched.
- **Fix:** Changed the field to `u64`.

**4. Renamed `MAX_DEPTH` → `MAX_PARSE_DEPTH`** for an unambiguous public const that satisfies the acceptance grep `MAX_PARSE_DEPTH|depth` and reads clearly alongside `MAX_FORMULA_LEN`.

### Synthetic-seam scope note

The linter/parser/DAG consume only the synthetic `CellSource`/`ParsedCell`/`DefinedName` types — never 93-02's owned cell model — exactly as the plan's dependency note requires. The synthetic-input path held; no fallback to `depends_on: ["93-01","93-02"]` was needed. The real model implements `CellSource` in the Plan 04 wiring.

## Known Stubs

None. All three modules are fully implemented; no placeholder values flow to any output. (The `compile_workbook` driver in `lib.rs` remains a Wave-1 stub owned by Plan 07 — out of scope for this plan.)

## Self-Check: PASSED

- created files exist: dialect/linter.rs, formula/{token,parser,rebase}.rs, dag/{graph,resolve,topo}.rs, fuzz/fuzz_targets/fuzz_formula_parser.rs — all present.
- commits exist: 98a29fb3 (Task 1), fb10baaf (Task 2), a8363404 (Task 3) — all in `git log`.
