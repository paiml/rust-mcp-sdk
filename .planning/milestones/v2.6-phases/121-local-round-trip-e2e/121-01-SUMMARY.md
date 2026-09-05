---
phase: 121-local-round-trip-e2e
plan: 01
subsystem: testing
tags: [cargo, makefile, clippy, wiremock, pmcp-package, test-harness, quality-gate]

requires:
  - phase: 120-config-server-packaging
    provides: the london-tube config-slot shape (endpoint/secret/auth-mode) and the `pmcp-package` 0.2 slot API that the round-trip E2E will drive
provides:
  - "`crates/pmcp-openapi-server` can dev-depend on the workspace-EXCLUDED `crates/pmcp-package` (path dep resolves — previously unexercised)"
  - "`tests/pmcp_package_pin.rs` — a pin tripwire proven to FAIL on a drifted requirement"
  - "`make test-openapi-server` — the first gate that executes `crates/pmcp-openapi-server/tests/`, chained into `test-all` -> `quality-gate`"
  - "A nonzero-test-count guard AND a `REQUIRED_TEST_BINARIES` named-binary guard, both proven to have teeth"
  - "`tests/common/mod.rs` — six shared helpers as `pub` items, `mount_london_tube` parameterized by credential"
  - "`common::EnvVarGuard` — a `pub` panic-safe RAII env-var restore that plan 121-03 consumes but cannot create"
affects: [121-02, 121-03, pmcp-openapi-server tests, quality-gate]

actuals:
  tokens: 6982
  tasks: 3
  commits: 3

tech-stack:
  added: ["pmcp-package 0.2 (dev-dep, path)", "toml 1 (dev-dep)"]
  patterns:
    - "Named-binary gate assertion alongside the count guard — a nonzero SUM proves the package ran, not that a NAMED suite ran"
    - "Shared `tests/common/mod.rs` module for this crate's integration binaries"
    - "Panic-safe RAII `EnvVarGuard` created in the shared module by the plan that CAN reach the file, for the plan that cannot"

key-files:
  created:
    - crates/pmcp-openapi-server/tests/pmcp_package_pin.rs
    - crates/pmcp-openapi-server/tests/common/mod.rs
    - .planning/phases/121-local-round-trip-e2e/deferred-items.md
  modified:
    - crates/pmcp-openapi-server/Cargo.toml
    - crates/pmcp-openapi-server/tests/parity_replay.rs
    - Makefile

key-decisions:
  - "Used `toml = \"1\"` (root-workspace line), NOT RESEARCH's suggested `0.8` — `0.8` is `pmcp-package`'s pin and that crate is its own standalone workspace; adding a second major `toml` to the root graph would be a regression"
  - "Mutation-proved the pin tripwire with `0.2.0` rather than `0.3`, so the failure lands INSIDE the test rather than in Cargo resolution before the binary is built"
  - "`REQUIRED_TEST_BINARIES` matches on `tests/<name>.rs` (the cargo `Running` line) rather than a bare substring, so a name cannot be satisfied incidentally"
  - "Annotated the two PRE-EXISTING `clippy::await_holding_lock` sites with `// Why:` allows rather than restructuring — holding the guard across the awaited `run_serving` is the documented POINT of `tfl_env_lock`"
  - "Left `contoso_m365_parity.rs`'s duplicate `fixtures_dir`/`examples_dir` alone — D-02 names only `parity_replay.rs`, and widening a costly-reversibility edit to a second green file buys no PKG-04 benefit"

patterns-established:
  - "Gate targets assert BOTH a nonzero summed count and the presence of each required named test binary"
  - "`REQUIRED_TEST_BINARIES` is APPEND-ONLY across a phase — a name added before its binary exists turns the gate red for every commit in between"

requirements-completed: [PKG-04]

