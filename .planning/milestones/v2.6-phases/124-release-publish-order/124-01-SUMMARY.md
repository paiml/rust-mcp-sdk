---
phase: 124-release-publish-order
plan: 01
subsystem: infra
tags: [release, ci, bash, make, cargo, quality-gate, publish-order]

requires:
  - phase: 108-pmcp-package
    provides: crates/pmcp-package as a workspace-EXCLUDED publishable crate — the case the gate could not see
  - phase: 122
    provides: pmcp-package 0.3.0 and the four consuming manifests whose publish order this plan now machine-checks
provides:
  - Release-coverage gate that discovers workspace-EXCLUDED publishable crates by filesystem scan (24 -> 25 covered members)
  - Repo-wide scan-scope tripwire that converts the narrow crates/ glob from an implicit allowlist into a checked, declared scope
  - Bounded D-10 publish-ORDER assertion (pmcp-package precedes its four in-repo consumers)
  - Eight-fixture guard self-test declared as a prerequisite of the gate it proves
affects: [124-02, 124-03, 124-04, 124-05, 124-06, 124-07, release-workflow, quality-gate]

actuals:
  tokens: 6438
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Two-source discovery with ONE classification predicate: Cargo's own `.publish == null` classifies both root members and filesystem-discovered excluded crates, so a filesystem heuristic can never disagree with Cargo"
    - "Scan-scope tripwire: a narrow glob is only legitimate when a repo-wide assertion proves nothing qualifying lies outside it"
    - "Shell no-op sentinels (`: 'BEGIN ...'`) delimit a region that must survive comment-stripping"
    - "Per-fixture doctoring assertions (line-delta AND byte-difference) so a self-test cannot go green having doctored nothing"

key-files:
  created: []
  modified:
    - scripts/check-release-coverage.sh
    - Makefile
    - deploy/cloudflare/Cargo.toml
    - examples/wasm-mcp-server/Cargo.toml
    - examples/wasm-mcp-server/deployments/fermyon-spin/Cargo.toml

key-decisions:
  - "Declare, do not widen: the three tracked manifests matching D-01's RULE but lying outside its GLOB got `publish = false` (the script's own single documented opt-out) rather than the glob being widened repo-wide"
  - "Zero discovered excluded manifests is an explicit FAILURE, not a pass — the review finding that this is 'stronger than D-01 requires' was deliberately rejected"
  - "D-10 sentinels are shell no-ops rather than comments, because the excision criterion strips comments first"
  - "The D-10 consumer cluster is the single sanctioned hard-coded crate list in the script; DISCOVERY carries none"

patterns-established:
  - "Gate prerequisite edge: `check-release-coverage: check-release-coverage-guard-selftest` — a gate whose red direction is unproven is indistinguishable from a gate that always passes"
  - "Self-terminating `awk ... exit` instead of `... | head -1` wherever a first match is needed under `set -o pipefail`"

requirements-completed: [PKGR-01]

coverage:
  - id: D1
    description: "The release-coverage gate discovers workspace-EXCLUDED publishable crates by filesystem scan; crates/pmcp-package is now covered (member total 24 -> 25)"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "./scripts/check-release-coverage.sh — final line 'release-coverage: all 25 publishable workspace members have a publish step.', exit 0"
        status: pass
      - kind: integration
        ref: "make check-release-coverage-guard-selftest fixture 'excluded_step_removed' — gate exits non-zero naming pmcp-package"
        status: pass
    human_judgment: false
  - id: D2
    description: "Filesystem DISCOVERY (not merely the matcher) works: a previously-unknown workspace-excluded crate is found by scan against an INTACT workflow"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "make check-release-coverage-guard-selftest fixture 'synthetic_excluded_uncovered' (CRATES_DIR=$tmp/crates, intact release.yml) — exits non-zero naming zz-synthetic-uncovered"
        status: pass
    human_judgment: false
  - id: D3
    description: "Repo-wide scan-scope tripwire proves the narrow crates/ glob is a checked scope, not an implicit allowlist"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "manual falsifiability run — removing 'publish = false' from deploy/cloudflare/Cargo.toml makes the gate exit 1 naming that manifest; restored"
        status: pass
    human_judgment: false
  - id: D4
    description: "pmcp-package's publish step is machine-checked to precede all four in-repo consumers' (D-10)"
    verification:
      - kind: integration
        ref: "make check-release-coverage-guard-selftest fixture 'order_inverted' — exits non-zero with comment-stripped ordinals pmcp-package=574, pmcp-cfn-renderer=407"
        status: pass
      - kind: integration
        ref: "manual boundary proof — bounded matcher returns <none> on the prefix_shadow fixture where an unbounded one silently reads ordinal 426 (the -extra line)"
        status: pass
    human_judgment: false
  - id: D5
    description: "The gate's red direction is permanently proven across eight failure modes, with a fixture-count assertion, and runs inside make quality-gate and CI with no workflow edit"
    verification:
      - kind: integration
        ref: "make check-release-coverage-guard-selftest — 8 fixtures, exit 0"
        status: pass
      - kind: integration
        ref: "falsifiability — deleting one check invocation yields '✗ ... executed 7 fixtures, expected 8 — a fixture was lost', exit 2; restored"
        status: pass
      - kind: integration
        ref: "RUSTFLAGS=\"\" make quality-gate — exit 0; log lines 2278/2281 show the self-test and the 25-member reading inside the gate chain"
        status: pass
    human_judgment: false

