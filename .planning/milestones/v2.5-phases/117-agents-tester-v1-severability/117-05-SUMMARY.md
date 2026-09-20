---
phase: 117-agents-tester-v1-severability
plan: 05
subsystem: infra
tags: [ci, github-actions, cargo-features, severability, tripwire, serde_yaml, quality-gate]

# Dependency graph
requires:
  - phase: 117-01
    provides: "the default-on `v1-compat` Cargo feature, the `full-v2` severance profile, `tests/v1_severability_tripwire.rs`, and the proven severance command"
provides:
  - "A `v1-severance` CI job running the four-fence severance build with a ~50-line rationale block and its own cache key"
  - "The three coordinated `gate` wirings (needs / env binding / if-chain + failure echo) that make a broken severance build a red required check"
  - "`tests/ci_severance_gate_wiring.rs`: an automated tripwire that proves all three wirings FROM `ci.yml`, with the non-blocking `feature-flags` job as a live negative control"
  - "`serde_yaml` in root `[dev-dependencies]` as the declared YAML parser for workflow-file assertions"
  - "D-117-05-A: the runtime-blocking claim, deferred with a named owner rather than asserted unproved"
affects: [117-14, phase-verifier, any-plan-adding-a-blocking-ci-job, v1-sunset]

# Tech tracking
tech-stack:
  added: ["serde_yaml 0.9 (root dev-dependency only; 0.9.34 already resolved via crates/mcp-tester, zero new packages)"]
  patterns:
    - "Blocking-CI-job wiring is THREE edits, not one (Pattern S1), and all three are machine-asserted"
    - "A gate's blocking status is proved from the WORKFLOW FILE, never from the Makefile (CORRECTION-116-DOC)"
    - "Non-vacuity floors guard NEGATIVE assertions only; positive `contains` assertions use the raw reader so they keep their targeted diagnosis"

key-files:
  created:
    - "tests/ci_severance_gate_wiring.rs"
  modified:
    - ".github/workflows/ci.yml"
    - "Cargo.toml"
    - ".planning/phases/117-agents-tester-v1-severability/deferred-items.md"

key-decisions:
  - "Option A (Q3.6) chosen over Option B: a dedicated `v1-severance` job rather than a step inside `quality-gate`, for an isolated cache key and a failure message that names the exact cause"
  - "`serde_yaml` chosen over an interpreter-based YAML parser: PyYAML is undeclared in this repository and a BLOCKING gate must not rest on a package that merely happens to be installed on a runner (T-117-SC2)"
  - "Task 3 disposition: Option B (DEFER) by owner decision — no PR was open for the branch, so the adversarial observation was unavailable"
  - "The non-vacuity floor was moved OFF the positive membership assertion after an executed negative control showed it produced a misleading diagnosis; the floor value stays at 6"

patterns-established:
  - "Pattern: split a guarded reader into `x_raw()` (pure) and `x()` (floored). Route `contains` assertions through raw — they fail safely on a vacuous read — and `!contains` assertions through the floored form, which is where vacuity would produce a false PASS."
  - "Pattern: a non-vacuity floor equal to the current exact count silently doubles as a wiring assertion. Its failure message must name BOTH causes (broken reader OR genuine removal) with their opposite remedies."

requirements-completed: [SMPL-01]

# Metrics
duration: 95min
completed: 2026-08-08
---

# Phase 117 Plan 05: CI Severance Gate Wiring Summary

**The `full-v2` severance build now runs as its own `v1-severance` CI job wired into the org-required `gate` aggregate in all three places `gate` actually reads, and the wiring is asserted from `ci.yml` itself by a five-test tripwire whose two removal controls were executed rather than assumed.**

## Performance

- **Duration:** ~95 min (including one blocking checkpoint pause)
- **Tasks:** 3 of 3 (Task 3 resolved by owner decision at its checkpoint)
- **Files modified:** 4 (1 created, 3 modified)

## Accomplishments

- **A broken severance build is now a red required check.** The `v1-severance` job runs
  `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` and is
  wired into `gate` via `needs:`, a `SEVERANCE_RESULT` env binding, and the `if` chain plus its
  failure echo. Pattern S1's trap — that a `needs:` entry alone yields a job which is *awaited* but
  whose result is *never checked* — is closed and machine-asserted.
- **The blocking claim is proved from the workflow file, not from the Makefile.**
  `tests/ci_severance_gate_wiring.rs` parses `ci.yml` structurally and asserts all three wirings
  plus their mutual consistency (the env var that is BOUND must be the env var that is READ).
