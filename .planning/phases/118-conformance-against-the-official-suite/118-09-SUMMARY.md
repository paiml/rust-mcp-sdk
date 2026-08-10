---
phase: 118-conformance-against-the-official-suite
plan: 09
subsystem: ci
tags: [ci, conformance, gate-wiring, era-matrix, tripwire]
requires:
  - "scripts/run-conformance-suite.sh + scripts/run-era-matrix.sh (118-08)"
  - "conformance/ pinned Node manifest + README § 7/§ 10 (118-02)"
  - "examples/s54_v2_dual_conformance.rs (118-04)"
  - "crates/pmcp-team-servers/tests/{conformance,era_matrix,era_baseline}.rs (118-06/07)"
  - ".github/workflows/ci.yml `gate` aggregate + tests/ci_severance_gate_wiring.rs (117)"
provides:
  - "Two BLOCKING CI jobs: conformance-suite (CONF-01) and era-matrix (CONF-02/CONF-03)"
  - "Eight `gate` edits — both jobs awaited, bound, evaluated and named"
  - "tests/ci_conformance_gate_wiring.rs — a BIJECTIVE serde_yaml wiring reader (16 tests)"
  - "Mechanical binding of the two upstream conformance pins"
  - "First CI execution of the Phase-109 team-servers harness"
affects:
  - ".github/workflows/ci.yml"
  - "tests/ci_severance_gate_wiring.rs (floors raised 6->8, 8->12)"
tech-stack:
  added: ["actions/setup-node@v4 (first in this repo)", "actions/upload-artifact@v7 (first in ci.yml)"]
  patterns: ["bijective job->env->conditional->echo wiring proof", "declaration-line data pins over comment-stripped commands"]
key-files:
  created:
    - "tests/ci_conformance_gate_wiring.rs"
    - ".planning/phases/118-conformance-against-the-official-suite/118-09-SUMMARY.md"
  modified:
    - ".github/workflows/ci.yml"
    - "tests/ci_severance_gate_wiring.rs"
decisions:
  - "D-21 honoured: the gate is SCOPED to the MRTR surface + era matrix; no allowlist, no pass-count assertion"
  - "The bijection replaces the count-based invariant; controls (c) and (d) prove it is strictly stronger"
  - "Data pins read DECLARATION lines from comment-stripped commands — control (e) proved whole-file pins were satisfiable by prose"
metrics:
  tasks: 3
  commits: 4
  tests-added: 16
  duration: "~1h"
  completed: 2026-08-09
---

# Phase 118 Plan 09: Blocking Conformance CI Gates Summary

Two blocking CI jobs — `conformance-suite` (CONF-01) and `era-matrix` (CONF-02/CONF-03) — wired into the org-required `gate` aggregate in all four places each, with the wiring proved as a **bijection** by a `serde_yaml`-parsing reader rather than by a count.

## Commits

| # | Hash | Type | What |
|---|------|------|------|
| 1 | `27f83537` | feat | Two CI jobs with numbered rationale blocks, Node 22, npm cache, timeouts, artifact upload |
| 2 | `a4e296af` | feat | The eight `gate` edits (needs / env / if chain / failure echo, twice) |
| 3 | `4c4fd22f` | test | `tests/ci_conformance_gate_wiring.rs` + raised floors in the severance analog |
| 4 | (this doc) | docs | SUMMARY |

## The gate is genuinely blocking

Final `gate.needs`, verbatim (`ci.yml:761`):

```yaml
    needs: [test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity, v1-severance, conformance-suite, era-matrix]
```

**The four counts all agree at 8** (measured with digit-inclusive regexes — an early `[A-Z_]+` regex silently excluded `WASM32_RESULT` and reported 7):

| Wiring | Count |
|--------|-------|
| `gate.needs` entries | 8 |
| `env:` `_RESULT` bindings | 8 |
| `if`-chain clauses | 8 |
| failure-echo `job=$VAR` pairs | 8 |

`CONFORMANCE_RESULT` and `ERA_MATRIX_RESULT` each occur **exactly 3** times (binding, clause, echo) — not "at least", since a fourth occurrence would mean a duplicate binding and break the bijection.

`feature-flags` remains absent from `gate.needs` (live negative control intact). `security_audit` and `workspace-test` remain untouched and deferred — **zero non-comment occurrences** in `ci.yml`.

## Job configuration