duration: ~35 min
completed: 2026-08-27
status: complete
---

# Phase 124 Plan 01: Close the Release-Coverage Gate's Structural Blind Spot Summary

**`scripts/check-release-coverage.sh` now discovers workspace-EXCLUDED publishable crates by filesystem scan (24 -> 25 covered members, `crates/pmcp-package` included), asserts its publish step precedes all four consumers' with a boundary-carrying line-ordinal comparison, and proves its own red direction across eight fixtures wired as a Make prerequisite of the gate.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-08-27T16:22Z (approx.)
- **Completed:** 2026-08-27T16:57Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- **The headline blind spot is closed.** Before this plan, deleting `crates/pmcp-package`'s publish step from `release.yml` produced `release-coverage: all 24 publishable workspace members have a publish step.` and **exit 0**. The same input now exits **1** naming `pmcp-package`. The gate's own final line moved from 24 to **25** members.
- **Discovery is a scan, not a list.** The rule is "a manifest carrying its own `[workspace]` table that has not opted out with `publish = false`", classified by Cargo's own `.publish == null` — the same predicate the root half uses, so a filesystem heuristic cannot disagree with Cargo about what "publishable" means. Verified: `crates/pmcp-package/Cargo.toml:20` says `publish = true` and Cargo reports `publish=null`, so the predicate transfers.
- **The narrow glob is now a CHECKED scope.** A repo-wide tripwire over `git ls-files` fails naming any tracked manifest that matches the rule while lying outside `crates/`. Three such manifests existed and were **declared** (`publish = false`) rather than the glob being widened.
- **Publish ORDER is machine-checked** for the `pmcp-package` cluster, reading comment-stripped ordinals, with an explicit not-found failure arm on both sides and a `-ge` comparison so an equal reading (a matcher fault) fails rather than passes.
- **The gate's red direction is permanent**, not demonstrated once: eight fixtures with a fixture-count assertion and per-fixture doctoring assertions, declared as a prerequisite of `check-release-coverage`, which `quality-gate` (Makefile:1486 pre-edit) and `.github/workflows/ci.yml:218` already invoke — so CI inherits the extension with **no workflow edit**.

## Task Commits

1. **Task 1 (tracer): the gate sees crates/pmcp-package, red direction proven** — `d7232498` (feat)
2. **Task 2: machine-check the publish ORDER for the pmcp-package cluster (D-10)** — `eee87801` (feat)
3. **Task 3: expand self-test to eight fixtures; retire the stale header** — `73b43fb0` (test)

## Files Created/Modified

- `scripts/check-release-coverage.sh` — second discovery loop, scan-scope tripwire, empty-scan failure arm, D-10 order assertion, rewritten header, discipline list 4 -> 6 bullets
- `Makefile` — new `check-release-coverage-guard-selftest` target (8 fixtures + fixture table + doctoring assertions); `check-release-coverage` now declares it as a prerequisite
- `deploy/cloudflare/Cargo.toml`, `examples/wasm-mcp-server/Cargo.toml`, `examples/wasm-mcp-server/deployments/fermyon-spin/Cargo.toml` — `publish = false` added to each `[package]` table (no other change)

## SC1 re-verification transcript (Task 1 Step D)

Re-verified on this branch before any edit. **The premise had NOT drifted** — no adaptation was required.

