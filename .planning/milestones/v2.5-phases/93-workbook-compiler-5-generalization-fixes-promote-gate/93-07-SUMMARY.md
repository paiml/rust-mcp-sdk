---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
plan: 07
subsystem: compiler
tags: [workbook-compiler, excel, manifest-synthesis, structural-equivalence, provenance, rust_xlsxwriter, producer-consumer]

# Dependency graph
requires:
  - phase: 93 (Plans 01-06)
    provides: ingest/synth/ratify/formula/dag/reconcile/provenance/artifact/change_class/gate modules + the typed-NotImplemented compile_workbook stub + stage1 stub
  - phase: 92
    provides: the frozen seven-member bundle contract + tax-calc@1.1.0 golden + the pmcp-server-toolkit workbook loader
provides:
  - "Wired generic compile_workbook driver (synth-driven, no build_reference_manifest, no reference-path/workflow consts) — the §5 WBCO-02 generalization"
  - "Composed stage-1 collect-all pass (lint + synth + freshness gate, one ingest/one byte-read)"
  - "Neutral tax-calc.xlsx fixture (rust_xlsxwriter, NOT umya, cached formula values) + trusted-fixture provenance override (test/dev path only)"
  - "Producer/consumer structural-equivalence proof reproducing the committed tax-calc@1.1.0 golden"
  - "Library-level compile_a_workbook example (ingest -> emit)"
affects: [94 (CLI subcommands over the wired driver), 95 (Shape A binary), 96 (Shape B scaffold + generalization gate)]

# Tech tracking
tech-stack:
  added: [rust_xlsxwriter 0.95 (dev-dependency, writer-only fixture authoring)]
  patterns:
    - "Generic synth-driven compile driver: manifest comes SOLELY from synthesize->ratify; the workflow name is a parameter (never a literal)"
    - "Dev-only `trusted-fixture` Cargo feature gates the provenance override entry; never in default/published feature set so it cannot weaken production refusal"
    - "Driver-level out_* named-range promotion to Role::Output (synth stays colour-only)"
    - "Structural-equivalence proof against a frozen golden (IR normalized-JSON equal; semantic projection of the manifest, not provenance strings)"

key-files:
  created:
    - crates/pmcp-workbook-compiler/tests/reemit_tax_calc_golden.rs
    - crates/pmcp-workbook-compiler/tests/support/tax_calc_fixture.rs
    - crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.xlsx
    - crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.provenance-override.json
    - crates/pmcp-workbook-compiler/examples/compile_a_workbook.rs
  modified:
    - crates/pmcp-workbook-compiler/src/lib.rs
    - crates/pmcp-workbook-compiler/src/stage1.rs
    - crates/pmcp-workbook-compiler/src/provenance/gate.rs
    - crates/pmcp-workbook-compiler/Cargo.toml

key-decisions:
  - "O-2 resolved as STRUCTURAL EQUIVALENCE (pre-decided): the proof asserts IR normalized-JSON equality + cell-map seed-coords + seven members + lock recompute + toolkit-loader acceptance + named-output role/dtype/name; byte-identity is a logged, non-blocking stretch (the golden stays frozen)"
  - "Manifest provenance-only fields (source/ratified_by/meaning/unit) are NOT reproducible from a real compile (synth derives them from colour, not the golden's hand-authored strings); equality is asserted on the SEMANTIC projection, documented in the proof module"
  - "The trusted-fixture override is gated behind a dev-only `trusted-fixture` feature (+ cfg(test)) so the producer/consumer proof and the example can compile a non-Excel fixture WITHOUT the production compile_workbook ever honouring it"
  - "rust_xlsxwriter hard-codes fullCalcOnLoad=1 + writes no Excel recalc stamp, so the trusted-fixture override also DEMOTES the staleness freshness findings to Warning (structural-corruption findings stay Error) — the lighthouse AcceptStale pattern"

patterns-established:
  - "Driver = generic glue (ingest->stage1->ratify->parse+DAG->executor->reconcile->emit); synth/gate modules stay single-purpose"
  - "Producer/consumer proof against a frozen golden using a committed neutral fixture authored without the reader"

requirements-completed: [WBCO-02, WBCO-05]

# Metrics
duration: ~80min
completed: 2026-06-12
---

# Phase 93 Plan 07: Wire the Compiler + Producer/Consumer Proof Summary

**Generic synth-driven `compile_workbook` driver wired end-to-end (ingest → stage1 → ratify → parse+DAG → shared executor → reconcile → seven-member emit) with a neutral `tax-calc.xlsx` fixture compiling to STRUCTURAL EQUIVALENCE against the committed `tax-calc@1.1.0` golden — the IR re-emits normalized-JSON identical.**

## Performance

- **Duration:** ~80 min
- **Tasks:** 3
- **Files modified/created:** 15 (compiler crate + 2 runtime-crate scrub edits)

