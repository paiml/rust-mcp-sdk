---
phase: 118-conformance-against-the-official-suite
plan: 08
subsystem: ci-gates
tags: [conformance, ci, shell-drivers, vacuity-guards, d-21]
requires:
  - "conformance/ — the pinned @modelcontextprotocol/conformance@0.2.0-alpha.11 manifest (118-02)"
  - "examples/s54_v2_dual_conformance.rs — the dual-version target (118-04, 118-05)"
  - "crates/pmcp-team-servers/tests/era_matrix.rs + era_baseline.rs (118-06, 118-07)"
  - "crates/pmcp-team-servers/tests/conformance.rs — the 33-case v1 corpus (118-10)"
  - "scripts/lint-plan-verify-commands.sh + make lint-plans (118-10)"
provides:
  - "scripts/run-conformance-suite.sh — the CONF-01 driver, commands and thresholds as DATA"
  - "scripts/run-era-matrix.sh — the CONF-02/CONF-03 driver, per-target flags as DATA"
  - "make test-conformance / make test-era-matrix — the local spelling, outside quality-gate"
affects:
  - "plan 118-09, which wires both scripts into blocking CI jobs and pins their constants"
tech-stack:
  added: []
  patterns:
    - "launch-and-probe: pre-flight port check, readiness poll, process-group liveness between runs, group teardown with a released-port assertion, per-run + total timeouts"
    - "vacuity guards that live OUTSIDE the compilation unit they police"
    - "bidirectional exact-set-equality gates instead of known-fail allowlists"
key-files:
  created:
    - scripts/run-conformance-suite.sh
    - scripts/run-era-matrix.sh
  modified:
    - Makefile
decisions:
  - "D-21 applied: the gate is scoped to surfaces that genuinely pass, never to a whole requirement set. No allowlist of any shape."
  - "ZERO_CHECK_SCORED_SCENARIOS is EMPTY (measured) and a second array, ZERO_CHECK_NOT_SCORED_SCENARIOS, pins the two measured not-scored entries so the gate is bidirectional today rather than only in principle."
  - "`set -m` job control, not `setsid`, puts the server in its own process group — setsid forks when already a group leader, reintroducing the parent-pid trap the fence exists to avoid, and does not exist on macOS."
  - "The era zero-count guard's diagnosis is keyed on the TARGET NAME, not on the flags present at failure time."
metrics:
  duration: ~3h
  completed: 2026-08-09
---

# Phase 118 Plan 08: Conformance Drivers Summary

Two shell drivers put the CONF-01 and CONF-02/CONF-03 commands in one reviewable place, with
the zero-check policy made executable, the D-21 surface scoping enforced, and every vacuity
guard proved by an executed negative control.

## Commits

| # | Hash | Type | What |
|---|------|------|------|
| 1 | `0e18d9c7` | feat | `scripts/run-conformance-suite.sh` — the CONF-01 driver |
| 2 | `ae699bf6` | feat | `scripts/run-era-matrix.sh` — the CONF-02/CONF-03 driver |
| 3 | `13b08d40` | feat | `Makefile` — `test-conformance` and `test-era-matrix` |
| 4 | `cae76a31` | docs | corrected two header claims the file made about its own fences |

---

## D-21: what the gate asserts, and the plan text it supersedes

The plan's `must_haves.truths` says *"The script fails when either requirement set exits
non-zero."* That is **unimplementable and was not implemented.** `118-CONFORMANCE-GAPS.md` is
normative and overrides the plan text: neither requirement set exits 0, and nine structural SDK
gaps (G-1..G-9) explain it. A script asserting whole-suite exit 0 would be permanently red,
which is a gate people delete.

Satisfying the intent — *a gate that can genuinely fail* — the driver blocks on the surfaces
that genuinely pass:

| Gate | Assertion | Measured today |
|------|-----------|----------------|
| MRTR surface | every `input-required-result-*` scenario has ≥1 passing and 0 failing checks; ≥ `MIN_MRTR_SCENARIOS` present | 14 scenarios, 36 checks, 0 failures |
| Check floors | each run's total `SUCCESS`+`FAILURE` count ≥ its hard-coded floor | v1 66 ≥ 66; v2 178 ≥ 178 |
| Zero-check sets | exact set equality, BOTH directions, against two committed lists | 0/0 scored, 2/2 not-scored |
| One process (D-06) | process-group liveness asserted BEFORE and AFTER each run | one pid across both runs |
| Non-vacuity | each run's own `Running requirements <rev> (N scenarios)` header has N ≥ 1 | 33 and 50 |

The suite's own exit status is **captured and printed** (`suite exit 1` for both), never
suppressed, and the script prints the declared non-conformance on every run pointing at
`118-CONFORMANCE-GAPS.md`. **No `--expected-failures`, no allowlist, no baseline of known
failures anywhere in either script** — verified by grep after stripping comments (0 hits).

---

## Task 1 — `scripts/run-conformance-suite.sh`

### Successful run, verbatim (the gate + summary sections)

```
=== MCP conformance suite (CONF-01) ===
node v22.22.2 (>= 22) OK
PMCP_REQUEST_STATE_KEY is set (value not shown) OK
port 8151 is free OK
...
--- starting target/debug/examples/s54_v2_dual_conformance on 127.0.0.1:8151 ---
server pid 15099, process group 15099
readiness poll: ready after 2 attempt(s) (HTTP 400)
...
requirements 2025-11-25: suite exit status 1 (captured; see section 0 for why it is not the verdict)
requirements 2026-07-28: suite exit status 1 (captured; see section 0 for why it is not the verdict)

=== gates ===
requirements 2025-11-25: 33 scenario dir(s), 66 check(s) executed (floor 66) OK
requirements 2026-07-28: 50 scenario dir(s), 178 check(s) executed (floor 178) OK
zero-check SCORED scenarios: 0 listed, 0 observed, exact match OK
zero-check NOT-SCORED scenarios: 2 listed, 2 observed, exact match OK
MRTR surface: 14 scenario(s) (floor 14), 36 check(s), 0 failures OK

=== CONF-01 summary ===
  requirements 2025-11-25: 33 scenario(s), 66 check(s) executed (floor 66), suite exit 1
      Total: 51 passed, 15 failed
  requirements 2026-07-28: 50 scenario(s), 178 check(s) executed (floor 178), suite exit 1
      Total: 123 passed, 55 failed
  MRTR surface: 14 scenario(s), 36 check(s), 0 failures
  zero-check gate: exact match on both lists
  results: target/conformance-results/
  server: one process (pid 15099, group 15099), alive across both runs

  DECLARED NON-CONFORMANCE (D-21): neither requirement set exits 0. Nine
  structural SDK gaps (G-1..G-9) are recorded with source citations in
  .planning/phases/118-conformance-against-the-official-suite/118-CONFORMANCE-GAPS.md.
  ...

CONF-01 gates PASSED.
SCRIPT_EXIT=0
```

Four full green runs were executed. `target/conformance-results/2025-11-25/` (33 scenario
dirs) and `target/conformance-results/2026-07-28/` (50) are populated, every one carrying a
`checks.json`.

### Provenance of every constant