coverage:
  - id: D1
    description: "The `pmcp-package` dev-dep resolves from a workspace-EXCLUDED path crate and is pinned to the caret `0.2`, machine-checked"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test pmcp_package_pin -- --test-threads=1 (pmcp_package_pin_is_the_expected_caret_line)"
        status: pass
      - kind: other
        ref: "cargo test -p pmcp-openapi-server --no-run (proves the [dev-dependencies] path dep resolves — RESEARCH A2)"
        status: pass
    human_judgment: false
  - id: D2
    description: "The pin tripwire has teeth — it FAILS inside the test on a drifted requirement string"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation: manifest pin 0.2 -> 0.2.0, test exits 101 with the tripwire's own assert text at pmcp_package_pin.rs:83; reverted, passes again"
        status: pass
    human_judgment: false
  - id: D3
    description: "`make quality-gate` now executes `crates/pmcp-openapi-server/tests/` via `test-openapi-server` chained into `test-all`"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "make test-openapi-server (exit 0, 33 tests)"
        status: pass
      - kind: other
        ref: "grep -n '^test-all:' Makefile -> line 631 contains test-openapi-server"
        status: pass
    human_judgment: false
  - id: D4
    description: "The gate fails rather than passes when zero tests are selected"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "canned inputs through the target's own awk program: no `test result:` line -> 0; `test result: ok. 5 passed` -> 5"
        status: pass
    human_judgment: false
  - id: D5
    description: "The gate fails on a missing NAMED test binary, not only on a zero summed count"
    requirement: PKG-04
    verification:
      - kind: manual_procedural
        ref: "mutation: added `no_such_binary_xyz` to REQUIRED_TEST_BINARIES -> make exits 2 naming it, DESPITE a summed total of 33; removed, passes again"
        status: pass
    human_judgment: false
  - id: D6
    description: "The six helpers are shared in `tests/common/mod.rs`, `mount_london_tube` is parameterized by credential, and `parity_replay.rs` is behaviourally unchanged"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1 -> exactly 3 passed, 1 ignored (identical to the measured pre-change baseline)"
        status: pass
    human_judgment: false
  - id: D7
    description: "`common::EnvVarGuard` exists as a `pub` panic-safe RAII restore with both `Drop` branches, for plan 121-03 to consume"
    requirement: PKG-04
    verification:
      - kind: other
        ref: "structural: grep -c 'pub struct EnvVarGuard' = 1, 'impl Drop for EnvVarGuard' = 1; Drop body read and confirmed to carry BOTH set-back and remove branches"
        status: pass
    human_judgment: true
    rationale: "Only STRUCTURAL existence is proven here. The guard's BEHAVIOUR — that it restores on panic, in both the previously-set and previously-unset directions — is deliberately unproven in this plan: a `#[test]` under `tests/common/` compiles into every consuming binary and would have inflated parity_replay's required 3-passed/1-ignored count. Plan 121-03 Task 1 owns the `catch_unwind` proof. Until that lands, nothing executes this guard."
  - id: D8
    description: "`pmcp-openapi-server`'s own code is clippy-clean at a bar stricter than the repo gate"
    requirement: PKG-04
    verification:
      - kind: other
        ref: "cargo clippy -p pmcp-openapi-server --all-targets -> exit 0, ZERO `pmcp-openapi-server ... generated N warnings` lines"
        status: pass
    human_judgment: true
    rationale: "The plan's LITERAL command (`... -- -D warnings`) exits 101, but every failure is a PRE-EXISTING lint in a dependency crate (mcp-tester x2, pmcp-server-toolkit x1) outside this plan's files_modified and outside the repo gate's reach. The plan's intent is met for this crate; a human should confirm the scope-boundary call rather than have it auto-passed. See Deviations and deferred-items.md D1."

duration: 34 min
completed: 2026-08-24
status: complete
---

# Phase 121 Plan 01: Regression Net + Helper Lift Summary

**`crates/pmcp-openapi-server/tests/` becomes gate-executed for the first time — a `test-openapi-server` target with proven count AND named-binary guards — plus a `pmcp-package` 0.2 dev-dep with a mutation-proved pin tripwire and the D-02 helper lift into `tests/common/`.**

## Performance

- **Duration:** 34 min
- **Started:** 2026-08-23T23:33:00Z (approx — baseline compilation preceded the first commit)
- **Completed:** 2026-08-24T00:07:19Z
- **Tasks:** 3
- **Files modified:** 6 (3 created, 3 modified)

