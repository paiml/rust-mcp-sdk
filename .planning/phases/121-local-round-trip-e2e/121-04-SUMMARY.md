---
phase: 121-local-round-trip-e2e
plan: 04
subsystem: infra
tags: [cargo, crates-io, release, publish-order, dev-dependencies, pmcp-package, tripwire]

requires:
  - phase: 121-local-round-trip-e2e
    provides: "121-01's [dev-dependencies].pmcp-package entry and tests/pmcp_package_pin.rs, whose versioned shape is the CR-01 defect this plan closes"
provides:
  - "Path-only [dev-dependencies].pmcp-package in crates/pmcp-openapi-server/Cargo.toml — cargo publish -p pmcp-openapi-server no longer requires pmcp-package on crates.io"
  - "A two-assertion pmcp_package_pin tripwire that CATCHES the publish-breaking shape instead of mandating it, plus a re-anchored D-03 drift guard against the resolved crate's own version"
  - "CLAUDE.md release-ledger entries at both ends of the ordering constraint (item 9b and item 13)"
  - "Measured, executed evidence of cargo's dev-dep stripping behaviour in both the red and green direction"
affects: [phase-124-pkgr-01, release-workflow, pmcp-package-version-bumps]

actuals:
  tokens: 8847   # chars/4 over the three realized changed files (35,388 chars total)
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Path-only dev-dep into an unpublished sibling for any crate that publishes BEFORE that sibling in release.yml"
    - "Drift guards anchored to the resolved crate's own [package].version rather than to a version-requirement string, when the dep is a path dep"

key-files:
  created: []
  modified:
    - crates/pmcp-openapi-server/Cargo.toml
    - crates/pmcp-openapi-server/tests/pmcp_package_pin.rs
    - CLAUDE.md

key-decisions:
  - "Took plan option (a) — drop the version key — over reordering release.yml (one-way, inverts the stated experimental-0.x risk posture) or deleting the dev-dep (deletes PKG-04's deliverable)"
  - "Re-anchored D-03's drift guarantee from the caret requirement string to crates/pmcp-package's own [package].version, because a path dep never consults the registry a caret would constrain"
  - "Proved sensitivity of both new assertions with executed red controls rather than by re-reading source"

patterns-established:
  - "Publish-order constraint recorded at BOTH ends of the CLAUDE.md ledger so a reader arriving from either crate finds it"
  - "Probe isolation for cargo package: neutralize masking requirements by RETARGETING normal [dependencies] onto published versions; only [dev-dependencies] may be de-versioned"

requirements-completed: [PKG-04]

coverage:
  - id: D1
    description: "cargo publish -p pmcp-openapi-server no longer requires pmcp-package to exist on crates.io"
    requirement: "PKG-04"
    verification:
      - kind: integration
        ref: "cargo package --no-verify --allow-dirty on an isolated copy of the real manifest, FORM B (path-only) — exit 0, 'Packaged 5 files'"
        status: pass
      - kind: integration
        ref: "same probe, FORM A (version key present) — exit 101, 'failed to select a version for the requirement `pmcp-package = \"^0.2\"`' (negative control)"
        status: pass
      - kind: integration
        ref: "tar -xzO of the FORM B .crate tarball's generated Cargo.toml | grep pmcp-package -> 0 matches"
        status: pass
    human_judgment: false
  - id: D2
    description: "The pin tripwire enforces the publish-safe path-only shape and catches a re-added version key"
    requirement: "PKG-04"
    verification:
      - kind: unit
        ref: "crates/pmcp-openapi-server/tests/pmcp_package_pin.rs#pmcp_package_dev_dep_is_path_only"
        status: pass
      - kind: unit
        ref: "red control: version key re-added -> 'test result: FAILED. 1 passed; 1 failed' naming pmcp_package_dev_dep_is_path_only"
        status: pass
    human_judgment: false
  - id: D3
    description: "D-03's drift guarantee is retained against the crate the path dep actually resolves to"
    requirement: "PKG-04"
    verification:
      - kind: unit
        ref: "crates/pmcp-openapi-server/tests/pmcp_package_pin.rs#pmcp_package_resolved_crate_is_on_the_0_2_line"
        status: pass
      - kind: unit
        ref: "red control: crates/pmcp-package version -> 0.3.0 (with sibling reqs relaxed so the workspace resolves) -> FAILED naming pmcp_package_resolved_crate_is_on_the_0_2_line"
        status: pass
    human_judgment: false
  - id: D4
    description: "The publish-order constraint is recorded in the hand-maintained CLAUDE.md crate ledger at both ends"
    requirement: "PKG-04"
    verification:
      - kind: other
        ref: "grep -c 'path-only' CLAUDE.md -> 2; git diff --numstat CLAUDE.md -> 25 insertions, 0 deletions"
        status: pass
    human_judgment: true
    rationale: "A prose ledger's usefulness to the next human reader — whether the entry is findable and states the constraint unambiguously — is not something a grep can assert. The ledger has already drifted twice (2026-07-27, 2026-08-21), and both times the text existed."
  - id: D5
    description: "Non-regression floor: all four ROADMAP Phase 121 success criteria remain VERIFIED"
    requirement: "PKG-04"
    verification:
      - kind: e2e
        ref: "cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1 -> 8 passed; 0 failed"
        status: pass
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1 -> 3 passed; 0 failed; 1 ignored"
        status: pass
      - kind: other
        ref: "make test-openapi-server -> exit 0, 42 tests; make quality-gate -> exit 0; scripts/check-release-coverage.sh -> exit 0"
        status: pass
    human_judgment: false