| Constant | Value | Traced to |
|---|---|---|
| `REQUIREMENT_SETS` | `(2025-11-25 2026-07-28)` | D-04; `conformance/README.md` § 6 |
| `ZERO_CHECK_SCORED_SCENARIOS` | **empty** | 118-04-SUMMARY.md `## Zero-check observations` ("No SCORED scenario reported zero checks at 2025-11-25") and 118-05-SUMMARY.md `### Zero-check observations — the final runs` ("No SCORED scenario reported zero checks in either run"). Re-measured here: 0 observed at both revisions. |
| `ZERO_CHECK_NOT_SCORED_SCENARIOS[0]` | `2025-11-25:server-sse-polling` | 118-04-SUMMARY.md zero-check table (`pending`; all three records `INFO`) and 118-05-SUMMARY.md final-runs table |
| `ZERO_CHECK_NOT_SCORED_SCENARIOS[1]` | `2026-07-28:tasks-status-notifications` | 118-05-SUMMARY.md `### Zero-check observations — v2-premeasure` and the final-runs table (`extension`; only record is `SKIPPED`) |
| `MIN_CHECKS_V1` | `66` | 118-04-SUMMARY.md `## Total executed checks` ("**`MIN_CHECKS_V1` should be derived from 66**"), independently re-derived in 118-05-SUMMARY.md. Re-measured here: 51 SUCCESS + 15 FAILURE = 66. |
| `MIN_CHECKS_V2` | `178` | 118-05-SUMMARY.md `### Total executed checks — the numbers 118-08 hard-codes` (`MIN_CHECKS_V2 = 178`). Re-measured here: 124 SUCCESS + 54 FAILURE = 178. |
| `MIN_MRTR_SCENARIOS` | `14` | 118-05-SUMMARY.md line 49 and `## Task 3` ("**All 14 `input-required-result-*` scenarios are green**"). Re-measured here: 14 dirs, 36 checks, 0 failures. |

Both floor constants carry a doc comment forbidding lowering and stating that a re-pin RAISES
or restates with the fresh measurement in the re-pin commit.

### The zero-check list: the explicit choice README § 7 asked 118-08 to make

118-04 and 118-05 both ended with *"118-08 must decide explicitly whether `ZERO_CHECK_SCENARIOS`
covers not-scored scenarios."* The decision is **both, in two separate arrays**:

- `ZERO_CHECK_SCORED_SCENARIOS` implements README § 7 rule 3 verbatim (scored scope) and is
  **empty** — the strongest state, meaning the referee ran assertions in every scenario it
  scored.
- `ZERO_CHECK_NOT_SCORED_SCENARIOS` pins the two measured not-scored entries under the same
  bidirectional equality. Without it, rule 3's scored-only scope would leave the only two
  zero-check scenarios that actually exist completely unpinned, and the sole enforced array
  would be empty — a gate that can fire in one direction only.

This is a strengthening, not an exemption: every entry PASSES, and adding an entry cannot
silence a failure (a failing scenario is by definition not a zero-check scenario, README § 7
rule 5). Negative controls (e) and (e2) prove both directions fire on both arrays.

### Audited lines

**`PMCP_REQUEST_STATE_KEY` — the value is never printed (T-118-42).** All four occurrences:

| Line | Text | Verdict |
|---|---|---|
| 412 | `if [ -z "${PMCP_REQUEST_STATE_KEY:-}" ]; then` | existence test only |
| 413 | `fail "PMCP_REQUEST_STATE_KEY is not set in the environment.` | names the VARIABLE |
| 420 | `export PMCP_REQUEST_STATE_KEY=\$(openssl rand -hex 32)` | remediation advice; `\$` is escaped, so no expansion |
| 423 | `echo "PMCP_REQUEST_STATE_KEY is set (value not shown) OK"` | explicit non-disclosure |

Negative control (d) confirms the failure message is name-only.

**Timeouts — both named:**

| Line | Constant | Value | Role |
|---|---|---|---|
| 213 | `PER_RUN_TIMEOUT_SECONDS` | 900 | wraps each `conformance server … --requirements <rev>` invocation via `timeout(1)`; a 124 exit is reported as a TIMEOUT, never folded into "the suite failed" |
| 214 | `TOTAL_BUDGET_SECONDS` | 3600 | the whole-script wall clock, checked at six checkpoints (pre-flight, npm ci, cargo build, readiness poll, and after each run) |
| 217/218 | `READINESS_MAX_ATTEMPTS` / `READINESS_BACKOFF_SECONDS` | 120 / 0.5 | bounds the readiness poll at 60 s |

`timeout(1)` is resolved at run time (`timeout`, else `gtimeout`, else a `fail` with install
instructions) because macOS ships neither.

**Sleeps — three, none of them a readiness wait:**

