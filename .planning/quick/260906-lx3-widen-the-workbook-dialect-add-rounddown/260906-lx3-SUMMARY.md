---
quick_id: 260906-lx3
plan: 01
status: complete
subsystem: workbook-dialect
tags: [workbook, dialect, excel, parser, evaluator, bugfix]
commits: 3
plan_head_before: 6bd7e0e492b084075b9b872ae8d26badfaffe1f6
completed: 2026-09-06
requires:
  - pmcp-workbook-runtime sheet_ir semantics layer
  - pmcp-workbook-dialect WHITELIST + version contract
  - pmcp-workbook-compiler formula parser
provides:
  - 17-name dialect WHITELIST bound to the published spec
  - dialect version 1.1 (supported), baseline unchanged at 1.0
  - ROUNDDOWN / MAX / MIN / XLOOKUP evaluator bodies
  - ParseError::UnsupportedCallShape — a located, BA-actionable shape refusal
  - enforced EXACT-match contract for VLOOKUP / MATCH at two layers
affects:
  - every governed workbook authoring a bare VLOOKUP/MATCH (now refused, was silently wrong)
tech-stack:
  added: []
  patterns:
    - two-layer refusal (parse-time located repair + eval-time typed ExcelError)
    - static checks fire only on measurable shapes; unmeasurable shapes defer to the backstop
key-files:
  created:
    - crates/pmcp-workbook-compiler/examples/dialect_widening_demo.rs
  modified:
    - crates/pmcp-workbook-runtime/src/sheet_ir/rounding.rs
    - crates/pmcp-workbook-runtime/src/sheet_ir/semantics.rs
    - crates/pmcp-workbook-runtime/src/sheet_ir/mod.rs
    - crates/pmcp-workbook-runtime/src/sheet_ir/eval_value.rs
    - crates/pmcp-workbook-dialect/src/lib.rs
    - crates/pmcp-workbook-compiler/src/formula/parser.rs
    - crates/pmcp-workbook-compiler/src/fixture_author.rs
    - docs/workbook-dialect-spec.md
    - CHANGELOG.md
decisions:
  - D-Q1 option (a) held — the exact-match argument is REQUIRED; measured blast radius was zero
  - D-Q2 the refusal lives at parse time; the evaluator keeps an independent backstop
  - D-Q3 dialect 1.0 -> 1.1, baseline stays 1.0; reasoning written into spec §7.6
  - D-Q4 XLOOKUP accepted only in the narrow 3/4-arg exact form
  - D-Q5 no [package].version touched; CHANGELOG states the coordinated bump owed
metrics:
  duration: ~55m
  tasks: 3
  commits: 3
  tests_added: 38
---

# Quick Task 260906-lx3: Widen the workbook dialect (ROUNDDOWN/MAX/MIN/XLOOKUP) Summary

The constrained Excel dialect went from 13 to 17 first-class functions and, in the
same change, stopped returning a silently wrong number for approximate
`VLOOKUP`/`MATCH` — a bug whose exact-match contract had been *published* in the
evaluator's own rustdoc since the dialect shipped but never *checked*.

## Commits

| # | Hash | What |
|---|------|------|
| 1 | `9e3398ff` | Runtime evaluators: `excel_rounddown`, `f_rounddown`, `f_max`/`f_min`, `f_xlookup`, and the `require_exact_match_flag` backstop |
| 2 | `8f563a33` | Dialect contract: 17-name `WHITELIST`, `SUPPORTED_DIALECT_VERSION` 1.1, spec §3 table + §3.1/§3.2/§7.5/§7.6 |
| 3 | `c0d84d14` | Parse-time `validate_call_shape` gate, `dialect_widening_demo` example, `## [Unreleased]` CHANGELOG entry |

Measured, not narrated: `git rev-list --count 6bd7e0e4..HEAD` = **3**.

## The bug, and how it is now closed