| Item | conformance-suite | era-matrix |
|------|-------------------|------------|
| Script total budget | **3600 s = 60 min** (`TOTAL_BUDGET_SECONDS`) | **none declared** |
| Job `timeout-minutes` | **75** (`ci.yml:632`) | **45** (`ci.yml:735`) |
| Relationship | job timeout ABOVE script budget — script's diagnosis wins the race | job timeout is the **sole** backstop |
| Cache key | `${{ runner.os }}-cargo-conformance-` | `${{ runner.os }}-cargo-era-matrix-` |

**Artifact upload** (`ci.yml:672-677`): `if: always()` at line **673**, `actions/upload-artifact@v7` at **674**, `path: target/conformance-results/` at **677**. Deliberately *not* `if: failure()` (the `fuzz.yml:69-81` shape) — D-14 needs the not-scored `extension`/`pending` results reviewable on GREEN runs.

**Advisory escape hatches:** `grep -cE 'continue-on-error|\|\| true'` = **0** on both HEAD and the new file. (First draft scored 1 — my own rationale prose literally wrote `continue-on-error` while forbidding it. Reworded rather than excused: prose must not be what makes a real grep fence fire.)

**`PMCP_REQUEST_STATE_KEY` audit:** exactly one line names it — `ci.yml:640`, the job-level `env:` binding to a fixed non-production 64-hex sentinel. **Zero** lines in `ci.yml` contain both the variable name and `echo`. The target example reads it for presence only (`examples/s54_v2_dual_conformance.rs:1403`).

**Elevated permissions:** `git diff | grep -cE '^\+\s*permissions:'` = **0**.

## The bijection

The core loop (`tests/ci_conformance_gate_wiring.rs`, `every_awaited_job_is_bound_read_and_named`):

```rust
for job_name in &needs {
    let expression = format!("needs.{job_name}.result");
    let bound: Vec<String> = env.iter().filter_map(|(name, expr)| {
        let expr = expr.as_str()?;
        if expr.contains(&expression) { name.as_str().map(str::to_owned) } else { None }
    }).collect();
    assert_eq!(bound.len(), 1, /* AWAITED but NEVER CHECKED ... */);
    let var = bound.into_iter().next().unwrap_or_default();
    let clause = format!("[[ \"${var}\" != \"success\" ]]");
    assert!(run.contains(&clause), /* bound but never compared ... */);
    let pair = format!("{job_name}=${var}");
    assert!(echo_line.contains(&pair), /* red gate names no cause ... */);
    mapping.push((job_name.clone(), var));
}
```

Plus a **reverse** direction (every `_RESULT` binding resolves to exactly one awaited job, so a stale binding for a removed job fails) and an **injectivity** arm (no two jobs share a variable). `grep -c 'len() >= '` = **0**: no count-style invariant remains.

## The seven negative controls

Each was applied by hand, run, and reverted **from a scratchpad backup copy** (never `git checkout --`), with the restored file's `shasum` re-verified each time.

| # | Mutation | New reader | Old (count) reader | Arm that fired |
|---|----------|-----------|--------------------|----------------|
| a | remove `conformance-suite` from `gate.needs` | **FAIL** (3 tests) | — | names the job |
| b | remove `ERA_MATRIX_RESULT` binding, job still awaited | **FAIL** | — | "AWAITED but NEVER CHECKED" |
| c | second binding of the same result under an alias | **FAIL** | **PASS** | "exactly one" |
| d | echo pair `era-matrix=$WRONG` | **FAIL** | **PASS** | echo-pair |
| e | delete `2026-07-28` from `REQUIREMENT_SETS` | **FAIL** (after fix) | — | declaration pin |
| f | `--expected-failures` on a live command | **FAIL** | — | forbidden-flag |
| g | delete the sha line from `conformance/README.md` | **FAIL** | — | two-pin binding |

**(c) and (d) are the load-bearing pair:** both make the new bijective reader red while `ci_severance_gate_wiring` stays green (`NEW-EXIT=101`, `SEVERANCE-EXIT=0`). That is the direct measurement that the bijection is strictly stronger than the `env.len() >= needs.len()` invariant it replaces.

Verbatim transcripts:

**(a)** `test result: FAILED. 13 passed; 3 failed`
```
FAILURE MODE: `conformance-suite` is not listed in `gate.needs` in .github/workflows/ci.yml. ...
needs read: ["test", "quality-gate", "purity-check", "pmcp-agent-targets", "wasm32-purity", "v1-severance", "era-matrix"]
```

**(b)** `test result: FAILED. 15 passed; 1 failed`
```
FAILURE MODE: `era-matrix` is awaited by `gate.needs` but exactly 0 variable(s) in the evaluation
step's `env:` block bind `needs.era-matrix.result` — expected EXACTLY ONE.
CONSEQUENCE (zero): the job is AWAITED but NEVER CHECKED. ...
bindings found: []
```

