---
phase: 92-bundlesource-served-tool-toolkit-module
plan: 07
subsystem: workbook
tags: [workbook, served-tool, fail-closed, validation, provenance, security, WBSV-06]

# Dependency graph
requires:
  - phase: 92-06
    provides: "seed-preserving executor (env.get guard) — which turned the unfiltered override accept arm into a live output-forging vector this plan closes"
provides:
  - "role-filtered override gate rejecting Role::Output/Formula overrides with unsupported_option"
  - "fail-closed project_outputs (invalid_input on a declared-but-uncomputed output)"
  - "absent-anchor-rejecting verify_stamp_binding (StampMismatch <absent> instead of vacuous empty==empty)"
  - "three regression tests proving the override, output-projection, and stamp-binding paths fail closed"
affects: [93-workbook-compiler, 95-shape-a-binary]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Role-filtered match arm guarding the override accept path against computed cells"
    - "let-else fail-closed rejection of an absent provenance anchor (no empty-string default)"

key-files:
  created: []
  modified:
    - crates/pmcp-server-toolkit/src/workbook/input.rs
    - crates/pmcp-server-toolkit/src/workbook/handler.rs
    - crates/pmcp-workbook-runtime/src/bundle_loader.rs

key-decisions:
  - "A forbidden-role (Output/Formula) override surfaces the same unsupported_option allowed-list as an unknown key, mirroring variable_tier_keys' own filter so the accept arm cannot contradict the module's allow-list"
  - "A declared-but-uncomputed output is a cell_map/IR skew (invalid_input), never a silent success — preserves outputSchema/payload parity (WBSV-07)"
  - "An absent layout.source_workbook_hash is rejected explicitly (member_value <absent>), never defaulted to an empty string that would pass vacuously against an empty lock hash"

patterns-established:
  - "Fail-closed validation across every untrusted trust boundary (agent overrides, cell_map/IR skew, bundle provenance anchor)"

requirements-completed: [WBSV-06]

# Metrics
duration: 18min
completed: 2026-06-11
---

# Phase 92 Plan 07: Close Three Fail-Open Validation Paths Summary

**Role-filtered override gate (rejects Role::Output/Formula), fail-closed output projector, and absent-anchor-rejecting stamp gate — closing the three remaining fail-open holes so WBSV-06's "fails closed on any validation gap" claim holds.**

## Performance

- **Duration:** ~18 min
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- WR-02: override accept arm now rejects a `Role::Output`/`Role::Formula` key with an `unsupported_option` isError envelope before it can be seeded or echoed in `accepted_overrides` — closing the live output-forging vector that 92-06's seed-preserving executor would otherwise expose (a caller pinning a served output under a valid provenance stamp).
- WR-04: `project_outputs` fails closed with `invalid_input` ("…was not computed by the bundle IR") on a declared output absent from the run result, instead of silently dropping it — so the advertised `outputSchema` (WBSV-07) and the served payload can never diverge.
- WR-07: `verify_stamp_binding` rejects an absent `layout.source_workbook_hash` with `StampMismatch` `member_value: "<absent>"`, instead of `unwrap_or("")` which let an absent anchor + empty `lock.workbook_hash` pass the boot security gate vacuously.
- Four regression tests added (two for WR-02 output/formula, two for WR-04 missing/all-present, one for WR-07), all passing; full toolkit (176 unit + 35 doctest) and runtime (148) suites green.

## Task Commits

1. **Task 1: Close override role-filter (WR-02) + output-projection (WR-04)** - `a9adad15` (fix)
2. **Task 2: Reject absent stamp anchor (WR-07) + regression tests** - `c55bbf1e` (fix)

## Files Created/Modified
- `crates/pmcp-server-toolkit/src/workbook/input.rs` - added the `Some(r) if matches!(r.role, Role::Output | Role::Formula)` reject arm before the accept arm; added WR-02 output/formula override-rejection tests + `manifest_with_computed_cells` fixture.
- `crates/pmcp-server-toolkit/src/workbook/handler.rs` - replaced `else { continue; }` with a fail-closed `return Err(invalid_input(...))` in `project_outputs`; added WR-04 missing-output and all-present tests.
- `crates/pmcp-workbook-runtime/src/bundle_loader.rs` - replaced `unwrap_or("")` with a `let-else` that returns `StampMismatch { member_value: "<absent>" }`; added the `golden_with_absent_anchor_and_empty_lock_hash` helper + WR-07 test.

## Decisions Made
- Forbidden-role overrides reuse `unsupported_option` with `variable_tier_keys(manifest)` (identical to the unknown-key None arm) so a forbidden-role and an unknown key surface the same machine-actionable allowed-list, and the forbidden key is never offered as an allowed override.
- The WR-04 fail-closed message is prefixed `internal:` to mark it as a cell_map/IR consistency failure (not a caller error), matching the existing `finite_output_value` `invalid_input` shape.
- The WR-07 fixture builds a fully integrity-consistent golden (evidence/lock recomputed over the exact absent-anchor bytes) so the integrity gate passes and the stamp gate is provably what fires.

## Deviations from Plan

None - plan executed exactly as written. (One acceptance-criterion grep — `grep -c 'unwrap_or("")' == 0` — initially flagged the literal string inside a new test comment; the comment was reworded to "empty-default"/"empty == empty" so the code change is the only `unwrap_or("")` removal and the grep is honest. No behavioral change.)

## Issues Encountered
- The pre-existing `cargo build` warning `unused import: pmcp_code_mode::CodeExecutor` at `crates/pmcp-server-toolkit/src/code_mode.rs:557` surfaced again. It is unrelated to this plan's files (input.rs/handler.rs/bundle_loader.rs), already documented in `deferred-items.md`, not on the CI clippy-gate scope, and left untouched per the executor scope boundary. `make lint` (the real CI gate, root `pmcp` scope) passes clean.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- WBSV-06 is now fully satisfied: validation is fail-closed across the override, output-projection, and stamp-binding paths. The bundle contract is locked from the consumer side, ready for the Phase 93 compiler re-cut to target it.
- No blockers.

---
*Phase: 92-bundlesource-served-tool-toolkit-module*
*Completed: 2026-06-11*

## Self-Check: PASSED
- FOUND: crates/pmcp-server-toolkit/src/workbook/input.rs
- FOUND: crates/pmcp-server-toolkit/src/workbook/handler.rs
- FOUND: crates/pmcp-workbook-runtime/src/bundle_loader.rs
- FOUND commit: a9adad15
- FOUND commit: c55bbf1e
