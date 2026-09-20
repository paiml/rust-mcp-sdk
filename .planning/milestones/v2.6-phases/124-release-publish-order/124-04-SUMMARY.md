---
phase: 124-release-publish-order
plan: 04
subsystem: infra
tags: [release, ci, documentation, publish-order, ledger, semver, crates-io]

requires:
  - phase: 124-release-publish-order
    provides: "plan 01's release-coverage gate + D-10 publish-ORDER assertion — the mechanism this prose now names as its enforcement"
  - phase: 124-release-publish-order
    provides: "plan 03's closed bump list and `make release-sweep` — the caret exception this plan records, and the target the Pre-Flight step now points at"
  - phase: 122
    provides: "the nine-emitter inventory at 122-08-SUMMARY.md:464-476, the authority the corrected enumeration is measured against"
provides:
  - Reconciled publish-order comments in .github/workflows/release.yml — every order and pin claim now agrees with the file's own steps and with the manifests
  - A single authoritative ordering-constraint statement in CLAUDE.md (marker phrase asserted at exactly one occurrence), with items 13a/14/15/15a cross-referencing it
  - Phase 124's ⚠ ORDERING CONSTRAINT obligation discharged in writing against the measured manifests
  - A corrected nine-emitter enumeration that lists nine, including the path-only UNCHANGED row
  - The caret exception for PATCH bumps in Version Bump Rules
  - A Pre-Flight Checklist that no longer prescribes the version oracle the same document forbids
  - The mcp-tester six-pin/four-before-publish note, labelled inherited release risk
affects: [124-05, 124-06, 124-07, release-workflow, future-releasers]

actuals:
  tokens: 5100
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "State pin VALUES by reference, not by literal: a version quoted in a comment has no guard and rots silently; name the manifest that owns it instead"
    - "Consolidation without erasure: one authoritative statement plus distinctly-worded pointers, with the dated correction notes preserved as the reason the constraint is trusted"
    - "A prose list that mirrors executable steps is kept COMPLETE (one entry per step), because a partial list cannot be checked against the steps it mirrors"
    - "Discharge an open obligation in place: record the measurement and its date where the obligation was written, so the next reader inherits a closed item rather than an open one"

key-files:
  created: []
  modified:
    - .github/workflows/release.yml
    - CLAUDE.md

key-decisions:
  - "Completed the head publish-order list to all 25 steps rather than only fixing its tail — a list omitting 9 of 25 steps is not checkable against them, and that incompleteness is how the tail rotted unnoticed"
  - "Pin values in release.yml comments moved to by-reference form rather than being refreshed to current literals — refreshing preserves the rot mechanism, which has now fired twice"
  - "Rejected the reviewer's 'reduce Nine to Eight' fix; 122-08-SUMMARY.md:464-476 lists nine and the enumeration was short by the path-only row"
  - "Pre-Flight now describes the forbidden oracle by class and points at item 13 for the names, rather than naming it in place — satisfies the literal criterion while keeping the prohibition discoverable"

patterns-established:
  - "Byte-equality of ALL non-comment, non-blank lines as the proof of a comment-only edit — stronger than greping the diff for the specific command lines you meant not to touch"
  - "Capture a gate's exit status inside a script that writes $? to a file, because an output proxy can report its own status alongside a truncated log"

requirements-completed: [PKGR-01]

