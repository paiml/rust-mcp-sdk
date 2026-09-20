---
verification: Phase 75 Wave 5 — D-07 CI gate empirical verification
date: 2026-04-25
pmat_version: pmat 3.15.0
ci_pin: =3.15.0 --locked
---

# Wave 5 — D-07 Gate Empirical Verification

This doc records the evidence that the new
`pmat quality-gate --fail-on-violation --checks complexity` step in
`.github/workflows/ci.yml`'s `quality-gate` job fails-closed when complexity
regresses, and that the failure propagates through the `gate` aggregate job
that the org-required-checks ruleset uses to PR-block merges.

## Plan deviation: empirical CI run was substituted with local evidence

**Original plan (Task 5-02):** open a regression PR (deliberately-complex fixture
in `src/__regression_test_75_05.rs`), observe the CI `quality-gate` + `gate`
jobs go red, then close the PR.

**Replanned mid-execution per user decision (option A → option B):**

The user invoked `/gsd-execute-phase 75 --wave 5` and challenged the value of
spending CI minutes on a regression-PR test, since the underlying behavior
(`pmat quality-gate --fail-on-violation --checks complexity` exits non-zero on
violations) was already empirically established by every prior red-badge run
of `quality-badges.yml`. Discussion concluded the unique increment Task 5-02
was supposed to prove was the structural wiring inside `ci.yml`, not the PMAT
exit code itself.

The fork-PR variant (option A) was attempted: branch
`regression-pr/75-05-gate-empirical-test` was created, fixture committed,
PR #3 opened against `guyernest/rust-mcp-sdk:main`. GitHub did NOT trigger
the `CI` workflow on that PR — only `PR Gate` (`pull_request_target`) and
`Security Audit` (`push`) fired. Closing+reopening did not unblock it. The
likely cause is fork-main divergence (`origin/main` is 21+ commits behind
`paiml/main`, including phase 64 work that never merged upstream); the
resulting `mergeable: CONFLICTING` state appears to suppress `pull_request`
workflow runs. Reconciling fork main is out of scope for Wave 5, so the user
elected option B — record local evidence and skip the empirical CI run.

PR #3 was closed without merging; the throwaway branch was deleted from
local and from `origin`; the fixture file and `mod` line never existed on
`main`.

## Evidence — what was actually verified

### 1. Bare gate command exit code (already established by prior runs)

Verified by `badge-vs-gate-verification.md` (2026-04-23):

```text
pmat quality-gate --fail-on-violation                        → exit 1 (94 complexity + others)
pmat quality-gate --fail-on-violation --checks complexity    → exit 1 (94 complexity)
```

After Waves 1-4 brought complexity to 0, the same `--checks complexity`
command exits 0 (verified post-wave-4 in 75-04-FINAL-COUNT.md).

### 2. Fail-closed on regression — local empirical proof

A deliberate-complexity fixture identical to what would have been used in
the fork-PR variant was created at `src/__regression_test_75_05.rs` (cog 77,
threshold 25) and run through PMAT locally with the exact ci.yml command:

```
$ pmat quality-gate --fail-on-violation --checks complexity
$ echo "exit code: $?"
exit code: 1

$ pmat quality-gate --fail-on-violation --checks complexity --format json | \
    jq '.violations[] | select(.file | contains("__regression_test_75_05"))'
{
  "check_type": "complexity",
  "severity": "error",
  "file": "./src/__regression_test_75_05.rs",
  "line": 15,
  "message": "deliberately_complex_for_gate_test: cognitive-complexity - Cognitive complexity of 77 exceeds maximum allowed complexity of 25 (complexity: 77, threshold: 25)"
}
```

This is the same command body that runs in `.github/workflows/ci.yml`
`quality-gate` job (lines added by commit `79a952bc`). GitHub Actions runs
the step in a bash shell on `ubuntu-latest`; the step's exit code is the
process exit code; a non-zero exit code marks the step (and therefore the
job) as failed. There is no environmental delta between local and CI that
could change this exit code.

