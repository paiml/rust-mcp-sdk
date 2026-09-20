---
phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
reviewed: 2026-06-15T00:00:00Z
depth: deep
files_reviewed: 12
files_reviewed_list:
  - crates/pmcp-workbook-compiler/src/dialect_version.rs
  - crates/pmcp-workbook-compiler/src/lib.rs
  - crates/pmcp-workbook-compiler/src/fixture_author.rs
  - crates/pmcp-workbook-compiler/src/quirks_reconcile.rs
  - crates/pmcp-workbook-compiler/src/reemit_loan.rs
  - crates/pmcp-workbook-compiler/fuzz/fuzz_targets/dialect_version_parse.rs
  - crates/pmcp-workbook-dialect/src/lib.rs
  - crates/pmcp-workbook-runtime/src/scalar_eval.rs
  - cargo-pmcp/src/templates/workbook_server.rs
  - cargo-pmcp/src/commands/new.rs
  - cargo-pmcp/src/commands/workbook/compile.rs
  - docs/workbook-dialect-spec.md
findings:
  critical: 0
  high: 1
  medium: 1
  low: 3
  total: 5
status: resolved
resolved_at: 2026-06-15
resolution: >
  All 5 findings fixed (HI-01, ME-01, LO-01, LO-02, LO-03). HI-01 wired the
  WBDL-02/D-04 dialect-version fail-closed gate into the gated-update lane via a
  shared validate_dialect_version_step called by both compile lanes, with
  gated-update regression tests. ME-01 added the pmcp-server-toolkit pin
  drift-guard test. LO-01/LO-02 removed the unused clap dep and the gratuitous
  include-dir rename. LO-03 replaced the tautological corpus-count range check
  with an exact assertion. fmt + clippy clean on touched crates;
  cargo test -p pmcp-workbook-compiler (315) and -p cargo-pmcp --lib (436) pass.
---

# Phase 96: Code Review Report

**Reviewed:** 2026-06-15
**Depth:** deep (cross-file: traced the dialect-version gate across both compile
lanes, the `in_*`/`out_*` manifest naming convention against the served-schema
path, and the test-only provenance override across `compile_workbook` vs
`prepare_candidate`)
**Files Reviewed:** 12
**Status:** issues_found (1 HIGH, 1 MEDIUM)

## Summary

This is a strong, defensively-engineered phase. The dialect-version parser is
fail-closed, total over hostile bytes (fuzz target + overflow handling verified),
and bound to the spec doc by a drift guard. The quirk reconcile harness genuinely
grades recomputed values through the real `within_tol` penny path (it cannot pass
on compile-success alone — proven by `a_wrong_oracle_does_not_reconcile...`), and
the scalar_eval layer-1 tests assert against the `excel_round` source of truth, not
literals. The `#[cfg(test)]` provenance-override is well-isolated: every fixture
plan ships a production-refusal counter-test asserting bare `compile_workbook`
(Enforce) still rejects authored fixtures, so there is no production bypass. The
scaffold purity posture (`default-features = false`, `workbook-embedded` + `http`,
never `code-mode`) is enforced by emitter-exercising tests and the `in_*`/`out_*`
named-input convention correctly mirrors `promote_named_outputs` (naming only,
never re-roling; tax-calc has no `in_*` names so it cannot regress).

The one significant defect is a **fail-closed gap**: the WBDL-02 dialect-version
compatibility check is wired into `compile_workbook_inner` (the SEED lane) but NOT
into `prepare_candidate_inner` (the GATED-UPDATE lane). Because `cargo pmcp
workbook compile` routes every re-compile of an existing workbook through
`prepare_candidate`, an author who bumps `pmcp_dialect_version` to an incompatible
value on an already-seeded workbook is silently accepted instead of being refused
— directly contradicting D-04 on a production path.

## Narrative Findings (AI reviewer)

### HIGH

#### HI-01: Dialect-version compatibility check is absent from the gated-update lane (D-04 fail-closed gap on a production path)