```
$ ./scripts/check-release-coverage.sh
release-coverage: all 24 publishable workspace members have a publish step.
EXIT=0

$ /usr/bin/grep -c 'pmcp-openapi-server' CLAUDE.md
6
$ /usr/bin/grep -c 'pmcp-openapi-server' .github/workflows/release.yml
9
```

Both occurrence counts are non-zero, so `pmcp-openapi-server` holds slot 9b in the prose ledger and has publish steps in the workflow. Per `<flagged_assumptions>`, the correction block at `REQUIREMENTS.md:42-49` was treated as the requirement, never the superseded bullet at `:37`.

## Step A2 scope decision, with the measured inventory

**Decision: declare three manifests, keep the scan rooted at `crates/`.**

Measured on this branch with `git ls-files '*Cargo.toml'` + a whitespace-tolerant `[workspace]` match:

| Reading | Value |
|---|---|
| TRACKED manifests carrying their own `[workspace]` table | **26** |
| of those, WITHOUT `publish = false` | **5** |

The five, exactly as the plan predicted:

```
Cargo.toml                                                (the root workspace itself)
crates/pmcp-package/Cargo.toml                            (the target — inside the scan)
deploy/cloudflare/Cargo.toml                              -> declared publish = false
examples/wasm-mcp-server/Cargo.toml                       -> declared publish = false
examples/wasm-mcp-server/deployments/fermyon-spin/Cargo.toml -> declared publish = false
```

**Measured correction to the plan (recorded, not silently absorbed):** the plan states **32** tracked manifests carry their own `[workspace]` table; the measurement on this branch is **26**. The set of five lacking `publish = false` is byte-for-byte the set the plan names, so the decision is unaffected — the 32 figure most likely counted untracked spike copies under `.agents/`, `.claude/` and `.planning/spikes/`, which `git ls-files` correctly excludes. The script header records **26**, the measured figure.

**Why declare rather than widen.** Widening the glob repo-wide self-matches the root `Cargo.toml` (re-enumerating all 24 members through a second path) and sweeps in untracked spike copies, `fuzz/` manifests and `pmcp-macros/tests/fixtures/` manifests — making the gate's behaviour depend on which untracked directories a given developer happens to have. Declaring uses the script's OWN single documented opt-out (`:11-12`) and turns three accidental invisibilities into three deliberate declarations.

**Falsifiability of the tripwire (run by hand, then restored):**

```
$ /usr/bin/grep -v '^publish = false' <backup> > deploy/cloudflare/Cargo.toml
$ ./scripts/check-release-coverage.sh
::error::scan-scope violation — the following tracked manifest(s) carry their own
::error::[workspace] table, do NOT declare 'publish = false', and lie OUTSIDE the
::error::scanned scope ('crates/'), so this gate cannot see them at all:
  - deploy/cloudflare/Cargo.toml
EXIT=1
$ cp <backup> deploy/cloudflare/Cargo.toml   # restored; grep -c '^publish = false' == 1
```

## Step A4 rejection rationale (review finding: "zero excluded manifests is stronger than D-01 requires")

**Deliberately rejected, as the plan directs.** The arm stands: `excluded_seen -eq 0` is an explicit failure.

Two reasons. First, the script's own empty-list-is-failure discipline (`:47-51`, for the member list) is the direct precedent — this is its twin, not a new severity. Second, a reading of zero here means the scan scope or the working directory is wrong far more often than it means the last workspace-excluded crate was legitimately removed; this repo has had at least one such crate continuously since Phase 108. The Step A3 scan-scope tripwire is what makes a future legitimate removal a **deliberate one-line edit to this arm** rather than a surprise, which is precisely the trade the reviewer's concern was about.

## Task 2 verbatim record: `step_line`'s body (SIGPIPE shape absent)

```bash
step_line() {      # $1 = exact FIXED-STRING fragment (paths contain . and /)
  awk -v target="$1" 'index($0, target) { print NR; exit }' <<<"$PUBLISH_LINES"
}
step_line_re() {   # $1 = ERE; used wherever a crate NAME needs an end boundary
  awk -v re="$1" '$0 ~ re { print NR; exit }' <<<"$PUBLISH_LINES"
}
```

Here-string plus self-terminating `awk ... exit`. No `| head -1` pipeline exists on any executable line (measured 0; the single raw-file hit is the comment at `:234` that explains why the shape is banned).

## Task 2 behavioural red checks (run by hand)

**Order inverted** — publish line deleted from position and appended at EOF (line count unchanged at 659, so the coverage half still passes and only the order half can catch it):

