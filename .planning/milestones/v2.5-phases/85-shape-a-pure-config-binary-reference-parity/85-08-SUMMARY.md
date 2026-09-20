---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 08
subsystem: pmcp-sql-server (reference-parity test gate)
tags: [test-validity, gap-closure, code-mode-policy, sc-3, gating-assertion]
gap_closure: true
requires:
  - "85-07 (require_limit enforcement so the no-LIMIT rejection genuinely fires)"
  - "85-09 (prompt synthesis so start_code_mode scenario resolves)"
provides:
  - "per-step gating of the parity replay — every StepResult.success asserted, continue_on_failure-independent"
  - "presence guard for the 5 policy-rejection scenarios (cannot silently vanish from generated.yaml)"
affects:
  - "crates/pmcp-sql-server/tests/parity_chinook.rs"
tech-stack:
  added: []
  patterns:
    - "Gate on result.step_results[i].success (per-step, pre-exclusion) instead of result.success (continue_on_failure-excluding aggregate)"
    - "Const name-list presence guard so a deleted/renamed scenario fails the test loudly"
key-files:
  created: []
  modified:
    - "crates/pmcp-sql-server/tests/parity_chinook.rs"
decisions:
  - "Fix lives in the test, not the fixtures — generated.yaml byte-unchanged (no continue_on_failure removed, no `failure` assertion relaxed)"
  - "Gate on StepResult.success rather than removing continue_on_failure from fixtures: StepResult.success is the per-step truth computed BEFORE the aggregate exclusion (scenario_executor.rs:111-118), so it is a continue_on_failure-independent gate"
  - "Added a const REQUIRED_REJECTION_SCENARIOS presence guard + WHY doc-comment to prevent a future regression-to-`assert!(result.success)`"
metrics:
  duration: ~12m
  completed: 2026-05-27
---

# Phase 85 Plan 08: Make Policy-Rejection Scenarios Individually Gating Summary

Strengthened `parity_chinook.rs` so the SC-3 negative-path parity proof is actually
gating: it now asserts every `result.step_results[i].success` (the per-step truth,
computed before the `continue_on_failure` exclusion that weakens `result.success`),
plus a presence guard for the 5 policy-rejection scenarios — closing VERIFICATION
Gap 2 without weakening any fixture.

## What Changed

`crates/pmcp-sql-server/tests/parity_chinook.rs`:

1. **Replaced the weak aggregate gate.** The old `assert!(result.success, …)`
   excluded every `continue_on_failure` step (`scenario_executor.rs:111-118`), so a
   regressed policy rejection (e.g. the pre-fix no-LIMIT `SELECT * FROM Artist`) was
   silently dropped and the test stayed green. Now two assertions gate the test:
   - **(b) Per-step gate:** `result.step_results.iter().filter(|s| !s.success)` must
     be empty. `StepResult.success` is set from the step's own
     `assertion_results.all(passed)` (`scenario_executor.rs:154`) BEFORE the aggregate
     exclusion, so this binds every step — INCLUDING the `continue_on_failure`
     rejection scenarios — regardless of `continue_on_failure`.
   - **(a) Presence guard:** a `const REQUIRED_REJECTION_SCENARIOS: &[&str]` lists the
     5 rejection scenario names verbatim; each must appear in
     `result.step_results.iter().map(|s| &s.step_name)`. Without this, deleting or
     renaming a rejection scenario would shrink the suite and make the per-step gate
     trivially pass.

2. **Added a WHY doc-comment** on the const explaining the Gap-2 masking (quoting the
   `scenario_executor.rs` exclusion) and a "DO NOT simplify back to
   `assert!(result.success)`" warning, so a future reader does not reintroduce the gap.

3. **Left the readiness/spawn/replay logic unchanged**, including `handle.abort()`.

The 5 presence-guarded names (verbatim from `generated.yaml`, all
`continue_on_failure: true` + `type: failure`):
`Validate: DELETE should be rejected`, `Validate: DDL (CREATE TABLE) should be rejected`,
`Validate: DROP TABLE should be rejected`, `Validate: SELECT without LIMIT should be rejected`,
`Execute: with invalid token (should fail)`.

## Two-Sided Regression Proof

The acceptance criterion requires proving the strengthened test FAILS if Gap 1 were
unfixed and PASSES now. Performed against the live tree:

### Side 1 — fix present (current tree): PASS

```
$ CODE_MODE_SECRET=parity-chinook-code-mode-secret-32b cargo test -p pmcp-sql-server \
    --no-default-features --features sqlite --test parity_chinook -- --test-threads=1
cargo test: 1 passed (1 suite, 0.59s)
```