duration: 30 min
completed: 2026-08-24
status: complete
---

# Phase 121 Plan 04: Close CR-01 — Path-Only `pmcp-package` Dev-Dep Summary

**Dropped the `version` key from `pmcp-openapi-server`'s `pmcp-package` dev-dep so cargo strips the entry from the published manifest entirely, proven with executed `cargo package` runs in both directions against an isolated copy of the real manifest; re-pointed the pin tripwire from mandating the publish-breaking shape to catching it, with D-03's drift guarantee re-anchored to the resolved crate's own version.**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-08-24T02:22Z (approx — first task commit at 02:37:16Z)
- **Completed:** 2026-08-24T02:55Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- **CR-01 is closed and the closure is *measured*, not asserted.** `crates/pmcp-openapi-server/Cargo.toml`'s `[dev-dependencies].pmcp-package` is now `{ path = "../pmcp-package" }`. An isolated copy of the *real* manifest packages successfully without it, and the generated manifest inside the produced `.crate` carries no `pmcp-package` entry at all.
- **The tripwire inverted.** It previously asserted the caret string that *causes* the release failure. It now asserts the entry is path-only and carries no `version` key, with a failure message that states the release consequence (`release.yml:339` vs `:440`) rather than the rule.
- **D-03's drift guarantee survived the change and got stronger.** It moved from the requirement string to `crates/pmcp-package/Cargo.toml`'s own `[package].version` — the crate the path dep genuinely resolves to. A caret only constrains a registry lookup a path dep never performs.
- **Both new assertions were proven sensitive by executed red controls**, then reverted.
- **The publish-order constraint is recorded at both ends of the CLAUDE.md ledger** (item 9b and item 13), insertion-only, with no crate ordinal renumbered.
- **`release.yml` was not touched.** The rejected one-way reorder stayed rejected.

## Task Commits

1. **Task 1 (tracer): Prove the publish shape end-to-end, then apply the path-only fix** — `a4e1cc68` (fix)
2. **Task 2: Re-point the pin tripwire at the shape it must now enforce** — `6080d2d1` (test)
3. **Task 3: Record the constraint in the release ledger and clear the non-regression floor** — `ffb6754c` (docs)

## Files Created/Modified