- **The tripwire is demonstrably able to tell blocking from non-blocking.** `ci.yml`'s
  `feature-flags` job — visible, green-looking, and absent from `gate.needs` — is asserted as a live
  negative control, so a reader broken in the direction of "everything looks wired" fails.
- **The four fences carry written reasons a contributor will read before "simplifying".** A ~50-line
  rationale block above the job explains `-p pmcp`, `--no-default-features`, `--features full-v2`
  and `-D warnings`, and records why `--all-features` can NEVER prove severance and why
  `--all-targets` must not be added.
- **The runtime-blocking remainder is stated, not overstated.** Deferred as D-117-05-A with a named
  owner rather than claimed.

## Task Commits

1. **Task 1: Add the `v1-severance` job and make all three `gate` edits** — `6dcb3da0` (ci)
2. **Task 2: Automated tripwire proving the three-part gate wiring** — `e4788be3` (test)
3. **Task 3: Record the adversarial observation (Option B disposition)** — `23992fba` (docs)

## Files Created/Modified

- `.github/workflows/ci.yml` (+79/-4) — new `v1-severance` job adjacent to `purity-check` with its
  own `-cargo-severance-` cache key, plus the three `gate` edits.
- `tests/ci_severance_gate_wiring.rs` (457 lines, new) — the five-test wiring tripwire.
- `Cargo.toml` (+1) — `serde_yaml = "0.9"` in `[dev-dependencies]`, with an inline comment naming
  its consumer.
- `.planning/phases/117-agents-tester-v1-severability/deferred-items.md` — D-117-05-A.

## Task ordering (recorded as the plan requires)

Task 1's `<automated>` verify block is `cargo test --test ci_severance_gate_wiring`, but that test
file is Task 2's deliverable. Task 1 was therefore committed with its verification pending, and the
command was first run — green — at Task 2. Both tasks landed in this plan. The severance build
itself was run manually BEFORE Task 1 was committed (`exit 0`, 0 warnings), so no red job was ever
wired into `gate`.

## The YAML-parsing route, and why the interpreter route was rejected

`serde_yaml = "0.9"` in root `[dev-dependencies]`. An earlier draft of this check shelled out to a
`PyYAML`-based one-liner. `PyYAML` is **not a declared dependency of this repository** — it happens
to be present on some GitHub-hosted runner images and on some workstations, and absent elsewhere.
This test is reached by `make test-integration`, which `make quality-gate` runs and CI enforces, so
a BLOCKING gate would have rested on an undeclared, unversioned, out-of-band interpreter package
(T-117-SC2).

The cost was verified to be zero, not assumed:

- `grep -n 'serde_yaml' crates/mcp-tester/Cargo.toml` → `26:serde_yaml = "0.9"` — the same version.
- `Cargo.lock` already resolves `serde_yaml 0.9.34+deprecated`. No new package enters the graph.
- `cargo tree -p pmcp --features full -e normal | grep -c serde_yaml` → **0**. It is a
  dev-dependency and never reaches `pmcp`'s published runtime graph or its wasm posture.
- `grep -rnE 'python3|import yaml' tests/ci_severance_gate_wiring.rs` → 0. `ci.yml`'s single
  `python3` hit is pre-existing (`:111`, the example-checking step); `git diff -U0` confirms this
  plan added no such line.

The workflow is PARSED, not string-matched — text scanning would happily "find" `v1-severance`
inside a comment and report a wiring that does not exist.

## Negative controls — both EXECUTED and reverted

**NC1 — remove ONLY the `SEVERANCE_RESULT` line from the `gate` step's `env:` block** (leaving
`needs:` and the `if` chain intact). `severance_result_is_bound_and_evaluated` failed as required:

```
FAILURE MODE: no variable in the `gate` evaluation step's `env:` block is bound to
`needs.v1-severance.result`. A `needs:` entry alone produces a job that is AWAITED but whose result
is NEVER CHECKED — `gate` declares `if: always()` and only ever compares the named variables it
reads, so an unbound result can never turn it red.
WHAT TO DO: add `SEVERANCE_RESULT: ${{ needs.v1-severance.result }}` to the `env:` block AND
evaluate it in the `if` chain. Binding without evaluating is the same defect one step later.
```

`the_workflow_parse_is_not_vacuous` also failed, with the general form of the same invariant:
`the gate evaluation step binds 5 env var(s) for 6 awaited job(s)`. Restored → `5 passed; 0 failed`.