## Accomplishments

- **Closed a measured gate blind spot.** Nothing in this repo executed `crates/pmcp-openapi-server/tests/` before this plan — `parity_replay.rs` had been in that hole for its whole life. `make quality-gate` now reaches it.
- **Both gate guards are proven, not asserted.** The count guard and the named-binary guard were each demonstrated to fire, without breaking the tree.
- **Proved a previously-unexercised dependency edge.** A `[dev-dependencies]` path dep on the workspace-EXCLUDED `crates/pmcp-package` resolves (RESEARCH A2 — both prior in-repo precedents used `[dependencies]`).
- **The costly-reversibility lift landed clean.** `parity_replay.rs` reports the identical 3 passed / 1 ignored as the pre-change baseline.
- **Unblocked plan 121-03.** `common::EnvVarGuard` now exists in a file 121-03 cannot reach.

## Task Commits

1. **Task 1: pmcp-package dev-dep + pin tripwire** — `a87793b7` (test)
2. **Task 2: test-openapi-server gate target, chained into test-all** — `0b440227` (chore)
3. **Task 3: helper lift into tests/common + EnvVarGuard** — `c7035dd8` (refactor)

## Files Created/Modified

- `crates/pmcp-openapi-server/Cargo.toml` — added `[dev-dependencies].pmcp-package = { version = "0.2", path = "../pmcp-package" }` and `toml = "1"`, each with a `# Why:` comment
- `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — **new.** Parses this crate's own manifest with a real TOML parser and asserts the caret `0.2`
- `crates/pmcp-openapi-server/tests/common/mod.rs` — **new.** Six lifted `pub` helpers + the new `EnvVarGuard`
- `crates/pmcp-openapi-server/tests/parity_replay.rs` — helpers removed, `mod common;` + `use common::{...}` added, call site passes `DUMMY_APP_KEY`, dead imports pruned, two pre-existing clippy sites annotated
- `Makefile` — new `test-openapi-server` target; appended to the `test-all` prerequisite list (line 631)
- `.planning/phases/121-local-round-trip-e2e/deferred-items.md` — **new.** Two out-of-scope discoveries

## Measured Baselines and Required Proofs

### Executor's own pre-change baseline

Measured in this worktree before any edit, and it **agrees with the planner's 2026-08-23 measurement**:

- `cargo test -p pmcp-openapi-server -- --test-threads=1` -> **32 passed, 2 ignored, 8 suites, exit 0**
- `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` -> **3 passed, 1 ignored**

After this plan: **33** summed (32 + the new pin test), 9 suites, exit 0.

### Proof 1 — the pin tripwire fails on a drifted manifest

Mutated the requirement to the three-component `0.2.0` (**not** `0.3`, which Cargo could reject during resolution before building the test binary). The test binary compiled and RAN, and the failure is the tripwire's own assert text:

```
thread 'pmcp_package_pin_is_the_expected_caret_line' panicked at
crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:83:5:
assertion `left == right` failed: pmcp-package pin in [dev-dependencies] must be
the caret "0.2" (PKG-04 / D-03); do NOT use `=0.2.0` or a fully-qualified `0.2.0`
  left: "0.2.0"
 right: "0.2"
test result: FAILED. 0 passed; 1 failed; 0 ignored
```

Exit 101 — **inside the test**, not a Cargo resolution error. Reverted to `0.2`; passes again (1 passed).

### Proof 2 — the count guard has teeth

Piped canned cargo-style output through the target's own awk program:

| Input | Output |
|---|---|
| output containing no `test result:` line | `0` (the target treats this as a failure) |
| output containing `test result: ok. 5 passed; 0 failed` | `5` |

### Proof 3 — the named-binary guard has teeth

Temporarily added `no_such_binary_xyz` to `REQUIRED_TEST_BINARIES`:

```
✗ required test binary 'no_such_binary_xyz' did not run — a nonzero total (33)
  does not prove a NAMED suite ran