| Line | Context |
|---|---|
| 249 | `cleanup()` — spacing the poll that waits for the PORT TO BE RELEASED after `kill -TERM` |
| 254 | `cleanup()` — the settle after the escalated `kill -KILL` |
| 518 | the readiness poll's BACKOFF BETWEEN attempts |

Lines 94/95/97 and 485 are comments naming the anti-pattern. Commit 4 corrected the header,
which had claimed there was only one `sleep`.

**Port hygiene — `lsof -ti :8151` empty after BOTH outcomes:**

| Outcome | Observation |
|---|---|
| after a successful run | `lsof AFTER: []` (recorded on all four green runs) |
| after each forced failure | `lsof :8151 after control: []` (recorded on all eight edit-based controls) |

### Acceptance-criteria greps

```
bash -n:                        exit=0
executable:                     yes
REQUIREMENT_SETS count:         5      (>= 2); literal at :138 = (2025-11-25 2026-07-28)
ZERO_CHECK_* counts:            3 + 3  (>= 2 each)
MIN_CHECKS_V1 / _V2 counts:     3 / 3  (>= 2 each)
forbidden flags in COMMANDS:    0      (--expected-failures | || true | --all-features
                                        | --spec-version | --suite  | --scenario )
bare 'results/' outside target: 0      (only target/conformance-results, whose preceding
                                        char is '-' and so is excluded by the criterion's
                                        own [^-a-zA-Z/] class)
set -euo pipefail:              1
trap:                           2      (the EXIT trap + its definition reference)
cargo run in COMMANDS:          0
target/debug/examples/s54_...:  1      (>= 1)
timeout:                        16     (>= 2)
python|pyyaml:                  0
```

### Negative controls — nine, each executed, recorded and reverted

Every edit-based control was reverted **from a scratchpad backup copy**, never with
`git checkout --` (which, on a still-untracked file, would have deleted it — the 118-07
process hazard). Each restore was checksum-verified: `01f65a2f…` before and after every
control, `diff` empty.

**(a) Node 20 on PATH** → `EXIT=1`

```
FAILURE: Node v20.20.0 is too old; this suite needs Node >= 22.
CONSEQUENCE: the suite imports globSync from node:fs at MODULE SCOPE. ...
WHAT TO DO: switch the runner to Node >= 22 ... conformance/.npmrc sets engine-strict=true
so the install itself normally refuses first; reaching THIS message means something bypassed it.
```

**(b) server killed BETWEEN the two runs** (a `kill -KILL -- "-$SERVER_PGID"` injected at the
end of `run_requirement_set`) → `EXIT=1`

```
FAILURE: the server process group 22767 is no longer alive (before the 2026-07-28 run).
CONSEQUENCE: D-06 claims BOTH requirement sets were graded against ONE process. ...
```

**(c) readiness probe pointed at a dead port** (8152, attempts lowered to 6) → `EXIT=1`

```
FAILURE: the server never became ready: 6 attempt(s) at
0.5s intervals; last curl exit status 7,
last reported HTTP code '000'.
```

**(d) `PMCP_REQUEST_STATE_KEY` unset** → `EXIT=1`, message names the VARIABLE and not its value
(full text in the audited-lines table above).

**(e) fabricated extra `ZERO_CHECK_SCORED_SCENARIOS` entry** → `EXIT=1`, "listed but not
observed" direction

```
FAILURE: zero-check SCORED scenarios: listed in ZERO_CHECK_SCORED_SCENARIOS but NOT observed:
    2026-07-28:fabricated-control-scenario
CONSEQUENCE: the list is STALE. ...
WHAT TO DO: DELETE the listed entries above ... This direction is what makes the gate
bidirectional and therefore not a known-fail allowlist (conformance/README.md § 7 rule 5).
```

**(e2) a committed zero-check entry DELETED** → `EXIT=1`, the OTHER direction

```
FAILURE: zero-check NOT-SCORED scenarios: observed but NOT listed in ZERO_CHECK_NOT_SCORED_SCENARIOS:
    2025-11-25:server-sse-polling
CONSEQUENCE: a scenario STOPPED BEING EXERCISED. ...
```

