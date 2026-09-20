---
phase: 114-tasks-extension-migration
plan: 20
subsystem: governance
tags: [contract-first, provable-contracts, pmat, compliance, waiver, d-18, spec-gate]

# Dependency graph
requires:
  - phase: 114-01
    provides: "114-SPEC-RECHECK.md — the D-18 hold record whose Trigger Condition, Third Outcome Policy and STILL-ABSENT branch this plan's obligation row plugs into"
  - phase: 113-stateless-http-multi-round-trip-elicitation
    provides: "113-SPEC-RECHECK.md Deviation 3 — the executor-authored, ungated contract-first deferral this decision deliberately corrects"
provides:
  - "An OWNER-DECIDED ruling on Phase 114's contract-first obligation, recorded before the seventeen implementation plans run rather than discovered at the end of them"
  - "114-CONTRACT-DECISION.md § 4 — Chosen: option-b, Decided by: Guy Ernest (owner), Date: 2026-07-28"
  - "A measured correction of this plan's own premise: contracts/ IS the in-repo authoring destination and IS graded by pmat comply; the waiver rests solely on D-18 provisional values"
  - "A condition-worded, STILL-ABSENT-terminating obligation row in 114-SPEC-RECHECK.md that re-enters the contract question at the final-schema gate"
affects: [114-18, 114-01, contract-first, provable-contracts, future-phase-waivers]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Owner-decided waiver: a repository-level mandatory rule is waived only by a recorded Chosen/Decided by/Date ruling, never by executor inference"
    - "Condition-gated obligation: a waiver terminates in a trigger condition with a defined third outcome, never in 'revisit later'"
    - "Precedent hygiene: a withdrawn rationale is recorded as withdrawn, so a later phase cannot cite it"

key-files:
  created: []
  modified:
    - .planning/phases/114-tasks-extension-migration/114-CONTRACT-DECISION.md
    - .planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md

key-decisions:
  - "Chosen: option-b — an explicit Phase-114 waiver of the CLAUDE.md contract-first step. Decided by Guy Ernest (owner) on 2026-07-28 at the Task 2 gate=blocking checkpoint. NOT executor-inferred."
  - "The waiver rests SOLELY on the D-18 provisional-values argument. The 'nowhere to write it' rationale was measured FALSE in §1.5 and withdrawn BEFORE the ruling; it may not be cited as precedent."
  - "Residual cost accepted, not resolved: contracts/mcp-protocol-sdk-v1.yaml stays stale (116 days, zero task/extension coverage, describes SDK v2.1 while the crate is 2.17), and CB-1409 keeps flagging 114-01's commits."
  - "TASK-01..TASK-06 were NOT marked complete. This plan implements none of them, and the phase's own D-18 hold forbids flipping any of the six except on a PUBLISHED-CONFIRMED landing."

patterns-established:
  - "Waiver-with-a-gate: option-b corrects Phase 113's two measured defects — owner-decided instead of executor-authored, and gated instead of terminating in a record"
  - "Falsified-premise recording: when a plan's own objective is measurably wrong, the correction is recorded against BOTH options and the decision states which rationales it does NOT rest on"

requirements-completed: []  # Deliberately empty — see "Deviations from Plan" §1. This plan implements no requirement; D-18 forbids flipping TASK-01..06 outside a PUBLISHED-CONFIRMED landing.

# Metrics
duration: ~8min (Task 2 session; total plan wall-time spans an owner checkpoint and is not agent time)
completed: 2026-07-28
---

# Phase 114 Plan 20: Contract-First Decision Summary

**The owner ruled `option-b` — Phase 114 ships under an explicit, gated contract-first waiver resting solely on D-18 provisional values, after this plan measured and withdrew its own "there is nowhere to write it" premise.**

## Performance

