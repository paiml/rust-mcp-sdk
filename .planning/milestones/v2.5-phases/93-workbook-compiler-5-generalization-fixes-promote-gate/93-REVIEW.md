---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
reviewed: 2026-06-12T00:00:00Z
depth: standard
files_reviewed: 38
files_reviewed_list:
  - crates/pmcp-workbook-compiler/Cargo.toml
  - crates/pmcp-workbook-compiler/examples/compile_a_workbook.rs
  - crates/pmcp-workbook-compiler/fuzz/fuzz_targets/fuzz_formula_parser.rs
  - crates/pmcp-workbook-compiler/fuzz/fuzz_targets/fuzz_provenance_reader.rs
  - crates/pmcp-workbook-compiler/src/artifact/bundle_lock.rs
  - crates/pmcp-workbook-compiler/src/artifact/cell_map.rs
  - crates/pmcp-workbook-compiler/src/artifact/executable.rs
  - crates/pmcp-workbook-compiler/src/artifact/mod.rs
  - crates/pmcp-workbook-compiler/src/artifact/serialize.rs
  - crates/pmcp-workbook-compiler/src/change_class/ir_identity.rs
  - crates/pmcp-workbook-compiler/src/change_class/mod.rs
  - crates/pmcp-workbook-compiler/src/change_class/schema_diff.rs
  - crates/pmcp-workbook-compiler/src/dag/resolve.rs
  - crates/pmcp-workbook-compiler/src/dag/topo.rs
  - crates/pmcp-workbook-compiler/src/dialect/linter.rs
  - crates/pmcp-workbook-compiler/src/formula/mod.rs
  - crates/pmcp-workbook-compiler/src/formula/parser.rs
  - crates/pmcp-workbook-compiler/src/formula/rebase.rs
  - crates/pmcp-workbook-compiler/src/formula/token.rs
  - crates/pmcp-workbook-compiler/src/gate/accept.rs
  - crates/pmcp-workbook-compiler/src/gate/corpus.rs
  - crates/pmcp-workbook-compiler/src/gate/governed_artifact.rs
  - crates/pmcp-workbook-compiler/src/gate/mod.rs
  - crates/pmcp-workbook-compiler/src/ingest/cell_map.rs
  - crates/pmcp-workbook-compiler/src/ingest/mod.rs
  - crates/pmcp-workbook-compiler/src/lib.rs
  - crates/pmcp-workbook-compiler/src/manifest/projections.rs
  - crates/pmcp-workbook-compiler/src/manifest/ratify.rs
  - crates/pmcp-workbook-compiler/src/manifest/synth.rs
  - crates/pmcp-workbook-compiler/src/provenance/gate.rs
  - crates/pmcp-workbook-compiler/src/provenance/mod.rs
  - crates/pmcp-workbook-compiler/src/provenance/raw_parts.rs
  - crates/pmcp-workbook-compiler/src/provenance/region_hash.rs
  - crates/pmcp-workbook-compiler/src/reconcile/classifier.rs
  - crates/pmcp-workbook-compiler/src/reconcile/drift.rs
  - crates/pmcp-workbook-compiler/src/reconcile/mod.rs
  - crates/pmcp-workbook-compiler/src/sheet_ir/eval_bridge.rs
  - crates/pmcp-workbook-compiler/src/stage1.rs
findings:
  critical: 1
  warning: 6
  info: 4
  total: 11
status: issues_found
---

# Phase 93: Code Review Report

**Reviewed:** 2026-06-12T00:00:00Z
**Depth:** standard
**Files Reviewed:** 38
**Status:** issues_found

## Summary

The offline Excel→MCP compiler is a security-conscious, well-documented crate. The
quarantine boundaries (umya in `ingest`/`provenance` only; owned types at every seam),
the zip-bomb / XML-depth / cell-count / formula-length DoS guards, the whitelist-at-parse
function gate, the deterministic JSON choke point, the non-overwriting versioned promote,
and the fingerprint-bound approvals are all present and largely sound. Panic-freedom is
enforced by the crate-level deny gate and the value paths I traced return typed errors
rather than panicking.

