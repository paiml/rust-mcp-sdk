---
phase: 121-local-round-trip-e2e
plan: 05
subsystem: testing
tags: [makefile, awk, quality-gate, regression-net, cargo-test, ci]

requires:
  - phase: 121-local-round-trip-e2e
    provides: "`test-openapi-server` target, `REQUIRED_TEST_BINARIES` list, and the `roundtrip_e2e` / `parity_replay` / `pmcp_package_pin` suites the guard names"
provides:
  - "`scripts/named-test-binary-count.awk` — a single anchored per-binary passed-count extractor"
  - "A `test-openapi-server` guard that fails when a NAMED test binary executes zero tests"
  - "`test-openapi-server-guard-selftest` — five-fixture proof that the extractor is sensitive, chained into `make quality-gate`"
  - "Per-binary passed counts in the gate's output, making a green run auditable from CI logs"
affects: [quality-gate, ci, phase-124-pkgr-01, any phase adding a required test binary]

actuals:
  tokens: 3564
  tasks: 3
  commits: 2

tech-stack:
  added: []
  patterns:
    - "Gate logic lives in `scripts/`, read by BOTH the gate and its self-test, so a green self-test is evidence about the gate rather than about a copy of it"
    - "Structural (field-equality) parsing of cargo output instead of substring matching"
    - "Sentinel return values (-1 / -2) instead of an empty string, so an unreadable gate fails loudly rather than passing vacuously"

key-files:
  created:
    - scripts/named-test-binary-count.awk
  modified:
    - Makefile

key-decisions:
  - "Gate on field 4 of the `test result:` line (the passed count), NOT the `running N tests` line — measured in this repo: an all-#[ignore]d binary prints `running 1 test` alongside `0 passed`"
  - "Anchor on `$1 == \"Running\" && $2 == want` field equality, never substring or regex — a substring is the defect being fixed, and `tests/<name>.rs` contains `.` characters a regex would treat as wildcards"
  - "Extractor lives in `scripts/` rather than inline in the Makefile so the self-test exercises the same bytes the gate runs, and to avoid `$$`-escaping every awk field reference"
  - "`-2` sentinel exists because the obvious spelling prints an empty string, which a shell numeric test treats as false — the guard would pass vacuously exactly when the output is untrustworthy"
  - "The guard branches ONLY on sentinels and zero/non-zero; no expected literal count is hardcoded, so the sibling plan 121-04 changing `pmcp_package_pin`'s count from 1 to 2 cannot turn this gate red"
  - "Fixtures are built with `printf` rather than heredocs: make recipe lines are backslash-joined into one shell command, in which a heredoc cannot be expressed"

patterns-established:
  - "Prove a guard behaviourally in both directions (red against a real defective fixture, green against the real suite) before claiming it works — a source-level assertion that the guard 'contains the right call' repeats the defect one level up"
  - "Ship a guard together with a self-test that re-proves its sensitivity inside the same gate"

requirements-completed: [PKG-04]

coverage:
  - id: D1
    description: "`make test-openapi-server` fails when a name in `REQUIRED_TEST_BINARIES` corresponds to a binary that ran but passed zero tests"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "make test-openapi-server (with throwaway all-#[ignore] crates/pmcp-openapi-server/tests/zz_guard_probe.rs named in REQUIRED_TEST_BINARIES) -> EXIT=2"
        status: pass
      - kind: integration
        ref: "control: same probe against the PRE-PLAN substring guard -> EXIT=0"
        status: pass
    human_judgment: false
  - id: D2
    description: "`make test-openapi-server` exits 0 on the unmodified tree and prints an observable per-binary passed count for each required binary"
    requirement: PKG-04
    verification:
      - kind: integration
        ref: "make test-openapi-server -> EXIT=0; parity_replay=3, pmcp_package_pin=1, roundtrip_e2e=8"
        status: pass
    human_judgment: false
  - id: D3
    description: "The extractor's sensitivity is re-proven on every gate run by a five-fixture self-test reading the same awk file"
    requirement: PKG-04
    verification:
      - kind: unit
        ref: "make test-openapi-server-guard-selftest -> EXIT=0 (real=8, all_ignored=0, cfg_empty=0, diagnostic_only=-1, truncated=-2)"
        status: pass
      - kind: unit
        ref: "make test-openapi-server-guard-selftest under a one-field extractor mutation ($4 -> $6) -> EXIT=2, names fixture 'real'"
        status: pass
    human_judgment: false
  - id: D4
    description: "Non-regression floor: the Phase 121 regression net and the full quality gate remain green"
    requirement: PKG-04
    verification:
      - kind: e2e
        ref: "cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1 -> 8 passed; 0 failed"
        status: pass
      - kind: e2e
        ref: "cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1 -> 3 passed; 0 failed; 1 ignored"
        status: pass
      - kind: integration
        ref: "make quality-gate -> QG_EXIT=0"
        status: pass
    human_judgment: false