### 3. Structural wiring — ci.yml inspection

```
$ grep -E '^  gate:' -A3 .github/workflows/ci.yml
  gate:
    runs-on: ubuntu-latest
    needs: [test, quality-gate]
    if: always()
```

The `gate` aggregate job's `needs:` array includes `quality-gate`, so a
failure in the `quality-gate` job (which now contains the PMAT step)
propagates to `gate`. The `gate` job's evaluation logic (lines 270-283 of
ci.yml) explicitly checks `needs.quality-gate.result` and emits a non-zero
exit when it isn't `success`, which propagates to the `gate` check status
that the org-required-checks ruleset gates merges on.

### 4. Badge command alignment — D-11-B

`.github/workflows/quality-badges.yml` line 92 was updated in the same commit
(`79a952bc`) to use `pmat quality-gate --fail-on-violation --checks complexity`,
matching the ci.yml gate command. Per `badge-vs-gate-verification.md` (D-11-B
outcome), without this alignment the README badge would stay red on
duplicate/SATD/entropy/sections dimensions even after Phase 75's complexity
work brings complexity to 0.

## Empirical fail-closed evidence

Per the deviation above, this section records local-pmat evidence rather than
a CI run URL. The value of a CI run vs local pmat for "does PMAT exit 1 on
this fixture" is identical — both invoke `pmat quality-gate --fail-on-violation
--checks complexity` against the same fixture file; the only difference is the
host (laptop vs ubuntu-latest runner).

PR URL: closed; https://github.com/guyernest/rust-mcp-sdk/pull/3 (fork-internal,
closed without merging — fixture and `mod` line never landed on any persistent
branch)
Failing job URL: n/a — CI workflow did not trigger; fork-main divergence
suppressed `pull_request` workflow runs
Failing step: equivalent local invocation `pmat quality-gate --fail-on-violation
--checks complexity` exits 1
Exit code observed: 1
Excerpt:

```
deliberately_complex_for_gate_test: cognitive-complexity - Cognitive complexity
of 77 exceeds maximum allowed complexity of 25 (complexity: 77, threshold: 25)
```

Gate aggregate job: not exercised on PR (deviation); structural review of
ci.yml `gate` job at lines 266-283 confirms `needs: [test, quality-gate]` plus
the explicit failure-propagation step that exits non-zero when `QG_RESULT`
isn't `success`.

## Badge flip observation

**Deferred.** The README badge will only flip after the Wave 5 changes
(this commit + 5-01) land on `paiml/rust-mcp-sdk:main`. Until then, the
existing `quality-badges.yml` on upstream main runs the bare gate command
and the badge stays red on D-11-B dimensions. Once the Wave 5 PR merges
upstream, the next `quality-badges.yml` cron (or a manual
`gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk`) will:

1. Run the updated badge command (`--checks complexity` now matches the gate)
2. See 0 complexity violations (Wave 4 final count)
3. Push a green "Quality Gate: passing" SVG to the badge URL
4. README displays "Quality Gate: passing"

Workflow run URL: pending upstream merge of Wave 5
Workflow exit: pending
README diff: pending
Date observed: pending

## Risk-residual

The deviation removes one piece of evidence — that the new ci.yml step's
exit code is correctly bound to the GitHub Actions step status. This is a
generic GitHub feature that has been exercised by every other step in
ci.yml for years. The marginal risk of "PMAT returns 1 but GitHub Actions
records the step as success" is effectively zero — it would require a
GitHub platform bug, not a project-specific issue.

If the user wants stronger empirical proof later (e.g. before relying on
the gate for a high-stakes change), they can:

1. Reconcile fork main with upstream (separate task, out of Wave 5 scope)
2. Re-create the regression branch
3. Open a PR — CI will then fire normally

The mechanism for this is documented; the cost is a fork-main reconciliation,
not Wave 5 rework.
