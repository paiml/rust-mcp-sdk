---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
verified: 2026-06-13T03:55:14Z
status: passed
score: 16/16
overrides_applied: 0
---

# Phase 93: pmcp-workbook-compiler Verification Report

**Phase Goal:** `pmcp-workbook-compiler` ports the full offline pipeline (ingest → lint → manifest synth → formula parse → DAG compile → penny-reconcile → artifact emit → promote-time gate) with `umya` isolated to this crate, and ships the §5 generalization fixes at extraction time (not copied): a fully manifest-driven emit path, symmetric change-class classification, versioned non-overwriting bundle writes, enum-tiering correctness, umya fabricated-provenance refusal, and the change-class + golden-corpus promote gate with a BA approval flow.

**Verified:** 2026-06-13T03:55:14Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | umya is confined to pmcp-workbook-compiler; served-tree crates (runtime/dialect/toolkit) carry no umya dep | VERIFIED | `make purity-check` passes: Phase 93 positive assertion "umya-spreadsheet found, reader confined to the compiler" + served-crate negatives all clean |
| 2 | The full offline pipeline compiles: ingest → stage1 → ratify → parse+DAG → reconcile → emit | VERIFIED | `compile_workbook` in `src/lib.rs:188` routes all 8 stages; no `CompileError::NotImplemented` on production path |
| 3 | Manifest synthesis is fully workbook-driven; build_reference_manifest is absent | VERIFIED | `grep -rn 'build_reference_manifest' src/` exits 1 (zero matches); `synth.rs` `synthesize()` takes `workflow: &str` as parameter; test asserts two different workflow names produce different manifests |
| 4 | The WBCO-07 umya fabricated-provenance refusal is in force and NOT downstream-bypassable (CR-01 closed) | VERIFIED | `Cargo.toml [features]` contains only `default = []` — no publishable `trusted-fixture` feature. `compile_workbook_with_fixture_override` and `gate_with_fixture_override` are gated `#[cfg(test)]` only. `SOFTENABLE_FRESHNESS_RULES` does NOT contain `"oracle/non-excel-app"`. Test `override_cannot_soften_fabricated_identity` asserts even WITH the test override, UmyaFabricated identity is still refused. 247 tests pass. |
| 5 | Symmetric change-class classifier: demotion-direction changes each produce a non-empty class | VERIFIED | `change_class/mod.rs` `classify()` handles `Constant|Formula → Input` demotion; `effective_policy` uses derived `Ord`; property test `classify_is_symmetric_in_cardinality` asserts symmetry across all direction pairs |
| 6 | Versioned non-overwriting bundle writes: promote-twice → two distinct @version dirs | VERIFIED | `gate/governed_artifact.rs` `atomic_promote_dir` refuses to overwrite; test `promote_twice_two_dirs` asserts two distinct dirs + prior baseline byte-identical |
| 7 | WBGV-07 enum-tiering correctness: ratify_tiers skips frozen-enum inputs | VERIFIED | `artifact/mod.rs:99` `ratify_tiers` skips `allowed_values: Some(_)` inputs; test `ratify_skips_frozen_enum_inputs` verified against committed golden |
| 8 | WBGV-04 golden-corpus promote gate blocks over-tolerance named-output deltas | VERIFIED | `gate/corpus.rs` `derive_corpus` + `gate/mod.rs` `gate()` returns `GateDecision::Blocked`; test `over_tolerance_blocks` passes |
| 9 | WBGV-05 fingerprint-bound ApprovalRecord: accept() re-baselines corpus | VERIFIED | `gate/corpus.rs` `candidate_fingerprint()` + `gate/accept.rs` `accept()` writes to `approvals/<fingerprint>.json` atomically; tests `accept_records_and_passes` + three mismatch tests pass |
| 10 | Producer/consumer proof: tax-calc.xlsx re-emits structural-equiv to tax-calc@1.1.0 golden | VERIFIED | `src/reemit_golden.rs` (in-crate `#[cfg(test)]`) asserts all 5 structural-equivalence dimensions (IR normalized-JSON, cell-map seed-coords, 7 members, BUNDLE.lock recompute, toolkit loader acceptance, named-output roles); runs as part of the 247-test suite |
| 11 | WBCO-04 penny-reconcile uses operand-anchored rounding (no naïve delta.abs()) | VERIFIED | `grep -rn "delta\.abs()" src/reconcile/` exits 1 (zero matches); `classifier.rs:33` defines `BOUNDARY_EPSILON`; uses `excel_round`/`excel_roundup`/`excel_ceiling` from runtime |
| 12 | WBDL-03 dialect linter: collect-all, located, with repair guidance | VERIFIED | `src/dialect/linter.rs:1` "collect-all, located linter (WBDL-03)"; 11 tests over synthetic CellSource double; whitelist-AT-PARSE-time enforced in `formula/parser.rs` |
| 13 | WBCO-06 inline DV → closed JSON-Schema enum (≤10 literals); range sources get precise reason-code WARNING | VERIFIED | `manifest/projections.rs:132` `resolve_inline_list()` returns `None` for >10 values; `synth.rs` DV fork emits `allowed_values: Some(Vec<String>)` for inline; WARNING emitted for range/named-range sources |
| 14 | WBCO-03 formula parser + Kahn DAG with no petgraph | VERIFIED | `formula/parser.rs` Pratt parser building runtime `Expr`; `dag/topo.rs` Kahn toposort; no `petgraph` in `Cargo.toml` |
| 15 | WBGV-03 canonical IR sub-DAG identity hash (numeric drift vs semantic redefinition) | VERIFIED | `change_class/ir_identity.rs:34` `ir_subdag_hash()` with transitive precedent set + deterministic topological order |
| 16 | make quality-gate passes (fmt, clippy pedantic+nursery, build, test, audit) | VERIFIED | commit c078e0d4 message: "default + --all-features build clean, 247 tests pass, make purity-check green, clippy/fmt green"; cargo test confirms 247 passed |