**NC2 — remove `v1-severance` from `gate.needs` only.** `severance_job_is_in_gate_needs` failed as
required:

```
FAILURE MODE: `v1-severance` is not listed in `gate.needs` in .github/workflows/ci.yml. `gate` is
the org ruleset's required status check, so a job outside its `needs:` array is visible,
green-looking and completely non-blocking — exactly the state the `feature-flags` job is in today.
WHAT TO DO: add `v1-severance` to `gate.needs`, AND check the other two wirings (the `env:` binding
and the `if` chain) — all three are required.
needs read: ["test", "quality-gate", "purity-check", "pmcp-agent-targets", "wasm32-purity"]
```

Restored → `5 passed; 0 failed`. NC2 was run TWICE; the first run is what exposed the Rule 1
deviation below.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The non-vacuity floor hijacked the diagnosis of negative control 2**

- **Found during:** Task 2, first execution of NC2 (measured, not predicted).
- **Issue:** `severance_job_is_in_gate_needs` DID fail when `v1-severance` was removed from
  `gate.needs` — satisfying the plan's acceptance criterion literally — but it failed with the
  *non-vacuity floor* message, `"parsed 5 entr(ies) ... below the 6 floor ... fix the reader; never
  lower the floor"`. That instruction is actively wrong for this defect: nothing is wrong with the
  reader, a required check has stopped gating merge, and a maintainer following the message would
  go and audit the parser. `the_feature_flags_job_is_still_not_in_gate_needs` misfired the same way.
- **Root cause:** `MINIMUM_GATE_NEEDS = 6` equals the exact current `needs` count, so the floor
  silently doubles as a wiring assertion — any legitimate reduction reads as reader breakage. The
  floor was also being applied inside the shared reader, ahead of every targeted assertion.
- **Fix:** split the reader. `gate_needs_raw()` is a pure structural read; `gate_needs()` adds the
  floor. Positive `contains` assertions use the raw form — asserting a name IS present in an empty
  list fails safely, so a floor there only trades a precise diagnosis for a misleading one. Negative
  `!contains` assertions (the `feature-flags` control) keep the floored form, because that is where
  a vacuous read would produce a false PASS. The floor's own message now names BOTH causes and their
  opposite remedies. Separately, the "every awaited job is bound" check in
  `the_workflow_parse_is_not_vacuous` was re-pointed from the constant to the actual `needs.len()`,
  so it no longer misfires when `needs` legally changes size.
- **The floor VALUE stays at 6**, as the plan's `<behavior>` mandates. Only its placement and its
  message changed.
- **Verification:** both negative controls were re-run after the fix. NC2 now produces the targeted
  message quoted above; NC1 is unchanged. Suite green at `5 passed; 0 failed`.
- **Files modified:** `tests/ci_severance_gate_wiring.rs`
- **Commit:** `e4788be3` (folded into the Task 2 commit — the defect was in that task's own
  deliverable and was found before it landed)

### Plan defects (NOT implementation deviations)

**1. The plan's `<verification>` block contradicts Task 2's acceptance criteria on `Cargo.toml`.**

`<verification>` states `git diff --stat Cargo.toml` is empty (no new dependency). Task 2's
`<acceptance_criteria>` states the opposite in detail — root `[dev-dependencies]` gains EXACTLY ONE
entry, `serde_yaml = "0.9"`, with an inline comment naming its consumer — and the plan frontmatter
lists `Cargo.toml` in `files_modified`. Threat `T-117-SC2` also mandates the declared parser.

**Resolution:** Task 2 + T-117-SC2 were followed. The `<verification>` line is stale from the
rejected interpreter route, where no Rust dependency would have been needed. Flagged here so the
phase verifier does not read `Cargo.toml`'s one-line diff as an unauthorized change.

## Adversarial gate-blocking check

**Disposition: Option B — DEFERRED.** Recorded as **D-117-05-A** in
`.planning/phases/117-agents-tester-v1-severability/deferred-items.md`.
**Owner: Guy Ernest** (guy@mlguy.us).

**The exact unproved claim, as one sentence:** *GitHub Actions evaluates a failed `needs` job as a
failed `gate` conclusion (rather than skipped/pending).*

**Standing evidence in its place:** `tests/ci_severance_gate_wiring.rs`. It proves, from `ci.yml`
itself, that `v1-severance` is in `gate.needs`, that a variable is bound to
`needs.v1-severance.result`, and that the SAME variable is evaluated inside the `gate` step's `run:`
script — with the non-blocking `feature-flags` job asserted as a live negative control, and with
both removal controls executed rather than assumed.

