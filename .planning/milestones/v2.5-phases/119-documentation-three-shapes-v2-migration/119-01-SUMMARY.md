---
phase: 119-documentation-three-shapes-v2-migration
plan: 01
subsystem: planning-ledger
tags: [spec-recheck, requirements-ledger, v2-schema, conformance-pin, task-zero]
status: complete

requires:
  - "113-SPEC-RECHECK.md § Re-verification obligation (binding) — both arms"
  - "schema/vendored/core-2026-07-28/PROVENANCE.md — the pinned blob SHAs"
  - "tests/v2_conformance_pin.rs — arm 2's in-repo half"
provides:
  - "A settled `[x]` surface for HTTP-01..08 / CLNT-01/02/05 that every downstream Phase-119 chapter is written against"
  - "113-SPEC-RECHECK.md `## Verdict` = PUBLISHED-CONFIRMED, obligation DISCHARGED"
  - "A dated, reproducible run record of both arms + the PR #2678 re-check"
affects:
  - ".planning/REQUIREMENTS.md"
  - ".planning/ROADMAP.md (Phase 113 marker only)"
  - ".planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md"

tech-stack:
  added: []
  patterns:
    - "Evidence-gated ledger mutation: the flip is authorized only by a landing state computed from a live re-run, not from a cited prior finding"
    - "binary(...) nextest selector with an asserted non-zero count (test(...) selects zero and still exits 0)"
    - "Content-addressed provenance: blob SHA identity is what makes a vendored copy a valid stand-in for an upstream grep"

key-files:
  created:
    - ".planning/phases/119-documentation-three-shapes-v2-migration/119-01-SUMMARY.md"
  modified:
    - ".planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md"
    - ".planning/REQUIREMENTS.md"
    - ".planning/ROADMAP.md"

decisions:
  - "Proceed on the one-way ledger discharge was taken by the user at the Task-0 checkpoint, conditional on this plan's own re-run agreeing with the recorded evidence."
  - "Arm 2 was RE-RUN rather than cited (A-03). It landed against a newer conformance main HEAD (74edef34, 2026-08-17) than the 2026-07-27 record, so it is an independent observation rather than a restatement."
  - "The trailing `— *implemented; pending final schema*` annotation was updated on the eleven flipped lines. It is `[~]`-marker metadata, not requirement prose — see Deviation 1."
  - "DOCS-05 was NOT booked complete by this plan, despite being in its frontmatter — this plan writes no documentation, and five later plans in the phase also claim it. See Deviation 4."

metrics:
  duration: "~35 min"
  completed: 2026-08-18
  tasks_completed: 3
  commits: 2
  files_changed: 3
  insertions: 308
  deletions: 58

actuals:
  tokens: 13583
  tasks: 3
  commits: 2
---

# Phase 119 Plan 01: Task Zero — Discharge the Phase-113 Re-verification Obligation Summary

Ran both arms of Phase 113's binding re-verification obligation against live upstream, upgraded `113-SPEC-RECHECK.md`'s `## Verdict` from `PENDING` to `PUBLISHED-CONFIRMED`, and flipped eleven HTTP/CLNT requirements to `[x]` — so every downstream Phase-119 chapter is written against a settled surface instead of hedging on eleven shipped, measured behaviours.

## What Was Done

**Task 0 (checkpoint:decision)** — Presented the one-way ledger discharge with a measured pre-edit baseline and the safety property that the plan stops without mutating anything if the blob gate fails. User selected `proceed`.

**Task 1 — both arms run, nothing cited.** Every check was executed by this plan; `119-RESEARCH.md`'s command block was treated as a script to run, not a result to quote. Literal output captured to `target/119-01-taskzero-capture.txt` (gitignored; the transcription into the ledger is the durable record).

| Check | Result |
|---|---|
| Arm 1 step 1 — trigger | `schema/2026-07-28` **exists** upstream (6 dirs, vs 5 on 2026-07-26) |
| Arm 1 step 1b — pin | `271ecc9a…` still resolves and is still the **newest** commit on the path (2 commits ever, 22 days quiet) |
| **Arm 1 step 1c — the gate** | `schema.ts` `9b55feeb…`/98426 and `schema.json` `213c58f6…`/181474 — upstream `main` **==** vendored **==** `PROVENANCE.md`. **PASS** |
| Arm 1 step 2 | 3 identifiers, exactly 2 hits each (`:434/:468`, `:442/:518`, `:450/:488`) |
| Arm 1 step 3 | All **9** assertions hold — values `-32020/-32021/-32022`, three `400 Bad Request` mappings, `supported: string[]` / `requested: string` / `requiredCapabilities: ClientCapabilities` |
| PR #2678 re-check | `open`, unchanged since `2026-06-23T18:02:47Z`; touches only `docs/` + `schema/draft/`; **zero** hits on the `-3202x` block |
| Arm 2 (a) | conformance `main` HEAD `74edef34` (2026-08-17) — predicate diffed **byte-identical, 34/34 lines**; disjunct set unchanged. **NO DRIFT** |
| Arm 2 (b) | `binary(v2_conformance_pin)` — **5 run, 5 passed, 0 skipped** |