duration: 21 min
completed: 2026-08-24
status: complete
---

# Phase 121 Plan 05: Per-Binary Test-Count Guard Summary

**CR-02 closed: `test-openapi-server` now derives each required binary's PASSED count from the `test result:` line following that binary's `Running` line via `scripts/named-test-binary-count.awk`, proven red against a real all-`#[ignore]` binary and green against the real suite, with a five-fixture self-test chained into `make quality-gate`.**

## Performance

- **Duration:** 21 min
- **Started:** 2026-08-24T02:28:00Z (approximate — earliest artifact timestamp)
- **Completed:** 2026-08-24T02:49:00Z
- **Tasks:** 3
- **Files modified:** 2 (1 created, 1 modified)

## Accomplishments

- Replaced the unanchored substring check with an anchored per-binary passed-count assertion, and demonstrated the behaviour change in both directions against the SAME real zero-test binary.
- Extracted the count logic into `scripts/named-test-binary-count.awk` with a documented four-value contract (`N` / `0` / `-1` / `-2`), so the gate and the proof of the gate read one file.
- Added `test-openapi-server-guard-selftest`, wired as `test-openapi-server`'s first prerequisite (0.02s, no cargo, no network), and proved it goes red under a one-field mutation of the extractor.
- Made a green gate auditable: the run now prints a per-binary passed count instead of a single aggregate line.

## Task Commits

1. **Task 1: Anchored per-binary count extractor + RED/GREEN demonstration** — `dd064c84` (fix)
2. **Task 2: Self-test that keeps the extractor's sensitivity proven** — `8d2f7bf8` (test)
3. **Task 3: Non-regression floor + gate-chain confirmation** — no commit; this is the verification task and it modified no file. `make quality-gate` passed, so nothing was appended to `deferred-items.md`.

## Files Created/Modified

- `scripts/named-test-binary-count.awk` (new, 84 lines) — Takes `-v want="tests/<name>.rs"`. Prints the passed count (field 4) of the first `test result:` line following the matching `Running` line; `-1` if no target line was found, `-2` if a target line was found with no result line after it. Header comments record the measured all-`#[ignore]` numbers, the `.`-escaping hazard a regex would carry, the doctest-trailer reason for `exit`-on-first-match, and why `-2` exists rather than an empty string.
- `Makefile` (+136/-14) — New `.PHONY: test-openapi-server-guard-selftest` target with five inline fixtures; `test-openapi-server` gains it as its FIRST prerequisite; the `REQUIRED_TEST_BINARIES` loop now calls the extractor and branches with a four-arm POSIX `case`; the comment block at the target rewritten to describe what is actually checked.

## Verbatim Evidence

Escape sequences appear literally below because the repo's Makefile echoes `\033[...]` under `/bin/sh`; that is pre-existing behaviour, not introduced here.

### 1. RED — new guard against the throwaway zero-test binary

`make test-openapi-server` → **EXIT=2**. Failure line, verbatim:

```
\033[0;31m✗ required test binary 'zz_guard_probe' RAN but passed ZERO tests. A #[cfg] gate turned false, an #[ignore] sweep landed, or the test module was renamed away. The summed total (41) stays nonzero from the other suites and the lib tests, so the count guard above CANNOT catch this. This is the regression net PKG-04 exists to keep running — restore the tests, do not relax this guard.\033[0m
make: *** [test-openapi-server] Error 1
```

The three real binaries reported their counts immediately above it, so the failure is isolated to the probe:

```
\033[0;32m  ✓ parity_replay passed 3 tests\033[0m
\033[0;32m  ✓ pmcp_package_pin passed 1 tests\033[0m
\033[0;32m  ✓ roundtrip_e2e passed 8 tests\033[0m
```

### 2. CONTROL — the SAME probe against the PRE-PLAN substring guard

**Exit status: `EXIT=0`.** The old guard passed a binary that executed nothing.

**How the control was staged:** it was run **BEFORE any fix was applied**, not by stashing. Sequence: (a) create `crates/pmcp-openapi-server/tests/zz_guard_probe.rs` containing one `#[test] #[ignore]` function; (b) append `zz_guard_probe` to `REQUIRED_TEST_BINARIES` in the still-unmodified Makefile; (c) run `make test-openapi-server`; (d) only then write the awk extractor and rewrite the loop; (e) re-run for the RED result above.

The control run's own output confirms the plan's measured claim inside this repo — the probe printed `running 1 test` while passing zero, which is the case a `running N tests` check would wrongly accept:

```
     Running tests/zz_guard_probe.rs (target/debug/deps/zz_guard_probe-61f7c811c87ef950)

running 1 test
test probe_never_runs ... ignored

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

and the old guard's verdict on that same run:

```
\033[0;32m✓ pmcp-openapi-server tests passed (41 tests)\033[0m
```

The same run also contains the doctest trailer with a bare `test result:` and no `Running tests/...` line (`Doc-tests pmcp_openapi_server` → `test result: ok. 4 passed`), which is why the extractor stops at the first result line after its match.

### 3. GREEN — per-binary counts on the restored tree

After deleting the probe and restoring the three-name list, `make test-openapi-server` → **EXIT=0**:

```
\033[0;32m✓ count extractor self-test passed: real=8, all_ignored=0, cfg_empty=0, diagnostic_only=-1, truncated=-2\033[0m
...
\033[0;32m  ✓ parity_replay passed 3 tests\033[0m
\033[0;32m  ✓ pmcp_package_pin passed 1 tests\033[0m
\033[0;32m  ✓ roundtrip_e2e passed 8 tests\033[0m
\033[0;32m✓ pmcp-openapi-server tests passed (41 tests)\033[0m
```

The self-test line is the FIRST output of the target, ahead of cargo's banner — the prerequisite is wired, not merely declared.

**`pmcp_package_pin` reported 1, not 2.** This worktree forked from `b23393ce` **before** sibling plan 121-04 landed, so the tree was in the pre-121-04 state. The count is recorded as observed. **No expected literal count is hardcoded anywhere in the Makefile** — the guard branches only on the `-1` / `-2` / `0` / non-digit / non-zero sentinels, so 121-04 raising this to 2 cannot turn the gate red.

### 4. Self-test mismatch under the reverted extractor mutation

With `scripts/named-test-binary-count.awk` temporarily printing `$6` (the failed count, 0 in every fixture) instead of `$4`, `make test-openapi-server-guard-selftest` → **EXIT=2**:

```
\033[0;31m✗ guard self-test fixture 'real': expected 8, actual 0\033[0m
make: *** [test-openapi-server-guard-selftest] Error 1
```

After reverting, the self-test exits 0 again and `git diff --stat scripts/named-test-binary-count.awk` reports no change relative to Task 1's committed state (empty output, rc=0). Timed at `real 0.02` with no `Compiling` or cargo `Running` banner in its output.

### 5. Non-regression floor

| Command | Result |
|---|---|
| `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | EXIT=0 — `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s` |
| `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` | EXIT=0 — `test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.11s` |
| `make test-openapi-server` | EXIT=0 (output in §3) |
| `make quality-gate` | `QG_EXIT=0` — `\033[0;32m        ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED        \033[0m` |

The quality-gate log confirms the gate reached this target: the self-test confirmation appears at line 7962 and the three per-binary counts at lines 8067-8069 of that run.

**No exit status was read through a pipe.** Every `make` and `cargo` invocation used a direct redirect (`> file 2>&1; echo "EXIT=$?"`), so `$?` is the command's own status. `${PIPESTATUS[0]}` was therefore not needed.

### 6. Gate chain and diff scope

- `grep -n '^test-all:' Makefile` → `753:test-all: test-unit test-doc test-property test-examples test-integration test-tester test-cargo-pmcp test-openapi-server` — `test-openapi-server` still present.
- `grep -c 'MAKE) test-all' Makefile` → `1` (inside the `quality-gate` recipe).
- `grep -c 'grep -q "tests/' Makefile` → `0` — the unanchored substring check is gone.
- `grep -c 'named-test-binary-count.awk' Makefile` → `4` (2 comment references, 2 actual `awk -f` invocations at lines 340 and 453). No second copy of the awk program exists in the Makefile; the only other awk uses are the pre-existing summed-count guards and unrelated coverage/version helpers.
- `grep -c 'test-openapi-server-guard-selftest' Makefile` → `4`.
- `grep -c 'REQUIRED_TEST_BINARIES="parity_replay pmcp_package_pin roundtrip_e2e"' Makefile` → `1`.
- `grep -c '#\[ignore\]' crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` → `0`.
- `git diff --name-only b23393ce..HEAD` → exactly `Makefile` and `scripts/named-test-binary-count.awk`. Nothing under `crates/pmcp-openapi-server/`, nothing in `.github/workflows/release.yml`, `CLAUDE.md`, or `COVERAGE.md`.

### 7. Throwaway artifacts confirmed gone

- `crates/pmcp-openapi-server/tests/zz_guard_probe.rs` deleted; `git status --porcelain crates/pmcp-openapi-server/tests/` reports nothing (clean).
- `REQUIRED_TEST_BINARIES` restored to the three original names — confirmed by the grep above and by the diff containing no change to that list's contents.
- The `$4` → `$6` extractor mutation reverted, confirmed by an empty `git diff --stat` on that file.
- `make check-todos` → EXIT=0; the new script introduces no marker the repo forbids.

## Decisions Made

See `key-decisions` in the frontmatter. The load-bearing one: the guard reads the **passed count**, not the `running N tests` line. `scripts/run-era-matrix.sh` gates on `running [1-9]`, which is correct for ITS failure mode (a `#![cfg]`-emptied file printing `running 0 tests`) but would sail straight past an `#[ignore]` sweep — measured here at `running 1 test` / `0 passed`. Inheriting that idiom would have reproduced CR-02 in a new spelling.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixtures built with `printf`, not heredocs**

- **Found during:** Task 2 (self-test target)
- **Issue:** The plan specified "five synthetic cargo-output fixtures (shell heredocs)". A make recipe's lines are backslash-joined into a **single** shell command, so a heredoc — which requires real newlines and a line-initial terminator — cannot be expressed there. The only way to get true heredocs would be `.ONESHELL:`, which is global and would change the semantics of every other recipe in this 1300-line Makefile.
- **Fix:** Each fixture is assembled as a shell variable via `printf '%s\n' 'line' 'line' ...` and passed to a `check()` helper defined in the same shell. The fixtures are byte-identical to the intended heredoc content, including the leading whitespace on the target line and the trailing fields of the result line, and they are fed through the same `awk -f scripts/named-test-binary-count.awk` invocation the gate uses.
- **Secondary consequence:** the per-fixture labelling comments the plan required ("label `all_ignored` as the CR-02 regression fixture", "label `diagnostic_only` as CR-02 point 2") could not live inside the recipe either — a `#` there would comment out the remainder of the joined line. They were placed in the comment block immediately above the target, as a five-row roll-call, with a note explaining why they are not inline.
- **Verification:** `make test-openapi-server-guard-selftest` exits 0 across all five fixtures and exits 2 naming `real` under the `$4`→`$6` mutation — so the fixtures do exercise the extractor.
- **Committed in:** `8d2f7bf8`

**2. [Rule 1 - Bug] First control measurement was untrustworthy; re-measured with absolute binary paths**

- **Found during:** Task 1 (control run)
- **Issue:** This environment runs an `rtk` shell hook that transparently rewrites bare `make`, `cargo` and `grep` invocations and returns **summarized** output plus its own exit status. The first control run reported `EXIT=0` over a 1957-byte log whose last line was literally `... (363 lines truncated)` and which contained **no `test result:` line at all**. That is the same class of false green the plan warns about for pipes — a status that describes the wrapper, not the command. Reading only the `EXIT=0` would have "confirmed" the control on evidence that did not exist.
- **Fix:** All measurement commands re-run through absolute binary paths — `/usr/bin/make`, `/Users/guy/.cargo/bin/cargo`, `/usr/bin/grep` — which bypass the hook and emit raw output. Every result recorded in this SUMMARY comes from an absolute-path invocation. The re-measured control agreed with the first run's verdict (`EXIT=0`), but only the re-measured run has the raw `Running tests/zz_guard_probe.rs` → `0 passed` evidence backing it.
- **Verification:** the re-run control log is 5286 bytes / 112 lines and contains every `Running`/`test result:` pair quoted in §2 above.
- **Committed in:** n/a (measurement methodology; no source change)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** No scope creep — the diff is still exactly the two declared files. Deviation 1 is a mechanical substitution that preserves the fixture semantics the plan specified. Deviation 2 changed no code; it corrected how the evidence was gathered, and is recorded because a future executor in this environment will hit the same false green.

## Issues Encountered

- **Shell-hook output filtering (see Deviation 2).** Worth carrying forward: in this repo's environment, `EXIT=$?` after a bare `make`/`cargo` can report the wrapper's status over truncated output. Use absolute binary paths for anything whose exit status or raw output is load-bearing.
- Nothing else. `make quality-gate` passed on the first attempt, so **nothing was appended to `deferred-items.md`** — there was no pre-existing failure to log.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- CR-02 is closed behaviourally, not by rewording. The `test-openapi-server` → `test-all` → `quality-gate` chain is intact and now carries its own sensitivity proof.
- All four ROADMAP Phase 121 success criteria remain VERIFIED: no test was deleted, weakened or `#[ignore]`d, and no name was removed from `REQUIRED_TEST_BINARIES`.
- **For the wave merge:** this worktree forked before 121-04. Both plans touch `Makefile`? No — 121-04's declared files are `crates/pmcp-openapi-server/Cargo.toml`, `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` and `CLAUDE.md`, all disjoint from this plan's two files, so no textual conflict is expected. After the merge `pmcp_package_pin` will report 2 rather than 1; the guard tolerates that by construction.
- Open cross-reference, unchanged by this plan: `scripts/check-release-coverage.sh` remains blind to workspace-excluded crates (deferred-items D11, owned by Phase 124 / PKGR-01).

## Self-Check: PASSED

- `scripts/named-test-binary-count.awk` — FOUND on disk.
- `.planning/phases/121-local-round-trip-e2e/121-05-SUMMARY.md` — FOUND on disk.
- Commits `dd064c84`, `8d2f7bf8`, `42a3af47` — all present in `git log --all --grep="121-05"`.
- All task acceptance criteria re-run and passing (§1-§7 above).
- Plan-level `<verification>` commands re-run and passing (§5, §6 above).
- Working tree carries only regenerated `.pmat/` tool caches (`context.db`, `context.idx/manifest.json`, `deps-cache.json`, `metrics/dependencies.json`, `project.toml`), touched by `make quality-gate`'s pmat steps. They were already dirty in the parent repo before this plan started, are generated tool state rather than source, and are deliberately NOT committed.

---
*Phase: 121-local-round-trip-e2e*
*Completed: 2026-08-24*
