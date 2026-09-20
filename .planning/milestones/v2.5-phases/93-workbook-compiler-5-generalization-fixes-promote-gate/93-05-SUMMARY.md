---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
plan: 05
subsystem: pmcp-workbook-compiler (artifact emit + change-class)
tags: [bundle-emit, deterministic-serialization, change-class, promote-gate, generalization]
requires:
  - pmcp-workbook-runtime (artifact_model, changelog, render, bundle_loader, manifest_model)
  - pmcp-workbook-compiler/ingest (WorkbookMap/SheetRecord/CellRecord — wave 1-3)
  - pmcp-server-toolkit (workbook feature — Phase 92 served loader, dev-dep only)
provides:
  - artifact::emit_bundle (seven-member deterministic bundle emit, bundle_id BUNDLE.lock)
  - artifact::serialize (deterministic JSON choke point)
  - artifact::{build_cell_map, build_layout_descriptor, ParserEquivalence, ratify_tiers WR-01 skip}
  - change_class::{classify, effective_policy, policy, block_message, GatePolicy}
  - change_class::ir_subdag_hash (canonical IR sub-DAG identity hash)
  - change_class::diff_outputs (redefinition predicate -> VersionChangelog)
affects:
  - Plan 06 (reconcile -> the compiler driver wires emit + change-class gate)
  - Plan 07 (the compile_workbook driver composes emit + change-class against the prior baseline)
