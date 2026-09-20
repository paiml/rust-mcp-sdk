---
verification: D-11 badge/gate command alignment
pmat_version: pmat 3.15.0
date: 2026-04-23
---

# Badge vs CI Gate Command Alignment Verification

## Bare gate command (drives the README badge today)

Command: `pmat quality-gate --fail-on-violation`
Exit code: **1**

Per-check failure contribution (from `--format json` violations array):

| Check        | Count | Severity mix                                     | Fails gate? |
|--------------|-------|--------------------------------------------------|-------------|
| complexity   | 94    | 85 error + 9 warning                             | **yes**     |
| duplicate    | 1545  | all warning                                      | **yes**     |
| entropy      | 13    | all warning                                      | **yes**     |
| satd         | 33    | all info                                         | **yes** (verified by isolating with `--checks satd`: bare gate exits 1) |
| sections     | 2     | all warning                                      | **yes**     |

`pmat quality-gate --fail-on-violation` returns non-zero whenever `results.passed == false`, and `passed` is false when ANY check has a non-zero violation count — independent of severity. This was confirmed by running `pmat quality-gate --fail-on-violation --checks satd`: 33 info-level SATD violations still produce exit code 1.

Cross-reference with CONTEXT.md baseline:

| Dimension  | CONTEXT.md count | This run's count | Note                                                                                                       |
|------------|------------------|------------------|------------------------------------------------------------------------------------------------------------|
| complexity | 94               | 94               | Match                                                                                                       |
| duplicate  | 439              | 1545             | **Diverged ~3.5×** — Phase 76 landed on main between baseline gather (2026-04-22) and Wave 0 execution (2026-04-23) and added significant new code in `cargo-pmcp/src/commands/deploy/` and IAM validator paths. See `pmat-inventory-summary.md` Reconciliation section. Duplicates are not gating per D-01, just informational here. |
| satd       | 33               | 33               | Match                                                                                                       |
| entropy    | 4                | 13               | **Diverged** — also Phase-76-attributable                                                                   |
| sections   | 2                | 2                | Match                                                                                                       |

## Complexity-only gate command (planned for Wave 5 ci.yml)

Command: `pmat quality-gate --fail-on-violation --checks complexity`
Exit code: **1**
Violation count: **94** (same as the complexity row in the bare gate)

After Waves 1-4 reduce complexity to 0, this command will exit 0.

## Decision (per CONTEXT.md D-11)

outcome: D-11-B

(Other dimensions also fail today: 1545 duplicates + 33 SATD + 13 entropy + 2 sections all gate-fail the bare command alongside the 94 complexity violations.)

### Rationale

After Waves 1-4 land and complexity hits 0, `pmat quality-gate --fail-on-violation` (the BADGE command, no `--checks` filter) will STILL exit 1 because duplicate (1545), SATD (33), entropy (13), and sections (2) continue to fail. The README badge will stay RED.

This violates the phase goal stated in CONTEXT.md D-01: "Phase 75 done = `pmat quality-gate --fail-on-violation` exits 0 and the auto-generated README badge flips back to `Quality Gate: passing`."

To meet the phase goal under D-11-B, **Wave 5 MUST update `quality-badges.yml`** so the badge command matches the CI gate command. Two equivalent fix shapes:

1. **`--checks complexity` on both** (recommended; simplest; matches D-01 — "complexity is the gating dimension"). The badge then reflects only complexity, which is consistent with the phase's narrowed scope.
2. **Use a `.pmatignore`-restricted scope on both** (per D-09 spike result). Less direct because `.pmatignore` doesn't filter checks, only files — it would still leave the duplicate/SATD/entropy/sections counts non-zero from in-tree code, so this does NOT actually fix the badge unless combined with `--checks complexity`. Therefore option 1 is the only standalone fix.

### Recommended patch shape (D-11-B branch)

In `.github/workflows/quality-badges.yml` around line 72, replace:

```yaml
if pmat quality-gate --fail-on-violation --format json > quality_gate.json 2>/dev/null; then
```

With:

```yaml
if pmat quality-gate --fail-on-violation --checks complexity --format json > quality_gate.json 2>/dev/null; then
```

This is a 1-flag addition; preserves the rest of the workflow logic. Wave 5 Task 5-01 should land both this change and the corresponding `--checks complexity` flag in the new CI gate job in `ci.yml`.

## Wave 5 plan delta (filled in based on outcome)

Files Wave 5 MUST modify:
- `.github/workflows/ci.yml` — **yes (always)** — add new job that installs pinned PMAT and runs `pmat quality-gate --fail-on-violation --checks complexity`.
- `.github/workflows/quality-badges.yml` — **yes (D-11-B-required)** — add `--checks complexity` to the existing badge gate command at line ~72.

## Implications for Wave 5 acceptance criteria

The "badge flipped" verification (Task 5-03) is the binding goal per D-01. If the bare gate is still failing post-Wave 4 due to non-complexity dimensions, the badge will NOT flip — and Wave 5 was incomplete.

This is now the operative test for "phase done":
1. Wave 5 lands `--checks complexity` in BOTH workflow files.
2. After Waves 1-4 land, `pmat quality-gate --fail-on-violation --checks complexity` exits 0 in CI.
3. The next quality-badges.yml run on main pushes a green "Quality Gate: passing" SVG to the badge URL.
4. The README displays "Quality Gate: passing".

Without the D-11-B `quality-badges.yml` patch, step 3 never happens regardless of how clean the complexity count gets.

## Note on D-11-B + D-10-B interaction

Both load-bearing assumptions in this phase resolved unfavorably (D-10-B: PMAT ignores `#[allow]`; D-11-B: bare gate fails on multiple dimensions). The combined implication:

- Every flagged complexity violation must be reduced ≤25 by real refactor (D-10-B).
- The badge command must scope to `--checks complexity` (D-11-B), otherwise the work in Waves 1-4 won't visibly flip the badge.

Both are surfaced in `75-00-SUMMARY.md` "SCOPE EXPANSION DETECTED" / "WAVE 5 ADDITIONAL EDIT REQUIRED" headers per the plan's `<output>` requirement.