The defects that matter cluster on the **provenance refusal logic** (the untrusted-input
boundary the prompt flagged): the umya-fabrication detection is effectively neutralized by
its own false-positive policy, and the `trusted-fixture` escape hatch is a publishable
Cargo feature rather than a test-only gate, which turns a documented "test-only" bypass
into an externally enableable one. The remaining findings are robustness/quality issues
(seed-map override semantics, error-class swallowing, a redundant promote precheck).

No structural-findings substrate was supplied with this review, so all findings below are
narrative.

## Critical Issues

### CR-01: `trusted-fixture` is a PUBLISHED Cargo feature that disables provenance refusal

**File:** `crates/pmcp-workbook-compiler/Cargo.toml:34-35`, `crates/pmcp-workbook-compiler/src/lib.rs:302-318`, `crates/pmcp-workbook-compiler/src/provenance/gate.rs:243-251`, `crates/pmcp-workbook-compiler/src/stage1.rs:191-204`
**Issue:** The umya-fabrication / non-Excel / unknown-stale refusal is the core security
property of the provenance boundary (WBCO-07). It is bypassed by
`compile_workbook_with_fixture_override` → `gate_with_fixture_override`, which demotes the
`oracle/non-excel-app` / `oracle/stale-cache` / `oracle/no-recalc` / `oracle/missing-cache`
findings from `Error` to `Warning` (`SOFTENABLE_FRESHNESS_RULES`, gate.rs:71-76, applied at
gate.rs:465-471). That override path is gated on `#[cfg(any(test, feature = "trusted-fixture"))]`.

`trusted-fixture` is a real, publishable Cargo feature (`[features] trusted-fixture = []`),
not a `cfg(test)`-only or `cfg(debug_assertions)` gate. Any downstream consumer (or any
build that turns it on, e.g. via feature unification in a workspace) can call
`compile_workbook_with_fixture_override` on attacker-supplied bytes and have a
umya-fabricated / non-Excel workbook accepted and emitted as a governed bundle. The repeated
doc claim "cannot weaken production refusal" holds ONLY under the unenforced convention that
the feature is never enabled — Cargo features are additive and globally unifiable, so a
single dependency enabling `pmcp-workbook-compiler/trusted-fixture` silently arms the bypass
for the whole build graph. The provenance gate is exactly the untrusted-input boundary the
phase is supposed to harden, so an externally-flippable disable switch is a Critical gap.
**Fix:** Gate the override on a compile configuration that cannot be turned on by a
downstream consumer — e.g. `#[cfg(test)]` alone, or a `RUSTFLAGS`-only `--cfg` that is not a
Cargo feature:
```rust
// In Cargo.toml: REMOVE the publishable `trusted-fixture` feature.
// In lib.rs / gate.rs / stage1.rs, gate the override entry on a non-feature cfg the
// build system sets explicitly for the proof harness, never via feature unification:
#[cfg(any(test, wbc_trusted_fixture))]   // set only by `RUSTFLAGS=--cfg wbc_trusted_fixture`
```
If the example/proof genuinely need it outside `cfg(test)`, move them into the test harness
(`tests/`) where `cfg(test)` applies, rather than exposing a feature on the published crate.
At minimum, exclude `oracle/non-excel-app` from `SOFTENABLE_FRESHNESS_RULES` so the override
can relax *staleness* (the documented fixture need) without ever relaxing the
*fabricated-identity* refusal:
```rust
const SOFTENABLE_FRESHNESS_RULES: &[&str] = &[
    "oracle/no-recalc",
    "oracle/stale-cache",
    "oracle/missing-cache",
    // "oracle/non-excel-app" REMOVED — a fabricated identity must hard-refuse even for a fixture.
];
```

## Warnings

### WR-01: `classify` neutralizes its own umya-sentinel detection; `is_sentinel_calc_id` is dead in the accept path