tech-stack:
  added: [tempfile (dev), pmcp-server-toolkit/workbook (dev)]
  patterns: [re-export-don't-re-declare, deterministic-serialization-choke-point, runtime-shared-hash-helpers]
key-files:
  created:
    - crates/pmcp-workbook-compiler/src/artifact/serialize.rs
    - crates/pmcp-workbook-compiler/src/artifact/bundle_lock.rs
    - crates/pmcp-workbook-compiler/src/artifact/cell_map.rs
    - crates/pmcp-workbook-compiler/src/artifact/evidence.rs
    - crates/pmcp-workbook-compiler/src/artifact/executable.rs
    - crates/pmcp-workbook-compiler/src/artifact/layout.rs
    - crates/pmcp-workbook-compiler/src/change_class/ir_identity.rs
    - crates/pmcp-workbook-compiler/src/change_class/schema_diff.rs
  modified:
    - crates/pmcp-workbook-compiler/src/artifact/mod.rs
    - crates/pmcp-workbook-compiler/src/change_class/mod.rs
    - crates/pmcp-workbook-compiler/src/lib.rs
    - crates/pmcp-workbook-compiler/Cargo.toml
decisions:
  - "emit_bundle conformed to the runtime's EXACT seven-member contract (NOT the lighthouse's broader EvidenceInputs/renderer_equivalence/version-stamp-gate shape) — the served loader's ALLOWED_MEMBERS allow-set fails closed on any extra member"
  - "CellMap drops the lighthouse's hardcoded supply_total_cell (§5 generalization): the runtime CellMap is {inputs, outputs} with no privileged headline; build_cell_map fails loud only on zero outputs"
  - "WR-01 verified against the COMMITTED golden by resetting its real frozen-enum cell's tier to None (the golden authored it WITH a tier), proving ratify_tiers skips it"
  - "evidence.rs folds exactly the loader's EVIDENCE_FOLD_MEMBERS via the runtime fold_evidence_hash — a permutation-equality test guards against drift"
metrics:
  duration: ~50min
  tasks: 2
  files: 12
  tests_added: 28 (artifact group) + 28 (change_class group) net-new
  completed: 2026-06-12
---

# Phase 93 Plan 05: Bundle-Emit + Change-Class Summary

Seven-member deterministic bundle emit (`bundle_id` BUNDLE.lock via the runtime's shared hash helpers, WR-01 enum-tier skip) plus the symmetric demotion-aware change-class classifier, strictest-policy reducer, and canonical IR sub-DAG identity hash — lifted from the lighthouse WITH their tests, conformed to the Phase 92 served contract, and proven by a direct toolkit loader round-trip.

## What shipped

### Task 1 — Deterministic serialize + seven-member emit + WR-01 (WBCO-05/WBGV-07) — commit `461cdd96`

- **`serialize.rs`** — the single deterministic-JSON choke point. Policy pinned from the committed Phase 92 golden (`tax-calc@1.1.0/`): pretty 2-space, NO trailing newline (every golden file's last byte is `}`), stable struct field order, and a `HashMap`→`BTreeMap` sorted-key projection for the IR map. Two emits of the same content are byte-identical (threat T-93-05-NONDET closed).
- **`mod.rs::emit_bundle`** — writes EXACTLY the seven members the Phase 92 loader's `ALLOWED_MEMBERS` allow-set requires: `executable.ir.json`, `manifest.json`, `cell_map.json`, `layout.json`, `BUNDLE.lock`, `evidence/changelog.json`, `evidence/parser_equivalence.json`. Emits to a `tempfile::TempDir` in tests (no workspace leakage).
- **`bundle_lock.rs`** — re-export shim over the runtime's `build_bundle_lock`/`fold_evidence_hash`/`sha256_hex`. The combined hash is NEVER hand-rolled (threat T-93-05-HASH); `BUNDLE.lock` emits `bundle_id` (D-17), not the lighthouse `workflow` field.
- **`evidence.rs`** — folds exactly the loader's `EVIDENCE_FOLD_MEMBERS` (`cell_map.json` + `evidence/changelog.json` + `evidence/parser_equivalence.json` + `layout.json`) through the runtime's own `fold_evidence_hash`; a permutation-equality test guards the member set.
- **`ratify_tiers`** WR-01 enum-tier skip lifted verbatim — a frozen-enum (`allowed_values: Some`) input is never auto-seeded a default-bearing tier (threat T-93-05-ENUM). Verified against the COMMITTED golden manifest.
- **`cell_map.rs`** — §5 generalization: the runtime `CellMap{inputs, outputs}` drops the lighthouse's hardcoded `supply_total_cell` headline; no single output is privileged.
- **`emitted_bundle_loads_via_toolkit`** — direct round-trip: an emitted temp bundle loads through `pmcp-server-toolkit::workbook`'s fail-closed loader, exposing the stamp the emitter wrote.

### Task 2 — Symmetric classifier + strictest-policy reducer + IR identity (WBGV-01/02/03) — commit `64043ca1`

- **`change_class/mod.rs`** — CR-01 symmetric classifier: assumption involvement on EITHER side → `Assumption`; role flips AWAY from Input/Output are schema changes; the `Constant | Formula` arm no longer silently drops a demotion (threat T-93-05-PROMO). `effective_policy` is the derived-`Ord` strictest-policy reducer (any `Assumption` forces `NeverAutoPromote`).
- **`ir_identity.rs`** — `ir_subdag_hash`: a stable canonical IR sub-DAG identity hash (transitive-precedent set, deterministic topological order, length-prefixed fold via the runtime's `update_field`) distinguishing numeric drift from semantic redefinition.
- **`schema_diff.rs`** — `diff_outputs` redefinition predicate → `VersionChangelog`.
- `ChangeClass`/`OutputDelta`/`VersionChangelog` re-exported from the runtime (the served `diff_version` tool reads the SAME enum; no local re-declaration).
- The 8 demotion tests came over green; added a **symmetry-cardinality PROPERTY test** (ALWAYS requirement) asserting `classify(A→B).len() == classify(B→A).len()` across Input⇄Constant, Output⇄Formula, assumption demotion/promotion, and add⇄remove scenarios.

## Verification

| Gate | Result |
|------|--------|
| `cargo test -p pmcp-workbook-compiler artifact::` | 25 passed |
| `cargo test -p pmcp-workbook-compiler change_class::` | 28 passed |
| Full crate suite (`cargo test -p pmcp-workbook-compiler`) | 215 passed, 0 failed |
| `serialize_is_deterministic` | pass (two emits byte-identical) |
| `emitted_bundle_loads_via_toolkit` | pass (direct loader round-trip) |
| `ratify_skips_frozen_enum_inputs` (vs committed golden) | pass |
| `any_assumption_forces_never_auto_promote` | pass |
| `classify_is_symmetric_in_cardinality` (property) | pass |
| Runtime hash-helper grep gate (bundle_lock.rs) | pass |
| No local `pub enum ChangeClass` re-declaration | pass |
| `tempfile::TempDir` in artifact tests | pass |
| Customer-identifier scrub (ufh/towelrad/coil/heat_pump/…) | ZERO |
| `cargo clippy -p pmcp-workbook-compiler --all-targets -- -D warnings` | zero warnings |
| `cargo fmt -p pmcp-workbook-compiler -- --check` | clean |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] WR-01 golden test precondition corrected**
- **Found during:** Task 1 (first test run)
- **Issue:** The committed Phase 92 golden authored its frozen-enum input (`1_Inputs!B3`) WITH a tier already present, so a test asserting "an untiered frozen-enum input exists in the golden" failed its precondition.
- **Fix:** The WR-01 invariant is the SKIP, not the golden's authored state. The test now takes the golden's REAL frozen-enum cell (its committed `allowed_values` domain), resets every input's tier to `None` to model the pre-ratification candidate, and asserts `ratify_tiers` leaves the frozen-enum cell untiered while tiering a plain input to `Variable` — verified against the committed golden's actual data.
- **Files modified:** `crates/pmcp-workbook-compiler/src/artifact/mod.rs`
- **Commit:** `461cdd96`

**2. [Rule 3 - Blocking] Conformed emit_bundle to the runtime's seven-member contract**
- **Found during:** Task 1 (design)
- **Issue:** The lighthouse `emit_bundle` emits a broader/different member set (renderer_equivalence, EvidenceInputs corpus, a version-stamp gate) and a `CellMap` with `supply_total_cell` + `plot3_json_key` fields — none of which match the runtime's frozen `ALLOWED_MEMBERS` / `CellMap{inputs,outputs}` / `CellEntry{json_key,seed_coord,unit}` shapes. A verbatim lift would fail the served loader's fail-closed membership gate.
- **Fix:** Rewrote `emit_bundle`/`cell_map.rs`/`evidence.rs` to the EXACT runtime contract (seven members, the loader's `EVIDENCE_FOLD_MEMBERS` fold, `bundle_id` rename, no `supply_total_cell`). `ratify_tiers` and `build_layout_descriptor` were lifted verbatim (paths renamed). This realizes the §5 generalization the plan calls for.
- **Files:** `artifact/{mod,cell_map,evidence}.rs`
- **Commit:** `461cdd96`

### Doc/format adjustments

- One clippy `doc_lazy_continuation` warning in `evidence.rs` (a module-doc line beginning with `+` read as a markdown list continuation) — rephrased the frozen-member-set sentence. Zero warnings after.

## Authentication gates

None.

## Known Stubs

None. The artifact and change_class stubs (Plan 01) are fully filled. The downstream `compile_workbook` driver (`lib.rs`) remains a documented `NotImplemented` stub by design — it is Plan 07's composition site, not in this plan's scope.

## Self-Check: PASSED

- All eight created files exist on disk (verified).
- Both commits exist: `461cdd96`, `64043ca1` (verified via `git log`).
- 215/215 crate tests pass; zero clippy warnings; zero customer identifiers.
