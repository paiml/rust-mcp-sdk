---
phase: 93
slug: workbook-compiler-5-generalization-fixes-promote-gate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-06-11
---

# Phase 93 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from 93-RESEARCH.md `## Validation Architecture`.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `cargo test`; `proptest`/`quickcheck` for property tests; `cargo fuzz` for fuzz targets (CLAUDE.md ALWAYS); `insta` snapshots (lighthouse dev-dep) |
| **Config file** | none beyond `Cargo.toml` `[dev-dependencies]`; CI runs `--test-threads=1` |
| **Quick run command** | `cargo test -p pmcp-workbook-compiler --lib` |
| **Full suite command** | `make quality-gate` then `make purity-check` |
| **Estimated runtime** | ~60–120 seconds (lib) / several minutes (full gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-workbook-compiler --lib`
- **After every plan wave:** Run `make quality-gate && make purity-check`
- **Before `/gsd:verify-work`:** Full suite green + byte-identical re-emit + umya-fabrication-refused regression + the `delta.abs()` / `build_reference_manifest` grep gates
- **Max feedback latency:** ~120 seconds (quick lib run)

---

## Per-Task Verification Map

| Req ID | Behavior | Threat Ref | Test Type | Automated Command | File Exists | Status |
|--------|----------|------------|-----------|-------------------|-------------|--------|
| WBDL-03 | collect-all located dialect findings with repair | T-injection | unit | `cargo test -p pmcp-workbook-compiler dialect::` | ❌ W0 (lift) | ⬜ pending |
| WBCO-01 | ingest captures cached values as oracle | T-provenance | unit | `cargo test -p pmcp-workbook-compiler ingest::` | ❌ W0 | ⬜ pending |
| WBCO-02 | manifest fully synth-driven; `build_reference_manifest` deleted from emit path | — | unit + grep-gate | `cargo test ... manifest::` + `! grep -rn 'build_reference_manifest' src/ --include='*.rs' \| grep -v test` | ❌ W0 | ⬜ pending |
| WBCO-03 | whitelist-at-parse rejection; Kahn DAG | T-injection | unit + property | `cargo test ... formula:: dag::` | ❌ W0 | ⬜ pending |
| WBCO-04 | operand-anchored reconcile; no naïve `delta.abs()` | — | unit + **grep-gate** | `cargo test ... reconcile::` + `! grep -n 'delta.abs()' src/reconcile/classifier.rs` | ❌ W0 | ⬜ pending |
| WBCO-05 | seven-member bundle emit | — | integration | `cargo test ... artifact::` | ❌ W0 | ⬜ pending |
| WBCO-06 | inline DV ≤10 → enum; range-DV → dynamic input + reason code | T-injection | unit | `cargo test ... enum_` | ❌ W0 | ⬜ pending |
| WBCO-07 | umya-authored workbook REFUSED `oracle/non-excel-app` | T-spoof-provenance | regression (NEW) | `cargo test ... provenance::umya_fabricated_refused` | ❌ W0 (NEW) | ⬜ pending |
| WBGV-01 | symmetric demotion classification | T-auto-promote | unit (lift) | `cargo test ... change_class::` | ❌ W0 (lift 8 tests) | ⬜ pending |
| WBGV-02 | assumption change hard-blocks amid hot-reload | T-auto-promote | unit (lift) | `cargo test ... any_assumption_forces` | ❌ W0 | ⬜ pending |
| WBGV-03 | IR sub-DAG hash distinguishes drift vs redefine | — | unit (lift) | `cargo test ... ir_identity::` | ❌ W0 | ⬜ pending |
| WBGV-04 | over-tolerance delta blocks without matching approval | T-auto-promote | integration | `cargo test ... gate::` | ❌ W0 | ⬜ pending |
| WBGV-05 | `--accept` records fingerprint-bound `ApprovalRecord` | T-approval-inherit | integration | `cargo test ... accept::` | ❌ W0 | ⬜ pending |
| WBGV-06 | promote-twice → two `@version` dirs, baseline byte-identical | T-baseline-destroy | integration (lift) | `cargo test ... promote_twice` | ❌ W0 | ⬜ pending |
| WBGV-07 | committed manifest: no out-of-enum seeded default | — | invariant (lift) | `cargo test ... ratify_skips_frozen_enum` | ❌ W0 | ⬜ pending |
| producer/consumer | re-emit reproduces `tax-calc@1.1.0` byte-identical (or structural) | — | integration (NEW) | `cargo test ... reemit_tax_calc_golden` | ❌ W0 (NEW; O-2) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-workbook-compiler/Cargo.toml` + crate skeleton + `[lib]` (NEW crate)
- [ ] Lift + scrub all module tests from the lighthouse (~34 files carry customer identifiers)
- [ ] NEW: `provenance::umya_fabricated_refused` regression test (WBCO-07 record→refuse upgrade)
- [ ] NEW: `reemit_tax_calc_golden` producer/consumer test (depends on O-2 + a neutral fixture `.xlsx`)
- [ ] NEW: D-09 auto-corpus generator + grid-coverage tests
- [ ] grep-gate tests: `delta.abs()` absent in `reconcile/classifier.rs`; `build_reference_manifest` absent on non-test paths; customer-identifier grep returns zero in non-fixture paths
- [ ] Extend `make purity-check`: positive `cargo tree -p pmcp-workbook-compiler -i umya` (reader IS here) + re-run served-crate negatives + `quick-xml`/`zip` single-version guard
- [ ] fuzz targets: formula parser; provenance raw reader (untrusted `.xlsx` ZIP/XML bytes)

---

## ALWAYS Requirements (CLAUDE.md — every new feature)

- **FUZZ:** `cargo fuzz` target over the formula parser AND the provenance raw reader (both attacker-controlled-input surfaces).
- **PROPERTY:** `classify(A→B)`/`classify(B→A)` symmetry-cardinality invariant; "no seeded default is ever outside its cell's `allowed_values`"; "operand-anchored reconcile never accepts a divergence larger than one rounding step."
- **UNIT:** 80%+ coverage per module (lift the lighthouse tests).
- **EXAMPLE:** `cargo run --example compile_a_workbook` demonstrating ingest→emit on the neutral fixture (library-level; CLI is Phase 94).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| BA-lifecycle UX walkthrough (author→build→fix→deploy→update→promote) | acceptance lens | Narrative spans library outputs the CLI (Phase 94) renders; no automated end-to-end shell yet | Drive the library verbs against the neutral fixture; inspect findings/diff/gate structured outputs match the D-07 action buckets |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