- `crates/pmcp-openapi-server/Cargo.toml` — `[dev-dependencies].pmcp-package` is now path-only; the `# Why` block above it was rewritten (its caret-pin instructions became false with the edit, and leaving them is how the next reader re-adds the key).
- `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — module doc rewritten; `EXPECTED_PIN`, `dependency_version_req` and `pmcp_package_pin_is_the_expected_caret_line` removed; `pmcp_package_dev_dep_is_path_only` and `pmcp_package_resolved_crate_is_on_the_0_2_line` added, plus `PMCP_PACKAGE_CARGO_TOML`. The "What this tripwire does NOT cover" four-emitter caveat is preserved.
- `CLAUDE.md` — Release & Publish Workflow item 9b gains the path-only constraint; item 13 gains a one-line cross-reference back to 9b.

## Verbatim Evidence

### Exit-status capture method

**No pipes were used.** Every `cargo` / `make` invocation redirected combined output to a file with `> file 2>&1` and `$?` was read on the very next line, so the status is cargo's own. `${PIPESTATUS[0]}` was therefore never needed. This is the discipline the plan mandated after a plan-time probe printed `EXIT=0` for a failing run piped into `tail`.

Separately: the **rtk proxy hook** rewrites bare `cargo`/`make`/`grep` invocations and *condenses or truncates their output* (a `make test-openapi-server` run produced a 51-line file ending `... (28 lines truncated)`, and a `cargo test` run replaced `test result: ok. 2 passed; 0 failed` with `cargo test: 2 passed (1 suite, 0.00s)`). Exit statuses were unaffected, but no verbatim `test result:` line survived. **All evidence below was therefore re-captured through absolute binary paths** (`/Users/guy/.cargo/bin/cargo`, `/usr/bin/make`, `/usr/bin/grep`).

### FORM A — RED direction (`version` key present)

`cargo package --no-verify --allow-dirty --manifest-path $PROBE/Cargo.toml`

**Cargo's own exit status: `101`.**

```
   Packaging pmcp-openapi-server v0.1.1 (/…/scratchpad/cr01-probe)
    Updating crates.io index
error: failed to prepare local package for uploading

Caused by:
  failed to select a version for the requirement `pmcp-package = "^0.2"`
  candidate versions found which didn't match: 0.1.1, 0.1.0
  location searched: crates.io index
  required by package `pmcp-openapi-server v0.1.1 (/…/scratchpad/cr01-probe)`
```

Independently confirmed against the sparse index: `pmcp-package` published versions are `['0.1.0', '0.1.1']`; the local crate is `0.2.0` and unpublished.

### FORM B — GREEN direction (path-only)

Same command, same status-capture discipline. **Cargo's own exit status: `0`.**

```
   Packaging pmcp-openapi-server v0.1.1 (/…/scratchpad/cr01-probe)
    Updating crates.io index
    Packaged 5 files, 85.6KiB (23.6KiB compressed)
```

### Generated manifest from the FORM B tarball

```
tar -xzOf …/target/package/pmcp-openapi-server-0.1.1.crate \
    pmcp-openapi-server-0.1.1/Cargo.toml > generated-Cargo.toml
grep -c 'pmcp-package' generated-Cargo.toml   ->   0
```

The generated `[dev-dependencies]` section contains only `serde_json`, `tempfile`, `tokio`, `toml`, `url` and `wiremock`. Both path-only dev-deps (`pmcp-package` and, in the probe, `mcp-tester`) were stripped wholesale — the entry does not appear in any form.

### Task 2 red control (a) — `version` key re-added

`test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out`

```
---- pmcp_package_dev_dep_is_path_only stdout ----
thread 'pmcp_package_dev_dep_is_path_only' panicked at
crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:119:5:
[dev-dependencies].pmcp-package must NOT carry a `version` key (CR-01). Cargo strips a dev-dep
from the published manifest only when it has no version requirement; one that has a requirement
is retained, and `cargo publish -p pmcp-openapi-server` must then resolve it against crates.io
while preparing the manifest. pmcp-openapi-server publishes at release.yml:339 and pmcp-package
at release.yml:440, ~100 steps later, so that lookup cannot succeed — and the step's fallback
tolerates only "already exists", so the whole release job dies. The `exclude` list does not save
it: the failure is at manifest-prep time, and excluding tests/ removes the consumers, not the
manifest entry.
```

### Task 2 red control (b) — `pmcp-package` version bumped off the 0.2 line

`test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out`

```
---- pmcp_package_resolved_crate_is_on_the_0_2_line stdout ----
thread 'pmcp_package_resolved_crate_is_on_the_0_2_line' panicked at
crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:148:5:
crates/pmcp-package is at version 0.3.0, off the expected 0.2.x line (PKG-04 / D-03).
roundtrip_e2e.rs was written against the 0.2 pmcp-package slot and package APIs, and this is a
PATH dependency — there is no version requirement left to refuse a drifting sibling, so the E2E
would silently compile against whatever the sibling became. Move the E2E to the new line
deliberately, then update this constant.
```

The failure message quotes `0.3.0` — a value read from disk, not from a constant — which is what proves the assertion tracks the real file.

### Green state after both controls were reverted

```
running 2 tests
test pmcp_package_dev_dep_is_path_only ... ok
test pmcp_package_resolved_crate_is_on_the_0_2_line ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`git status --porcelain crates/pmcp-package/` reported **no modification** after the revert, and the subsequent cargo run logged `Downgrading pmcp-package v0.3.0 -> v0.2.0`, confirming the sibling crate is back on 0.2.0.