make: *** [test-openapi-server] Error 1
```

It fired **despite a summed total of 33** — exactly the blind spot it exists to close. Removed; the target passes again.

### Final `REQUIRED_TEST_BINARIES`

```
parity_replay pmcp_package_pin
```

Two names, as this plan's point in the phase requires. The list is documented APPEND-ONLY: plan **121-02 Task 1 adds `roundtrip_e2e`** when that binary first exists.

### `common::EnvVarGuard`

Exists as `pub struct EnvVarGuard` with `set`/`unset` constructors and a `Drop` impl carrying **both** branches — `Some(old) => set_var`, `None => remove_var`. A `Drop` that only removed would turn a previously-SET variable into an unset one, which is a different leak rather than a fix. The module declares **no `#[test]` functions** (verified: 0 matches), because items under `tests/common/` compile into every consuming binary and a test here would have inflated `parity_replay`'s count away from the required 3 passed / 1 ignored.

## Decisions Made

- **`toml = "1"`, not RESEARCH's `0.8`.** `0.8` is `crates/pmcp-package`'s pin, and that crate is its own standalone workspace. `pmcp-openapi-server` is a root member and every root-side consumer is on 1.x. The copied tripwire already compiles and passes against `toml = "1.0"` in `cargo-pmcp`.
- **Mutation with `0.2.0`, not `0.3`.** `0.2.0` satisfies the path crate's `^0.2.0` so it reaches the tripwire; `0.3` could be rejected during resolution, leaving the tripwire itself unproven.
- **`tests/<name>.rs` matching for the named-binary guard**, so a required name is satisfied only by cargo's actual `Running` line.
- **Annotated allows over restructuring** for `clippy::await_holding_lock` — the guard must outlive the awaited `run_serving` assembly, which is precisely what `tfl_env_lock`'s doc comment documents.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Task 3's clippy verify command cannot exit 0 — pre-existing dependency-crate lints**

- **Found during:** Task 3 (helper lift), running the plan's literal `<verify>` command
- **Issue:** `cargo clippy -p pmcp-openapi-server --all-targets -- -D warnings` exits **101**. The plan asserted it would exit 0, but its `measured_baseline` records no clippy run for this crate — the claim was never measured. The failures are 2 x `clippy::manual_filter` in `crates/mcp-tester/src/scenario_executor.rs:653,671` (plus 1 `redundant_guard` warning in `pmcp-server-toolkit`). Clippy lints workspace path dependencies, and `-D warnings` escalates theirs to errors.
- **Fix:** Confirmed out of scope and did **not** touch either crate — `git diff --stat f3f55f3d..HEAD` shows this plan touches only `Makefile`, `crates/pmcp-openapi-server/Cargo.toml`, and files under `crates/pmcp-openapi-server/tests/`. Verified the plan's *intent* instead: `cargo clippy -p pmcp-openapi-server --all-targets` exits 0 with **zero** `pmcp-openapi-server ... generated N warnings` lines. Logged to `deferred-items.md` D1.
- **Verification:** exit 0, no own-crate warning line
- **Committed in:** `c7035dd8` (deferred-items.md)

**2. [Rule 1 - Bug] Two `clippy::await_holding_lock` warnings in `parity_replay.rs`, proven pre-existing**

- **Found during:** Task 3
- **Issue:** The lift surfaced 2 warnings in `parity_replay.rs` (the `tfl_env_lock()` guard held across awaits). Because the lift moved that function, the warnings *could* have been a regression I introduced — and a costly-reversibility task must not guess about that.
- **Fix:** Measured rather than assumed. Restored the **original pre-lift file** from `f3f55f3d` and re-ran clippy: it emits the **identical 2 warnings**. So the lift introduced zero new lints; they had simply never been linted, because `make lint` (`Makefile:169`) has no `-p` and reaches only the root `pmcp` package. Restored my version and annotated both sites with `#[allow(clippy::await_holding_lock)]` + a `// Why:` comment explaining that holding the guard across the awaited `run_serving` is the documented point of `tfl_env_lock` and that an async-aware mutex would not help (the hazard is the process-global environment).
- **Files modified:** `crates/pmcp-openapi-server/tests/parity_replay.rs`
- **Verification:** `cargo clippy -p pmcp-openapi-server --all-targets` -> own crate generates 0 warnings; `parity_replay` still 3 passed / 1 ignored
- **Committed in:** `c7035dd8`

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug-investigation resolved as pre-existing)
**Impact on plan:** No scope creep. Neither deviation changed behaviour; one was a scope-boundary call and one was an annotation on pre-existing, previously-unlinted code. All three tasks landed their intended artifacts.