- **Duration:** ~8 min of agent time in the Task 2 continuation session (Task 1 landed in a prior session; the wall-clock gap between them is the owner's checkpoint deliberation, not agent time)
- **Started:** 2026-07-28T05:29Z (continuation session)
- **Completed:** 2026-07-28T05:33Z
- **Tasks:** 2 of 2 (Task 1 in the prior session, Task 2 here)
- **Files modified:** 2 (both under `.planning/`; zero source files)

## Accomplishments

- **The decision is the owner's, and the record says so unambiguously.** `114-CONTRACT-DECISION.md` § 4 now carries `Chosen: option-b`, `Decided by: Guy Ernest (owner)`, `Date: 2026-07-28`, taken at the `type="checkpoint:decision" gate="blocking"` checkpoint. The executor filled in a ruling; it did not make one. This is threat **T-114-106**'s mitigation, and it is the single substantive way this waiver departs from the Phase 113 precedent it otherwise continues.
- **The plan's own premise was corrected before the ruling, not after it.** `114-20-PLAN.md` asserted that `make comply` gives contract-first step 1 "no destination". Task 1 measured that as **FALSE** — `contracts/` is in-repo, git-tracked (38 files, 3 YAMLs) and graded live by `pmat comply check --path .` (CB-1200/1202/1205/1305); the absent `../provable-contracts/` holds the `pv` CLI and `proof-status.json`, not the authoring destination. The `## Decision` section states this explicitly so that **no future reader can cite this waiver as precedent for "there was nowhere to put it."**
- **The waiver terminates in a condition, not a calendar.** One row was appended to the existing `114-SPEC-RECHECK.md`, matching that file's own `### ⚠` + `| Field | Value |` carried-obligation shape (the same shape the `-32003` vs `-32021` row uses). It is worded as a trigger — *WHEN a versioned (non-`draft`) schema directory exists in BOTH upstream repositories* (the DQ6 both-repos condition) — with **`STILL-ABSENT`** as its explicit third outcome and partial publication landing there. The phrase "revisit later" appears nowhere in it. This is **T-114-107**'s mitigation: the failure mode Phase 113 exhibited in the wild.
- **The cost was recorded as accepted, not quietly resolved.** `contracts/mcp-protocol-sdk-v1.yaml` stays stale at 116 days with zero `task`/`extension` coverage while describing "SDK v2.1" against a 2.17 crate, and CB-1409 continues to flag this phase's own `114-01` commits. Both are stated in the decision and in the obligation row, with a note telling a re-runner to expect them unchanged rather than read them as drift.
- **`114-18` is now bound to cite rather than decide.** Both artifacts state that 114-18 references this ruling rather than declining the contract on its own authority.

## Task Commits

1. **Task 1: Measure the contract dependency and draft both options** — `82043473` (docs) — *prior session; verified present via `git show --stat` before resuming, not redone*
2. **Task 2: Owner decides — author the contract or record the waiver** — `991910f2` (docs)

**Plan metadata:** see the final `docs(114-20)` commit for SUMMARY + STATE + ROADMAP.

## Files Created/Modified

- `.planning/phases/114-tasks-extension-migration/114-CONTRACT-DECISION.md` — `## 4. Decision` filled with the owner's block; added the `### Rationale — and what this waiver does NOT rest on`, `### Residual cost — accepted, not resolved`, and `### What this decision binds` sub-sections; header `**Status:**` and the "does not choose" framing updated to reflect that §4 is now decided. (+76 / −15)
- `.planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md` — one carried-obligation row appended at the end of `## Wire-Value Inventory`, immediately before `## Verdict`. **Pure insertion: +26 / −0 — every other byte of that file is unchanged.** Placed inside the section `## Procedure` step 3 walks, so a gate re-runner encounters it; explicitly marked as *not* a numbered inventory row so the 1–39 wire-value count is not disturbed.

## Decisions Made

- **`option-b`, by the owner.** Recorded verbatim from the owner's block, including the follow-up obligation naming a concrete artifact (`114-SPEC-RECHECK.md` row) and a concrete gate (the DQ6 both-repositories condition) with a named change detector (114-01's SHA-256 provenance tripwire).
- **Obligation row placed inside `## Wire-Value Inventory`, not appended at end-of-file.** End-of-file sits under `### Verdict re-verification`, where every future dated run appends — the row would have been buried under later churn. Inside the inventory it is in the section the re-check procedure explicitly walks. Placement was chosen as a pure insertion so the "rest of the file byte-unchanged" requirement holds (verified: `git diff --numstat` = `26  0`).
- **Row shape copied from the file's own precedent** rather than invented: the `### ⚠ Known upstream disagreement — -32003 vs -32021` sub-section is already a non-numbered `| Field | Value |` carried obligation that `## Procedure` step 3 calls "its own row below". The new row matches it field-for-field in spirit (`THE CONDITION`, third outcome, change detector).
- **`## Procedure` step 3's "two carried forward risks" sentence was left untouched.** Editing it to say "three" would have been an improvement to discoverability but would have broken the byte-unchanged requirement on 114-01's file. The new row instead states its own condition and landing self-containedly. Flagged here so a future editor can make that one-word change deliberately.