### Non-regression floor

| Command | Exit | Verbatim result |
|---|---|---|
| `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | 0 | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.90s` |
| `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` | 0 | `test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.20s` |
| `make test-openapi-server` | 0 | `✓ pmcp-openapi-server tests passed (42 tests)` |
| `make quality-gate` | 0 | `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` / `🎯 ALWAYS Requirements Validated` |
| `bash scripts/check-release-coverage.sh` | 0 | `release-coverage: all 24 publishable workspace members have a publish step.` |
| `cargo clippy -p pmcp-openapi-server --all-targets` | 0 | no `pmcp-openapi-server … generated N warnings` line |
| `cargo fmt --all -- --check` | 0 | (no output) |

### Source assertions

| Assertion | Result |
|---|---|
| `grep -c 'version = "0.2"' crates/pmcp-openapi-server/Cargo.toml` | `0` |
| `grep -c 'path = "../pmcp-package"' crates/pmcp-openapi-server/Cargo.toml` | `1` |
| `grep -c 'EXPECTED_PIN' …/tests/pmcp_package_pin.rs` | `0` |
| `grep -c 'dependency_version_req' …/tests/pmcp_package_pin.rs` | `0` |
| `grep -c 'does NOT cover' …/tests/pmcp_package_pin.rs` | `1` |
| `grep -c 'path-only' CLAUDE.md` | `2` |
| `git diff --numstat CLAUDE.md` | `25  0  CLAUDE.md` (insertion-only) |
| `git diff --name-only b23393ce..HEAD` | exactly `CLAUDE.md`, `crates/pmcp-openapi-server/Cargo.toml`, `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` |

`release.yml`, `Makefile`, `scripts/check-release-coverage.sh`, `crates/pmcp-package/**`, `roundtrip_e2e.rs`, `parity_replay.rs`, `tests/common/mod.rs` and `COVERAGE.md` are all untouched, as the plan required. In particular **`Makefile` and `scripts/named-test-binary-count.awk` were not edited** — those are owned by the concurrent 121-05 executor.

## Decisions Made

- **Option (a), path-only**, as the plan chose. Reversibility `reversible`; no `checkpoint:decision` inserted.
- **The D-03 drift guard was re-anchored, not dropped.** Asserting `crates/pmcp-package`'s own version is a statement about what the E2E actually compiled against; asserting a caret string was a statement about a registry lookup a path dep never performs.
- **Mode resolution (recorded for transparency):** `workflow.auto_advance` is unset and `workflow._auto_chain_active` is `false`, which by the strict reading of the executor's auto-mode probe would make this an *interactive* run — and would have required halting at a `checkpoint:human-verify` after the tracer. This run was treated as **autonomous** on the basis of `.planning/config.json`'s `"mode": "yolo"` and the plan's `autonomous: true`, which is also the only reading compatible with executing in a disposable worktree where no human can answer. The tracer feedback gate was therefore run in its autonomous form: the tracer's `<verify>` was re-executed end-to-end after its commit and passed (`cargo build -p pmcp-openapi-server --tests` exit 0; caret-literal count 0) before any expansion task began.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The plan's probe-isolation recipe is rejected by cargo for normal `[dependencies]`**

- **Found during:** Task 1 (first FORM A run)
- **Issue:** The plan instructed: *"for every dependency entry that carries a `path` key — `pmcp`, `pmcp-server-toolkit`, `mcp-tester` — delete its `version` key."* Cargo refuses this outright for non-dev dependencies, and fails **before** any registry resolution:
  ```
  error: failed to verify manifest at `…/cr01-probe/Cargo.toml`
  Caused by:
    all dependencies must have a version requirement specified when packaging.
    dependency `pmcp` does not specify a version
  ```
  The probe would have "failed" for a reason that has nothing to do with CR-01 — exactly the vacuous-red failure mode the plan was written to avoid, mirrored.
