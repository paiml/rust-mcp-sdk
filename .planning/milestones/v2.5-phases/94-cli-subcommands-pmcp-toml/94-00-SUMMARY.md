---
phase: 94-cli-subcommands-pmcp-toml
plan: 00
subsystem: pmcp-workbook-compiler
tags: [library-seam, governed-excel, gate-marker, version-accessor, candidate-facade]
requires:
  - "pmcp-workbook-compiler private internals: build_ir_and_dag, seed_from_inputs, comparison_from_outputs, promote_named_outputs, sheet_ir::eval, stage1::run_stage1"
  - "artifact::to_bundle_json (deterministic choke point) + sha256_hex (runtime hash helper)"
  - "ingest::ingest + the owned WorkbookMap"
provides:
  - "pub fn read_workbook_version(workbook_path) -> Result<String, CompileError> (D-02/D-11 version-from-workbook accessor)"
  - "pub fn prepare_candidate(workbook_path, workflow) -> Result<Candidate, CompileError> (gated-update candidate-build facade, gate-before-write)"
  - "pub struct Candidate (ir/dag/manifest/computed/candidate_workbook_hash/parser_equivalence/layout/version; lines up 1:1 with gate::accept::PromoteInputs)"
  - "pub fn write_gate_marker / read_gate_marker + EVIDENCE_GATE_MARKER/EVIDENCE_GATE_DIGEST consts + GateMarker (hash-covered ungated/gated emit marker, D-08)"
affects:
  - "94-03 gated compile lane (WBCL-01): consumes prepare_candidate + Candidate"
  - "94-04 ungated emit marker (WBCL-03): consumes write_gate_marker"
tech-stack:
  added: []
  patterns:
    - "Re-export/compose existing private internals as PUBLIC API — no new compiler/business logic"
    - "#[cfg(test)]-only fixture-override entry (CR-01 reachability pattern) for in-crate proofs"
    - "Self-contained, hash-covered additive evidence channel OUTSIDE the served frozen fold"
key-files:
  created:
    - "crates/pmcp-workbook-compiler/src/version.rs"
    - "crates/pmcp-workbook-compiler/src/prepare_candidate_tests.rs"
  modified:
    - "crates/pmcp-workbook-compiler/src/lib.rs"
    - "crates/pmcp-workbook-compiler/src/artifact/evidence.rs"
    - "crates/pmcp-workbook-compiler/src/artifact/mod.rs"
decisions:
  - "prepare_candidate SKIPS ratify (ratify writes a sidecar; build_ir_and_dag reads only manifest Roles, not the ratification stamp) — keeps prepare write-free without changing the IR"
  - "version_override is a #[cfg(test)]-only Option arg on prepare_candidate_inner: the committed neutral fixture predates the `version` named-range convention, so the test supplies '1.1.0'; production NEVER constructs the override and always reads the workbook"
  - "gate marker is a self-contained channel (its own evidence/gate.sha256 digest), NOT a new EVIDENCE_FOLD_MEMBERS member — adding it to the frozen set would change the served seven-member contract (out of scope)"
metrics:
  duration: ~40m
  completed: 2026-06-13
  tasks: 3
  files_created: 2
  files_modified: 3
  tests_added: 21
---

# Phase 94 Plan 00: Wave-0 Library Seams Summary

Published the three PUBLIC library seams the thin-shell `cargo pmcp workbook` CLI needs — `read_workbook_version` (D-02/D-11 version-from-workbook accessor), `prepare_candidate` + `Candidate` (gated-update candidate-build facade, gate-before-write), and `write_gate_marker`/`read_gate_marker` (hash-covered tamper-evident ungated/gated emit marker, D-08) — by EXPOSING/COMPOSING existing private internals in `pmcp-workbook-compiler`, adding no new compiler/business logic and leaving the served loader's frozen evidence contract provably untouched.

## What Shipped

### Task 1 — `read_workbook_version` (commit `dff5ffe4`)
- New `src/version.rs`: `pub fn read_workbook_version(&Path) -> Result<String, CompileError>` re-uses `ingest::ingest` then resolves the declared version from the owned `WorkbookMap` by MIRRORING the existing `out_*` single-cell defined-name convention (`promote_named_outputs`): it scans `defined_names` for a `version`/`wb_version` (case-insensitive, alias-accepting) single-cell name and reads that cell's cached value.
- A missing declaration is a typed `Err(CompileError::Lint)` naming the convention — NEVER a default (no hardcoded semver on the value path; all semver literals are inside `#[cfg(test)]`).
- Read-only over the owned map — no new umya linkage, no new dependency.
- 8 unit tests (happy path, `wb_version` alias, case-insensitivity, whitespace trim, missing-version → None, empty-cell → None, range-target rejected) + a lossless round-trip property over a 7×7×7 semver grid (no proptest dependency added).

