---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 28
subsystem: planning
tags: [gate, publication, spec-recheck, decision-record, maintainer-decision, trigger-condition, upstream-probe]

requires:
  - phase: 113
    provides: "113-01 Task 1's 113-SPEC-RECHECK.md — its ## Verdict, its ## Recorded Exception and the binding re-verification obligation whose step 4 this plan gives a third branch"
  - phase: 113
    provides: "113-SPEC-RECHECK-ADDENDUM-2026-07-26.md — the fourteen findings, and the Open Question this plan answers"
  - phase: 113
    provides: "113-32's two-armed obligation (Arm 1 schema / Arm 2 conformance predicate), and the two prose issues it routed here"
  - phase: 113
    provides: "113-23's measurement answering addendum Finding 5 — HTTP-07's wording is CORRECT and no change was proposed"
provides:
  - "113-PUBLICATION-DECISION-BRIEF.md — the evidence pack: a re-run upstream probe with UTC timestamps, the addendum's findings re-measured or corrected, the round's measured gate numbers, and three options with their consequences and NO recommendation"
  - "113-SPEC-RECHECK.md § Third Outcome Policy — the maintainer's `hold` decision in the Recorded Exception's format, with explicit `none stated` rows for the slots nobody filled"
  - "A THREE-branch step 4: PUBLISHED-CONFIRMED / PUBLISHED-DRIFT / STILL-ABSENT, so a re-verification run cannot end in an undefined state"
  - "The gate's TRIGGER restated as a CONDITION — a versioned schema directory exists — at all three sites that previously said `on or after 2026-07-28`"
  - "A written record that both of 113-32's prose corrections are AUTHORISED but deferred to the re-verification run"
  - "deferred-items.md § D-113-U — a THIRD cog-25 violation, introduced by this round, blocking the org-required `gate` check"
affects: [the re-verification run, HTTP-01..08 closure, CLNT-01/02/05 closure, whoever owns D-113-U]

tech-stack:
  added: []
  patterns:
    - "A binding procedure needs a branch for the outcome its author thought unlikely — otherwise the likely outcome lands nowhere and the status quo persists by default rather than by decision"
    - "A gate waits on an ARTIFACT, never on a DATE: a date-triggered gate is simultaneously due and un-runnable the moment the date passes"
    - "Decision briefs record `none stated` in unfilled slots rather than omitting the row, so a later reader cannot mistake silence for an unrecorded condition"
    - "Probe evidence is re-measured at decision time even when a prior record exists — a decision on stated dated evidence is legitimate; a decision on evidence silently assumed current is not"

key-files:
  created:
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-PUBLICATION-DECISION-BRIEF.md
  modified:
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md
    - .planning/STATE.md

key-decisions:
  - "MAINTAINER DECISION: `hold` — the eleven [~] requirements are held indefinitely; STILL-ABSENT is a legitimate non-failing landing state; the Verdict stays PENDING and the obligation rolls forward"
  - "MAINTAINER DECISION: `prose: correct` — both of 113-32's routed corrections authorised, and deliberately NOT applied; they land at the re-verification run so every requirement-text change happens in one reviewable place"
  - "No conditions, review date or scope narrowing were stated; the policy table records each as `none stated` rather than leaving the row out or inventing a value"
  - "The trigger is a CONDITION (a versioned schema directory exists), not a date — amended at all three sites, with the historical measurements left byte-intact"
  - "The brief carries NO recommendation: task 1 assembled consequences, the maintainer chose, and the executor transcribed"
  - "D-113-U recorded rather than fixed — plan 113-28 changes no source file, and write_canonical is the AEAD AAD canonicalizer whose depth semantics 113-26 spent a plan pinning"

requirements-completed: []

duration: 42min
completed: 2026-07-27
---

# Phase 113 Plan 28: Third-Outcome Policy Summary

**The binding re-verification procedure now has a defined branch for the outcome the evidence says is most likely — the schema directory still not existing — recorded as `hold` against a named maintainer and a date, with the gate's trigger corrected from a calendar date to the artifact it was always waiting for.**

## Performance

- **Duration:** ~42 min across two agent sessions (Task 1, then the checkpoint answer and Task 3)
- **Tasks:** 3/3 (one of them a blocking maintainer decision)
- **Files:** 4 (1 created, 3 modified)
- **Source files changed:** **0**