- **Fix:** Only `[dev-dependencies]` may omit a version (that omission is precisely the stripping mechanism under test). Normal deps were neutralized by **retargeting** their requirement onto a version that *is* on crates.io rather than by removing it: `pmcp-server-toolkit` `^0.1.2` → `^0.1.1` (max published is 0.1.1; local 0.1.2 still satisfies `^0.1.1`), `pmcp` `^2.9.0` left as-is (already resolvable), `mcp-tester` de-versioned (it is a dev-dep). This preserves the plan's intent — make `pmcp-package` the only requirement that can fail — by the only means cargo permits.
- **Files modified:** none in the repo; the probe lives entirely in the session scratchpad.
- **Verification:** FORM A then failed on `pmcp-package` specifically, and FORM B passed. Both directions behaved as the plan predicted once the probe was constructible.
- **Committed in:** n/a (throwaway probe, never committed)

**2. [Rule 1 - Wrong expected value] The FORM A acceptance criterion's expected string does not occur**

- **Found during:** Task 1
- **Issue:** The criterion required the FORM A output to contain **both** `no matching package named` **and** `pmcp-package`. It contains the latter but not the former. The reviewer's original plan-time probe used a *nonexistent* crate name (`zz-planner-probe-leaf`), which produces `no matching package named`. `pmcp-package` **does** exist on crates.io at 0.1.x, so cargo emits the other variant: `failed to select a version for the requirement` / `candidate versions found which didn't match: 0.1.1, 0.1.0`.
- **Fix:** None needed in code — the criterion's expected string was wrong, not the behaviour. The observed message is a *stronger* reproduction: it names the exact requirement (`^0.2`) and the exact candidate set. The release consequence is unchanged and if anything better evidenced: `release.yml:344`'s fallback tolerates only `already exists`, and `failed to select a version` does not match it either, so the publish job still dies.
- **Verification:** exit 101 captured from cargo directly; verbatim block recorded above.
- **Committed in:** `a4e1cc68` (the fix itself)

**3. [Rule 3 - Blocking] Red control (b) as specified is unachievable — the workspace resolver preempts the test**

- **Found during:** Task 2
- **Issue:** The plan said to bump `crates/pmcp-package/Cargo.toml`'s version to `0.3.0` and confirm the failure **names** `pmcp_package_resolved_crate_is_on_the_0_2_line`. It does not. Four sibling crates pin `pmcp-package = "0.2"` in `[dependencies]` — `crates/pmcp-agent:18`, `crates/pmcp-team-servers:24`, `crates/pmcp-cfn-renderer:10`, `cargo-pmcp:87` — so the bump makes the **whole workspace unresolvable** and `cargo test -p pmcp-openapi-server` dies before compiling anything:
  ```
  error: failed to select a version for the requirement `pmcp-package = "^0.2"`
  candidate versions found which didn't match: 0.3.0
  required by package `pmcp-agent v0.3.0 (…)`
  ```
  Exit was non-zero, so a careless reading would have recorded this as the criterion satisfied. **It was not** — the named test never ran. Recording it as a pass would have been precisely the "source-text assertion mistaken for a behavioural proof" pattern CR-01 and CR-02 exist to punish.
- **Fix:** Widened the temporary mutation to make the workspace resolvable at 0.3 — the four sibling requirements were bumped to `"0.3"` alongside `pmcp-package`'s version — then the test ran and failed by name with the drift message quoting `0.3.0`. **All five files were then reverted**; `git status --porcelain` confirms only the tripwire file remained modified, and the plan's own `git diff --name-only` criterion confirms none of the five appear in the final change set.
- **Files modified:** temporarily only — `crates/pmcp-package/Cargo.toml`, `crates/pmcp-agent/Cargo.toml`, `crates/pmcp-team-servers/Cargo.toml`, `crates/pmcp-cfn-renderer/Cargo.toml`, `cargo-pmcp/Cargo.toml`. All reverted.
- **Verification:** post-revert `git status --porcelain` shows only `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs`; the suite is green again (2 passed); cumulative `git diff --name-only b23393ce..HEAD` lists exactly the three declared files.
- **Committed in:** `6080d2d1` (the tripwire; the mutations themselves were never committed)

---

**Total deviations:** 3 auto-fixed (2 blocking, 1 wrong-expected-value). **Impact:** no scope creep — all three are corrections to *how the plan's own proofs are obtained*, not changes to what was built. The plan's three declared `files_modified` are exactly the three files changed. Notably, deviations 2 and 3 are both cases where an acceptance criterion could have been recorded as met on a non-zero exit status alone while the underlying behaviour went unproven; both are reported rather than absorbed.

## Issues Encountered

- **The rtk proxy hook silently condenses and truncates command output.** A `make test-openapi-server` run produced a 51-line capture ending `... (28 lines truncated)`, and `cargo test` output had its `test result:` lines replaced by an rtk summary line. Exit statuses were correct throughout, but *no verbatim test-count evidence survived*. Every piece of evidence in this SUMMARY was re-captured through absolute binary paths. This matters beyond this plan: a `<verify>` block whose acceptance criterion greps for `test result:` or reads `$(grep -c …)` will behave differently under the hook than outside it — `grep -c` returned the string `0 matches for '…'` rather than `0`, which would break any `test "$(grep -c …)" -eq 0` comparison written literally as the plan's Task 1 `<verify>` was.
- **`.pmat/` cache and metric files** (`context.db`, `context.idx/manifest.json`, `deps-cache.json`, `metrics/dependencies.json`, `project.toml`) are modified by `make quality-gate` as a tooling side effect. Three of them were already dirty at session start. They were deliberately **not** staged; only the three declared files were committed.

## Deferred Items

Nothing was appended to `deferred-items.md` — `make quality-gate` exited 0, so there was no pre-existing gate failure to log.

## Known Stubs

None. No placeholder, `TODO`, `FIXME` or hardcoded-empty value was introduced; no test was deleted, weakened or `#[ignore]`d; no name was removed from the Makefile's `REQUIRED_TEST_BINARIES` list (the Makefile was not touched at all).