**File:** `crates/pmcp-workbook-compiler/src/provenance/gate.rs:128-146`
**Issue:** The classifier computes `is_sentinel_calc_id` then branches:
```rust
if has_app_version && !is_sentinel_calc_id {
    ProvenanceClass::ExcelTrusted
} else if has_app_version && is_sentinel_calc_id {
    ProvenanceClass::ExcelTrusted    // same result as the branch above
} else { ProvenanceClass::UmyaFabricated }
```
Both `has_app_version` branches return `ExcelTrusted`, so the whole thing reduces to
`if has_app_version { ExcelTrusted } else { UmyaFabricated }`. `is_sentinel_calc_id` never
changes the outcome — the calcId == 122211 fingerprint is checked but discarded. The
documented "false-positive policy" intends exactly this (a present AppVersion admits even a
sentinel calcId), but that means the only thing standing between an attacker and
`ExcelTrusted` is presence of ANY non-empty `<AppVersion>` string — a value the attacker
fully controls in the original bytes. The umya-fabrication defense therefore rests entirely
on the assumption that umya writes no AppVersion; a fabricator who adds
`<AppVersion>16.0</AppVersion>` to a hand-built `docProps/app.xml` passes. This weakens the
WBCO-07 guarantee well below what the module docs advertise.
**Fix:** Either (a) make the sentinel meaningful — refuse `Microsoft Excel` + sentinel calcId
even with an AppVersion, accepting the documented (rare) real-Excel false-positive risk; or
(b) if the false-positive policy is truly intended, delete the dead branch and the unused
variable so the code states the actual policy honestly:
```rust
let anchored_excel = app.trim_start().starts_with("Microsoft Excel");
if !anchored_excel { return ProvenanceClass::NonExcel; }
if app_version.is_some_and(|v| !v.trim().is_empty()) {
    ProvenanceClass::ExcelTrusted
} else {
    ProvenanceClass::UmyaFabricated
}
```
Pair this with a stronger positive Excel marker (e.g. an AppVersion build-string shape check)
so the gate is not satisfied by an arbitrary non-empty string.

### WR-02: `seed_from_inputs` blanket-seeds EVERY non-formula cell, overriding role-scoped seeding and ignoring `Role::Output` cached values as inputs

**File:** `crates/pmcp-workbook-compiler/src/lib.rs:439-468`
**Issue:** The function first seeds the manifest's `Role::Input | Role::Constant` cells, then
unconditionally loops over `value_by_key` and seeds *every* non-formula cell:
```rust
for (key, value) in &value_by_key {
    env = env.seed_cell(key, value);   // seeds ALL non-formula cells
}
```
This makes the role-scoped seeding above redundant, and — more importantly — it seeds cached
values for cells that are NOT governed inputs/constants (decorative literals, un-roled
helper numbers, even cached values that sit at `Role::Output` coordinates if an output cell
is non-formula). The reconcile stage is then comparing the executor's output against the
oracle while having pre-seeded some of those very leaf values, which can mask a genuine
formula divergence (a cell whose formula should have recomputed a different value is instead
satisfied by the seeded cached value of a precedent that was itself wrong). The comment
frames this as "seed any leaf the DAG depends on," but it is unbounded over all non-formula
cells, not the DAG's leaf set.
**Fix:** Seed only the DAG's actual leaf precedents (cells with no formula that are
referenced by some formula cell), not every non-formula cell. Compute the precedent-leaf set
from the parsed refs (you already build `parsed`/`dag` in `build_ir_and_dag`) and restrict
the fallback loop to that set; drop cells that are role-tagged `Output` from the seed.

### WR-03: Replay/seed round-trip silently drops malformed values, hiding corpus-derivation gaps

**File:** `crates/pmcp-workbook-compiler/src/gate/corpus.rs:407-417` (`json_to_seed`), `:323-347` (`replay_outputs`)
**Issue:** `json_to_seed` skips any value that fails `from_value::<CellValue>` (`if let Ok(cv)
= ... { seed.insert(...) }`), and `replay_outputs` omits any output region whose computed
value is non-finite or non-numeric. A case whose stored `input` JSON was written by an older
schema, or whose output legitimately computes to a non-number, silently disappears from the
seed / from `expected_outputs`. Because the gate's fingerprint and block decision are
computed over whatever survived, a dropped region produces no delta and no block — a
regression in a region that stopped being numeric would pass unnoticed. The driver comment
says "the gate separately HARD-BLOCKS a missing output," but that backstop only fires for
regions present in `case.expected_outputs`; a region dropped at derivation time never enters
that map.
**Fix:** Make the lossy paths explicit. In `json_to_seed`, return a `Result` (or collect the
keys it could not parse) and fail corpus derivation loudly if a stored input cannot be
rehydrated. In `replay_outputs`, record a region that computed a non-finite/non-numeric value
as an explicit sentinel (or a `CorpusError`) rather than omitting it, so the gate can block
on the shape change.