With Plan 85-07's `cfg.sql_require_limit = section.require_limit;` in place, the
no-LIMIT `SELECT * FROM Artist` is correctly rejected, every `StepResult.success` is
`true`, and the per-step gate passes.

### Side 2 — Gap 1 reverted (temporary, NOT committed): FAIL on the no-LIMIT step

Temporarily neutralized the 85-07 mapping at
`crates/pmcp-server-toolkit/src/code_mode.rs:502`
(`cfg.sql_require_limit = section.require_limit;` → `cfg.sql_require_limit = false;`)
and re-ran. The mcp-tester scenario summary STILL printed `Status: PASSED` (proving the
old `result.success` aggregate would have masked the regression — Gap 2), but the
strengthened per-step gate caught it:

```
Scenario Summary:
  Status: PASSED            <-- the masking aggregate would have been green
  Steps: 29/29

thread 'chinook_reference_parity_through_real_binary_path' panicked at
crates/pmcp-sql-server/tests/parity_chinook.rs:255:5:
every reference-parity step must pass its own assertions — rejection scenarios are
individually gating, so a `continue_on_failure` step that failed its `type: failure`
assertion is no longer masked (VERIFICATION Gap 2). 29/29 steps completed; error=None;
failed steps=[
    (
        "Validate: SELECT without LIMIT should be rejected",
        None,
        [ ("Failure", Some(Null), Some("Expected failure response with error")) ],
    ),
]
test result: FAILED. 0 passed; 1 failed; ...
```

The test failed on EXACTLY the `Validate: SELECT without LIMIT should be rejected`
step — the precise Gap-2 masking class. This is the proof that the negative-path
parity proof is now gating.

### Side 3 — fix restored: PASS

Reverted `code_mode.rs:502` back to `cfg.sql_require_limit = section.require_limit;`.
`git diff --stat crates/pmcp-server-toolkit/src/code_mode.rs` is empty (byte-identical
to HEAD). Re-ran the parity test: `1 passed`.

## Verification

| Check | Command | Result |
|-------|---------|--------|
| Parity test passes with fix present | `cargo test -p pmcp-sql-server --no-default-features --features sqlite --test parity_chinook -- --test-threads=1` | `1 passed` |
| Per-step gating present (not just diagnostic) | `grep -n "step_results" crates/pmcp-sql-server/tests/parity_chinook.rs` | matches at the gate (lines 218, 240) |
| Presence guard for the 5 names | `grep -n "SELECT without LIMIT should be rejected" crates/pmcp-sql-server/tests/parity_chinook.rs` | matches (line 109) |
| Fixtures byte-unchanged | `git status --porcelain crates/pmcp-sql-server/tests/fixtures/` | empty |
| code_mode.rs fully restored | `git diff --stat crates/pmcp-server-toolkit/src/code_mode.rs` | empty |
| Touched file clippy-clean | `cargo clippy -p pmcp-sql-server --no-default-features --features sqlite --tests -- -D warnings` | zero warnings attributable to `parity_chinook.rs` |

## Deviations from Plan

None — plan executed exactly as written. The fix lives entirely in
`parity_chinook.rs`; no fixtures touched; the two-sided regression proof was performed
with a temporary, reverted `code_mode.rs` change that is NOT committed.

## Deferred Issues

The local toolchain (rust-1.95.0) reports a pre-existing
`clippy::field_reassign_with_default` at `crates/pmcp-server-toolkit/src/code_mode.rs:471-472`
(`build_cm_config`'s `let mut cfg = CodeModeConfig::default(); cfg.enabled = …`). This is
the SAME pre-existing Phase 83 lint already logged in
`.planning/phases/85-shape-a-pure-config-binary-reference-parity/deferred-items.md`
(under Plans 85-03 and 85-07) and is explicitly deferred per the gap context
("pmcp-server-toolkit pedantic clippy lints (rust-1.95.0) are deferred; keep YOUR
touched files clean"). It is NOT in this plan's touched file and was NOT introduced by
this work. No new entry required.

## Known Stubs

None.

## Commit

- `eec2941a`: test(85-08): make policy-rejection scenarios individually gating (`crates/pmcp-sql-server/tests/parity_chinook.rs`)

## Self-Check: PASSED

- FOUND: `crates/pmcp-sql-server/tests/parity_chinook.rs`
- FOUND: `.planning/phases/85-shape-a-pure-config-binary-reference-parity/85-08-SUMMARY.md`
- FOUND: commit `eec2941a` (test(85-08))
- No file deletions in the task commit.