**Score:** 16/16 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-workbook-compiler/Cargo.toml` | Compiler crate manifest; umya-spreadsheet direct dep; path deps on runtime+dialect; [features] default=[] only | VERIFIED | umya-spreadsheet="3.0", path deps present, thiserror="2", NO trusted-fixture publishable feature |
| `crates/pmcp-workbook-compiler/src/lib.rs` | Wired compile_workbook driver; crate-deny header; re-export surface | VERIFIED | 23.2K; `#![deny(clippy::unwrap_used, expect_used, panic)]`; full pipeline at line 188; re-exports from pmcp_workbook_runtime + pmcp_workbook_dialect |
| `crates/pmcp-workbook-compiler/src/error.rs` | CompileError with NotImplemented sentinel | VERIFIED | 2.6K; staged variants per pipeline stage |
| `crates/pmcp-workbook-compiler/src/provenance/gate.rs` | ProvenanceClass + WBCO-07 refusal + test-only override | VERIFIED | Confirmed: oracle/non-excel-app absent from SOFTENABLE_FRESHNESS_RULES; both gate() and gate_with_fixture_override gated correctly |
| `crates/pmcp-workbook-compiler/src/manifest/synth.rs` | synthesize(wb, rules, workflow) — workflow as parameter | VERIFIED | workflow parameter at line 127; test at line 617 asserts two distinct workflow names |
| `crates/pmcp-workbook-compiler/src/reconcile/classifier.rs` | Operand-anchored MismatchClass; BOUNDARY_EPSILON; no delta.abs() | VERIFIED | BOUNDARY_EPSILON=1e-6; imports excel_round/roundup/ceiling; zero delta.abs() matches |
| `crates/pmcp-workbook-compiler/src/artifact/serialize.rs` | Deterministic JSON (stable key order, pinned policy) | VERIFIED | BTreeMap projection for HashMap; pretty 2-space policy |
| `crates/pmcp-workbook-compiler/src/artifact/mod.rs` | emit_bundle (7 members) + ratify_tiers WR-01 skip | VERIFIED | ratify_tiers at line 99; allowed_values skip; emitted_bundle_loads_via_toolkit test |
| `crates/pmcp-workbook-compiler/src/change_class/mod.rs` | Symmetric classifier + effective_policy strictest-policy reducer | VERIFIED | GatePolicy Ord-derived; effective_policy uses iterator max; property test |
| `crates/pmcp-workbook-compiler/src/change_class/ir_identity.rs` | Canonical IR sub-DAG hash | VERIFIED | ir_subdag_hash() at line 34 |
| `crates/pmcp-workbook-compiler/src/gate/corpus.rs` | Auto-derived corpus + candidate_fingerprint + ApprovalRecord | VERIFIED | candidate_fingerprint at line 147; MAX_CORPUS_CASES=50 |
| `crates/pmcp-workbook-compiler/src/gate/accept.rs` | accept() + promote() + EmitLane CR-02 | VERIFIED | atomic_promote_dir; EmitLane Seed/GatedUpdate; promote_twice test |
| `crates/pmcp-workbook-compiler/src/reemit_golden.rs` | Producer/consumer proof (in-crate #[cfg(test)]) | VERIFIED | 11.0K; 5 structural-equivalence dimensions; runs via plain cargo test |
| `crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.xlsx` | Neutral fixture authored without umya | VERIFIED | 6.6K; present in tests/fixtures/ |
| `crates/pmcp-workbook-compiler/examples/compile_a_workbook.rs` | Library-level ingest→emit demonstration | VERIFIED | 3.6K; uses public compile_workbook API; honestly demonstrates refusal |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| Cargo.toml [features] | provenance refusal bypass | trusted-fixture feature | VERIFIED ABSENT | Only `default = []` in [features]; CR-01 closed |
| gate_with_fixture_override | test paths only | #[cfg(test)] | VERIFIED | Both gate.rs:255 and lib.rs:306 gated `#[cfg(test)]`; non-test builds get the stub that routes to production gate |
| oracle/non-excel-app | SOFTENABLE_FRESHNESS_RULES | exclusion | VERIFIED | SOFTENABLE_FRESHNESS_RULES = ["oracle/no-recalc", "oracle/stale-cache", "oracle/missing-cache"] — fabricated-identity rule is NOT in the softenable set |
| compile_workbook_inner | synthesize + ratify | manifest-driven path | VERIFIED | stage1::run_stage1 routes to manifest::synthesize; no build_reference_manifest in src/ |
| artifact::emit_bundle | pmcp_workbook_runtime::build_bundle_lock | shared hash helpers | VERIFIED | bundle_lock.rs re-export shim; BUNDLE.lock emits bundle_id |
| classify() | ChangeClass re-export | pmcp_workbook_runtime | VERIFIED | change_class/mod.rs imports from re-exported runtime type; no local pub enum ChangeClass |
| derive_corpus | manifest defaults + enum domains | auto-derived grid | VERIFIED | corpus.rs generates from synthesized Manifest; BA authors no cases |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `src/lib.rs compile_workbook_inner` | `map` (WorkbookMap) | `ingest::ingest()` reads umya from disk | Yes — real .xlsx bytes | FLOWING |
| `src/lib.rs compile_workbook_inner` | `stage1` (Stage1Output) | `stage1::run_stage1()` with original bytes + map | Yes — real lint/synth/provenance pass | FLOWING |
| `src/provenance/gate.rs gate()` | `class` (ProvenanceClass) | `classify(app.application, app.app_version, calc_id)` from raw bytes | Yes — reads original .xlsx docProps/app.xml | FLOWING |
| `src/artifact/mod.rs emit_bundle` | bundle JSON files | `serialize::to_bundle_json` over manifest/ir/cell_map/layout | Yes — deterministic, from real pipeline outputs | FLOWING |
| `src/gate/corpus.rs derive_corpus` | `cases` | manifest defaults + allowed_values + numeric boundaries | Yes — from synthesized Manifest, bounded grid | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| 247 compiler tests pass | `cargo test -p pmcp-workbook-compiler` | 247 passed, 0 failed (2 suites, 0.16s) | PASS |
| purity-check: umya confined to compiler | `make purity-check` | "reader confined to the compiler" — PASSED | PASS |
| No publishable trusted-fixture feature | `grep "trusted.fixture" Cargo.toml` | 0 matches | PASS |
| No build_reference_manifest in src/ | `grep -rn "build_reference_manifest" src/` | 0 matches | PASS |
| No delta.abs() in reconcile | `grep -rn "delta\.abs()" src/reconcile/` | 0 matches | PASS |
| No TBD/FIXME/XXX debt markers | `grep -rn "TBD\|FIXME\|XXX" src/` | 0 matches | PASS |
| No customer identifiers | `grep -rn "ufh\|towelrad\|coil\|heat_pump\|plot3" src/` | 0 matches | PASS |
| No todo!()/unimplemented!() | `grep -rn "todo!\|unimplemented!" src/` | 0 matches | PASS |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| WBCO-01 | 93-02 | Compiler ingests .xlsx (umya, compiler-isolated) + captures cached cell values as oracle | SATISFIED | `ingest/mod.rs` reads via umya → owned WorkbookMap; cached `<v>` values captured; MAX_CELL_COUNT DoS guard |
| WBCO-02 | 93-04, 93-07 | Fully workbook-driven manifest synthesis; no build_reference_manifest | SATISFIED | `synthesize(wb, rules, workflow)` — workflow as parameter; build_reference_manifest absent; wired in compile_workbook_inner |
| WBCO-03 | 93-03 | Compiler parses formulas + reconstructs dependency DAG | SATISFIED | `formula/parser.rs` Pratt parser; `dag/topo.rs` Kahn toposort; no petgraph |
| WBCO-04 | 93-04 | Penny-reconcile: operand-anchored rounding, not naïve abs-delta | SATISFIED | `reconcile/classifier.rs` BOUNDARY_EPSILON + excel_round/roundup/ceiling; zero delta.abs() matches |
| WBCO-05 | 93-05, 93-07 | Compiler emits 7-member bundle; producer/consumer proof | SATISFIED | `artifact/emit_bundle` → 7 members; `src/reemit_golden.rs` 5-dimension proof; toolkit loader round-trip |
| WBCO-06 | 93-04 | Synthesizes closed JSON-Schema enums from inline DV (≤10 values) | SATISFIED | `projections.rs` `resolve_inline_list()`; range sources get WARNING |
| WBCO-07 | 93-02, CR-01 fix | Distinct ProvenanceClass for umya-stamped workbooks; fabricated identity refused | SATISFIED | ProvenanceClass::UmyaFabricated → oracle/non-excel-app; test-only override cannot soften; CR-01 closed via cfg(test)-only gate |
| WBGV-01 | 93-05 | Symmetric change-class classifier; demotion-direction changes produce non-empty class | SATISFIED | `change_class/mod.rs` symmetric; property test `classify_is_symmetric_in_cardinality` |
| WBGV-02 | 93-05 | Strictest-policy reducer; assumption change hard-blocks | SATISFIED | `effective_policy` derived-Ord max; `NeverAutoPromote > BlockUntilAccept > HotReload` |
| WBGV-03 | 93-05 | Gate distinguishes numeric drift from semantic redefinition via IR sub-DAG hash | SATISFIED | `ir_identity.rs` `ir_subdag_hash()` with transitive precedent set + canonical fold |
| WBGV-04 | 93-06 | Golden-corpus gate blocks over-tolerance named-output deltas | SATISFIED | `gate/corpus.rs` derive_corpus + `gate/mod.rs` gate(); test `over_tolerance_blocks` |
| WBGV-05 | 93-06 | BA records approval via content-hash-fingerprinted ApprovalRecord | SATISFIED | `gate/accept.rs` `accept()`; `candidate_fingerprint`; three approval-mismatch tests |
| WBGV-06 | 93-06 | Promotion writes to new @version dir; never overwrites baseline | SATISFIED | `atomic_promote_dir`; test `promote_twice_two_dirs` + `refuse_overwrite_promote` |
| WBGV-07 | 93-05 | Enum inputs skip Variable-tier assignment | SATISFIED | `artifact/mod.rs` `ratify_tiers()` skips `allowed_values: Some(_)`; test verified vs committed golden |
| WBDL-03 | 93-03 | Developer can lint workbook against dialect; collect-all, located, with repair guidance | SATISFIED | `dialect/linter.rs` WBDL-03 collect-all linter; CellSource synthetic seam; 11 tests |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | — | — | — | Zero TBD/FIXME/XXX, zero todo!()/unimplemented!(), zero customer identifiers in src/ |

---

### CR-01 Verification (Special Focus)

The code review (93-REVIEW.md) found CR-01 BLOCKER: `trusted-fixture` was a publishable Cargo feature enabling downstream bypass of WBCO-07 provenance refusal. Commit `c078e0d4` addressed this.

**Evidence that CR-01 is CLOSED:**

1. `Cargo.toml [features]` contains ONLY `default = []` — no `trusted-fixture` key exists.
2. `gate_with_fixture_override` (gate.rs:255) is `#[cfg(test)]`.
3. `TrustedFixtureMarker` struct (gate.rs:217) is `#[cfg(test)]`.
4. `compile_workbook_with_fixture_override` (lib.rs:306) is `#[cfg(test)]`.
5. `trusted_fixture_gate` in stage1.rs: the `#[cfg(test)]` version calls gate_with_fixture_override; the `#[cfg(not(test))]` production stub routes to production gate (can never be armed by feature unification).
6. `SOFTENABLE_FRESHNESS_RULES` excludes `"oracle/non-excel-app"` — defense-in-depth: fabricated-identity refusal is not softenable even under the test override.
7. Test `override_cannot_soften_fabricated_identity` explicitly asserts both production AND override paths refuse UmyaFabricated bytes with `oracle/non-excel-app`.
8. The golden proof is now in `src/reemit_golden.rs` (`#[cfg(test)]`), not an integration test, so `cfg(test)` is reachable.
9. The example (`compile_a_workbook.rs`) HONESTLY demonstrates refusal — no feature, no override.

**Verdict:** CR-01 is CLOSED. The provenance refusal boundary is not downstream-bypassable.

---

### Human Verification Required

None — all truths are verifiable programmatically for this build-time-only crate. The pipeline produces file system artifacts (bundle JSON files) that are proven by the in-crate structural-equivalence test suite (247 tests passing). No UI, no real-time behavior, no external service integration.

---

### Gaps Summary

No gaps found. All 16 observable truths are VERIFIED, all 15 requirement IDs covered, all artifacts substantive and wired, quality gate passes, purity-check passes, CR-01 closed.

The WR-01 code-review warning (is_sentinel_calc_id branch redundancy / false-positive policy) is a known design decision: both `has_app_version` branches return `ExcelTrusted` because the documented false-positive policy admits even a sentinel calcId when a positive AppVersion build string is present. This is a WARN in the code review, not a functional gap in the phase goal. The behavior is tested and matches the documented intent.

---

_Verified: 2026-06-13T03:55:14Z_
_Verifier: Claude (gsd-verifier)_