**(c)** `NEW-EXIT=101 / SEVERANCE-EXIT=0`
```
FAILURE MODE: `conformance-suite` is awaited by `gate.needs` but exactly 2 variable(s) ... — expected EXACTLY ONE.
CONSEQUENCE (two or more): two variables carry the same result, so a later edit that deletes
"the redundant binding" can silently delete the one the `if` chain reads, and the count-based
invariant this replaced would not have noticed.
bindings found: ["CONFORMANCE_RESULT", "CONFORMANCE_RESULT_ALIAS"]
```

**(d)** `NEW-EXIT=101 / SEVERANCE-EXIT=0`
```
FAILURE MODE: the `gate` failure echo does not carry the pair `era-matrix=$ERA_MATRIX_RESULT`.
echo read:   echo "Required checks failed: ... era-matrix=$WRONG"
```

**(e)** — see "Defect found by control (e)" below.
```
FAILURE MODE: `scripts/run-conformance-suite.sh` no longer names the requirement set `2026-07-28`.
declaration read: REQUIREMENT_SETS=(2025-11-25)
```

**(f)**
```
FAILURE MODE: a COMMAND in `scripts/run-conformance-suite.sh` contains `--expected-failures`.
CONSEQUENCE: `--expected-failures` IS the forbidden shape — a known-fail allowlist turns a red run
green while changing nothing about the SDK ...
```

**(g)**
```
FAILURE MODE: conformance/README.md no longer names `a865118206d4d8cc8dbc5f5201607839281d0c3b`.
CONSEQUENCE: this repo carries TWO independent pins to the same upstream project ...
```

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 1 - Bug] Every data pin was satisfiable by the script's own prose**

- **Found during:** Task 3, negative control (e)
- **Issue:** Control (e) deleted `2026-07-28` from `REQUIREMENT_SETS` and the test **passed** (`EXIT=0`, 16/16). The first draft asserted `source.contains(rev)` over the whole script, and the script's rationale block quotes both revisions in prose (`--requirements 2026-07-28 -> 124 passed, 54 failed`). The fence was satisfied by the documentation explaining it. The same weakness affected **every** data pin: `ZERO_CHECK_DECLARATIONS`, `MATRIX_TESTS`, `HTTP_FEATURE_TARGETS` (the era script's fence-6 comment quotes a complete `cargo test … --features http --test era_matrix` line), `DEV_DEP_FREE_FENCES`, the nonzero-count guards and the budget declaration.
- **Fix:** Added `commands_only`-backed accessors and a `declaration_line(commands, name, rel)` helper asserting **exactly one** `name=` COMMAND line; every data pin now reads a declaration or a comment-stripped command, never the whole file. `MATRIX_TESTS` members are matched in their `"<name>:<flags>"` entry shape.
- **Verified:** control (e) re-run → `EXIT=101`, `declaration read: REQUIREMENT_SETS=(2025-11-25)`.
- **Commit:** `4c4fd22f`

**2. [Rule 1 - Bug] Clippy `map_unwrap_or` in `gate_failure_echo_line`**

- **Found during:** Task 3, `make quality-gate` (exit 2 at the clippy stage)
- **Fix:** rewrote the `.map(...).unwrap_or_else(panic!)` chain as a `let ... else`.
- **Verified:** `make quality-gate` → `EXIT=0` with the literal `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner.
- **Commit:** `4c4fd22f`

**3. [Rule 3 - Blocking] Worktree base was wrong at startup**

The agent's worktree HEAD was on an unrelated `docs: update quality badges` lineage; `git merge-base HEAD 2f879819` returned `47ee632b`. Corrected with the sanctioned `git reset --hard 2f8798191a70d1d90747723388556c4f7cd20997` from the startup check (working tree was clean, nothing lost).

### Plan/reality discrepancies — intent satisfied, letter not

**1. `ZERO_CHECK_SCENARIOS` does not exist.** The plan specified `ZERO_CHECK_DECLARATIONS = ["ZERO_CHECK_SCENARIOS", "MIN_CHECKS_V1", "MIN_CHECKS_V2"]`. 118-08 actually declares **two** arrays: `ZERO_CHECK_SCORED_SCENARIOS` (empty — the strongest state) and `ZERO_CHECK_NOT_SCORED_SCENARIOS`. Pinning the plan's literal would have asserted against a name that appears nowhere. Both real names are pinned instead, which is strictly stronger.

**2. The `security_audit|workspace-test` diff criterion contradicts the plan's own action text.** The criterion requires `git diff ci.yml | grep -cE '^\+.*(security_audit|workspace-test)'` = 0, while the same task's `<action>` *requires* the era-matrix rationale to name `workspace-test` twice (fences 1 and 2) and the conformance rationale to cite the PR #319 precedent. The added lines are 4, **all comment lines** (`+  #`). Intent — "do not wire these deferred jobs into the graph" — is satisfied: **zero non-comment occurrences** of either name exist in `ci.yml`.