coverage:
  - id: D1
    description: "release.yml's comments state that pmcp-package publishes BEFORE its four consumers, and no comment claims it publishes last or has no in-repo consumers (SC4, D-09)"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "grep -n 'publishes BEFORE' .github/workflows/release.yml -> :449 naming all four consumers on one line"
        status: pass
      - kind: integration
        ref: "grep -ci 'consumers yet' over the heading-anchored publish-order block -> 0"
        status: pass
    human_judgment: false
  - id: D2
    description: "No comment in release.yml quotes a caret-pin literal that disagrees with the manifest it describes; pin values are stated by reference"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "grep -c 'pmcp-package = \"0.1\"' release.yml -> 0; grep -cE '(pmcp-cfn-renderer|pmcp-agent|pmcp-team-servers) = \"0\\.1\"' -> 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "Every non-comment, non-blank line of release.yml is byte-identical before and after — asserted over the whole file, not merely over cargo publish lines"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "diff of comment-stripped HEAD vs working copy -> identical, 512 lines both sides, exit 0"
        status: pass
      - kind: integration
        ref: "count of changed diff lines that are neither comment nor blank -> 0"
        status: pass
    human_judgment: false
  - id: D4
    description: "CLAUDE.md states the pmcp-package-precedes-its-four-consumers constraint in exactly one place, with items 13a/14/15/15a cross-referencing it by a different phrase"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "grep -c 'AUTHORITATIVE ORDERING CONSTRAINT' CLAUDE.md -> exactly 1; grep -c 'see the authoritative statement' -> 5 (>= 4 required)"
        status: pass
    human_judgment: false
  - id: D5
    description: "CLAUDE.md's nine-emitter paragraph enumerates NINE, including crates/pmcp-openapi-server/Cargo.toml:124 (path-only, UNCHANGED); the count is not reduced to eight"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "grep -c 'Nine were measured' CLAUDE.md -> 1; the paragraph names pmcp-openapi-server/Cargo.toml:124 with 'UNCHANGED' and 'path-only' adjacent"
        status: pass
    human_judgment: false
  - id: D6
    description: "Version Bump Rules carry the caret exception for PATCH bumps, with the live pmcp 2.19.0 -> 2.19.1 instance and its two pin sites"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "heading-anchored Version Bump Rules extraction: grep -ci 'caret' -> 2, grep -c '\\^2\\.19\\.0' -> 1"
        status: pass
    human_judgment: false
  - id: D7
    description: "Pre-Flight Checklist no longer prescribes a Cargo registry-search command; it points at the crates.io API (with the mandatory User-Agent) and make release-sweep"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "heading-anchored Pre-Flight extraction: 'cargo search' -> 0, 'crates.io/api/v1/crates' -> 1, 'release-sweep' -> 1, 'User-Agent' -> 2; item 13's prohibition preserved (whole-file 'cargo search' -> 1)"
        status: pass
    human_judgment: false
  - id: D8
    description: "The machine-checked half is undisturbed and the project quality gate passes"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "make check-release-coverage -> exit 0, 8 fixtures, 25 members (run after each task)"
        status: pass
      - kind: integration
        ref: "RUSTFLAGS='' make quality-gate -> exit 0 via status-file capture; 11551-line log ending 'ALL TOYOTA WAY QUALITY CHECKS PASSED'"
        status: pass
    human_judgment: false
  - id: D9
    description: "The ⚠ ORDERING CONSTRAINT block's open Phase-124 obligation is discharged in writing"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "manifest measurement: pmcp-cfn-renderer:10, pmcp-agent:18, pmcp-team-servers:24, cargo-pmcp:88 all read pmcp-package = \"0.3\"; recorded in the authoritative block with its date"
        status: pass
    human_judgment: false

duration: ~40 min
completed: 2026-08-27
status: complete
---

# Phase 124 Plan 04: Reconcile Both Publish Ledgers Summary

**Both publish ledgers now agree with each other and with the manifests: `release.yml`'s comments state the D-09 cluster constraint on one line and quote no pin literals, while CLAUDE.md carries exactly one authoritative ordering statement (with four distinctly-worded cross-references), a nine-item emitter enumeration, the caret exception for patch bumps, and a Pre-Flight step that no longer prescribes the version oracle the same file forbids — all with every executable line of `release.yml` byte-unchanged.**

## Performance