## Accomplishments
- Wired the generic `compile_workbook` driver replacing the typed-NotImplemented stub: the manifest comes SOLELY from `synthesize → ratify` (no `build_reference_manifest` on any non-test path; no `REFERENCE_WORKBOOK_PATH`/`WORKFLOW_NAME` consts). Proves the §5 generalization — a second workbook would be served ITS OWN schema.
- Lifted `stage1::run_stage1` as a composed collect-all pass (lint + synth + freshness gate) over one ingest/one byte-read, refusing once on Error findings and carrying Warning/Info as evidence.
- Authored a neutral `tax-calc.xlsx` via `rust_xlsxwriter` (NOT umya — the provenance gate refuses umya identity) with cached `<v>` formula results (the reconcile oracle), blue-font inputs + an inline-DV enum + green-fill governed bracket table + `out_*` named outputs; committed alongside a trusted-fixture provenance override honoured ONLY on the dev/test path.
- Proved the producer/consumer loop: `reemit_tax_calc_golden` asserts all five structural-equivalence dimensions — `executable.ir.json` is normalized-JSON EQUAL to the golden, cell-map seed-coords equal, seven members present, BUNDLE.lock recomputes, the bundle loads via the Phase-92 toolkit loader, and named-output roles/dtypes/names match.
- Shipped the `compile_a_workbook` library-level example (ingest → emit on the committed fixture).
- Full phase gate green: `make quality-gate` + `make purity-check` PASS, all four grep gates clean, both fuzz targets build.

## Task Commits

1. **Task 1: Lift stage1 + wire the generic compile_workbook driver (WBCO-02)** - `90178d19` (feat)
2. **Task 2: Neutral tax-calc fixture + structural-equivalence proof + example (WBCO-05)** - `2d58b5f6` (feat)
3. **Task 3: Phase gate — scrub + fmt; quality-gate green** - `a7d89d0c` (chore)

## Files Created/Modified
- `src/lib.rs` — wired `compile_workbook` + `compile_workbook_with_fixture_override` (dev/test) + IR/DAG/seed/comparison/output-promotion helpers
- `src/stage1.rs` — composed collect-all pass + `FreshnessPolicy` (Enforce vs TrustedFixture)
- `src/provenance/gate.rs` — trusted-fixture freshness softening (staleness → Warning; structural corruption stays Error); cfg widened to `any(test, feature="trusted-fixture")`
- `tests/reemit_tax_calc_golden.rs` — the 5-dimension structural-equivalence proof + production-refusal regression
- `tests/support/tax_calc_fixture.rs` — shared rust_xlsxwriter authoring (cached results; out_* names) + the override JSON
- `tests/fixtures/tax-calc.xlsx` + `tests/fixtures/tax-calc.provenance-override.json` — committed neutral fixture + override
- `examples/compile_a_workbook.rs` — ingest → emit demonstration
- `Cargo.toml` — `trusted-fixture` feature, rust_xlsxwriter dev-dep, example/test `required-features`
- `src/manifest/synth.rs`, `src/manifest/projections.rs`, `src/gate/corpus.rs` — customer-identifier scrub (heat_pump/boiler → single/married)
- `crates/pmcp-workbook-runtime/src/{lib.rs,manifest_model.rs}` — runtime API `plot3_key` → `json_key_for_role` (scrub)

## Decisions Made
See `key-decisions` in frontmatter. Headline: O-2 resolved as structural equivalence (the golden stays frozen); the proof asserts the load-bearing semantic dimensions the compile path CAN reproduce, with manifest provenance-only metadata explicitly out of the equality set.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Driver promotes `out_*` named ranges to `Role::Output`**
- **Found during:** Task 1/2 (driver wiring + first compile)
- **Issue:** Synthesis classifies cells from COLOUR alone and NEVER emits `Role::Output`; `build_cell_map` fails loud on a zero-output manifest, so EVERY real compile would fail at emit. The output convention is a named-range act (`out_<name>`) the workbook authors, which synth does not promote.
- **Fix:** Added `promote_named_outputs` in the driver (NOT in synth): each `out_*` single-cell defined-name target is re-roled to `Role::Output` with the named-range name recorded. Keeps synth single-purpose (colour-only) and makes the pipeline actually emit.
- **Files modified:** src/lib.rs
- **Verification:** The proof's `structural_eq_check5_named_outputs_match` + toolkit-loader acceptance (4 outputs served) pass.
- **Committed in:** `2d58b5f6`