## Task Commits

1. **Task 1: Re-probe upstream, gate the round, assemble the decision brief** — `67d6cb30` (docs)
2. **Task 2: Maintainer decides the third-outcome policy** — no commit; this task produces a decision, not an edit
3. **Task 3: Record the decision in the Recorded Exception's format** — `a32335da` (docs)

---

## The maintainer's answer — verbatim

> DECISION: **hold**
>
> prose: **correct** — both items authorised.
>
> Record the policy exactly as the `hold` option describes it: the re-verification procedure gains a third branch in which "schema/2026-07-28/ still does not exist" is a legitimate, non-failing landing state. `## Verdict` stays `PENDING`, no requirement checkbox flips, and `[~]` stands. Do not upgrade the verdict and do not edit `.planning/REQUIREMENTS.md` — the two prose corrections are authorised but are to be recorded as *authorised for the re-verification run*, per your own §6 design, so that every requirement-text change in this phase lands in one reviewable place.
>
> No additional conditions, review date, or scope narrowing were stated — do not invent any. If your `## Third Outcome Policy` format has slots for those, record them as "none stated" rather than filling them in.

Two facts the maintainer directed into the policy text, and one open item they directed be recorded as needing an owner, are all carried — see § What was written, below.

**The instruction was followed literally.** The three slots are recorded as `none stated`; nothing was inferred into them. `hold` was transcribed, not interpreted.

---

## The probe — literal record

**Run 2026-07-27T14:17:03Z – 14:25:05Z. `gh` 2.64.0, authenticated as `guyernest`. Every probe exit 0. Nothing recorded `UNAVAILABLE`.**

```
$ gh api ".../contents/schema?ref=main"                        → 2024-11-05 2025-03-26 2025-06-18 2025-11-25 draft
$ gh api ".../contents/schema?ref=2026-07-28-RC"               → (identical five)
$ gh api ".../contents/schema?ref=docs/2026-07-28-release"     → (identical five)
$ gh api ".../contents/schema?ref=claude/docs-release-matrix-2026-07-28"  → (identical five)
$ gh api "search/code?q=repo:…+path:schema/2026-07-28"         → {"total_count":0}
```

**Finding 1 stands, at four refs instead of three** — the fourth branch did not exist when the addendum was written. The RC tag object confirms Finding 2 exactly: `"type":"commit"` (lightweight, no tagger timestamp), target `9d700ed6` dated **2026-05-29T12:49:07Z**, subject *"Merge pull request #2710 from gsdv/fix/number-schema-integer-type"* — an ordinary dependency fix, not a release-preparation commit.

### What changed since the addendum, and what did not

| Fact | Addendum (2026-07-26) | Re-measured (2026-07-27) |
|---|---|---|
| `main` HEAD | `76346843`, 2026-07-23 | **`31eefec6`, 2026-07-27T11:11:09Z** — 32 commits further |
| pin vs `main` schema files | sha256-identical | **still blob- AND sha256-identical** (`c56f0ad2…` / `9281c489…`) |
| RC vs pin | ahead 236 / behind 0 | **unchanged** |
| Drift window | 7 days | **11 days across 32 further commits** |

Finding 7's claim was **re-derived, not inherited**. That mattered: main had moved 32 commits, so quoting the addendum would have been quoting stale evidence for a decision.

---

## Three things the brief found that the addendum did not

### 1. How `schema/2026-07-28/` actually comes into being ⟳ NEW

`.github/workflows/cut-release.yml` exists on `main` and states its own contract. `kind=final` is a **`workflow_dispatch`** job whose promotion step is, verbatim:

```
cp -r docs/specification/draft "docs/specification/$VERSION"
cp -r schema/draft "schema/$VERSION"
sed -i "s|^export const LATEST_PROTOCOL_VERSION = .*|export const LATEST_PROTOCOL_VERSION = \"$VERSION\";|" "schema/$VERSION/schema.ts"
```

then `npm run generate` and a `create-pull-request` step opening `release/<version>` for core-maintainer review.

**This reframes Finding 1's inference without contradicting its observation.** No branch is *supposed* to carry the directory before someone dispatches — so "no in-flight change creates it" is the **expected** state rather than a signal about whether publication will happen. It also means the published `schema.ts` will be a **byte-copy of `schema/draft`** at dispatch time, modulo one stamped line: **a dispatch today would publish exactly the `-32020`/`-32021`/`-32022` this SDK already ships.**