## Deviations from Plan

### 1. [Rule 2 — correctness] `requirements-completed` left EMPTY despite the plan's frontmatter listing TASK-01..TASK-06

- **Found during:** Task 2, at the state-update step
- **Issue:** `114-20-PLAN.md` frontmatter carries `requirements: [TASK-01, TASK-02, TASK-03, TASK-04, TASK-05, TASK-06]`, and the executor contract says to run `requirements mark-complete` with a plan's requirement IDs on completion. Doing so here would have been **flatly false in two independent ways**: (a) this plan is a decision record that implements none of those six requirements — it writes no source code at all; and (b) the phase's own D-18 hold record, `114-SPEC-RECHECK.md` § *Recorded Exception*, states that TASK-01…TASK-06 flip **as a group** and **only on a `PUBLISHED-CONFIRMED` landing**, and that *"no checkbox is flipped to `[x]` except on a `PUBLISHED-CONFIRMED` landing."* The Verdict is `PENDING`; both upstream repositories still show only `draft`.
- **Fix:** `requirements mark-complete` was **not** run. `requirements-completed:` is `[]` with an inline note. `.planning/REQUIREMENTS.md` is untouched — TASK-01..06 remain `[ ]` / `Pending`, exactly as 114-01 recorded them.
- **Files modified:** none (the deviation is an omission of a state mutation)
- **Verification:** `grep -n "TASK-0" .planning/REQUIREMENTS.md` shows all six still `- [ ]` and all six traceability rows still `Pending`; `git status --porcelain .planning/REQUIREMENTS.md` is empty.
- **Committed in:** n/a — nothing to commit

### 2. [Rule 3 — blocking/procedural] `make quality-gate` not run for this docs-only commit

- **Found during:** Task 2, at the commit step
- **Issue:** `CLAUDE.md` directs that `make quality-gate` be run before committing. This plan touches **zero** source files and leaves `Cargo.toml`/`Cargo.lock` byte-unchanged, so fmt/clippy/build/test outcomes cannot be affected by it. Separately, the working tree carries **pre-existing unrelated modifications** (`.pmat/*`, `pmcp-course/*`) that are not this plan's and that the sequential-executor contract forbids touching — running the full gate risks surfacing failures outside this plan's scope boundary. There is also no `pre-commit` hook installed in this repository (`.git/hooks/pre-commit` absent), so nothing was bypassed.
- **Fix:** Substituted the plan's own scoped verification, which is the correct gate for a docs-only plan: `git status --porcelain src/ crates/ examples/ tests/ contracts/` → empty, and `git diff --stat -- Cargo.toml Cargo.lock` → empty. This matches the precedent set by Task 1 and by `114-01`, both docs-only commits in this phase.
- **Files modified:** none
- **Verification:** commands above, run before staging; both clean.
- **Committed in:** `991910f2` (the commit body records "zero source files touched")