**File:** `crates/pmcp-workbook-compiler/src/lib.rs:303` (present) vs `:730-801`
(`prepare_candidate_inner`, absent); reached via
`cargo-pmcp/src/commands/workbook/compile.rs:314` (`run_gated_lane`).

**Issue:** `resolve_dialect_version(&map)?` is called only inside
`compile_workbook_inner` at step (2a). `prepare_candidate_inner` — the body behind
the public `prepare_candidate`, which `cargo pmcp workbook compile` calls on the
GATED-UPDATE lane (every re-compile of a workbook that already has a prior
baseline) — never resolves or validates the dialect version. The flow:

1. A workbook is first compiled at `pmcp_dialect_version = 1.0` via the SEED lane
   (`compile_workbook`) → the check runs, passes.
2. The author later edits the cell to an incompatible value (`2.0`, or a
   newer-than-supported minor like `1.5`) and re-runs `cargo pmcp workbook
   compile`. A prior baseline now exists, so `run_gated_lane` →
   `prepare_candidate` → `prepare_candidate_inner` runs — and never validates the
   declaration.

The result is that an incompatible dialect declaration is accepted on a
production path, contradicting D-04 ("a different major OR a declared version newer
than the compiler supports → a hard, typed compile error"). The milestone's
fail-closed ethos is satisfied on first compile but silently bypassed on every
subsequent governed re-compile — the exact path most likely to encounter a version
bump. 96-04-SUMMARY explicitly wired `name_named_inputs` into BOTH lanes; the
dialect-version check (added earlier, in 96-01, before `prepare_candidate` was in
view) was not given the same treatment, and this was not recorded as a deviation.

**Fix:** Add the same step-(2a) call to `prepare_candidate_inner`, mirroring
`compile_workbook_inner`, so both lanes share the fail-closed gate:

```rust
// in prepare_candidate_inner, after `let (map, ingest_findings) = ingest::ingest(...)?;`
// and before stage1 (matching compile_workbook_inner step 2a):
let _dialect_version = dialect_version::resolve_dialect_version(&map)?;
```

Then add a gated-lane integration test asserting an incompatible
`pmcp_dialect_version` is refused through `prepare_candidate` (the current
`wired_path_integration` module only drives `resolve_dialect_version` directly and
`reemit_*` only exercise the SEED/`compile_workbook` path). Consider extracting a
single `validate_dialect_version_step(&map)` helper called by both `_inner`
functions so the two paths cannot drift again.

### MEDIUM

#### ME-01: Scaffold pins `pmcp-server-toolkit = "0.1.0"` with no drift-guard test (silent breakage when the toolkit bumps)

**File:** `cargo-pmcp/src/templates/workbook_server.rs:88` (the hardcoded
`version = "0.1.0"` in the emitted `Cargo.toml`); contrast with `:53` + the
`emitted_pmcp_version_matches_workspace_pin` test at `:465-482`.

**Issue:** The emitted `Cargo.toml` hardcodes `pmcp-server-toolkit = { version =
"0.1.0", ... }`. The `pmcp` pin (`PMCP_VERSION = "2.9.0"`) is protected by a
drift-guard test that parses the workspace-root `Cargo.toml` and fails the build
if they diverge — but there is NO equivalent guard for the `pmcp-server-toolkit`
pin. `pmcp-server-toolkit` is currently `0.1.0` (verified), so the pin is correct
today, but per the release workflow it is publish-item 5 and a strong candidate to
bump in the next release. When it does, the scaffold will emit a stale `0.1.0`
pin, and a `cargo pmcp new --kind workbook-server` user will get a crate that
either fails to resolve (if 0.1.0 is yanked/superseded with an incompatible API)
or silently builds against an old toolkit. The `pmcp_version` drift guard exists
precisely because hardcoded scaffold pins drift; the toolkit pin has the same
exposure and no guard.

**Fix:** Hoist the toolkit version to a `const TOOLKIT_VERSION: &str` and add a
drift-guard test mirroring `emitted_pmcp_version_matches_workspace_pin`, parsing
`crates/pmcp-server-toolkit/Cargo.toml`'s `[package] version`:

```rust
const TOOLKIT_VERSION: &str = "0.1.0";

#[test]
fn emitted_toolkit_version_matches_workspace_pin() {
    const TOOLKIT_CARGO_TOML: &str =
        include_str!("../../../crates/pmcp-server-toolkit/Cargo.toml");
    let parsed: toml::Value = toml::from_str(TOOLKIT_CARGO_TOML).expect("parse toolkit Cargo.toml");
    let v = parsed["package"]["version"].as_str().expect("toolkit version");
    assert_eq!(TOOLKIT_VERSION, v,
        "scaffold's toolkit pin drifted from the workspace pin — bump TOOLKIT_VERSION");
}
```

### LOW

#### LO-01: Emitted scaffold `Cargo.toml` declares `clap` but the emitted `main.rs` never uses it

**File:** `cargo-pmcp/src/templates/workbook_server.rs:90` (declares `clap`) vs
`:107-153` (`emitted_main_rs`, no `clap` usage).

**Issue:** The emitted `Cargo.toml` lists `clap = { version = "4", features =
["derive"] }`, but the canonical `main.rs` wiring dropped the `--bundle-dir`
harness branch (that was the only `clap` consumer in the source example — see the
`wiring_lines` normalization at `:264-285`). The scaffold therefore ships an unused
dependency: a published-crate user gets an extra compile-time dependency (clap +
its proc-macro tree) for nothing. It does not break the build (Rust does not error
on unused deps), but it is dead config that contradicts the scaffold's
"dependency-light, purity-conscious" intent and slows the user's first `cargo
build`.

**Fix:** Remove the `clap` line from `generate_cargo_toml`'s emitted content (the
emitted `main.rs` parses no args). If a future scaffold revision re-adds a
`--bundle-dir`-style flag, re-add `clap` together with its consumer.

#### LO-02: `include-dir`/`include_dir` package-rename inconsistency between scaffold's own Cargo.toml and the emitted Cargo.toml

**File:** `cargo-pmcp/Cargo.toml` (`include_dir = "0.7.4"`) and
`cargo-pmcp/src/templates/workbook_server.rs:89` (`include-dir = { version =
"0.7.4", package = "include_dir" }`).

**Issue:** cargo-pmcp's own manifest depends on the crate under its real name
`include_dir`. The emitted scaffold `Cargo.toml`, however, declares it as
`include-dir = { ..., package = "include_dir" }` — i.e. it renames the dependency
key to `include-dir` (hyphen) while pointing at the `include_dir` package. The
emitted `main.rs` then does `use include_dir::{include_dir, Dir};`, which resolves
via the package rename, so it compiles. But the renamed key is gratuitous and
mildly confusing: there is no reason to alias `include_dir` to `include-dir` in a
freshly-generated crate, and it diverges from how cargo-pmcp itself declares the
same dependency. The `#[ignore]`d `scaffold_crate_cargo_check_compiles` smoke
would catch a true break, but it is env-gated and not run in the default gate, so
this is unverified in normal CI.

**Fix:** Emit the dependency under its real name to match the import and the parent
manifest:

```toml
include_dir = "0.7.4"
```

#### LO-03: `each_named_reconcilable_quirk_has_a_reconcile_assertion` corpus-count assertion is effectively tautological

**File:** `crates/pmcp-workbook-compiler/src/quirks_reconcile.rs:336-341`.

**Issue:** The cross-check `assert!((5..=9).contains(&cases.len().saturating_add(1)))`
is intended to keep the corpus within the D-09 ~7-9 cap, but `quirk_cases()`
returns a hardcoded 5-element `vec!`, so `cases.len()` is a compile-time-fixed 5
and the assertion can only ever read `6`, which is always in `5..=9`. It therefore
asserts nothing that the literal vector does not already fix, and would not catch a
future drift in EITHER direction that stays a hardcoded length (e.g. someone adds
two cases making 7 → still passes; someone deletes down to 3 → `3+1=4`, would fail,
so it has weak lower-bound value only). The comment claims it tracks "8 quirks
across both layers" but the assertion only sees the reconcile-fixture count.

**Fix:** Either drop the assertion (the D-09 cap is a planning constraint, not a
runtime invariant) or make it meaningful by deriving the count from a single
source of truth shared with the scalar_eval layer (e.g. assert
`quirk_cases().len() == 5` exactly, with a comment pointing at the traceability
table, so adding/removing a reconcile fixture forces a deliberate edit here).

---

## Verification notes (cleared on inspection — no finding)

- **Dialect-version parser correctness (focus item):** `parse_dialect_version`
  correctly rejects empty, wrong-arity, non-digit, embedded-whitespace, and
  `u64`-overflow components via a typed `CompileError::Lint`, never a panic
  (`parse_component` checks `is_ascii_digit` before `parse::<u64>`, and maps the
  overflow `Err` to `malformed`). `is_compatible_with` uses `major == &&
  minor <=` — no off-by-one (the grid property test at `:494-510` exhaustively
  confirms the rule). Absent → baseline, present-incompatible → typed error: both
  correct in `resolve_dialect_version`. Fuzz target covers hostile bytes.
- **`in_*` named-input convention (focus item):** `name_named_inputs`
  (`lib.rs:632-651`) only sets `name` on cells ALREADY `Role::Input` (`c.role ==
  Role::Input` in the `find`), single-cell targets only — it never re-roles, and
  mirrors `promote_named_outputs` exactly. tax-calc declares no `in_*` names, so it
  cannot regress (witnessed by reemit_golden still passing per 96-04-SUMMARY).
- **Reconcile harness genuinely grades values (focus item):**
  `recompute_at_reconcile_key` loads the emitted bundle, seeds inputs, runs the
  real executor, retrieves the computed `CellValue`, and compares to the oracle via
  `within_tol`. `a_wrong_oracle_does_not_reconcile_proving_the_value_is_graded`
  (`:233-243`) proves it cannot pass on compile-success alone. No exact-float `==`
  on money anywhere in the harness.
- **No production bypass via the test override (focus item):**
  `compile_workbook_with_fixture_override` is `#[cfg(test)]`-only and
  `compile_workbook` always passes `FreshnessPolicy::Enforce`. Every fixture plan
  ships a production-refusal counter-test (`production_compile_refuses_*`) asserting
  bare `compile_workbook` rejects the authored bytes. (Note: HI-01 is a SEPARATE
  gate gap on the dialect-version check, not a provenance bypass — the provenance
  override itself is sound.)
- **Purity (focus item):** the emitted `Cargo.toml` is exercised through the real
  emitter by `emitted_cargo_toml_is_purity_safe` + the integration test, both
  asserting `default-features = false`, `["workbook-embedded", "http"]`, and NO
  `code-mode`. No reader deps leak into the served tree.
- **Path-traversal (focus item):** `execute_workbook_server` calls
  `validate_crate_name(name)?` FIRST, before any `fs::create_dir_all`/`fs::write`;
  `scaffold_rejects_path_traversal_name` confirms `../evil` is rejected.
- **Synthetic fixtures (focus item):** the loan fixture is toy data (rate table
  0.08/0.06/0.045, loan_amount 240000, credit_score 700); quirk fixtures are
  arithmetic edge cases. No customer/TowelRads material observed.
- **`#[ignore]` + env-gated regeneration:** `regenerate_fixtures` no-ops without
  `PMCP_REGEN_FIXTURES`, so a normal `cargo test` never mutates `tests/fixtures/`.

---

_Reviewed: 2026-06-15_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