### 2. Which open changes could move the file that gets copied ⟳ NEW

All **82** open PRs were enumerated and each one's file list fetched. **11 modify `schema/draft/schema.ts`.** Each of those 11 patches was then grepped for `3202`:

```
PR #3006 … #2778 … #2678 … #2632 … #2631 … #2614 … #2487 … #2293 … #2145 … #813 … #662
  → lines_mentioning_3202_in_schema.ts_patch = 0   (every one)
```

**Zero open changes touch the three constants under exception.** The nearest is **PR #2678**, which proposes `SERVER_ERROR = -32000` / `NOT_FOUND = -32001` / `RESOURCE_NOT_FOUND = -32002` in the adjacent implementation-defined range — contradicting the draft's current "codes … remain reserved and are never reused" text for `-32002`, which is the very rule plan 113-29 era-gated pmcp's two `-32002` sites against. Named in the policy as the one forward risk to re-check at every future run.

### 3. Finding 9 corrected ⟳ CORRECTED

The addendum says open PR #3006 *"still targets this exact surface."* True, but narrower than it reads. Its full `schema.ts` patch was fetched: it renames the TypeScript interface `SubscriptionsListenResultMeta` → `SubscriptionsListenResultMetaObject` and adds a `SubscriptionsListenResultResponse` wrapper.

```
$ … pulls/3006/files … | grep -n "subscriptionId\|NotificationMetaObject"
(no hunk line mentions either)
```

It touches **neither** the `io.modelcontextprotocol/subscriptionId` wire key **nor** `NotificationMetaObject` — i.e. neither of the bytes HTTP-07's wording rests on. It is `mergeable_state: "dirty"` and was updated 2026-07-27T04:45:11Z, so it is active and can still change. Recorded so the risk is neither overstated nor dismissed.

---

## The round's gate numbers

All at `4ac6ebeb`. Totals read from raw logs with **absolute binary paths** — the rtk shell proxy swallows `test result:` lines and corrupted a first attempt at summing them (`passed=1` for a log whose real total is 4487).

| Check | Result |
|---|---|
| `make quality-gate` (background job, polled) | **exit 0** |
| — test-result lines | **252** |
| — passed / failed / ignored | **4487 / 0 / 80** |
| — non-`ok` result lines | **0** |
| — `FAILED` occurrences in log | **0** |
| `make lint` (standalone, as mandated) | **exit 0**, 0 warnings, 0 errors |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| Round diff (`src/ tests/ examples/ fuzz/`, 40 commits) | 19 files, **+8177 / −198** |

`make lint` was run standalone in addition to its run inside the gate, per the standing constraint — pedantic + nursery + cargo with `RUSTFLAGS="-D warnings"`, strictly stronger than a bare `-D clippy::all`.

### ⚠ PMAT is the exception — 3 violations, up from 2

```
$ pmat quality-gate --fail-on-violation --checks complexity
Quality Gate: FAILED
Total violations: 3
  - ./src/server/streamable_http_server.rs:3084 - handle_post_fast_path: cognitive-complexity 30 > 25
  - ./src/server/streamable_http_server.rs:3520 - handle_post_with_middleware: cognitive-complexity 31 > 25
  - ./src/types/mrtr.rs:1299 - write_canonical: cognitive-complexity 26 > 25
```

The first two are D-113-F, pre-existing and better than their pre-Phase-113 baseline. **The third is new**, and it was proved new by direct measurement using D-113-F's own methodology — extract the file at a baseline commit into a scratch tree, run the identical analysis:

| Tree | `src/types/mrtr.rs` | violations |
|---|---|---|
| `1ba8138d` (last touch before 113-26) | 1846 lines | **0** |
| `4ac6ebeb` (HEAD) | 2720 lines | **1** — `write_canonical` = 26 |

Cause: plan 113-26 commit `323b2e1a`, the D-113-M fix that made the AAD canonicalizer fallible. The fix is correct and load-bearing; only its complexity cost went unmeasured, because 113-26 verified with `make quality-gate` — which per CLAUDE.md D-07 deliberately does **not** run PMAT — and not the CI gate.