```
::error::pmcp-package publishes AT OR AFTER pmcp-cfn-renderer in /tmp/124-t2/inverted.yml
::error::(comment-stripped ordinals: pmcp-package=574, pmcp-cfn-renderer=407).
EXIT=1
```

**Prefix shadow** — `cargo publish -p pmcp-agent` renamed to `... -p pmcp-agent-extra`:

```
::error::1 publishable workspace member(s) have no publish step in /tmp/124-t2/shadow.yml:
  - pmcp-agent
EXIT=1
```

Recorded honestly: this fires at the **root loop** (the boundary-carrying member matcher), which runs before the D-10 assertion, rather than at the D-10 not-found arm the plan anticipated. `pmcp-agent` is both a root member and a D-10 consumer, so the root loop reaches it first. The exit is non-zero and the output names `pmcp-agent` as required. Because that path does not exercise the D-10 matcher, the boundary was **additionally proven directly** against the same fixture:

```
bounded   'cargo publish -p pmcp-agent( |$)' -> ordinal '<none>'    (correct: not found)
unbounded 'cargo publish -p pmcp-agent'       -> ordinal '426'
line 426: OUTPUT=$(cargo publish -p pmcp-agent-extra 2>&1) && echo "$OUTPUT" || {
```

A boundary-less matcher silently reads the wrong step; the boundary-carrying one correctly reports not-found.

**File-exists guard survives:** `./scripts/check-release-coverage.sh /nonexistent/release.yml` -> `::error::/nonexistent/release.yml not found`, EXIT=1.

## Task 3 falsifiability of the fixture-count assertion (run by hand)

```
$ /usr/bin/grep -v "check prefix_shadow nonzero" <backup> > Makefile
$ make check-release-coverage-guard-selftest
✗ coverage gate self-test executed 7 fixtures, expected 8 — a fixture was lost
EXIT=2
$ cp <backup> Makefile   # restored; 8 fixtures, EXIT=0
```

## Header scope paragraph (quoted verbatim, per acceptance criteria)

```
# SCAN SCOPE is deliberately `crates/` rather than repo-wide, and it is CHECKED
# rather than assumed. Measured: 26 TRACKED manifests carry their own
# `[workspace]` table, and all but the root manifest, `crates/pmcp-package` and
# three declared deploy/example manifests opt out with `publish = false`.
# Widening the glob repo-wide was REJECTED: it self-matches the root manifest
# (re-enumerating every member through a second path) and sweeps in untracked
# spike copies, `fuzz/` and macro test fixtures, making the gate's behaviour
# depend on which untracked directories a given developer happens to have.
# Instead the narrow scope is PROVEN sufficient by a repo-wide scan-scope
# tripwire over `git ls-files` that fails naming any qualifying manifest outside
# it. That tripwire is what stops this glob from being the allowlist the rule
# above forbids.
```

## Decisions Made

- **Declare, don't widen** (Step A2) — three qualifying manifests got `publish = false`, the script's own single opt-out, instead of widening the discovery glob repo-wide.
- **Keep the empty-scan failure arm** (Step A4) — the review finding against it was rejected with the rationale above.
- **Shell no-op sentinels** — `: 'BEGIN D-10 ORDER ASSERTION'` rather than a comment, because the no-allowlist criterion strips comments before excising the region.
- **The D-10 cluster is the single sanctioned hard-coded list**, confined to the sentinel region and annotated in-file so a future reader does not "fix" it into a scan.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] D-10 sentinels must be shell no-ops, not comments**
- **Found during:** Task 2
- **Issue:** The plan directs "open the region with a comment line containing the exact sentinel". Task 1's no-allowlist acceptance criterion pipes the file through `grep -v '^[[:space:]]*#'` **before** the `awk` excision — so a commented sentinel is stripped first, the excision never triggers, and the criterion can never pass (the D-10 region's `pkg_line=...` line contains `pmcp-package`).
- **Fix:** Used `: 'BEGIN D-10 ORDER ASSERTION'` / `: 'END D-10 ORDER ASSERTION'` — shell null commands that survive comment-stripping. Annotated in-file so the reason is not lost.
- **Files modified:** `scripts/check-release-coverage.sh`
- **Verification:** `grep -c 'BEGIN D-10 ORDER ASSERTION'` = 1, `END` = 1; DISCOVERY-half `pmcp-package` count = 0.
- **Committed in:** `eee87801`