- **Duration:** ~40 min
- **Started:** 2026-08-27T18:25Z (approx.)
- **Completed:** 2026-08-27T19:02Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- **The workflow's own prose no longer contradicts its steps.** The head `# Publish order:` list placed `pmcp-package` LAST, after `cargo-pmcp` and `pmcp-server`, with a parenthetical asserting it "has NO in-repo consumers yet" and instructing a maintainer to "move it earlier only once a shipped crate actually pins it." Four crates pin it and it was moved earlier long ago. Both halves are corrected.
- **Pin literals are gone from the comments entirely.** All four stale `= "0.1"` quotes in the cluster block and the one in the `pmcp-cfn-renderer` block are replaced with by-reference wording naming the manifests that own the values. Refreshing them to `"0.3"` was rejected: that preserves the rot mechanism, which has already fired twice.
- **The D-09 constraint is stated, not inferred.** `release.yml:449` now carries it as a single indented sentence naming all four consumers, and CLAUDE.md carries the authoritative form under item 13.
- **Phase 124's inherited obligation is discharged.** The ⚠ block asked this phase to check the three consumers' `pmcp-package` requirements against the shipping version. Measured: all four read `"0.3"`, matching the `0.3.0` shipping at this tag. Recorded in place, dated, with the explicit caveat that the discharge is a measurement of this tag rather than a standing guarantee.
- **The nine-emitter count is defended, not conceded.** A cross-AI reviewer read the eight-item enumeration as evidence the count of nine was wrong. `122-08-SUMMARY.md:464-476` lists nine; row 6 is the path-only `crates/pmcp-openapi-server/Cargo.toml:124`, marked UNCHANGED. It is now named, with the reason it is listed despite never changing.
- **Comment-only editing is proven semantically, not by proxy.** All 512 non-comment, non-blank lines of `release.yml` are byte-identical before and after.

## Task Commits

1. **Task 1: reconcile every order and pin claim in release.yml's comments** — `5fac7423` (docs)
2. **Task 2: consolidate the ordering constraint in CLAUDE.md; fix the Pre-Flight contradiction** — `9c948e1c` (docs)

## Files Created/Modified

- `.github/workflows/release.yml` — Regions A–D rewritten (+58/−18, all comment/blank lines): head publish-order list corrected and completed to one entry per step; the false `pmcp-package` parenthetical replaced; cluster block restated with the D-09 sentence and by-reference pins; `pmcp-cfn-renderer` block given the same treatment.
- `CLAUDE.md` — (+125/−31): authoritative ordering constraint under item 13 with the obligation discharged; four cross-references; nine-emitter enumeration corrected; caret exception in Version Bump Rules; Pre-Flight step 2 replaced; `mcp-tester` inherited-risk note in Release Steps; item 17's stale "blind spot until Phase 124" sentence corrected.

## Semantic proof that only comments moved

```
$ git show HEAD:.github/workflows/release.yml | grep -vE '^[[:space:]]*(#|$)' > before.txt   # 512 lines
$ grep -vE '^[[:space:]]*(#|$)' .github/workflows/release.yml           > after.txt          # 512 lines
$ diff before.txt after.txt ; echo $?
0
```

Both files non-empty at **512 lines**. Secondary checks: `grep -c '^[+-][^+-]*cargo publish'` over the diff → **0**; count of changed diff lines that are neither comment nor blank → **0**.

## The nine emitters, quoted from the edited prose

1. `crates/pmcp-package/Cargo.toml` — the crate's own `[package].version`
2. `cargo-pmcp/Cargo.toml:88`
3. `crates/pmcp-agent/Cargo.toml:18`
4. `crates/pmcp-team-servers/Cargo.toml:24`
5. `crates/pmcp-cfn-renderer/Cargo.toml:10`
6. `cargo-pmcp/src/templates/agent.rs`'s `PMCP_PACKAGE_VERSION_REQ`
7. `cargo-pmcp/tests/pmcp_package_pin.rs`'s `EXPECTED_PIN`
8. `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs`'s `EXPECTED_VERSION_LINE`
9. **`crates/pmcp-openapi-server/Cargo.toml:124`** — "recorded in that inventory as row 6, marked UNCHANGED because it is path-only under CR-01 (item 9b)"