### Task 2 — `prepare_candidate` + `Candidate` (commit `53d73d9f`)
- `pub struct Candidate` carries `ir/dag/manifest/computed/candidate_workbook_hash/parser_equivalence/layout/version` — fields line up 1:1 with `gate::accept::PromoteInputs`.
- `pub fn prepare_candidate(&Path, workflow)` COMPOSES the existing private internals (`ingest::ingest` → `stage1::run_stage1` with `FreshnessPolicy::Enforce` → `promote_named_outputs` → `build_ir_and_dag` → `seed_from_inputs` + `sheet_ir::eval` → `comparison_from_outputs` reconcile) up to BUT NOT including `promote`. It reads the version via Task 1 and projects `Role::Output` finite-`f64` values into the gate's `computed` grading map (`project_named_outputs`).
- Writes nothing (no ratify sidecar, no bundle) — gate-before-write (T-94-00-WRITE). Relaxes no gate (production refuses the same stale fixture `compile_workbook` refuses).
- `build_ir_and_dag`/`seed_from_inputs`/`comparison_from_outputs` are CALLED, defined exactly once each (no duplicated impl).
- In-crate `#[cfg(test)] prepare_candidate_tests.rs` (reaches the `#[cfg(test)]`-only fixture override — CR-01 pattern): IR parity vs the seed lane's `executable.ir.json`, `computed` = manifest named outputs only, writes-nothing, same-gate-under-Enforce, and a `PromoteInputs`-assembled-from-`Candidate` type-check. 5 tests.

### Task 3 — `write_gate_marker` (commit `ce8053d2`)
- `artifact/evidence.rs`: `EVIDENCE_GATE_MARKER`/`EVIDENCE_GATE_DIGEST` path consts, `GateMarker { gated }`, `write_gate_marker(bundle_dir, gated) -> Result<String, EmitError>` (writes `evidence/gate.json` via the deterministic `to_bundle_json` choke point + records `sha256_hex` into `evidence/gate.sha256`, returns the digest), and `read_gate_marker(bundle_dir) -> Result<(gated, digest_ok), EmitError>` recomputing the digest for tamper-evidence (T-94-00-MARKER, D-08).
- SELF-CONTAINED additive channel: does NOT touch `EvidenceInputs`/`emit_evidence` or the served loader's FROZEN `EVIDENCE_FOLD_MEMBERS`/`ALLOWED_MEMBERS`. An unchanged-fold test asserts `emit_evidence` byte-stable + `EVIDENCE_FOLD_MEMBERS.len() == 4`, and the runtime crate `git diff` is EMPTY (T-94-00-FROZEN).
- 5 marker tests (ungated/gated round-trip, digest-covers-exact-bytes, tamper-detection, unchanged-fold). Re-exported from `artifact/mod.rs` + `lib.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] proptest is not a dependency of the compiler crate**
- **Found during:** Task 1.
- **Issue:** The plan's Task 1 action suggested a proptest round-trip property, but `pmcp-workbook-compiler` has no `proptest` dev-dependency, and the acceptance criteria explicitly forbid introducing a new dependency.
- **Fix:** Implemented the lossless round-trip as a deterministic exhaustive-grid property (7×7×7 semver components) using only `std` — same invariant, no new dependency.
- **Files modified:** `crates/pmcp-workbook-compiler/src/version.rs`
- **Commit:** `dff5ffe4`

**2. [Rule 3 - Blocking] committed neutral fixture declares no `version` named range**
- **Found during:** Task 2.
- **Issue:** `prepare_candidate` reads the workbook-declared version (Task 1), but the committed `tax-calc.xlsx` fixture predates the `version` convention and declares none — so the fixture-override test path errored before returning a `Candidate`, blocking the parity/writes-nothing/PromoteInputs tests.
- **Fix:** Added a `#[cfg(test)]`-only `version_override: Option<&str>` parameter to the private `prepare_candidate_inner`. The public `prepare_candidate` passes `None` (version comes SOLELY from the workbook in production — D-02/D-11 preserved); only the `#[cfg(test)]` `prepare_candidate_with_fixture_override` supplies `Some("1.1.0")`, matching the seed-lane proof. Production never constructs the override (same CR-01 reachability posture as the trusted-fixture freshness override).
- **Files modified:** `crates/pmcp-workbook-compiler/src/lib.rs`
- **Commit:** `53d73d9f`

## Verification
- `cargo test -p pmcp-workbook-compiler --lib` → 265 passed, 0 failed.
- `cargo test -p pmcp-workbook-compiler --lib version::` → 8 passed; `prepare_candidate` → 5 passed; `artifact::evidence` → 8 passed.
- `cargo clippy -p pmcp-workbook-compiler --all-targets` → zero warnings.
- `make quality-gate` → PASSED (fmt/clippy/build/test/audit + ALWAYS requirements + purity-check: umya confined to the compiler, served crates reader-free).
- `git diff --stat crates/pmcp-workbook-runtime/` → EMPTY (served frozen contract untouched).

## Known Stubs
None — all three seams are wired to real existing internals; no placeholder/empty data paths introduced.

## Self-Check: PASSED
- `crates/pmcp-workbook-compiler/src/version.rs` — FOUND
- `crates/pmcp-workbook-compiler/src/prepare_candidate_tests.rs` — FOUND
- `crates/pmcp-workbook-compiler/src/lib.rs` — FOUND (Candidate + prepare_candidate + read_workbook_version re-export)
- `crates/pmcp-workbook-compiler/src/artifact/evidence.rs` — FOUND (write_gate_marker/read_gate_marker)
- `crates/pmcp-workbook-compiler/src/artifact/mod.rs` — FOUND (re-exports)
- Commits `dff5ffe4`, `53d73d9f`, `ce8053d2` — present in git history