**2. [Rule 1 - Bug] Two Task 1 acceptance-criterion commands measure the raw file, and were already failing pre-phase**
- **Found during:** Task 1
- **Issue:** `grep -c 'mapfile'` (expect 0) and `grep -cE '\|[[:space:]]*(grep -q|head )'` (expect 0) read the **whole file including comments**. Both hits are pre-existing comment lines that document the banned shapes: `:28` (the bash-3.2 discipline bullet) and `:66` (the SIGPIPE rationale). Measured on the untouched pre-phase file via `git show HEAD:...`: **both counts are 1 there too**, so the criteria were unsatisfiable from the outset and are not a defect in this plan's code.
- **Fix:** Measured over comment-stripped (executable) lines instead — the plan's own established discipline for these greps, used verbatim in Task 3's no-allowlist criterion. Result: **0 and 0**.
- **Files modified:** none (measurement correction)
- **Verification:** `grep -v '^[[:space:]]*#' script | grep -c 'mapfile'` = 0; same pipeline for the SIGPIPE shape = 0.
- **Committed in:** n/a (no code change)

**3. [Rule 1 - Bug] Task 2's word-boundary criterion cannot hold as written**
- **Found during:** Task 2
- **Issue:** The criterion requires `grep -c 'cargo publish -p'` and `grep -c 'cargo publish -p ${[a-z_]*}( |\$)'` over executable lines to return the SAME number. Measured: **4 vs 0**. Two independent faults — (a) 2 of the 4 executable hits are `echo "::error::..."` message strings, not matchers, so they can never carry a matcher's boundary; (b) the BRE `\$` matches a literal `$`, but the text contains `\$` (backslash-dollar, the bash escape), so even the real matchers do not match. A corrected pattern escaping the backslash returns **2**, exactly the two matchers.
- **Fix:** Verified the criterion's intent directly instead — enumerated all four executable hits and confirmed both matchers (`:31` root loop, `:134` D-10 consumer loop) carry `( |\$)` and the other two are echo strings. Additionally proved the boundary discriminates behaviourally (bounded -> not found, unbounded -> ordinal 426).
- **Files modified:** none (measurement correction)
- **Verification:** recorded in the Task 2 red-check section above.
- **Committed in:** n/a (no code change)

**4. [Rule 2 - Missing Critical] Line-delta alone does not prove the 0-delta fixtures were doctored**
- **Found during:** Task 3
- **Issue:** The plan prescribes a per-fixture line-delta assertion (1 for deletion fixtures, 0 for comment-out and rename). For the three **0-delta** fixtures (`excluded_step_commented`, `order_inverted`, `prefix_shadow`) a delta of 0 is exactly what a completely failed `sed`/`grep` also produces — so the assertion cannot distinguish "doctored correctly" from "doctored nothing", which is the very false-green class it exists to prevent.
- **Fix:** `doctored_ok` asserts the expected line delta **and** `! cmp -s "$SRC" "$doctored"`, so a byte-identical copy fails with its own named message.
- **Files modified:** `Makefile`
- **Verification:** all 8 fixtures pass; the assertion is exercised on every doctored fixture.
- **Committed in:** `73b43fb0`

**5. [Rule 3 - Blocking] `set +e` / `set -e` prototype shape replaced with `|| actual=$?`**
- **Found during:** Task 1
- **Issue:** PATTERNS prescribes `set +e; ./script ...; actual=$?; set -e;`. Make recipes run under `/bin/sh` **without** `-e`, so that trailing `set -e` would newly *enable* errexit mid-recipe, aborting the harness on the first benign non-zero status (e.g. a `grep -q` miss) rather than reporting a fixture failure.
- **Fix:** `actual=0; ... || actual=$$?;` — equivalent status capture with no errexit juggling.
- **Files modified:** `Makefile`
- **Verification:** `make check-release-coverage-guard-selftest` reports 8 fixtures and correctly reports 7-with-failure when a fixture is dropped.
- **Committed in:** `d7232498` (introduced), `73b43fb0` (retained)

**6. [Rule 3 - Blocking] The synthetic discovery fixture needs a source file**
- **Found during:** Task 3
- **Issue:** `cargo metadata --manifest-path` fails on a manifest with no target ("no targets specified in the manifest"), which would have made `synthetic_excluded_uncovered` fail through the gate's `cargo metadata` failure arm rather than through discovery — a green-looking fixture that proves the wrong thing.
- **Fix:** The fixture creates `src/lib.rs` alongside the synthetic `Cargo.toml`.
- **Files modified:** `Makefile`
- **Verification:** the fixture's failure output names `zz-synthetic-uncovered` and cites the missing `--manifest-path` step, confirming it failed through the discovery path.
- **Committed in:** `73b43fb0`