`semantics.rs` documented "EXACT match only (we require `FALSE`/0 for
`range_lookup`)" and the mirrored claim for `MATCH`'s `match_type` — and then read
only args 0/1/2 and 0/1 respectively. Excel's **default** for both omitted
arguments is **APPROXIMATE**, so `MATCH(x, rng)`, `MATCH(x, rng, 1)`,
`VLOOKUP(x, tbl, 2)` and `VLOOKUP(x, tbl, 2, TRUE)` all evaluated as EXACT and
returned a wrong number with no `#VALUE!`, no lint finding, no signal at all.

Closed at **two independent layers**:

- **Parse time (BA-facing).** `validate_call_shape` runs inside `parse_call`,
  after the whitelist gate and before the `Expr::Call` node is built, so a refused
  shape never reaches the IR. `compile()` wraps the result as
  `CompileError::Lint("parse {sheet}!{addr}: {e}")`
  (`crates/pmcp-workbook-compiler/src/lib.rs:625-626`), so every refusal is
  cell-located. Verified end-to-end by the example's printed output.
- **Eval time (defense in depth).** `require_exact_match_flag` at the top of both
  `f_vlookup` and `f_match` returns `ExcelError::Value` for a missing argument, a
  range, or any value other than `FALSE`/`0`.

Both rustdocs were rewritten: the previous wording asserted a contract nothing
enforced, and that wording is precisely what made the bug invisible to readers.

## The BA-facing contract — the six refusal `Display` strings, verbatim

Captured from an actual run of
`cargo run -p pmcp-workbook-compiler --example dialect_widening_demo`, with the
`parse {sheet}!{addr}: ` prefix the compiler adds:

```text
parse 1_Inputs!B7: call to `MATCH` has an unsupported shape: write MATCH(key, range, 0) — the dialect supports EXACT match only; Excel's default is approximate
parse 1_Inputs!B7: call to `VLOOKUP` has an unsupported shape: write VLOOKUP(key, table, col, FALSE) — the dialect supports EXACT match only
parse 1_Inputs!B7: call to `XLOOKUP` has an unsupported shape: write XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found]) — match_mode and search_mode are not supported; the dialect is exact-match only
parse 1_Inputs!B7: call to `XLOOKUP` has an unsupported shape: return_array must be a SINGLE column — write XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found]) with a one-column return_array
parse 1_Inputs!B7: call to `XLOOKUP` has an unsupported shape: lookup_array and return_array must cover the SAME number of cells — write XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found]) with conformable arrays
```

(The first line covers both `MATCH(B2,D2:D4)` and `MATCH(B2,D2:D4,1)` — the bare
and approximate forms render identically by design, because the repair is the
same. `VLOOKUP(B2,D2:E4,2,TRUE)` and `…,1)` likewise share the VLOOKUP line.)

These strings are asserted by `shape_errors_render_a_named_actionable_repair`, so
they are a tested contract rather than incidental output.

## Fixture impact: ZERO edits, as D-Q1 predicted

**No authored fixture needed an edit.** `git diff --name-only 6bd7e0e4..HEAD`
touches no file under `crates/pmcp-workbook-compiler/tests/fixtures/`, and the one
change to `fixture_author.rs` is a single doc-comment word (`13-fn` → `17-fn`).
`cargo test -p pmcp-workbook-compiler` passes at **397 executed tests**.

This is the measurement that validates the option (a)/(b) choice: had any fixture
required editing, D-Q1's premise would have been falsified and the choice would
have needed revisiting before proceeding. It did not.

## Fuzz campaign — RUN, and clean

Per the plan's no-new-target decision (the new parse-time gate lands inside the
already-fuzzed `formula::parse` surface, so it gains coverage for free):

```
cargo +nightly fuzz run --fuzz-dir crates/pmcp-workbook-compiler/fuzz \
    fuzz_formula_parser -- -runs=20000
```

- **exit code: 0**
- **artifact count: 0** (`crates/pmcp-workbook-compiler/fuzz/artifacts/fuzz_formula_parser/` is empty)
- 20000 runs in ~1s; final `cov: 571 ft: 1323 corp: 376/1737b`; no crash, no hang,
  no timeout.

Both `cargo-fuzz` and the nightly toolchain were present, so this is a real
campaign result, not a NOT-RUN.

## ALWAYS requirements (CLAUDE.md)