The added rationale, verbatim: *"The ninth is listed precisely BECAUSE its correct action is to do nothing: an emitter whose right move is inaction is the one a reader silently drops from the list, and dropping it is how someone later 'restores a pin' there and breaks the publish order."*

## The caret exception, quoted

> **Caret exception — a PATCH bump requires no downstream pin bumps.** Cargo version requirements are carets by default, and `^X.Y.Z` already admits `X.Y.Z+1`, so nothing that pins the crate needs to change. The blanket form of the rule above over-fires here. Live instance in this release: `pmcp` moves **2.19.0 -> 2.19.1**, and `crates/mcp-tester/Cargo.toml:21` and `cargo-pmcp/Cargo.toml:68` both pin `pmcp = "2.19.0"` — `^2.19.0` admits 2.19.1, so **neither pin moves and neither crate is bumped**. Do not extend this exception to minor or major bumps.

The two pin sites are `crates/mcp-tester/Cargo.toml:21` and `cargo-pmcp/Cargo.toml:68`. This matches plan 03's closed decision (`## Task 3 Decision — DECIDED`, "The caret non-decision (settled, not open)") exactly. The surviving rule was additionally tightened to say *when* it applies — major bumps, or minor bumps on a pre-1.0 line — and to note that the caret exception does **not** rescue the `pmcp-package` one-set case, which is a pre-1.0 minor and therefore semver-incompatible.

## The `mcp-tester` note, quoted (labelled inherited risk)

> **⚠ Inherited release risk — bumping `mcp-tester` is not a free move.** Recorded here as **standing release risk, not as part of any phase's completion** (Phase 124 measured it; PKGR-01 does not close it). **Six** in-repo crates carry an `mcp-tester` `[dev-dependencies]` entry with BOTH a `path` and a `version` key, and **four of the six publish BEFORE `mcp-tester` itself** (`mcp-tester` at `release.yml:401`; `pmcp-server-toolkit` `:263`, `pmcp-sql-server` `:329`, `pmcp-openapi-server` `:344` and `pmcp-workbook-server` `:383` all ahead of it; `cargo-pmcp` `:525` and `pmcp-server` `:543` safely after).

It names four before-publish crates out of six pin sites, labels itself inherited risk rather than PKGR-01 completion, and points at `crates/pmcp-openapi-server/Cargo.toml:112-119` for the general rule. Enforcement lives in plan 03's checkpoint and plan 05's prohibitions, as the review said it should.

## Decisions Made

- **Completed the head publish-order list rather than only fixing its tail.** The plan's Region A asks for the tail; the list also omitted 9 of the 25 publish steps (`pmcp-workbook-runtime`, `-dialect`, `-compiler`, `-server`, `pmcp-cfn-renderer`, `pmcp-agent`, `pmcp-team-servers`, `pmcp-tasks`, and `pmcp-package`'s true position). A list that omits a third of what it mirrors cannot be checked against it, which is precisely how its tail rotted unnoticed. Recorded as a deviation below.
- **By-reference, not refreshed literals.** Every stale pin quote was removed rather than updated. An updated literal is correct exactly until the next bump and has no guard.
- **Rejected the reviewer's arithmetic on the emitter count.** Verified against `122-08-SUMMARY.md:464-476` before editing, as the plan directs.
- **Pre-Flight describes the forbidden oracle by class, not by name.** Item 13 still names and forbids it (whole-file `cargo search` count = 1). See deviation 3 for why.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] The head publish-order list omitted 9 of 25 steps**
- **Found during:** Task 1
- **Issue:** Region A's brief is to fix the tail. Measured against the file's own steps, the list also silently omitted `pmcp-workbook-runtime`, `pmcp-workbook-dialect`, `pmcp-workbook-compiler`, `pmcp-workbook-server`, `pmcp-cfn-renderer`, `pmcp-agent`, `pmcp-team-servers` and `pmcp-tasks`. A "publish order" ledger missing a third of the order is a false ledger, and its incompleteness is the mechanism by which the tail's rot went unnoticed — nobody could diff it against the steps.
- **Fix:** Rewrote the list to one entry per step (25 entries), in step order, with a header stating that the STEPS are the authority and that the list is kept complete on purpose. Comment-only.
- **Files modified:** `.github/workflows/release.yml`
- **Verification:** list entries compared against `grep -n 'cargo publish'` ordinals; `pmcp-package` at list position 27 precedes `cargo-pmcp` at 35; executable-line byte equality holds.
- **Committed in:** `5fac7423`

