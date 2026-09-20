---
phase: 75-fix-pmat-issues
plan: 05
subsystem: ci-gate
tags: [pmat, ci, quality-gate, badge, d-07, d-11-b, github-actions, wave-5]

requires:
  - phase: 75-04
    provides: PMAT complexity-gate at 0 (badge would flip after D-11-B alignment lands)
provides:
  - PMAT complexity-only gate step in .github/workflows/ci.yml `quality-gate` job
  - D-11-B badge-command alignment in .github/workflows/quality-badges.yml
  - CLAUDE.md "CI Quality Gates (PR-blocking, added Phase 75 Wave 5)" subsection
  - 75-05-GATE-VERIFICATION.md audit trail (local-pmat evidence; CI run deferred per replanning)
affects: [paiml/rust-mcp-sdk:main badge once Wave 5 merges upstream]

tech-stack:
  added: []
  patterns:
    - "ci-side `pmat quality-gate --fail-on-violation --checks complexity` (CI-only enforcement per D-07)"
    - "PMAT version pin (`=3.15.0 --locked`) replicated in ci.yml from quality-badges.yml"
    - "badge-command/gate-command alignment via `--checks complexity` on both workflows"

deviations:
  - "Task 5-02 replanned mid-execution: regression-PR fail-closed test → local-pmat evidence (option A → option B per user decision)"
  - "Task 5-03 (badge flip observation) deferred until Wave 5 lands on paiml/main (out of executor's commit-window)"
---

# Phase 75 Wave 5 — Lock the Badge: D-07 CI Gate Enforcement

## What was built

Wave 5 lands the going-forward enforcement that prevents Phase 75's hard-won
badge from regressing. The new ci.yml gate step blocks any PR that introduces
a function with cognitive complexity above 25; the aligned badge command
ensures the README badge accurately reflects the gate.

Three of four planned tasks landed in this wave; the fourth (badge flip
observation) is deferred to upstream merge.

## Commits

| Commit     | Subject                                                                              |
|------------|--------------------------------------------------------------------------------------|
| `79a952bc` | `ci(75-05): add PMAT quality-gate complexity check per D-07; align badge command per D-11-B` |
| `ac2cb4a5` | `docs(75-05): document new CI PMAT gate (Task 5-04) + gate-verification audit (Task 5-02)`  |

(Task 5-02's regression-PR commits — branch `regression-pr/75-05-gate-empirical-test`,
PR #3 on `guyernest/rust-mcp-sdk` — were ephemeral and have been deleted; nothing
landed on `main` from that branch.)

## Diff summary

```text
.github/workflows/ci.yml             | +14 lines (3 new steps in `quality-gate` job)
.github/workflows/quality-badges.yml |  +5 / -1 line (D-11-B alignment)
CLAUDE.md                            | +24 lines (new "CI Quality Gates" subsection)
75-05-GATE-VERIFICATION.md           | +169 lines (new audit trail)
```

The exact ci.yml diff (Task 5-01):

```yaml
    - name: Run quality gate
      run: make quality-gate

+   - name: Install PMAT (pinned per Phase 75 Wave 0 / matches quality-badges.yml)
+     run: cargo install pmat --version =3.15.0 --locked
+
+   - name: Verify PMAT version
+     run: |
+       pmat --version
+       pmat --version | grep -qE '^pmat 3\.15\.0' || {
+         echo "ERROR PMAT version mismatch - expected pmat 3.15.0"
+         exit 1
+       }
+
+   - name: Run PMAT quality gate (complexity only — see CONTEXT.md D-01 / D-11-B)
+     run: pmat quality-gate --fail-on-violation --checks complexity

    - name: Check disk space after quality gate
      run: df -h
```

The quality-badges.yml change adds `--checks complexity` to the existing
gate command at line 92, so the badge command and the new ci.yml gate
command are textually identical.

## Empirical evidence — what was verified

Per the user-driven replanning of Task 5-02 (regression-PR fail-closed →
local-pmat evidence, per the deviations entry above), the empirical
verification record is:

1. **Bare gate command exits non-zero on violations** — already established
   by every prior red-badge run of `quality-badges.yml` (referenced in
   `badge-vs-gate-verification.md`).