| Requirement | Status |
|---|---|
| UNIT | **38** new test functions, **0** removed — measured by `git diff 6bd7e0e4..HEAD \| grep -c '^+ *#\[test\]'` (and the `^-` counterpart). 27 in the runtime (rounding + evaluator), 11 in the parser |
| PROPERTY | 3 of those 38 are `proptest!` invariants: rounding magnitude never grows, `rounddown ≤ roundup` in magnitude, and MAX/MIN bound every range member with `MIN ≤ MAX` |
| FUZZ | existing `fuzz_formula_parser` target run at 20000 runs, exit 0, 0 artifacts |
| EXAMPLE | `dialect_widening_demo` — runs, exits 0, prints all six refusals with their located repairs |
| Zero SATD | verified: no TODO/FIXME/HACK in any touched tree |

## Verification results

| Check | Result |
|---|---|
| `cargo test -p pmcp-workbook-runtime` | exit 0, **220** executed |
| `cargo test -p pmcp-workbook-dialect` | exit 0, **6** executed |
| `cargo test -p pmcp-workbook-compiler` | exit 0, **397** executed |
| `doc_whitelist_table_matches_const` | `ok` (asserted by name) |
| `doc_versions_match_consts` | `ok` (asserted by name) |
| `every_whitelisted_function_dispatches_in_the_runtime` | `ok` (asserted by name) |
| Both examples run | exit 0 each |
| `make purity-check` | exit 0 |
| `make quality-gate` | exit 0 |
| No stale `13` count claim | verified — every surviving `13` is an enumerated must-NOT-change hit (`T-09-13`, `T-96-13`, `T-13-19`, `1e-13`, Phase 13, D-13/D-15) |
| No binding test weakened | verified — the dialect diff removes only count literals, the version const and doc prose; no assertion dropped, no `#[ignore]`, no loosened comparison |

Every executed-test count above is nonzero, as the plan requires — a filter that
selects zero tests exits 0, so the count is the load-bearing assertion.

## Deviations from plan

### 1. [Rule 3 — Blocking] The plan's `--exact-match` verify flag is not valid libtest

- **Found during:** Task 1 verification.
- **Issue:** the plan's second Task-1 `<automated>` block runs
  `cargo test -p pmcp-workbook-runtime rounddown -- --exact-match`. libtest has no
  such option; it is `--exact`. Run literally, the command fails with
  `error: Unrecognized option: 'exact-match'` (measured).
- **Fix:** dropped the invalid flag and ran
  `cargo test -p pmcp-workbook-runtime rounddown`, keeping the load-bearing part —
  the `grep -c` assertion that a nonzero number of `rounddown` tests actually
  reported. Result: **7** tests selected and passing.
- **Note:** this is the same family as the recorded `nextest test()/binary()`
  selector trap — a verify command that cannot run is not a passing verify.

### 2. [Rule 1 — Bug in the plan's stated property] The rounding proptest bound as written is falsifiable

- **Found during:** Task 1, writing the property tests.
- **Issue:** the plan specifies `excel_rounddown(x, d).abs() <= x.abs() + f64::EPSILON`
  for any finite `x`. `ROUND_EPSILON` is a **relative** (1e-9) nudge pushed away
  from zero before truncation — the same design the plan itself prescribes — so
  the result can legitimately exceed `|x|` by up to `|x| * 1e-9`, which is many
  orders of magnitude larger than `f64::EPSILON`. The stated property fails on
  ordinary inputs. The companion property (`rounddown ≤ roundup` in magnitude)
  additionally fails once `|x * 10^digits|` approaches `1/ROUND_EPSILON`, because
  the relative nudge then exceeds a whole unit at the truncation boundary, and
  fails again past 2^53 where `trunc`/`ceil` are the identity.
- **Fix:** implemented the properties with a correct tolerance
  (`x.abs() * (1.0 + 1e-8) + f64::EPSILON`) and generation bounds
  (`|x| <= 1000`, `|digits| <= 5`, keeping `|scaled| <= 1e8`), with a comment in
  the test module explaining exactly why the bounds exist so a later reader does
  not "widen" them back into failure.
- **Files:** `crates/pmcp-workbook-runtime/src/sheet_ir/rounding.rs`
- **Commit:** `9e3398ff`