---

**Total deviations:** 2 (1 × Rule 2 correctness, 1 × Rule 3 procedural)
**Impact on plan:** No scope creep. Deviation 1 **prevented** a false requirement booking that would have contradicted the phase's own hold record and silently flipped six requirements the D-18 gate exists to protect. Deviation 2 substituted a scoped, plan-appropriate verification for a whole-repo gate that this change cannot influence.

## Issues Encountered

**The plan's Task 2 acceptance criteria mandate an action in another plan's file.** Criterion 4 reads: *"a row is added to `114-SPEC-RECHECK.md` (114-01), and `114-18` records the waiver rather than declining the contract on its own authority."* The second half is not executable from here — `114-18` has not run, and editing its plan or pre-writing its summary would be out-of-scope work for another plan. Resolved by discharging the half that is this plan's to discharge (the row) and by **binding** the other half in writing: both `114-CONTRACT-DECISION.md` § *What this decision binds* and the obligation row's `Binds` field state explicitly that 114-18 cites this record and *"may not re-decide a question an owner has already settled, nor settle one an owner has not."* When 114-18 runs, the citation obligation is discoverable from both artifacts.

**No fix-attempt limit was approached; no auth gates; no package installs.**

## Known Stubs

None. This plan produces prose records only — there is no code, no placeholder value, and no unwired data path. The one deliberately unfilled thing is `requirements-completed: []`, which is documented above as correct rather than stubbed.

## Threat Flags

None. This plan created no network endpoint, auth path, file-access pattern or schema change. It touched two Markdown files under `.planning/`.

## Self-Check: PASSED

Claims verified against disk and git:

- `114-CONTRACT-DECISION.md` — FOUND; `grep -nE '^Chosen: option-b'` → line 381 (matches the plan's own `<verify>` regex); `Decided by:` line 382, `Date:` line 383, `Follow-up obligation:` line 384.
- `114-SPEC-RECHECK.md` — FOUND; `git diff --numstat` before commit → `26  0` (26 insertions, **0 deletions**), confirming the rest of 114-01's file is byte-unchanged; `grep -c "contract"` → 9 (the plan's verification requires ≥ 1); `grep -in "revisit later"` → no match.
- Commit `82043473` (Task 1) — FOUND in `git log`, verified by `git show --stat` before resuming; not redone.
- Commit `991910f2` (Task 2) — FOUND; 2 files changed, 102 insertions(+), 15 deletions(-); `git diff --diff-filter=D HEAD~1 HEAD` → no deleted files.
- `git status --porcelain src/ crates/ examples/ tests/ contracts/` → empty. **No contract YAML was authored and `contracts/` was not touched**, as option-b requires.
- Pre-existing unrelated modifications (`.pmat/*`, `pmcp-course/*`) confirmed still unstaged and uncommitted.

## Next Phase Readiness

- **Wave 1's blocking governance question is settled**, which is what the other seventeen implementation plans were waiting on. They may proceed under the recorded waiver.
- **`114-18` has a concrete obligation:** cite `114-CONTRACT-DECISION.md` § 4 (`Chosen: option-b`, Guy Ernest, 2026-07-28). It must not decline the contract on its own authority, and it must not re-open the ruling.
- **The D-18 gate now carries one more item.** A `PUBLISHED-CONFIRMED` landing flips TASK-01…TASK-06 together but does **not** by itself discharge the contract obligation — that needs either the authored equations plus `pmat comply check --path .`, or a further explicit owner waiver.
- **Optional one-word follow-up:** `## Procedure` step 3 still says "the two carried forward risks"; there are now three. Left unedited to preserve the byte-unchanged guarantee on 114-01's file.

---
*Phase: 114-tasks-extension-migration*
*Completed: 2026-07-28*
</content>
