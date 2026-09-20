---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 03
subsystem: pmcp-workbook-runtime
tags: [refactor, pmat-gate, cognitive-complexity, bundle-loader]
requires:
  - "PR #279 PMAT complexity gate (CI-only)"
provides:
  - "bundle_loader::load cleared from PMAT cog gate (28 -> 4)"
affects:
  - "crates/pmcp-workbook-runtime/src/bundle_loader.rs"
tech-stack:
  added: []
  patterns:
    - "multi-phase function split into per-phase helpers with a thin orchestrator"
    - "byte-reuse across phases (verify_integrity returns ir/manifest bytes the caller parses once)"
key-files:
  created: []
  modified:
    - "crates/pmcp-workbook-runtime/src/bundle_loader.rs"
decisions:
  - "Split load along its existing 5 numbered phases rather than per-member: phase 1 -> enforce_member_allow_set, phase 2 -> verify_integrity, phase 3 -> parse_members; phases 4/5 stay inline (already trivial)."
  - "verify_integrity returns (lock, ir_bytes, manifest_bytes) so the integrity recompute and the typed parse share a single read of those members (no behavior change, no extra source reads)."
  - "Introduced a small ParsedMembers struct so parse_members can return all five typed members without a 5-tuple."
metrics:
  duration: "~6 min"
  completed: "2026-06-17"
  tasks: 2
  files: 1
---

# Phase 99 Plan 03: Decompose bundle_loader::load below PMAT gate Summary

Refactored `pmcp-workbook-runtime/src/bundle_loader.rs::load` from cognitive complexity 28 to 4 by splitting its five numbered phases into named private helpers, clearing the PR #279 PMAT complexity gate with zero behavior change.

## What Was Done

`load` was a single 50-line function carrying all five fail-closed verification phases inline (membership allow-set, integrity recompute, member parse, stamp binding, DAG build), peaking at nesting depth 9 → cog 28.

Decomposition (Task 1):
- **`enforce_member_allow_set`** (cog 8) — phase 1: lists artifacts, maps source errors, rejects any member outside `ALLOWED_MEMBERS`.
- **`verify_integrity`** (cog 11) — phase 2: parses the lock, recomputes the artifact/combined hashes via the runtime's own `build_bundle_lock`, fails closed on mismatch. Returns `(BundleLock, ir_bytes, manifest_bytes)` so the bytes it already read are reused by the parse phase.
- **`member_utf8`** (cog 0) — UTF-8 decode helper shared by the integrity recompute (was two inline `from_utf8` blocks).
- **`parse_members`** (cog 8) + `ParsedMembers` struct — phase 3: total panic-free parse of all five members into typed values.
- **`load`** (cog 4) — now a thin orchestrator: `enforce_member_allow_set` → `verify_integrity` → `parse_members` → `verify_stamp_binding` → `build_dag` → assemble `WorkbookBundle`.

Every `BundleLoadError` variant (`Source`, `UnexpectedMember`, `Parse`, `IntegrityMismatch`, `StampMismatch`) and the exact part-presence/ordering requirements are preserved. `verify_stamp_binding` and the read/parse/evidence helpers were untouched.

## Verification

| Check | Result |
| ----- | ------ |
| `load` cognitive complexity | 28 → **4** |
| Max cog in `bundle_loader.rs` (any fn) | **11** (`verify_integrity`); target ≤ 23 |
| `bundle_loader.rs` in PMAT violation report | **absent** (was flagging `load`) |
| `pmat quality-gate --checks complexity` violations for the file | **none** |
| `cargo test -p pmcp-workbook-runtime` | **157 passed** (same as baseline) |
| `cargo clippy -p pmcp-workbook-runtime --all-features -- -D warnings` | **clean, no issues** |

Per-function cog after refactor: `read_member` 1, `parse_member` 0, `recompute_evidence_hash` 3, `enforce_member_allow_set` 8, `member_utf8` 0, `verify_integrity` 11, `parse_members` 8, `verify_stamp_binding` 9, `load` 4.

Regression guard `load_valid_golden_returns_populated_bundle` (and the sibling tamper/stamp-mismatch tests) green.

## Deviations from Plan

None — plan executed as written. The plan's verify command references `.violations[]` (the `pmat quality-gate` JSON shape); the `pmat analyze complexity --format json` oracle exposes the same data under `.files[].functions[].metrics.cognitive`. Both confirm the file is fully cleared. The two-task plan (decompose, then clippy+commit) was landed as a single `refactor(99)` commit per Task 2's commit directive.

## Commits

- `3838535c` refactor(99): decompose bundle_loader::load below PMAT gate

## Self-Check: PASSED

- FOUND: crates/pmcp-workbook-runtime/src/bundle_loader.rs (modified, committed)
- FOUND: commit 3838535c in git log
- Out-of-scope `.pmat/context.db*` runtime artifacts left unstaged (not part of this task)