Landing state: **`PUBLISHED-CONFIRMED`** via arm-1 step-4 **row 1**, with both arms run and recorded.

**Task 2** — `## Verdict` body replaced (17 lines out, `PENDING` gone from the body); new sibling sub-section `### Verdict re-verification — Phase 119 task zero (2026-08-18)` appended after the 2026-07-26 record, transcribing all eight required items plus a closing boundary paragraph.

**Task 3** — Eleven flips, the three authorized HTTP-08 text corrections, and a scoped ROADMAP amendment.

## Key Measurements

- **The blob gate is the load-bearing check.** It is what makes reading the vendored copy a valid substitute for a fresh upstream fetch (A-04). It passed, so steps 2/3 are sound; had it failed the plan would have stopped without mutating anything.
- **Arm 2 was a genuinely new observation.** Upstream had moved 2026-07-27 → 2026-08-17 and the predicate had relocated `:988` → `:997`, yet diffed byte-identical. A line move is not drift; a disjunct-set change would have been.
- **`ext-tasks` re-measured rather than assumed:** `schema/` carries `draft/` only, **0 tags, 0 releases** — so DQ6's both-repositories trigger is still `STILL-ABSENT` and TASK-01..06 correctly stay `[~]`.

## Deviations from Plan

Three deviations, all **Rule 1** (auto-fix a defect *introduced by this task's own change*) and all in the same narrow class: text that was true under `[~]` became **false** the moment the flip landed. Leaving any of them would have shipped a ledger that contradicts itself in the same commit that settles it.

**1. [Rule 1 — Bug] Status annotation updated on the eleven flipped lines**

- **Found during:** Task 3
- **Issue:** The plan says to leave "all following prose byte-identical". But all eleven lines ended `— *implemented; pending final schema*`, and `REQUIREMENTS.md:1128` defines the `[~]` marker as *exactly* that phrase ("the requirement's own SPEC-RECHECK gate has not landed `PUBLISHED-CONFIRMED`"). Measured confirmation that it is marker metadata rather than prose: **every** already-`[x]` requirement (VERS-01..05, CLNT-03/04) carries **no** such annotation. Flipping to `[x]` while keeping it would leave eleven completed requirements asserting the gate had not landed.
- **Fix:** Replaced with `— *verified 2026-08-18; 113-SPEC-RECHECK PUBLISHED-CONFIRMED*` on exactly those eleven lines. Substantive requirement wording untouched — including **HTTP-07, whose text is byte-identical** apart from the checkbox and this marker.
- **Files:** `.planning/REQUIREMENTS.md` — **Commit:** `34836231`

**2. [Rule 1 — Bug] The `[~]` hold note (`REQUIREMENTS.md:23-34`) was made false by the flip**

- **Issue:** It read "`113-SPEC-RECHECK.md`'s `## Verdict` is still `PENDING`" and "Re-run the checkpoint … and flip these to `[x]` only then" — sitting directly above eleven now-flipped requirements, and still carrying the superseded *date* trigger that plan 113-28 replaced with a condition.
- **Fix:** Rewrote as a discharge record. History retained rather than deleted (the `[~]` era, HTTP-09's `[ ]` exception, the 2026-07-26 `STILL-ABSENT` run), plus an explicit TASK-01..06 carve-out.
- **Files:** `.planning/REQUIREMENTS.md` — **Commit:** `34836231`

**3. [Rule 1 — Bug] The ROADMAP detail block (`:2985`) was made false by the flip**

- **Issue:** It asserted "`## Verdict` is still `PENDING`" and "**Arm 1 remains open** and the obligation as a whole is still undischarged". Plan Edit C named only the checklist clause at `:2219`, but this block is part of the same Phase 113 marker and directly contradicted the new state.
- **Fix:** Rewrote to record reason (2) discharged and both arms run, while stating twice that **reason (1) is untouched**. Scoped `Edit`, never `Write`.
- **Files:** `.planning/ROADMAP.md` — **Commit:** `34836231`

**4. [Rule 1 — Bug] DOCS-05 was booked `[x]` by the state-update step and REVERTED**

- **Found during:** post-task state updates
- **Issue:** The GSD state-update convention says to pass the plan frontmatter's `requirements:` IDs to `requirements mark-complete`. This plan's frontmatter carries `[DOCS-05]`, so that call flipped `DOCS-05` to `[x]` (`:957`) and its traceability row to `Complete` (`:1129`). **Both are false.** DOCS-05 is *"v2 migration guide + dual-version documentation"* — and this plan wrote **no documentation at all**; it is a ledger-discharge plan. DOCS-05 is claimed by **six** plans in this phase (`119-01/05/08/09/10` plus `119-06`'s sibling coverage), and the migration chapter itself — `ch12-17-migrating-to-mcp-2026-07-28.md`, plan `119-05` — does not yet exist. The convention assumes a 1:1 requirement↔plan mapping that does not hold here.
- **Fix:** `git checkout -- .planning/REQUIREMENTS.md` to restore the committed state. `DOCS-05` reads `[ ]` / `Pending` again; the eleven HTTP/CLNT flips and the six `[~]` TASK rows are unaffected (re-verified: `x_inscope=11`, `task_tilde=6`). **DOCS-05 should be booked by the last plan that actually satisfies it, not by this one.**
- **Why this matters here specifically:** booking a requirement complete on evidence that does not support it is the exact defect class this entire plan exists to prevent — and `REQUIREMENTS.md` already carries two recorded instances of it (`:301`, *"`115-13`'s `[x]` was premature — for the SECOND time on this requirement"*). Letting it through in the commit that settles eleven other requirements would have been self-refuting.
- **Files:** `.planning/REQUIREMENTS.md` (reverted; net zero change) — **Commit:** n/a (no false state was committed)

**Not a deviation, worth recording:** Task 1 produced **no commit**, by design — the plan scopes it to a gitignored scratch capture (`target/`). Its evidence is durable because Task 2 transcribes it into the ledger.

**Assumption A-02 did not apply** — this plan builds neither book, so no `pmcp-course/src/theme/` dirty-check was needed.

## Prohibitions — Verified Honored

| Prohibition | Verification | Result |
|---|---|---|
| TASK-01..06 not flipped | `grep -cE '^- \[~\] \*\*TASK-0[1-6]\*\*'` | **6** (unchanged) |
| No TASK line touched at all | `git diff` for `**TASK-0x**` | **0 hits** |
| HTTP-07 wording unchanged | line diff inspected | byte-identical apart from checkbox + marker |
| No requirement outside the eleven flipped | `grep -cE '^- \[x\] \*\*(HTTP-0[1-8]\|CLNT-0[125])\*\*'` | **11**, in-scope `[~]` = **0** |
| No fourth HTTP-08 correction | corrections applied | exactly 3 |
| `.planning/WINDOWS.md` NOT touched (119-05 owns it) | `git status` | untouched |
| Verdict not upgraded on arm 1 alone | both arms run + recorded | satisfied |
| ROADMAP not whole-file rewritten | `grep -c '^### Phase '` | **101**, unchanged |

## Verification

All seven plan verification steps pass:

1. `cargo nextest run -E 'binary(v2_conformance_pin)' --features full` — **5 run, 5 passed** (non-zero asserted)
2. in-scope `[~]` = **0** ✓
3. in-scope `[x]` = **11** ✓
4. `TASK-0x` `[~]` = **6** ✓
5. `## Verdict` prints `PUBLISHED-CONFIRMED` ✓ (and `PENDING` count in the verdict body = 0)
6. `### Verdict re-verification` sub-sections = **2** ✓
7. `make quality-gate` — **ALL TOYOTA WAY QUALITY CHECKS PASSED**

Post-commit deletion check: **no files deleted** in either commit.

## Known Stubs

None. This plan writes no code and leaves no placeholder, skipped test, or unrun `<verify>`.

## Threat Flags

None. This plan introduces no network endpoint, auth path, file-access pattern, or schema change; all upstream access was read-only `gh api`.

Threat register dispositions all satisfied: **T-119-01** (dated run record appended), **T-119-02** (flips gated on the blob-identity check with arm 2 re-run), **T-119-03** (TASK-01..06 asserted unchanged), **T-119-04** (scoped `Edit`; heading count unchanged), **T-119-05**/**T-119-SC** accepted as planned — zero packages installed.

## Self-Check: PASSED

- `FOUND: .planning/phases/113-.../113-SPEC-RECHECK.md`
- `FOUND: .planning/REQUIREMENTS.md`
- `FOUND: .planning/ROADMAP.md`
- `FOUND: .planning/phases/119-.../119-01-SUMMARY.md`
- `FOUND: commit 64d3f02d`
- `FOUND: commit 34836231`