**(f) `MIN_CHECKS_V1` raised to 67** → `EXIT=1`, both numbers echoed

```
FAILURE: the '2025-11-25' run executed 66 check(s); the committed floor is 67.
... The remedy is NEVER to lower MIN_CHECKS_V1/MIN_CHECKS_V2 (conformance/README.md § 7 rule 4).
```

**(g) `MIN_MRTR_SCENARIOS` raised to 15** → `EXIT=1`

```
FAILURE: MRTR surface: only 14 scenario(s) matched 'input-required-result-*'; the floor is 15.
```

**(h) the blocking surface re-pointed at a genuinely FAILING scenario** (`MRTR_SCENARIO_PREFIX`
= `json-schema-`, floor 1) → `EXIT=1`. This is the control that proves the D-21 surface gate is
not decorative — it fires on real failing data, not on a synthetic threshold:

```
FAILURE: MRTR surface: 2 failing check(s) across 2 scenario(s).

    2025-11-25:json-schema-2020-12 -> 1 passed, 1 failed
    2026-07-28:json-schema-2020-12 -> 1 passed, 1 failed

CONSEQUENCE: THIS is the regression this gate exists to catch. ...
WHAT TO DO: ... Do NOT add the scenario to any exemption list — conformance/README.md § 9
admits none.
```

**(i) port 8151 already held** (an `nc -l` squatter) → `EXIT=1`

```
FAILURE: port 8151 is already in use.
CONSEQUENCE: a stale listener would answer the suite and this run would grade the
WRONG BINARY while reporting a perfectly plausible result.
```

---

## Task 2 — `scripts/run-era-matrix.sh`

### Reported counts, against the counts prior plans recorded

| Target | Flags | This script | Prior plan | Agreement |
|---|---|---|---|---|
| `conformance` | (default features) | **11** tests (10 passed, 1 ignored) | 118-10-SUMMARY.md: "it now reports **11** (10 passed + 1 ignored)" | exact |
| `era_matrix` | `--features http` | **4** tests, 0 failed | 118-07-SUMMARY.md: "**4 passed**, exit 0" | exact |
| `era_baseline` | `--features http` | **10** tests, 0 failed | 118-07-SUMMARY.md: "**10 passed**, exit 0" | exact |

Both build fences exit 0 under `RUSTFLAGS="-D warnings"`:
`cargo build -p pmcp-team-servers --all-features` and
`cargo build -p pmcp-team-servers --no-default-features --features conformance`.

### The feature-sensitivity measurement that keys the guard

```
cargo test -p pmcp-team-servers               --test era_matrix   -> running 0 tests,  exit 0
cargo test -p pmcp-team-servers --features http --test era_matrix -> running 4 tests,  exit 0
cargo test -p pmcp-team-servers               --test era_baseline -> running 10 tests, exit 0
cargo test -p pmcp-team-servers --features http --test era_baseline -> running 10 tests, exit 0
```

### Acceptance-criteria greps

```
bash -n:                          exit=0        executable: yes
assert_nonzero_test_count:        2   (>= 2, definition + call)
'running [1-9]' guard:            1   (>= 1) — greps for a NONZERO count, not the word "running"
MATRIX_TESTS:                     5   (>= 2); members at :124 "conformance:",
                                  :127 "era_matrix:--features http", :129 "era_baseline:--features http"
test(/ | || true | continue-on-error in COMMANDS:  0
all-features:                     4   (>= 1)      no-default-features: 4  (>= 1)
RUSTFLAGS on both fence invocations: lines 203 and 207
set -euo pipefail: 1              trap: 1        HTTP_CFG_GUARDED_TARGETS: 2
```

### Negative controls — three, executed and reverted

**(d) `--features http` removed from the `era_matrix` entry** → `EXIT=1`