**7. [Rule 1 - Measurement correction] Tracked-manifest inventory is 26, not 32**
- **Found during:** Task 1
- **Issue:** The plan's Step A2 states 32 tracked manifests carry their own `[workspace]` table. Measured on this branch: **26**.
- **Fix:** Recorded 26 in the script header and in this SUMMARY, with the likely cause (the 32 figure counted untracked spike copies that `git ls-files` excludes). The set of 5 lacking `publish = false` is exactly as the plan names, so no decision changed.
- **Files modified:** `scripts/check-release-coverage.sh` (header text)
- **Verification:** inventory reproduced above; the tripwire passes with the three declarations in place and fails when any is removed.
- **Committed in:** `73b43fb0`

---

**Total deviations:** 7 auto-fixed (3 Rule 1 bugs, 1 Rule 2 missing-critical, 2 Rule 3 blocking, 1 Rule 1 measurement correction).
**Impact on plan:** No scope creep. Three of the seven (2, 3, 7) are corrections to plan *measurements and criterion commands* rather than to code — two of those criteria were already failing against the untouched pre-phase file, which is itself evidence for this plan's own thesis that an unproven check is indistinguishable from a passing one. The four code-affecting fixes each close a path by which a check could have looked green while proving nothing.

## Issues Encountered

None blocking. One process note: the tracer feedback gate (Task 1 -> Task 2) was run as an **autonomous end-to-end re-verify** rather than a mid-flight halt. Rationale: `.planning/config.json` sets `mode: yolo`, `workflow.human_verify_mode` is unset so it defaults to `end-of-phase` (#3309, "New projects do NOT halt mid-flight at checkpoint:human-verify"), the plan declares `autonomous: true`, and this executor runs in a detached worktree with no interactive channel. `make check-release-coverage` was re-run end-to-end after the tracer commit and passed (exit 0, 25 members) before any expansion task began. The human-facing verification content is carried in the `coverage:` block above for end-of-phase harvest.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plans 02-07 are unblocked.** Their shared premise — a coverage gate that can actually see what it claims to check — now holds: the gate covers 25 members including the workspace-excluded `crates/pmcp-package`, and its red direction is asserted on every `make quality-gate` run rather than demonstrated once.
- **CI needs no edit.** `.github/workflows/ci.yml:218` already runs `make check-release-coverage`, which now pulls the self-test in as a prerequisite.
- **Carried forward for plan 05:** the `⚠ ORDERING CONSTRAINT FOR PHASE 124` in CLAUDE.md item 13 — the D-10 assertion now locks the publish *order*, but the *version* half (pmcp-package and its three pinning crates must move as one set) is still prose-only and is plan 05's job.
- **One residual for a future phase:** `CLAUDE.md`'s prose publish ledger remains hand-maintained, and the machine check covers the workflow half only. Item 17 of that ledger still describes workspace-excluded crates as "a known blind spot of the check until Phase 124 (PKGR-01)" — that sentence is now stale and should be updated by whichever later plan in this phase touches the ledger.

## Self-Check: PASSED

- Artifacts on disk: `scripts/check-release-coverage.sh`, `Makefile`, and all three declared manifests FOUND; Makefile target `check-release-coverage-guard-selftest` FOUND; prerequisite edge FOUND.
- Commits: `d7232498` FOUND, `eee87801` FOUND, `73b43fb0` FOUND.
- Plan `<verification>` re-run at close: `make check-release-coverage` exit 0 (self-test line precedes the gate line; 25 members); `make check-release-coverage-guard-selftest` exit 0 across 8 fixtures; `RUSTFLAGS="" make quality-gate` exit 0.
- No stubs, no skipped tests, no unrun verify commands. The three `TODO|FIXME` hits in `Makefile` are the repo's own pre-existing `check-todos` target (`:1710`, `:1711`, `:2031`), untouched by this plan.
- Three deviation entries recorded in `.planning/WINDOWS.md` (the defective criterion commands, so a verifier re-running them literally is not misled).

---
*Phase: 124-release-publish-order*
*Completed: 2026-08-27*