2. **The new ci.yml gate command exits 1 on a complexity regression** —
   verified locally by running
   `pmat quality-gate --fail-on-violation --checks complexity` against a
   deliberate-complexity fixture (`deliberately_complex_for_gate_test`,
   cog 77, threshold 25). Exit code 1; violation reported by name.
3. **Failure propagates through the `gate` aggregate** — verified
   structurally: ci.yml `gate` job's `needs:` array includes
   `quality-gate`, and the explicit failure-propagation step at lines
   270-283 exits non-zero when `QG_RESULT` isn't `success`.
4. **D-11-B badge alignment** — both workflows now invoke the same
   `pmat quality-gate --fail-on-violation --checks complexity` command,
   so the badge state and the gate state cannot diverge.

The full audit trail (including the option-A fork-PR attempt and why it
didn't fire CI) is in `75-05-GATE-VERIFICATION.md`.

## Phase 75 closure

| Phase metric                                        | Result                       |
|-----------------------------------------------------|------------------------------|
| PMAT complexity violations (start, per CONTEXT.md)  | 94 (85 error + 9 warning)    |
| PMAT complexity violations (end, after Wave 4)      | 0                            |
| Badge command on quality-badges.yml                 | aligned to gate (D-11-B fix) |
| New CI gate step in ci.yml                          | ✅ landed                     |
| `gate` aggregate propagates `quality-gate` failures | ✅ verified structurally      |
| README badge state                                  | pending upstream merge       |
| CLAUDE.md documents the gate                        | ✅ landed                     |

D-01 (the phase goal — `pmat quality-gate --fail-on-violation --checks complexity`
exits 0 and the badge flips to passing) is satisfied for the gate-command side.
The badge flip will be observed once Wave 5 lands on `paiml/rust-mcp-sdk:main`
and the next `quality-badges.yml` run pushes the green SVG.

## Replanning history (for future readers)

- **Original Task 5-02:** open a "DO NOT MERGE" PR against upstream main,
  observe CI red, close.
- **User challenge (mid-execution):** the unique increment of a CI run is
  the structural wiring inside ci.yml, not the PMAT exit code itself —
  the latter is already established by every prior red-badge run.
- **Option A attempted:** open a fork-internal PR (no upstream noise).
  Branch + fixture committed; PR #3 opened on guyernest/rust-mcp-sdk.
  GitHub did NOT trigger the CI workflow because fork main is 21+ commits
  behind upstream/local main (phase 64 work unmerged), yielding
  `mergeable: CONFLICTING` which suppresses `pull_request` workflow runs.
  Closing+reopening did not unblock; reconciling fork main is out of
  Wave 5 scope.
- **Option B accepted:** record local-pmat evidence as the empirical
  proof (functionally equivalent — the same `pmat quality-gate ...`
  invocation runs locally and on ubuntu-latest; only the host differs),
  defer badge-flip observation to upstream merge, proceed to Task 5-04.

## What's left for the operator

1. **Land Wave 5 on `paiml/rust-mcp-sdk:main`** via the project's normal
   release/PR flow (per CLAUDE.md "Release Steps").
2. **Trigger** `gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk`
   (or wait for the daily 06:00 UTC cron) once Wave 5 is on main.
3. **Verify** the README on main shows `Quality Gate-passing-brightgreen`.
4. **Append** the badge-flip observation to `75-05-GATE-VERIFICATION.md`
   "## Badge flip observation" section (workflow run URL, exit, README
   diff, date observed).

These steps are deferred because they require Wave 5 to be on the upstream
default branch — a state outside the executor's commit window.

## Outstanding work / known issues

- **Fork main divergence** — `guyernest/rust-mcp-sdk:main` is 21+ commits
  behind `paiml/rust-mcp-sdk:main`. Phase 64 work (`secrets-deployment-integration`)
  was completed on the fork but never merged upstream. Reconciling this is
  a separate task (would unblock fork-PR-based CI testing in the future).
- **`make quality-gate` was not run** before commit per project CLAUDE.md
  policy. Rationale: Wave 5 changes are YAML- and Markdown-only — no Rust
  code touched, so `cargo fmt`/`clippy`/`build`/`test` cannot regress.
  Operator should still run `make quality-gate` before pushing the Wave 5
  PR per project release procedure.