```
observed: running 11 tests
observed: running 0 tests
FAILURE: `era_matrix` ran ZERO tests.
CONSEQUENCE: "ran and passed" and "never compiled" are different observations, ...
WHAT TO DO: the likeliest cause is THE MISSING `--features http` FLAG. `tests/era_matrix.rs` is
selected by a `#![cfg(… feature = "http")]` guard and `http` is NOT in this
crate's default feature set, so without the flag the file compiles to nothing and
`cargo test` prints `running 0 tests` and exits 0 — measured 0 tests without
it, 4 with it. Check the `era_matrix` entry in MATRIX_TESTS above FIRST.
```

**(e) one `#[test]` removed from `tests/conformance.rs`** → `EXIT=0`, count fell 11 → 10, and
the script still passed. This is the intended, complementary shape: the shell guard is a
NONZERO FLOOR, and the EXACT-count fence for that corpus lives in `tests/conformance.rs`
(`fixture_corpus_is_exactly_thirty_three`, `all_servers_replay_exactly_the_expected_total`)
where 118-10 put it. Two fences, not one duplicated.

```
running 10 tests
test result: ok. 9 passed; 0 failed; 1 ignored
EXIT=0
```

**(f) a stranded private helper added under `RUSTFLAGS="-D warnings"`** → `EXIT=101`, proving
fence 3 is load-bearing (`make lint` passes `-D clippy::all`, not a bare `-D warnings`):

```
error: function `stranded_negative_control_helper` is never used
error: could not compile `pmcp-team-servers` (lib) due to 1 previous error
```

Both source-file controls (e) and (f) were reverted from scratchpad backups and confirmed with
an empty `git status --short <file>`.

---

## Task 3 — the Makefile targets

```
make -n test-conformance  -> ./scripts/run-conformance-suite.sh   (exit 0)
make -n test-era-matrix   -> ./scripts/run-era-matrix.sh          (exit 0)
.PHONY: test-conformance  at Makefile:385
.PHONY: test-era-matrix   at Makefile:424
```

The plan's `<automated>` command, run verbatim, exits 0:

```
make -n test-conformance >/dev/null && make -n test-era-matrix >/dev/null
  && make -n quality-gate > target/118-08-qg.txt
  && ! grep -qE 'run-conformance-suite|run-era-matrix' target/118-08-qg.txt
  && grep -q 'lint-plan-verify-commands' target/118-08-qg.txt
```

Counts in the captured `make -n quality-gate` output: `run-conformance-suite` 0,
`run-era-matrix` 0, `lint-plan-verify-commands` 1. Neither new target appears in
`quality-gate`, `test-all` or `test-integration`; `lint-plans` chaining is untouched.

Each comment names its CI job (`conformance-suite`, `era-matrix`) and
`tests/ci_conformance_gate_wiring.rs` as the pin, and `test-conformance`'s comment contrasts
itself with `lint-plans` so a reader learns the rule rather than the exception.

`/usr/bin/make quality-gate` → **exit 0**, log contains the literal
`ALL TOYOTA WAY QUALITY CHECKS PASSED` at line 9606 and **zero** `lines truncated` markers
(the rtk-proxy hazard from the prior-wave findings).

`/usr/bin/make lint-plans` → **exit 0**: "scanned 10 plan file(s), inspected 733 verification
line(s) … PASSED".

---

## Deviations from Plan

### 1. [D-21 — normative override] The whole-suite exit-0 assertion was not written

**Where:** `must_haves.truths` line 2, and `<behavior>` "either requirement-set run exits
non-zero".
**Why:** `118-CONFORMANCE-GAPS.md` is normative and measured the premise false. Both runs exit
1. The gate was scoped to the passing surfaces instead, and the suite's status is captured and
printed rather than suppressed. **Detailed in the D-21 section above.**

### 2. [Rule 1 — bug] My own readiness poll had a false green

**Found during:** Task 1, first end-to-end run.
**Issue:** `curl … -w '%{http_code}' … 2>&1 || printf 'curl-error'` — on connection refused
curl prints `000` **and** exits non-zero, so the fallback CONCATENATED both into
`000curl-error`, which matched neither sentinel and was accepted as ready on attempt 1. The
poll reported ready before anything was listening.
**Fix:** capture curl's status and output separately (`set +e` / `$?`), require status 0 AND a
non-empty, non-`000` code. Re-measured: "ready after 2 attempt(s) (HTTP 400)". Negative control
(c) now reports "last curl exit status 7, last reported HTTP code '000'".
**Committed in:** `0e18d9c7` (fixed before the commit).