### WR-04: `numeric_step` integer heuristic can produce out-of-bounds boundary cases (`default - step` below `min`)

**File:** `crates/pmcp-workbook-compiler/src/gate/corpus.rs:200-206`, `:280-300`
**Issue:** For an integer-valued default the step is `1.0`, and the grid unconditionally pushes
`default - step` and `default + step` as `num:<cell>=default-step` / `default+step` cases
BEFORE (and independently of) the declared `[min, max]` bounds. A `BoundedVariable { default:
0, min: 0, max: 100 }` therefore generates a `-1` case that violates the declared minimum.
The prior version's IR is then replayed on an out-of-domain seed; if that formula divides by
the input, gates on it, or indexes a table with it, the captured "golden" reflects behavior
outside the workbook's own declared domain — and the candidate is graded against that bogus
golden. This is a correctness hazard for the auto-derived corpus (D-09).
**Fix:** Clamp the `default±step` boundary cases to the declared `[min, max]` when bounds
exist, and skip a boundary case that would duplicate `min`/`max` (which are already emitted):
```rust
let lo = input_bounds(role).and_then(|(m,_)| as_number(m));
let hi = input_bounds(role).and_then(|(_,m)| as_number(m));
let minus = (default - step).max(lo.unwrap_or(f64::MIN));
let plus  = (default + step).min(hi.unwrap_or(f64::MAX));
```

### WR-05: `derive_corpus` fails the WHOLE derivation on a single un-gradable case (fail-fast inside a collect-all crate)

**File:** `crates/pmcp-workbook-compiler/src/gate/corpus.rs:359-377`
**Issue:** `derive_corpus` propagates the first `CorpusError::Replay` with `?`, so one case that
trips a `dag/cycle` (or any executor finding) aborts the entire corpus build and no other
cases are captured. The crate's stated discipline everywhere else (ingest, lint, stage1,
reconcile) is COLLECT-ALL: surface every problem in one pass. A fail-fast here means a BA
fixing one cyclic case re-runs only to hit the next, instead of seeing all un-gradable cases
at once; worse, an attacker-crafted prior IR with one poisoned case can suppress derivation
of every later regression case.
**Fix:** Accumulate per-case errors into a `Vec<CorpusError>` (or a collect-all report) and
return the surviving cases plus the aggregated failures, mirroring `run_stage1`'s
collect-all aggregation, rather than `?`-aborting on the first.

### WR-06: Promote does a redundant non-atomic `final_dir.exists()` precheck that races the atomic rename

**File:** `crates/pmcp-workbook-compiler/src/gate/accept.rs:220-227`, `crates/pmcp-workbook-compiler/src/gate/governed_artifact.rs:82-99`
**Issue:** `promote` checks `if final_dir.exists() { return Err(...) }` (accept.rs:222) and then,
after emitting into staging, calls `atomic_promote_dir`, which checks `final_dir.exists()`
AGAIN before `rename`. The first check is a TOCTOU window: two concurrent promotes of the
same version both see `false`, both emit into (distinct, pid-tagged) staging dirs, and the
second `rename` over an existing target is platform-dependent — on Unix `rename` onto an
existing non-empty directory fails (so the second is rejected, good), but on some platforms /
filesystems renaming onto an existing empty or differing directory can succeed or clobber.
Relying on the early `exists()` for the CR-02 non-overwrite guarantee is unsound because it
is checked far from the rename; the guarantee must rest solely on the rename's own atomicity
+ the in-`atomic_promote_dir` recheck (which is itself still a TOCTOU before `rename`).
**Fix:** Drop the early `final_dir.exists()` precheck in `promote` (it gives a false sense of
atomicity and adds a second race) and make `atomic_promote_dir` use a create-exclusive
primitive instead of check-then-rename — e.g. attempt the `rename` and treat the
`AlreadyExists`/`ENOTEMPTY`/`EEXIST` error as the non-overwrite refusal, rather than
pre-checking `exists()`. That makes the non-overwrite property depend on a single atomic
syscall, not on a TOCTOU-prone stat.