### 3. [Scope boundary] `cargo clippy --all-targets -- -D warnings` was NOT clean at base

- **Found during:** Task 1 verification, and again at Task 3.
- **Issue:** the plan's Task-1 and Task-3 `<done>` blocks assert this command "is
  clean". It is not, and was not before this task began. Three findings survive,
  all in files this task never touched:
  `manifest_model.rs:941` (`map_or` simplification, inside a `#[cfg(test)]` block)
  and `render/mod.rs:420` + `:511` (`too_many_arguments`, 8/7, both private fns).
  A fourth, `pmcp-server-toolkit/src/workbook/render_resource.rs:43`
  (unused import), was present in the very first baseline build of this session.
- **Action:** NOT fixed — logged to `deferred-items.md` beside this summary.
  Refactoring the xlsx render writer inside a dialect-widening commit is the scope
  creep the executor's scope boundary forbids. The plan itself records why this is
  not a CI blocker: `make lint` (`Makefile:216-221`) resolves to the root `pmcp`
  package only, so no CI job lints these crates.
- **What was asserted instead:** clippy over all three workbook crates reports
  **zero** findings in any file this task modified (measured by filtering the
  output to the touched paths, and separately by re-running with only the three
  pre-existing lints allowed, which yields a clean compiler-crate lint).

**Honest statement:** verify command 4 of Task 1 and command 4 of Task 3 do **not**
exit 0 verbatim. They fail solely on pre-existing findings in untouched files. The
mandatory `make quality-gate` — which is the project's actual gate — exits 0.

### 4. [Addition] The example covers all six refusal shapes, not four

The plan asked the example to print four refusals. While capturing the verbatim
`Display` strings the summary is required to record, the two statically-knowable
`XLOOKUP` array refusals (multi-column `return_array`, non-conformable arrays)
were added, plus a `(d)` section demonstrating that an unmeasurable shape
(`XLOOKUP(B2,named_keys,named_values)`) is **not** guessed at and passes the static
gate to the evaluator's backstop. Folded into commit `c0d84d14` by amend, so the
task stays one atomic commit.

## Release obligation the release owner still has

**No `[package].version` was touched** — verified against the manifests:
`pmcp-workbook-dialect` is still `0.1.1`, `pmcp-workbook-runtime` still `0.2.0`,
`pmcp-workbook-compiler` still `0.1.1`.

At release time these three need a **coordinated 0.x MINOR bump**. A `0.x` minor
bump is semver-INCOMPATIBLE, so `pmcp-workbook-compiler`'s
`pmcp-workbook-dialect = "0.1.0"` pin
(`crates/pmcp-workbook-compiler/Cargo.toml:45`) must move in the **same commit**
under `CLAUDE.md` item 13's move-as-one-set rule. Doing half of a coordinated set
move is exactly the failure that rule exists to prevent. This is stated in the
`## [Unreleased]` CHANGELOG entry as well, so it is visible to whoever cuts the
release rather than only here.

The DIALECT version (1.0 → 1.1) is a separate contract from crate semver and **was**
bumped here.

## Known Stubs

None. Every function added dispatches to a real body; no placeholder, no
hardcoded empty value, no deferred data wiring.

## Self-Check: PASSED

- All 7 created/modified files verified present on disk.
- All 3 commit hashes verified present in `git log` (`9e3398ff`, `8f563a33`, `c0d84d14`).
- `commits: 3` is MEASURED (`git rev-list --count 6bd7e0e4..HEAD`), not narrated.
- `tests_added: 38` is MEASURED from the diff, not narrated. An earlier draft of
  this summary said 30; the measurement corrected it.
- `make quality-gate` re-run on the final committed tree: **exit 0**.

## Threat Flags

None. No new network endpoint, auth path, file-access pattern or trust-boundary
schema change. The one security-relevant surface touched — the parse-time
whitelist gate (T-93-03-INJ) — was **narrowed**, not widened: the new shape gate
runs after it and never bypasses it, proven by
`the_whitelist_gate_is_not_shadowed_by_the_shape_gate`, which asserts `OFFSET`
still returns `UnsupportedFunction`.