### 3. [Rule 1 — bug] The era guard's diagnosis deleted itself in the case that needed it

**Found during:** Task 2, negative control (d).
**Issue:** `assert_nonzero_test_count` selected its "missing `--features http`" hint by
inspecting the FLAGS present at failure time. Control (d) removes that flag — so the control
that proves the guard fires also removed the guard's correct diagnosis, and it printed the
generic cause instead. Directly contradicts the plan's criterion that the message "names the
missing feature as the likeliest cause".
**Fix:** `HTTP_CFG_GUARDED_TARGETS=(era_matrix)`, keyed on the target NAME. Control (d) re-run
and the message now names the flag first.
**Committed in:** `ae699bf6` (fixed before the commit).

### 4. [Rule 1 — bug] Two header claims the file made about its own fences were false

**Found during:** the final `grep -n sleep` audit.
**Issue:** fence 6's comment claimed "the only `sleep` below is the backoff between poll
attempts" — there are three. And `kill … || :` in `cleanup` had no explanation, so a reviewer
grepping for status erasers would have to guess.
**Fix:** both corrected; the `|| :` now states that the teardown's verdict comes from the
unmasked `port_is_held` assertions, which a `kill` status cannot mask.
**Commit:** `cae76a31`.

### 5. [Plan/measurement discrepancy] `era_baseline` does NOT need `--features http`

**Plan claim:** `<interfaces>` and the criterion "the two `http` ones carry the flag" assert
that `era_matrix` **and** `era_baseline` require `--features http`.
**Measured:** `tests/era_baseline.rs` carries **no** `#![cfg]` guard at all and reports 10 tests
with and without the flag. Only `era_matrix` is feature-sensitive.
**Resolution:** `MATRIX_TESTS` keeps `--features http` on `era_baseline` — matching the plan's
data array and 118-07's recorded invocation — because it is the schema gate for the baseline
`era_matrix` joins against, and validating it under a *different* feature configuration than
the matrix consuming it is a weaker statement. But the header records the measurement
explicitly so nobody mistakes belief for fact, and `HTTP_CFG_GUARDED_TARGETS` lists only
`era_matrix`, so the guard's diagnosis stays truthful.

### 6. [Plan criterion unsatisfiable] `test -f justfile` cannot fail — a `justfile` predates this phase

**Criterion:** "No `justfile` was created: `test -f justfile` fails."
**Reality:** `justfile` is tracked and was added in `0fa9ef48` (v2.3, many phases ago).
**Intent satisfied instead:** this plan neither created nor modified it —
`git status --short justfile` is empty and
`git diff --stat 8fca23a6..HEAD -- justfile` is empty. No `justfile` was introduced, per
CLAUDE.md's repo-follows-Makefile convention.

### 7. [Rule 2 — missing critical functionality] A second zero-check array

An empty `ZERO_CHECK_SCORED_SCENARIOS` can only fire in ONE direction, which leaves the two
zero-check scenarios that actually exist unpinned. `ZERO_CHECK_NOT_SCORED_SCENARIOS` was added
under the same bidirectional equality. **Detailed above**; controls (e) and (e2) prove both
directions on both arrays.

### 8. [Design choice] `set -m`, not `setsid`

The plan says "`setsid` where available". `setsid` **forks** when the caller is already a
process-group leader, which makes `$!` a parent pid — the exact trap fence 5 exists to close —
and macOS ships no `setsid` at all. Bash job control (`set -m`) puts each background job in a
new group whose id equals the job's own pid, uniformly on both platforms, and the id is then
MEASURED with `ps -o pgid=` rather than assumed. Recorded in the header.

---

## Findings

### A not-scored check in the v2 leg is NONDETERMINISTIC — and the gate held across it

