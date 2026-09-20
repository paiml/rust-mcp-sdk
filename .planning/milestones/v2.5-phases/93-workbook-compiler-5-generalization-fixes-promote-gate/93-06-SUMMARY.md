---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
plan: 06
subsystem: workbook-compiler/gate
tags: [promote-gate, auto-corpus, approval-fingerprint, cr-02, atomic-write]
requires:
  - pmcp-workbook-compiler/manifest (Plan 04 — defaults + allowed_values)
  - pmcp-workbook-compiler/sheet_ir (Plan 05 — pure-Rust evaluate_bundle / run_executor)
  - pmcp-workbook-compiler/artifact (Plan 05 — emit_bundle + deterministic serialize)
  - pmcp-workbook-compiler/change_class (Plan 05 — classify + effective_policy)
  - pmcp-workbook-runtime/changelog (VersionChangelog / ChangeClass)
provides:
  - "gate::corpus — auto-derived bounded corpus (D-09) + candidate_fingerprint + ApprovalRecord/ApprovalCase"
  - "gate::governed_artifact — atomic temp->rename approval store + atomic_promote_dir"
  - "gate::gate() — structured GateDecision (Pass|Blocked) with deltas + change class + --accept command"
  - "gate::accept::accept() — fingerprint-bound ApprovalRecord (atomic) + corpus re-baseline"
  - "gate::accept::promote() — CR-02 versioned non-overwriting promote (atomic, EmitLane Seed/GatedUpdate)"
affects:
  - "Phase 94 CLI — renders GateBlock.render() / accept_command; wires the --accept flag"
tech-stack:
  added: [sha2, hex, chrono]
  patterns: [auto-derived-corpus, content-hash-fingerprint, atomic-temp-rename, collect-all-gate]
key-files:
  created:
    - crates/pmcp-workbook-compiler/src/gate/corpus.rs
    - crates/pmcp-workbook-compiler/src/gate/governed_artifact.rs
    - crates/pmcp-workbook-compiler/src/gate/accept.rs
  modified:
    - crates/pmcp-workbook-compiler/src/gate/mod.rs
decisions:
  - "D-09 auto-corpus: the BA authors NO cases — the prior version's own behavior is the golden, captured by replaying an auto-derived grid through the named pure-Rust evaluator"
  - "Numeric-boundary step heuristic: 1 unit for an integer-valued default; 1% of |default| (min 0.01) for a float default — documented in code (numeric_step) + here"
  - "MAX_CORPUS_CASES=50, deterministic grid (default first, then per-enum-member, then numeric-boundary), linear (never combinatorial) — each non-default case varies exactly one input"
  - "Approval storage: <out_root>/<bundle_id>/approvals/<fingerprint>.json, one file per approval named by its content fingerprint; atomic temp->rename"
  - "CR-02 promote via staging-dir -> atomic rename; refuses to overwrite an existing baseline; EmitLane validates the changelog from_version shape (malformed -> ZERO bytes)"
  - "governed_artifact.rs repurposed: the lighthouse module was a governed-data CSV hot-reload channel; in the SDK it is the atomic approval/promote STORE the gate needs (the plan's artifact spec assigns the approval store + atomic write here)"
metrics:
  duration: ~50min
  completed: 2026-06-12
  tasks: 2
  files: 4
  tests_added: 24
---

# Phase 93 Plan 06: Workbook Promote Gate (auto-corpus + fingerprint-bound approval + CR-02) Summary

Lifted the promote-time governance gate and applied the one net-new design — the D-09
auto-derived regression corpus replacing the lighthouse's BA-curated checked-in case file —
concretized per reviewer feedback (named pure-Rust evaluator, numeric-boundary policy,
50-case bounded grid, atomic approval storage). The `candidate_fingerprint` /
`ApprovalRecord` / `accept` / CR-02 promote machinery is lifted verbatim; only case
GENERATION is new.

## What shipped

- **gate::corpus (Task 1, WBGV-04):** `derive_corpus` builds a deterministic bounded case
  grid from the synthesized `Manifest` alone (defaults + enum domains + numeric boundaries),
  replays it through the named pure-Rust `sheet_ir::eval` → `run_executor` for BOTH the prior
  version's IR and the candidate's IR, and captures the PRIOR version's outputs as the golden.
  `candidate_fingerprint` (sha256 over prev-hash + candidate-hash + region deltas),
  `ApprovalRecord`, `ApprovalCase`, `RegionDelta`, `approval_matches` lifted verbatim.
  `MAX_CORPUS_CASES = 50`; `numeric_step` documents the heuristic.
- **gate::governed_artifact (Task 1):** atomic `write_approval` /`read_approvals` at
  `<out_root>/<bundle_id>/approvals/<fingerprint>.json` (temp→rename), plus `atomic_write`
  and `atomic_promote_dir` (staging→rename, refuses to overwrite a baseline).
- **gate::gate() (Task 2, WBGV-04):** structured `GateDecision` (`Pass{fingerprint}` |
  `Blocked(GateBlock)`); a block carries the over-tolerance/missing deltas, the change
  classes, the fingerprint, and `GateBlock::render()` prints the exact
  `--accept --approver --effective-date` command (D-10). Collect-all, ±£0.01 tolerance.
- **gate::accept::accept() (Task 2, WBGV-05):** re-baselines the case golden to the candidate
  computed values AND writes a fingerprint-bound `ApprovalRecord` atomically; the bound
  approval lets the over-tolerance candidate pass.