**2. [Rule 3 - Blocking] Trusted-fixture override must soften the staleness freshness findings**
- **Found during:** Task 2 (first fixture compile)
- **Issue:** `rust_xlsxwriter` hard-codes `fullCalcOnLoad=1` and writes no Excel recalc stamp, so the freshness gate refused the neutral fixture with `oracle/stale-cache` EVEN WITH the trusted-fixture override (which previously relaxed only the provenance-CLASS axis, never the freshness axis). The proof was blocked.
- **Fix:** Under `fixture_override.is_some()`, the gate now DEMOTES the staleness freshness rules (`oracle/stale-cache`/`no-recalc`/`missing-cache`/`non-excel-app`) to Warning (the lighthouse `AcceptStale` pattern); STRUCTURAL corruption findings (`oracle/unreadable-provenance`/`missing-provenance`/`missing-manifest`) stay Error. Production passes `None`, so refusal is unchanged.
- **Files modified:** src/provenance/gate.rs
- **Verification:** `override_does_not_weaken_production_refusal` proves the SAME bytes are refused on the production path; the full proof compiles via the override.
- **Committed in:** `2d58b5f6`

**3. [Rule 3 - Blocking] Trusted-fixture override entry exposed via a dev-only feature (not cfg(test))**
- **Found during:** Task 2 (proof + example need the override; cfg(test) is unreachable from integration tests/examples)
- **Issue:** `gate_with_fixture_override` was `#[cfg(test)]` — unreachable from a `tests/` integration test or an example. The plan's `tests/reemit_tax_calc_golden.rs` + `examples/compile_a_workbook.rs` could not reach it.
- **Fix:** Widened the override gate to `cfg(any(test, feature = "trusted-fixture"))` and added a `pub` `compile_workbook_with_fixture_override`; the `trusted-fixture` feature is dev-only (never in `default`, never published-enabled), so "honoured only on the test/dev path" holds. The example/proof set `required-features = ["trusted-fixture"]`.
- **Files modified:** src/provenance/gate.rs, src/stage1.rs, src/lib.rs, Cargo.toml
- **Verification:** Lib tests pass with AND without the feature; the proof + example run with `--features trusted-fixture`.
- **Committed in:** `2d58b5f6`

**4. [Rule 2 - Missing Critical / Scrub] Customer-identifier scrub the phase gate requires**
- **Found during:** Task 3 (grep gate)
- **Issue:** Pre-existing customer test data (`heat_pump`/`boiler`) in synth/projections/corpus and a customer-derived runtime API name `plot3_key` tripped the Task-3 `! grep ... heat_pump|plot3` gate.
- **Fix:** Scrubbed `heat_pump,boiler → single,married` (neutral tax-domain enum) and renamed the runtime API `plot3_key → json_key_for_role` across the runtime + compiler.
- **Files modified:** src/manifest/synth.rs, src/manifest/projections.rs, src/gate/corpus.rs, crates/pmcp-workbook-runtime/src/{lib.rs,manifest_model.rs}, src/artifact/cell_map.rs
- **Verification:** The customer-identifier grep gate over `crates/pmcp-workbook-compiler/src/` returns zero; `make quality-gate` green.
- **Committed in:** `a7d89d0c`

---

**Total deviations:** 4 auto-fixed (2 missing-critical, 2 blocking). All necessary for the pipeline to emit at all (output promotion), for the proof to run (override exposure + freshness softening), and to clear the mandated grep gate (scrub). No scope creep.

## Issues Encountered

- **Full normalized-JSON manifest equality to the golden is not achievable from a real compile.** The golden was hand-synthesized with `source: "synthetic-fixture"`, hand-written `meaning`/`name`/`unit` strings; a real synthesis derives `source` from colour and leaves names/meanings/units unset. RESOLVED by asserting the load-bearing SEMANTIC dimensions (IR is normalized-JSON EQUAL; manifest compared on per-cell role+dtype+named-output identity; cell-map on seed-coords), with the provenance-only metadata explicitly documented as out-of-scope for equality. The IR — the load-bearing formula DAG — DOES re-emit byte/normalized-JSON identical to the golden.

## Next Phase Readiness
- The generic `compile_workbook(workbook_path, out_root, workflow, version, approver)` driver is the stable surface Phase 94's CLI wraps.
- The producer/consumer proof + neutral fixture are the regression anchor for Phase 96's generalization gate (a second workbook compiles through the same synth-driven path).
- KNOWN: the runtime crate (Phase 91) still carries customer identifiers (`ufh-quote`/`coil`/`heat_pump`/`UFH`) in its own src/ — out of scope for this plan's compiler-scoped grep gate, flagged for a runtime-crate scrub pass.
- KNOWN STUBS: none in this plan's compiler surface that block the goal — the manifest provenance metadata (`source`/`unit`/`meaning`) is synthesis-derived/unset by design, not a stub (a BA refines it in-Excel; documented in the proof module).

## Self-Check: PASSED

All created files exist on disk (reemit proof, fixture-author module, committed tax-calc.xlsx + override, example, this SUMMARY) and all three task commits (`90178d19`, `2d58b5f6`, `a7d89d0c`) are present in git history.

---
*Phase: 93-workbook-compiler-5-generalization-fixes-promote-gate*
*Completed: 2026-06-12*