Across four green runs the v2 console total moved between `124 passed, 54 failed` and
`123 passed, 55 failed`. The mover is `http-header-validation`'s
`ServerAcceptsWhitespaceHeaderValue`:

```
ServerAcceptsWhitespaceHeaderValue: Expected successful response, but body contains JSON-RPC
error {"code":-32603,"message":"Internal error: no server->client channel on this transport:
elicitation/create cannot be issued"}.
```

This is exactly the mechanism 118-05 recorded: the check picks a tool out of `tools/list` and
calls it with a whitespace-padded `Mcp-Name`; when it happens to pick an elicitation tool the
call errors under **G-3**. The scenario is `pending` at 2026-07-28 — **not scored** — so it can
never fail the job.

The gate was **unaffected in both directions**, which is direct evidence the D-21 scoping and
the README § 7 rule 4 floor design are right:

- `MIN_CHECKS_V2` is a floor on checks **executed**, and the check executes either way — 178
  both times.
- the blocking surface is the MRTR scenarios, green in every run.

A gate keyed on the suite's pass COUNT would have been intermittently red here for a reason
that is not a defect in this SDK. That is the failure mode an allowlist would then have been
invented to paper over.

### `-o <dir>` ACCUMULATES; the driver must rebuild the results tree

The suite writes one directory per scenario **named with a timestamp**
(`server-<scenario>-2026-08-10T01-16-29-499Z`), so a second run into a populated tree ADDS
directories rather than replacing them — and every count computed from disk silently doubles.
`RESULTS_ROOT` is therefore removed before the first run (fence 10). Not documented in any
prior plan; it would have made the check floors and the zero-check gate meaningless on the
second CI run of a cached workspace.

---

## Threat Flags

None. Both files are CI shell drivers; they introduce no network endpoint, no auth path and no
schema. The one install remains `npm ci --prefix conformance --ignore-scripts` against the
lockfile 118-02 pinned and slopcheck-cleared, and no new package is introduced.

---

## Known Stubs

None. Both scripts are executable end to end and were run to completion; every constant is a
measured value, not a placeholder.

---

## Self-Check: PASSED

Files:

```
FOUND: scripts/run-conformance-suite.sh   (executable, bash -n exit 0)
FOUND: scripts/run-era-matrix.sh          (executable, bash -n exit 0)
FOUND: Makefile                           (test-conformance :385, test-era-matrix :424)
```

Commits, verified with `git rev-parse --verify <hash>` (see the note below on why the usual
`git log --oneline --all | grep` spelling was NOT used):

```
0e18d9c7 -> 0e18d9c7784c3bb52220963aaf2f9187fa20285b   feat: run-conformance-suite.sh
ae699bf6 -> ae699bf650e61672f5a94247cd80597dfabf693c   feat: run-era-matrix.sh
13b08d40 -> 13b08d4086fe354d7734d2ec8f44cc2f1018ac42   feat: the two Makefile targets
cae76a31 -> cae76a31c9cc3eb13db34e5d99174f520133f7de   docs: header accuracy fixes
```

**Self-check tooling note (a live instance of a prior-wave finding).** The template's
`git log --oneline --all | grep -q <hash>` check reported `MISSING` for all four commits that
`git rev-parse --verify` resolves and that `git log --oneline -6` prints. This is the rtk-proxy
output-corruption hazard already recorded for `git diff` and `gh pr checks` — piped `git log`
output is rewritten, so the grep sees something other than what the terminal shows. **A
self-check built on `git log | grep` can therefore report a false MISSING.** `git rev-parse
--verify` takes the hash as an argument rather than parsing rewritten stdout and is the
reliable spelling.

Behaviour:

```
./scripts/run-conformance-suite.sh   exit 0, all five gates OK, lsof -ti :8151 empty
./scripts/run-era-matrix.sh          exit 0, 11 / 4 / 10 tests, both build fences green
/usr/bin/make quality-gate           exit 0, literal pass banner, no truncation marker
/usr/bin/make lint-plans             exit 0, 10 plans / 733 lines inspected
git status --short                   empty
```