**2. [Rule 1 - Bug] Task 1's acceptance criteria address by fixed line window a region this plan resizes**
- **Found during:** Task 1
- **Issue:** The criteria capture the head block with `sed -n '80,110p'`. Deviation 1 grows that block from 19 to 39 lines, pushing `cargo-pmcp` past line 110 — the ordinal comparison would have run against a window no longer containing both crates, and the `consumers yet` check against a window no longer covering the whole list. The plan already applied the heading-anchored fix to Task 2's three regions (Codex LOW, "fixed `sed` ranges will drift") but left Task 1 on a fixed window.
- **Fix:** Extracted with `awk '/^[[:space:]]*# Publish order:/{f=1} f && !/^[[:space:]]*#/{exit} f'` — anchored to the block's own heading, terminating at the first non-comment line. 39 lines captured.
- **Files modified:** none (measurement correction)
- **Verification:** `consumers yet` → 0 over the anchored block; `pmcp-package` list ordinal 27 < `cargo-pmcp` 35.
- **Committed in:** n/a
- **Recorded in:** `.planning/WINDOWS.md`

**3. [Rule 1 - Bug] Two Task 2 criteria cannot distinguish prescribing from forbidding**
- **Found during:** Task 2
- **Issue:** (a) `grep -c 'cargo search' /tmp/124-preflight.txt` must return 0, but the natural replacement text names the forbidden command *in order to forbid it* — the criterion cannot tell the two apart, and the first draft returned 1. (b) `grep -c 'Nine were measured' CLAUDE.md` must return exactly 1, but a correction note quoting the phrase it corrects returns 2 — the first draft did exactly that.
- **Fix:** (a) Pre-Flight now forbids "Cargo's own registry search/info subcommands" by class and points at item 13, which names them; the prohibition stays discoverable and the whole-file count remains 1. (b) The correction note paraphrases ("stated the count of nine correctly and then enumerated only eight") instead of quoting.
- **Files modified:** `CLAUDE.md`
- **Verification:** Pre-Flight `cargo search` → 0; whole-file → 1; `Nine were measured` → 1.
- **Committed in:** `9c948e1c`
- **Recorded in:** `.planning/WINDOWS.md`

**4. [Rule 1 - Bug] Two cross-references were line-wrapped, splitting the asserted marker phrase**
- **Found during:** Task 2
- **Issue:** `grep -c 'see the authoritative statement' CLAUDE.md` returned **3**, not the required ≥ 4. Items 14 and 15 carried the cross-reference with "see" ending one line and "the authoritative statement" beginning the next, so a line-oriented grep could not see them. The pointers were present and correct; the *assertion* could not detect them.
- **Fix:** Reflowed both so the phrase sits on one line.
- **Files modified:** `CLAUDE.md`
- **Verification:** count → 5, across items 13 (self-reference in the retained Phase-122 reasoning), 13a, 14, 15, 15a.
- **Committed in:** `9c948e1c`

**5. [Rule 2 - Missing Critical] `RUSTFLAGS="" make quality-gate` reported a green exit over a truncated log**
- **Found during:** Task 2 verification
- **Issue:** `RUSTFLAGS="" make quality-gate > /tmp/124-qg.log 2>&1; echo "EXIT=$?"` printed **EXIT=0** with a **908-line** log ending mid-clippy-echo in the literal text `... (9166 lines truncated)` — written *into the redirect target*. The output proxy truncated the capture and reported its own status. Accepting that reading would have been a green claim over a gate that could not be shown to have run.
- **Fix:** Ran the gate from a script that writes `$?` to a status file, so the status is produced by the same process that ran make.
- **Files modified:** none (verification method)
- **Verification:** status file → `0`; real log **11,551 lines** ending `ALL TOYOTA WAY QUALITY CHECKS PASSED`.
- **Committed in:** n/a
- **Recorded in:** `.planning/WINDOWS.md`