**3. The era-matrix `timeout-minutes` has no script budget to sit above.** The plan requires each job timeout to exceed "the corresponding script's total budget recorded in 118-08-SUMMARY.md". Only `run-conformance-suite.sh` declares one (3600 s). `run-era-matrix.sh` declares none, so the criterion is vacuous there. Handled honestly rather than papered over: the job's rationale fence 5 states plainly that this is the **sole** backstop and that a future script-level budget must be added *below* 45 minutes.

**4. The plan's `MINIMUM_JOBS` reasoning was right about the slack.** The analog's floor was 8 against a real 10; it is now 12 against a real 12 in both files.

### Not performed

**PR check-run observation.** The plan's `<verification>` asks to confirm from `gh api .../check-runs` that both jobs appear and that `gate` goes red when either does. No PR exists from this worktree — the orchestrator owns branch integration — so this is deferred to the first CI run after merge. The structural half of that claim (both jobs block through `gate`) is proved offline by the 16-test reader and the seven controls.

**Live artifact inspection (T-118-51).** No `target/conformance-results/` tree exists locally (the driver was not executed here; that was 118-08's measurement). What *is* verifiable statically: the upload `path:` is `target/conformance-results/` only, that tree is written solely by the suite's own `-o` flag, and no step in either job writes environment state into it. A first-run artifact inspection should confirm this empirically.

## Verification

| Check | Result |
|-------|--------|
| `cargo test --features full --test ci_conformance_gate_wiring` | **EXIT=0**, `running 16 tests`, 16 passed |
| `cargo test --features full --test ci_severance_gate_wiring` | **EXIT=0**, `running 8 tests`, 8 passed |
| `make quality-gate` | **EXIT=0**, literal `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner present, **0** `lines truncated` markers |
| `make lint-plans` | **EXIT=0** — 10 plan files, 733 verification lines, PASSED |
| `cargo package -p pmcp --list --allow-dirty` | **EXIT=0**, 519 entries |
| tarball excludes `conformance/` and `ci_conformance_gate_wiring` | grep **exit 1** (absent) — exclusion holds |
| `ci_severance_gate_wiring.rs` still ships | present (1) — correct, all its inputs ship |
| `grep -ciE 'python\|pyyaml\|yq '` in new test | **0** |
| `grep -c 'serde_yaml'` in new test | 4 |
| `grep -c 'len() >= '` in new test | **0** |
| `grep -c 'SPEC_RECHECK_PINNED_SHA'` | 4 (declaration + uses) |
| `grep -c 'feature-flags'` in new test | 3 |

### Tooling hazards hit (recorded for the next wave)

**The rtk proxy corrupts `head`, `wc` and `git diff` output.** A bare `head -528 ci.yml | wc -l` returned **264** against a 554-line file, and a first splice produced a 515-line file instead of 779. The same corruption made `git diff | grep -c` report `0` for `workspace-test` when the true count was 4. Every measurement in this SUMMARY was taken with **absolute binary paths** (`/usr/bin/head`, `/usr/bin/wc`, `/usr/bin/grep`, `/bin/cat`). Treat any bare-`grep`/`head`/`wc` count in a verification block as unreliable.

## Files

- `.github/workflows/ci.yml` — +228/−0 (jobs) then +7/−3 (gate); jobs at 624 (`conformance-suite`) and 728 (`era-matrix`); `gate` at 759
- `tests/ci_conformance_gate_wiring.rs` — new, 16 tests
- `tests/ci_severance_gate_wiring.rs` — floors only (+14/−2)

Uncommitted and deliberately left alone: `.pmat/*` cache files, mutated by the `make quality-gate` run. Tool-generated, out of this plan's scope.

## Self-Check: PASSED

- `.github/workflows/ci.yml` — FOUND
- `tests/ci_conformance_gate_wiring.rs` — FOUND
- `tests/ci_severance_gate_wiring.rs` — FOUND
- `.planning/phases/118-conformance-against-the-official-suite/118-09-SUMMARY.md` — FOUND
- commit `27f83537` — FOUND
- commit `a4e296af` — FOUND
- commit `4c4fd22f` — FOUND