## Info

### IN-01: `ratify` writes the manifest stamp in memory BEFORE the sidecar append can fail

**File:** `crates/pmcp-workbook-compiler/src/manifest/ratify.rs:55-99`
**Issue:** The function mutates `manifest.ratified = true` / `ratified_by` / `ratified_at`
(lines 63-66) and only afterward attempts the sidecar `create_dir_all` / open / write. If the
append fails, the caller receives `Err(RatifyError)` but the passed-in `&mut Manifest` has
already been stamped ratified. A caller that ignores the error (or logs and continues with
the same manifest value) would treat an un-audited manifest as ratified.
**Fix:** Perform the sidecar write first (or write to a local copy), and only commit the
in-memory stamp after the audit line is durably appended, so the stamp and the audit trail
are all-or-nothing.

### IN-02: `col_to_a1` and `String::from_utf8(...).unwrap_or_default()` silently yields an empty column key on overflow

**File:** `crates/pmcp-workbook-compiler/src/change_class/ir_identity.rs:161-170`
**Issue:** `col_to_a1` builds bytes `b'A' + rem as u8` and recovers them with
`String::from_utf8(...).unwrap_or_default()`. The bytes are always valid ASCII so the
`unwrap_or_default` is unreachable in practice, but the silent `unwrap_or_default()` would
turn any future regression (a non-ASCII byte) into a blank column, which would silently
collapse two distinct range members to the same key in the identity hash. Low risk today;
flagged as a latent silent-default.
**Fix:** Since the bytes are provably ASCII, build the string directly without the fallible
conversion (`s.iter().map(|&b| b as char).collect()`), or assert the invariant explicitly.

### IN-03: `parse_cell_value` treats every numeric-parseable string as a `Number`, losing text that looks numeric

**File:** `crates/pmcp-workbook-compiler/src/lib.rs:421-434`
**Issue:** `parse_cell_value` returns `CellValue::Number` for any string `f64::parse` accepts —
including `"NaN"`, `"inf"`, `"1e999"` (→ `inf`), and leading/trailing forms Rust's `f64`
parser allows. A cached oracle value of `"NaN"`/`"inf"` becomes a non-finite Number that the
reconcile `within_tol` then treats as always-out-of-tolerance, and a cell whose intended
content is the literal text `"inf"` is silently coerced to infinity. This is benign for
well-formed Excel caches but is an unguarded coercion at an untrusted-input boundary.
**Fix:** Reject non-finite parses explicitly (`trimmed.parse::<f64>().ok().filter(|n|
n.is_finite())`) so a `"NaN"`/`"inf"` cached string falls through to `CellValue::Text`, and
the reconcile compares it structurally.

### IN-04: `effective_policy` empty-set defaults to `HotReload` — a promote with zero derived classes silently hot-reloads

**File:** `crates/pmcp-workbook-compiler/src/change_class/mod.rs:337-346`
**Issue:** `effective_policy(&[])` returns `HotReload`. The docstring justifies it as "nothing
to block," but combined with the auto-derivation, a derivation that produces zero classes
(because a real change escaped classification — the very `T-93-05-PROMO` hazard CR-01 of the
change-class module guards against) would route to the most permissive policy. The symmetric
classifier and the dedup are the mitigations, but defaulting the *absence of evidence* to the
*most permissive* action is the opposite of fail-closed.
**Fix:** Consider making an empty class set on a promote that DID change content
(`prev_hash != candidate_hash`) route to `BlockUntilAccept` (fail-closed), reserving
`HotReload` for the provably-no-change case. At minimum, document that callers must not invoke
`effective_policy(&[])` when the bundle content changed.

---

_Reviewed: 2026-06-12T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