**Why Option A was unavailable:** Option A is the plan's preferred branch *if a PR is already open
for this phase*. At execution time there was **no open PR for `fix/mcp-publisher-oidc-audience`** —
the only open PRs on `paiml/rust-mcp-sdk` were seven dependabot PRs (#295, #304, #305, #306, #308,
#309, #310). Executing A would have required opening a PR and running a break-push / observe /
revert-push cycle, both outside this plan's scope.

**What this plan therefore does and does not claim.** It claims the severance build is **wired into
`gate` in all three places `gate` reads**, and that this wiring is asserted from the workflow file.
It does **not** claim the gate "blocks merge" as an observed fact. Per CORRECTION-116-DOC,
understating the evidence is acceptable; overstating it is the defect this task exists to prevent.
The correct phrasing in downstream artifacts is "wired to block", never "blocks merge", until
D-117-05-A's step 5 exists.

## Verification Evidence

| Check | Result |
|---|---|
| `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | exit 0, **0 warnings** (run BEFORE wiring the blocking job) |
| `cargo test --test ci_severance_gate_wiring` | **5 passed; 0 failed; 0 ignored** |
| `cargo test --test v1_severability_tripwire` (117-01, unregressed) | 9 passed; 0 failed |
| NC1 (env binding removed) | `severance_result_is_bound_and_evaluated` FAILED with the "awaited but never checked" message; reverted → green |
| NC2 (needs entry removed) | `severance_job_is_in_gate_needs` FAILED naming the missing entry; reverted → green |
| `grep -c 'continue-on-error' .github/workflows/ci.yml` | **0 before, 0 after** — unchanged |
| `git diff -U10 .github/workflows/ci.yml` regions | exactly **2** — the new job, and the `gate` job |
| `cargo tree -p pmcp --features full -e normal \| grep -c serde_yaml` | **0** |
| `grep -c 'CORRECTION-116-DOC' tests/ci_severance_gate_wiring.rs` | 1 |
| `grep -cE 'TODO\|FIXME\|XXX' tests/ci_severance_gate_wiring.rs` | **0** |
| `wc -l tests/ci_severance_gate_wiring.rs` | 457 (plan floor: 90) |
| `make lint` (pedantic + nursery, `RUSTFLAGS=-D warnings`) | "✓ No lint issues" |
| `cargo fmt --all -- --check` | clean (greps re-verified AFTER fmt) |
| `make quality-gate` | **exit 0** |

**The gate demonstrably ran this plan's new test** — not inferred from a green gate. Per the
LIM-116-10 / LIM-117-08-GATE warning, this was checked directly:
`grep -n 'ci_severance_gate_wiring' qg.log` returns 3 hits
(`Running tests/ci_severance_gate_wiring.rs`), so `make quality-gate` compiles and executes it.

## Notes for Future Phases

- **117-01's summary carries a stale forward-reference** saying "117-14 owns the `ci.yml` wiring".
  That is wrong — 117-05 owned it, and it is done. Any plan reading that line should ignore it.
- **Adding a job to `gate` is still three edits.** The rationale block in `ci.yml` and this
  tripwire now make that impossible to get wrong silently, but only for `v1-severance`. A future
  blocking job needs the same three edits and would benefit from the same treatment.
- **Do not "simplify" the severance command.** `--all-features` cannot prove severance (features are
  additive — it enables `full-v2` AND `v1-compat`), and `--all-targets` would drag ~29
  struct-literal sites in tests/examples into a deliberately lib-only build for zero additional
  proof. Both are asserted absent by `severance_job_exists`.
- **`feature-flags` is load-bearing as a counter-example.** If it is ever promoted into
  `gate.needs`, re-point `NON_BLOCKING_JOB` at another genuinely non-blocking job rather than
  deleting the control — the assertion's own failure message says so.

## Self-Check: PASSED

All five claimed files exist on disk; all four claimed commits resolve
(`6dcb3da0`, `e4788be3`, `23992fba`, `4f7cc32a`). Task 3's `<verify>` command,
`grep -c 'Adversarial gate-blocking check'`, returns 1. The only two occurrences
of the phrase "blocks merge" in this file are the guarded ones that explicitly
DISCLAIM it as an observed fact.

_Measurement note: `git log --oneline --all | grep` returned false MISSINGs under
the `rtk` proxy, which reformats that command's output. Commit existence was
re-checked with `git cat-file -e <sha>^{commit}` via an absolute-path `git`._