Recorded as **D-113-U**, unowned, with a fix shape (P2 extract-method on the object/array arms) and two hard constraints: **the canonical byte output must not change** (it is AEAD AAD — a one-byte difference invalidates every continuation token in flight), and **the 64/65 depth boundary must stay exact**. Per CLAUDE.md this gate blocks merge through the org-required `gate` check, so it needs an owner before this branch merges.

**Not fixed here** — plan 113-28 changes no source file, and refactoring the canonicalizer inside the plan that produces the publication decision would make neither reviewable.

---

## What was written

### `113-SPEC-RECHECK.md` — three amendments, 186 insertions / 6 deletions

**1. Step 4 now has THREE branches** (previously two, both presupposing publication):

| Step-1 result | Landing state | Action |
|---|---|---|
| exists, steps 2-3 agree | `PUBLISHED-CONFIRMED` | upgrade the Verdict — and only once **arm 2 has also been run** |
| exists, steps 2-3 disagree | `PUBLISHED-DRIFT` | upgrade the Verdict; phase-reopening event; nothing flips |
| **still does not exist** | **`STILL-ABSENT`** | apply `## Third Outcome Policy`; Verdict stays `PENDING`; obligation rolls forward |

`STILL-ABSENT` is stated to be legitimate and non-failing, and to weaken nothing — the mismatch clause is untouched.

**2. New `## Third Outcome Policy`**, in the `## Recorded Exception`'s format: Decided by / Decided via / Decision / UTC date / Question being answered / Verdict at time of decision / Evidence — **plus three explicit `none stated` rows** for conditions, review date and scope narrowing, with a sentence saying they are recorded as such deliberately and may not be read into later. It carries a five-point rule, a `What this policy does NOT permit` section (no promotion of the draft pin, no second exception, no time-based flip, no weakening of the drift consequence), the two measured facts the maintainer directed in, the authorised-but-deferred prose table, and the D-113-U note.

**3. The TRIGGER is restated as a CONDITION** at all three sites that previously said "on or after 2026-07-28" — the obligation's new `TRIGGER — a CONDITION, not a date` block, the 2026-07-26 run record, and the file's closing line. Each says the gate becomes runnable when *a versioned schema directory exists*, so it can be neither treated as due nor as discharged merely because a day passed; and each notes that arm 2 is not gated on this condition at all. **The historical measurements are left byte-intact** — only the forward-looking instruction embedded in them was superseded, and each site says so.

### `.planning/STATE.md`

- The first blocker is restated from an **open question** to the **recorded policy**, with a pointer to `§ Third Outcome Policy` and the evidence timestamp.
- **D-113-U added** as a second blocker entry, explicitly flagged as needing an owner before this branch merges.
- `Current Position` updated: 32 of 32 plans complete, and the next action named as the re-verification run — **when a versioned schema directory exists** — executing **both** arms under the three-branch procedure. The 113-29 entry was demoted to prior-wave context with a superseded marker.

### `deferred-items.md`

`## D-113-U` added with the measurement, the proof it is new, why it was not fixed here, the fix shape, and the two hard constraints.

---

## Authorised, and deliberately NOT applied

`prose: correct` authorises both corrections 113-32 routed here. **Neither was applied.** Both are recorded in the policy as *authorised for the re-verification run*:

| # | Change |
|---|---|
| 1 | HTTP-08's citation `stateless.ts:988-1015` → **`983-1016`** — the start is exact, the end is one line short of the consuming `listenRejected` closure at 1016, and 983–987 (the suite's own rationale comment) is omitted |
| 1b | HTTP-08's caveat sentence *"The gate needs a second arm pinning a conformance-repo sha"* → mark **SATISFIED**, since 113-32 added exactly that arm |
| 2 | Where HTTP-08 describes what **gates the stream**, name the **`resources.subscribe` capability** (what the predicate and pmcp both read), keeping `resourceSubscriptions` for the `SubscriptionFilter` field a client sends |

**HTTP-07's wording is explicitly excluded from that table.** Plan 113-23 measured pmcp's actual emission on a live socket and recorded that the current wording is correct and that no change is proposed. The plan anticipated an HTTP-07 question; it does not arise.

---

## Deviations from Plan

**One, and it is the behaviour the executor contract prescribes for out-of-scope discoveries.**

**1. [Rule 2 / SCOPE BOUNDARY — out-of-scope discovery logged, not fixed] D-113-U added to `deferred-items.md`**