**6. [Rule 2 - Missing Critical] Item 17's stale claim was outside the plan's named edits**
- **Found during:** Task 2
- **Issue:** Item 17 read "workspace-excluded crates (`pmcp-package`) are a known blind spot of the check until Phase 124 (PKGR-01)" — false since plan 01, and flagged forward by `124-01-SUMMARY.md`'s "One residual for a future phase". This plan is the one that touches the ledger.
- **Fix:** Corrected to record that Phase 124 closed it, with the 24 → 25 member move and the scan-scope tripwire.
- **Files modified:** `CLAUDE.md`
- **Verification:** the sentence now reads "**were** a blind spot ... Phase 124 closed it (PKGR-01)".
- **Committed in:** `9c948e1c`

---

**Total deviations:** 6 auto-fixed (3 Rule 1 bugs, 3 Rule 2 missing-critical).
**Impact on plan:** No scope creep — every edit stayed inside the two named files and inside comments/prose. Three of the six (2, 3, 5) are corrections to the plan's *verification method* rather than to its content, and one of those (5) would have produced a false green claim about the project quality gate. Deviations 1 and 6 extend the reconciliation to two further stale claims in the same two regions the plan targets; leaving either would have shipped a ledger this plan certified as true while it was not.

## Issues Encountered

None blocking. One process note, consistent with plan 01: `.planning/config.json` sets `mode: yolo`, `workflow.human_verify_mode` is unset (defaulting to `end-of-phase`, #3309), the plan declares `autonomous: true`, and this executor runs in a detached worktree with no interactive channel — so both tasks ran autonomously with end-to-end re-verification after each commit. Human-facing verification content is carried in the `coverage:` block for end-of-phase harvest.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 05 is unblocked and its premise is now written down.** The caret exception it depends on (`pmcp` 2.19.0 → 2.19.1 moves no downstream pin) is recorded in *Version Bump Rules*, and the one-set rule it must obey for `pmcp-package` is stated once, authoritatively, with an explicit note that the caret exception does not rescue that case.
- **The prose ledger and the workflow now agree,** and the order half of that agreement is machine-checked. A future prose-vs-workflow order bug of the items-2/12/PR-#303 class is a build failure.
- **Carried forward, unchanged:** the `mcp-tester` six-pin/four-before-publish hazard is documented as standing release risk. It is not closed by PKGR-01 and will need resolving the first time `mcp-tester` bumps.
- **Still hand-maintained:** CLAUDE.md's publish-order list. The machine check covers the workflow half only, so the prose can still drift on everything except the `pmcp-package` cluster order.

## Self-Check: PASSED

- Artifacts on disk: `.github/workflows/release.yml` FOUND, `CLAUDE.md` FOUND.
- Commits: `5fac7423` FOUND, `9c948e1c` FOUND.
- Plan `<verification>` re-run at close: `make check-release-coverage` exit 0 (8 fixtures, 25 members) after each task; executable-line byte equality re-asserted at close (512 = 512, diff exit 0); `RUSTFLAGS="" make quality-gate` exit 0 via status-file capture.
- All Task 1 and Task 2 acceptance criteria executed and passing, with two criteria adapted per deviations 2 and 3 and the adaptation recorded.
- No stubs, no skipped tests, no unrun verify commands.
- Three deviation entries recorded in `.planning/WINDOWS.md` (defective/fragile criterion commands and the truncated-log measurement hazard), so a verifier re-running them literally is not misled.

---
*Phase: 124-release-publish-order*
*Completed: 2026-08-27*