- **gate::accept::promote() (Task 2, WBGV-06):** CR-02 versioned non-overwriting write via
  `emit_bundle` into a staging root → `atomic_promote_dir` rename to a new
  `{bundle_id}@{version}/` dir. `EmitLane::Seed`/`GatedUpdate` validates the changelog
  `from_version` shape (malformed → ZERO bytes). D-11: `BUNDLE.lock` version ==
  `changelog.to_version`. D-12: first-version Seed no-op establishes the baseline.

## Behavior tests (24 added)

corpus (10): `corpus_auto_derives_from_manifest`, `corpus_grid_is_bounded`,
`corpus_grid_includes_enum_members_and_numeric_boundaries`, `corpus_replays_via_named_evaluator`,
`fingerprint_binds_content`, `approval_mismatch_on_prior_hash_change`,
`approval_mismatch_on_candidate_hash_change`, `approval_mismatch_on_region_delta_change`,
`over_tolerance_blocks`, `no_seeded_default_outside_allowed_values_property` (ALWAYS).
governed_artifact (5): fingerprint-path storage, round-trip+match, empty-set on missing dir,
no-temp-file atomic write, refuse-overwrite promote.
accept (6): `accept_records_and_passes`, `malformed_lane_writes_nothing`,
`first_version_gate_noop`, `promote_twice_two_dirs` (baseline byte-identical),
`promote_is_atomic`, `block_prints_accept_command`.

## Verification

- `cargo test -p pmcp-workbook-compiler` — 236 passed, 0 failed.
- `cargo clippy -p pmcp-workbook-compiler --all-targets` — 0 warnings.
- `cargo fmt -p pmcp-workbook-compiler -- --check` — clean.
- No checked-in case file on the gate production path; `MAX_CORPUS_CASES` present;
  approval storage + atomic temp→rename present; zero customer identifiers in `gate/`.

## Threat surface (from the plan's threat_model — all mitigated)

| Threat | Mitigation (this plan) |
|--------|------------------------|
| T-93-06-INHERIT (approval inherited across unrelated content) | `candidate_fingerprint` binds prev+candidate content hashes; three approval-mismatch tests |
| T-93-06-DESTROY (baseline destruction on promote) | CR-02 non-overwriting write to a new @version dir; `promote_twice_two_dirs` baseline byte-identical |
| T-93-06-BYPASS (over-tolerance change promoted without approval) | gate blocks over-tolerance deltas absent a fingerprint-matched approval; first-version no-op only when no prior baseline |
| T-93-06-PARTIAL (malformed lane / non-atomic write) | EmitLane from_version-shape check → ZERO bytes; atomic temp→rename for @version dirs + approval records |

## Deviations from Plan

**1. [Rule 3 - Blocking issue] `governed_artifact.rs` repurposed for the approval store.**
- Found during: Task 1 read of the lighthouse `gate/governed_artifact.rs`.
- Issue: the lighthouse `governed_artifact.rs` is a governed-data CSV hot-reload channel
  (`emit_governed_artifact` writing `governed.csv` for a runtime reload seam) — that concern
  is NOT part of this gate plan and depends on a customer-specific catalog loader that does
  not exist in the SDK. The plan's `<artifacts>` spec assigns `corpus.rs` the
  `candidate_fingerprint + ApprovalRecord` and needs an atomic approval store + atomic write
  somewhere in `gate/`.
- Fix: implemented `governed_artifact.rs` as the ATOMIC approval/promote STORE the gate
  requires (`write_approval`/`read_approvals` at the spec'd `approvals/<fingerprint>.json`
  path + `atomic_write` + `atomic_promote_dir`). This is the file the plan's atomic-storage
  interface ("ApprovalRecords are written under .../approvals/<fingerprint>.json … ATOMIC
  WRITE: temp→rename") maps onto. The CSV hot-reload channel is out of scope for the promote
  gate.
- Files: crates/pmcp-workbook-compiler/src/gate/governed_artifact.rs
- Commit: 5f4e5708

**2. [Rule 3 - Blocking issue] `Expr::BinaryOp` (not `Expr::Binary`) in a test fixture.**
- Found during: Task 1 build.
- Fix: used the runtime's actual AST variant name `Expr::BinaryOp` in the corpus test helper.
- Commit: 5f4e5708

**3. [Rule 1 - Bug] `LintFinding` has no `Display`.**
- Found during: Task 1 build (the executor returns `Box<LintFinding>`).
- Fix: rendered the replay error via `{e:?}` instead of `.to_string()` in `CorpusError::Replay`.
- Commit: 5f4e5708

## Notes for downstream (Phase 94 CLI)

- The library returns a structured `GateDecision`; the CLI renders `GateBlock::render()` and
  surfaces `accept_command(case_id)`. The `--accept` FLAG itself is Phase 94 — the gate
  BEHAVIOR, `ApprovalRecord` shape, and corpus mechanics are the library verbs built HERE.
- `accept()` and `promote()` are pure library entry points; the driver wiring
  (`compile_workbook`) into them stays a Plan-07/Phase-94 concern (this plan delivered the
  gate verbs, not the end-to-end driver).

## Self-Check: PASSED

- Files: all four `gate/*.rs` FOUND.
- Commits: 5f4e5708, deb9742e FOUND.
- Tests: corpus 10 / governed_artifact 5 / accept 6 — all green; full crate 236 passed.