- **Found during:** Task 1, step 2 (the mandated PMAT complexity delta)
- **Issue:** the delta against D-113-F's two known violations is **not zero** — `write_canonical` is at cognitive 26 and the PR-blocking gate reports 3 violations, up from 2. Measured at **0** in that file at the pre-113-26 baseline, so it was introduced by this round.
- **Disposition:** recorded, not fixed. `files_modified` for this plan is three planning documents; the function is the AEAD AAD canonicalizer with a boundary-exact depth contract; and per the executor's scope boundary an issue not caused by this task's changes is logged rather than repaired.
- **Files modified:** `deferred-items.md` (+ a cross-reference in the brief § 4.1, the policy, and STATE.md)
- **Commits:** `67d6cb30` (deferred item + brief), `a32335da` (policy + STATE.md)

The plan's file list did not name `deferred-items.md`; adding to it is the executor contract's prescribed destination for exactly this case.

**Two contingency branches the plan provided were resolved in the non-contingent direction** and are recorded as facts rather than deviations:

- **The probe `UNAVAILABLE` branch was NOT taken** — `gh` was available and authenticated, every probe exited 0, so the decision rests on evidence measured that hour rather than on the addendum's.
- **The HTTP-07 wording question did NOT arise** — 113-23 proposed no change. Two *different* prose corrections, routed by 113-32, took its place in the brief's § 6.

**A scope note the plan's own text needed correcting on:** the plan's `<interfaces>` describes the addendum as having "six findings and the Open Question". It has **fourteen**, plus three post-hoc resolution sections added by plans 113-23 and 113-30. All fourteen were read; the brief carries the ones that bear on the decision.

---

## Scope fence — held

| Fence | Evidence |
|---|---|
| `.planning/REQUIREMENTS.md` untouched | `git diff HEAD -- .planning/REQUIREMENTS.md` = **0 bytes**, checked before each commit |
| No requirement checkbox flipped | the eleven `[~]` and HTTP-09's `[ ]` are byte-identical |
| No requirement TEXT edited | including the two corrections the maintainer authorised |
| `## Verdict` not upgraded | still reads `PENDING` at line 232 |
| Mismatch clause not weakened | `PUBLISHED-DRIFT` is still a phase-reopening event; the third branch is additive |
| Both arms intact | 113-32's Arm 1 / Arm 2 split and its "arm 1 alone is not a run of this gate" statement are unmodified |
| No source file changed | the plan's whole diff is four planning documents |
| No `git stash` used | per the standing constraint; per-file staging only, no `git clean`, no blanket reset |

---

## Known Stubs

None. No placeholder values, no TODO/FIXME markers, no empty-collection defaults were introduced. The three `none stated` rows in the policy table are deliberate recorded content, not placeholders — they record that the decider stated nothing in those slots.

## Threat Flags

None. This plan added and amended planning documents only; it introduced no network endpoint, auth path, file-access pattern or schema change. The plan's five threat-register entries are all `mitigate` and all mitigated as planned:

| Threat ID | Mitigated by |
|---|---|
| T-113-134 | step 4's third branch, bound to a policy with a named decider and a date |
| T-113-135 | the Verdict stays `PENDING`; `hold` grants no permission to flip, and the policy says so explicitly |
| T-113-136 | no requirement checkbox or text changed; the authorised prose corrections are recorded as authorised-for-the-next-run, not applied |
| T-113-137 | the brief carries no recommendation — the three options are presented with consequences and the order is stated to carry no meaning |
| T-113-138 | the probe was re-run with UTC timestamps and literal output; Finding 7 was re-derived against a 32-commit-newer HEAD rather than quoted |

## Self-Check: PASSED

- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-PUBLICATION-DECISION-BRIEF.md` — FOUND (7 numbered sections + appendix)
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md` — FOUND, carries `## Third Outcome Policy`, `STILL-ABSENT` (8 occurrences) and `TRIGGER — a CONDITION, not a date`
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md` — FOUND, carries `## D-113-U`
- `.planning/STATE.md` — FOUND, first blocker restated, D-113-U added, Current Position updated
- `.planning/REQUIREMENTS.md` — **0-byte diff**, as required
- Commit `67d6cb30` — FOUND
- Commit `a32335da` — FOUND