## Threat Flags

None. The change removes a version requirement and edits two files plus a prose ledger. It adds no package, no endpoint, no auth path and no schema. `T-121-04-02` (a squatted `pmcp-package 0.2.x` being named in this crate's published manifest) is now structurally impossible rather than merely bounded: the entry is absent from the published manifest entirely, as the FORM B tarball inspection shows.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **CR-01 is closed.** `121-VERIFICATION.md`'s first gap can be re-verified against `a4e1cc68`, `6080d2d1` and `ffb6754c`, and the phase's `gaps:` frontmatter entry for CR-01 should move to `resolved`.
- **CR-02 remains open** and is being closed concurrently by plan 121-05 (the `REQUIRED_TEST_BINARIES` guard in the Makefile). This plan deliberately did not touch `Makefile` or `scripts/named-test-binary-count.awk`.
- **Two things a future reader should know:**
  1. The four sibling `pmcp-package = "0.2"` pins (`pmcp-agent`, `pmcp-team-servers`, `pmcp-cfn-renderer`, `cargo-pmcp`) mean the workspace resolver *already* refuses a 0.2→0.3 drift one layer earlier than this tripwire does. `pmcp_package_resolved_crate_is_on_the_0_2_line` still has a distinct job: once those four are bumped in lockstep (as a real 0.3 migration would), it is the remaining guard that flags the openapi round-trip E2E specifically.
  2. `scripts/check-release-coverage.sh` still cannot see workspace-excluded crates, and now also cannot see this *class* of defect at all — it checks only that a publish **step** exists per crate, never whether a crate's declared dependencies can resolve at its point in the order. Phase 124 (PKGR-01) owns closing that.

## Self-Check: PASSED

- `crates/pmcp-openapi-server/Cargo.toml` (119 lines), `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` (157 lines) and `CLAUDE.md` (418 lines) all present on disk.
- All three commit hashes (`a4e1cc68`, `6080d2d1`, `ffb6754c`) found in `git log --oneline --all`.
- All task-level `<acceptance_criteria>` re-run and passing, with the three documented deviations where a criterion's literal expected value was wrong or unachievable.
- All plan-level `<verification>` commands re-run: results in the Non-regression floor table above.

---
*Phase: 121-local-round-trip-e2e*
*Completed: 2026-08-24*