## Issues Encountered

- **The `rtk` shell hook silently truncates and summarizes command output.** A `make test-openapi-server` run captured through the default `Bash` path produced a file containing a literal `... (15 lines truncated)` marker, and `cargo test` output was replaced by a one-line summary with the per-binary `test result:` lines stripped. Because this plan's acceptance criteria depend on reading those exact lines, every measurement was re-taken using absolute binary paths (`/usr/bin/make`, `/Users/guy/.cargo/bin/cargo`, `/usr/bin/grep`, `/usr/bin/awk`). **This matters beyond this plan:** a verifier who reads gate output through the default path can see a truncated view and conclude a suite did not run. All counts in this summary come from absolute-path invocations.

## Verification Results (plan-level)

| # | Check | Result |
|---|---|---|
| 1 | `cargo test -p pmcp-openapi-server -- --test-threads=1` | exit 0, summed **33** (baseline 32 + pin test) |
| 2 | `make test-openapi-server` | exit 0, **33 tests** |
| 3 | `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` | **3 passed, 1 ignored** |
| 4 | `cargo fmt --all -- --check` | exit 0 |
| 5 | `cargo clippy -p pmcp-openapi-server --all-targets -- -D warnings` | exit 101 — **pre-existing dep-crate lints only**; own crate clean (Deviation 1) |
| 6 | `make pmcp-package-gate` | exit 0 — the new path edge did not disturb the excluded crate's standalone workspace |

Structural criteria: `pub ` count 10 (>=8); `allow(dead_code)` exactly 1; `pub struct EnvVarGuard` 1; `impl Drop for EnvVarGuard` 1; `DUMMY_APP_KEY` 3 (>=2, none in matchers); `#[test]` in `common/mod.rs` 0; `mod common;` in `parity_replay.rs` exactly 1; `fn mount_london_tube` in `parity_replay.rs` 0 (moved, not duplicated); non-comment `test-openapi-server` occurrences in `Makefile` 3.

## Known Stubs

None. No placeholder values, no `TODO`/`FIXME`, no skipped tests introduced.

## Follow-ups Recorded

- **`contoso_m365_parity.rs` residual duplication** — it keeps its own `fixtures_dir` (line 60) and `examples_dir` (line 66), now duplicated by `tests/common/mod.rs`. Deliberately untouched per D-02. Logged as `deferred-items.md` D2 for a later simplify pass.
- **Pre-existing dependency-crate clippy lints** — `deferred-items.md` D1.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for **121-02**. Specifically:

- The regression net exists BEFORE the risky work, which is what D-02 and RESEARCH Pitfall 5 required.
- **121-02 Task 1 must append `roundtrip_e2e` to `REQUIRED_TEST_BINARIES`** in the `test-openapi-server` target, in the same commit that first creates that binary — earlier turns the gate red, later leaves the new suite unguarded.
- **121-03 Task 1 must add the `catch_unwind` behavioural proof for `common::EnvVarGuard`** in both the previously-set and previously-unset directions. Until it does, the guard is structurally present but behaviourally unexecuted (coverage D7).
- `mount_london_tube(server, app_key)` is ready for two backends on two ports with two credentials (D-12).

## Self-Check: PASSED

- Files claimed created exist on disk: `tests/common/mod.rs`, `tests/pmcp_package_pin.rs`, `deferred-items.md` — all confirmed via `ls`.
- Commits claimed exist: `a87793b7`, `0b440227`, `c7035dd8` — all confirmed via `git log`.
- All task-level acceptance criteria re-run and passing, except the one documented deviation (Task 3 clippy `-D warnings`), which is recorded rather than silently skipped.

---
*Phase: 121-local-round-trip-e2e*
*Completed: 2026-08-24*
