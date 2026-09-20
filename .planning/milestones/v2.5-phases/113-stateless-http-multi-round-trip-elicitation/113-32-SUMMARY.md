---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 32
subsystem: testing
tags: [conformance, subscriptions, http-08, spec-pin, gate, drift-detection, falsifiability]

requires:
  - phase: 113
    provides: "113-01 Task 1's 113-SPEC-RECHECK.md — its § B.1 conformance pin, its § Recorded Exception and the binding re-verification obligation this plan gives a second arm"
  - phase: 113
    provides: "113-10's `advertises_subscriptions` / `supported_flags` single-source-of-truth in src/types/subscriptions.rs, and tests/v2_subscriptions.rs's CAPABILITY_NAMES vocabulary"
  - phase: 113
    provides: "113-11's document-driven-test idiom in tests/v2_mrtr.rs (phase_dir / section / table_rows / published-crate guard)"
provides:
  - "113-SPEC-RECHECK.md § B.6 — the `advertisesSubscriptions` predicate pinned VERBATIM at conformance sha a865118206d4d8cc8dbc5f5201607839281d0c3b, with provenance, a parseable disjunct table and a drift-consequence statement"
  - "A TWO-ARMED re-verification obligation: Arm 1 (schema) and Arm 2 (conformance predicate), with the explicit rule that running arm 1 alone does NOT discharge the gate"
  - "tests/v2_conformance_pin.rs — the runtime binding between § B.6.3's pinned disjunct list and pmcp's `advertises_subscriptions`, falsified in both directions"
  - "A recorded line-range correction: the addendum's and REQUIREMENTS.md's `stateless.ts:988-1015` citation closes one line short of the consuming closure at 1016"
affects: [113-28, any future run of the re-verification obligation, HTTP-08 closure]

tech-stack:
  added: []
  patterns:
    - "Two-armed gate: when a requirement's grading comes from a source the primary gate cannot see, the gate gains a NAMED second arm plus an explicit 'arm 1 alone is not a run of the gate' sentence — the failure mode being guarded is misplaced confidence, not neglect"
    - "Fetch-or-stop pinning: a pin reconstructed from the implementation it checks agrees BY CONSTRUCTION and is strictly worse than no pin; the fetch is a hard precondition"
    - "Strict document parsing: a planning-table row the test cannot parse FAILS rather than being skipped, because a lenient parser silently restores the blindness the pin exists to remove"

key-files:
  created:
    - tests/v2_conformance_pin.rs
  modified:
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md

key-decisions:
  - "The predicate was FETCHED from upstream via `gh`, never reconstructed — the plan's anti-tautology rule was the point of the plan and the fetch succeeded, so the STOP branch was not taken"
  - "§ B.6 cites § B.1 as the section's single pin and carries the sha only as a cross-checkable COPY; `b6_and_b1_record_the_same_conformance_sha` asserts they agree, because two disagreeing pins in one document are worse than one pin"
  - "The quoted range was WIDENED to 983-1016 rather than trusting the cited 988-1015 — the citation's start is exact but its end truncates the consuming `listenRejected` closure by one line and omits the suite's own five-line rationale comment"
  - "Arm 2 re-fetches at `main`/HEAD, explicitly NOT at § B.1's pinned sha: fetching the pin back would compare the pin against itself and can never detect drift — the same tautology the plan forbids in its construction"
  - "The new test does NOT replace `advertises_subscriptions_over_all_sixteen_capability_combinations`; that unit test proves internal self-consistency against a hardcoded four, and the hardcoded four is precisely what would silently disagree with a changed upstream — the new test supplies the missing external binding"
  - "The combination sweep is driven by the PARSED list's length, not a hardcoded 4, so it grows automatically if the pin and the mapping are ever extended together"

patterns-established:
  - "Falsify in BOTH directions and record both outputs: a binding demonstrated only one way leaves the other way unproven"
  - "The drift detector carries its own remediation instruction — the unmapped-disjunct panic tells the reader it is a phase-reopening event and explicitly forbids resolving it by deleting the row"

requirements-completed: []

duration: 17min
completed: 2026-07-27
---

# Phase 113 Plan 32: Second Gate Arm for HTTP-08 Summary

**The conformance predicate that is HTTP-08's only source of truth is now pinned verbatim from upstream at a recorded sha, bound to pmcp's `advertises_subscriptions` by a test that fails when either side moves, and the phase's re-verification obligation says out loud that running its schema arm alone is not a run of the gate.**

## Performance

- **Duration:** 17 min
- **Started:** 2026-07-27T10:07:49Z (the `gh` fetch)
- **Completed:** 2026-07-27T10:24:00Z
- **Tasks:** 2/2
- **Files modified:** 2 (1 created, 1 amended)

## Accomplishments

- **The fetch was REAL.** `gh` was available and authenticated, so the plan's STOP-rather-than-reconstruct branch was not exercised. The predicate in § B.6.2 is byte-for-byte upstream text, not an inference from `supported_flags`.
- **§ B.6 added** with provenance, the verbatim predicate, a machine-parseable disjunct table and a per-drift-kind consequence statement naming HTTP-08.
- **The obligation is now two-armed** and states, unmissably, that arm 1 alone does not discharge it. Steps 1–4 of the existing schema arm are byte-identical (the whole document change is 191 insertions, 0 deletions).
- **`tests/v2_conformance_pin.rs`** (446 lines, 5 tests) binds the two sides at runtime and was falsified in both directions.
- **A citation error was found and recorded** rather than propagated.

## Task Commits

1. **Task 1: Fetch and pin the predicate, make the gate two-armed** — `9be9e4b2` (docs)
2. **Task 2: Bind `advertises_subscriptions` to the pinned text mechanically** — `5f1089ef` (test)

## The fetch — literal record

| Field | Value |
|-------|-------|
| Fetched at (UTC) | **2026-07-27T10:07:49Z** |
| Outcome | **SUCCESS** (exit 0, 1343 lines) |
| `gh` version | 2.64.0, authenticated as `guyernest` (scopes `gist`, `read:org`, `repo`, `workflow`) |

```
gh api "repos/modelcontextprotocol/conformance/contents/src/scenarios/server/stateless.ts?ref=a865118206d4d8cc8dbc5f5201607839281d0c3b" --jq '.content' | base64 -d
```

Sha independently confirmed against the commit object:

```
$ gh api repos/modelcontextprotocol/conformance/commits/a865118206d4d8cc8dbc5f5201607839281d0c3b \
    --jq '{sha:.sha,date:.commit.committer.date,subject:(.commit.message|split("\n")[0])}'
{"date":"2026-07-23T06:04:40Z","sha":"a865118206d4d8cc8dbc5f5201607839281d0c3b","subject":"fix request metadata HTTP method handling (#409)"}
```

This matches § B.1's recorded commit date and subject exactly, so § B.6 pins the same commit § B.2's 23 check ids were enumerated at.

## Line range actually used — and a citation finding

**The addendum's `988-1015` did NOT match exactly. Range used: `983-1016`.**

| | Line(s) | What is there |
|---|---|---|
| Rationale comment | 983–987 | the suite's own statement of the rule ("it claims a feature it does not serve") — **omitted by the cited range** |
| `const advertisesSubscriptions = !!(` … `);` | **988–993** | the predicate proper — the cited range's start is **exact** |
| `discoverObserved` + `listenRejected` closure | 994–**1016** | the consumer that turns the predicate into SKIPPED-vs-FAILURE; the closure's terminating `};` is at **1016**, so the cited range is **one line short** |

No relocation was found — the predicate is where `.planning/REQUIREMENTS.md:49-53` and Finding 12 say it is. The citation is accurate at its start, truncated by one line at its end, and misses the rationale comment. Recorded in § B.6.1 for exactness, **not** raised as a defect. `.planning/REQUIREMENTS.md` was deliberately left untouched (scope fence); if the maintainer wants the caveat block's `988-1015` widened to `983-1016`, that is a 113-28 edit.

## The pinned disjunct list

Four disjuncts, in the order the predicate evaluates them:

| # | Conformance capability path | pmcp counterpart |
|---|---|---|
| 1 | `tools.listChanged` | `ServerCapabilities::tools.list_changed` (`supported_flags` index 0) |
| 2 | `prompts.listChanged` | `ServerCapabilities::prompts.list_changed` (index 1) |
| 3 | `resources.listChanged` | `ServerCapabilities::resources.list_changed` (index 2) |
| 4 | `resources.subscribe` | `ServerCapabilities::resources.subscribe` (index 3) |

## Does the pin AGREE with pmcp? — YES, EXACTLY

**No mismatch was found, so the scope fence's deferred-item branch was not exercised and no production source was changed.** The predicate's four disjuncts and pmcp's four `supported_flags` arms are the same four capability paths in the same order.

One nuance worth recording for 113-28: the predicate keys its fourth disjunct on the **`resources.subscribe` capability**, whereas HTTP-08's requirement text enumerates the four opt-ins as `toolsListChanged`/`promptsListChanged`/`resourcesListChanged`/**`resourceSubscriptions`** — the latter being a `SubscriptionFilter` *field*, a different surface. pmcp reads `resources.subscribe`, matching the predicate, so the implementation is correct; only the requirement's prose blends two vocabularies. Noted in § B.6.3, not "fixed" here.

## Falsification — both directions, both restored

### Direction A — upstream GAINED a disjunct

A fifth row (`logging.listChanged`) was temporarily added to § B.6.3. **3 of 5 tests failed**; two carried the unmapped-path message verbatim:

```
thread 'every_pinned_disjunct_maps_to_a_pmcp_counterpart' panicked at tests/v2_conformance_pin.rs:256:18:
113-SPEC-RECHECK.md § B.6.3 lists the conformance capability path `logging.listChanged`, which
pmcp's `advertises_subscriptions` has NO counterpart for.

The conformance predicate `advertisesSubscriptions` (conformance/src/scenarios/server/stateless.ts)
has GAINED a disjunct upstream, so HTTP-08's obligation has CHANGED: a server advertising only
`logging.listChanged` must now serve `subscriptions/listen`, and pmcp would answer -32601 and be
graded FAILURE — "claims a feature it does not serve".

Per § B.6.4 this is a PHASE-REOPENING event, NOT an advisory. Do NOT resolve it by deleting the row
or loosening this test: reconcile `supported_flags` in src/types/subscriptions.rs against the new
pin, then re-run arm 2 of the re-verification obligation.
```

`Summary [0.009s] 5 tests run: 2 passed, 3 failed, 0 skipped`

### Direction B — pmcp NARROWED its predicate

`supported_flags`'s fourth arm (`resources.subscribe`) was temporarily replaced with `false`. **2 of 5 tests failed**, and the sweep failed on exactly the singleton subset only that arm satisfies:

```
thread 'advertises_subscriptions_over_the_pinned_combination_space' panicked at tests/v2_conformance_pin.rs:381:9:
a server advertising ["resources.subscribe"] must advertise subscriptions: every one of those paths
is a disjunct of the pinned `advertisesSubscriptions`, so the conformance suite would require the
`subscriptions/listen` stream to be SERVED and would grade a -32601 as FAILURE. pmcp's
`supported_flags` disagrees

thread 'every_pinned_disjunct_maps_to_a_pmcp_counterpart' panicked at tests/v2_conformance_pin.rs:319:9:
§ B.6.3 pins `resources.subscribe` as a disjunct of `advertisesSubscriptions`, and pmcp maps it, but
`advertises_subscriptions` still reports false for a server advertising ONLY that path. pmcp's
`supported_flags` no longer reads it
```

`Summary [0.014s] 5 tests run: 3 passed, 2 failed, 0 skipped`

### Both sides restored

Restored with per-file `git checkout --` (never `git clean`, never a blanket reset). `git status --porcelain` afterwards showed **zero** modifications under `src/` or the phase directory; the only new path from this plan is `tests/v2_conformance_pin.rs`. Verified with the raw `/opt/homebrew/bin/git` binary because the rtk shell proxy corrupts `git diff`/`wc` output (a `wc -l` of an empty diff reported `1`).

## Verification results

| Check | Result |
|---|---|
| `cargo nextest run --features "full" --test v2_conformance_pin` | **5 passed, 0 failed** |
| `cargo nextest run --features "full" --lib -- advertises_subscriptions` | **1 passed** — the pre-existing 16-combination unit test untouched and green |
| `cargo clippy --features "full" --lib --tests -- -D clippy::all` | exit **0** |
| `make lint` (pedantic + nursery + cargo, `RUSTFLAGS=-D warnings`) | exit **0**, zero warnings |
| `cargo fmt --all -- --check` | clean (rustfmt applied to the new file, then re-verified) |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| `make quality-gate` (background job, polled) | exit **0** |
| quality-gate lib tests | **1615 passed; 0 failed** |
| quality-gate integration/doc tests | **399 passed; 0 failed; 78 ignored** |
| `## Verdict` | still reads **`PENDING`** |
| `.planning/REQUIREMENTS.md` | **untouched** (raw-git diff = 0 bytes) |
| `tests/v2_bounded_reads_tripwire.rs` (113-21) | **untouched, not weakened** |
| § B.6.3 disjunct-table parse | strict — an unparseable row FAILS, proven by direction A |

`make lint` was run explicitly per the standing constraint, not just the plan's narrower `-D clippy::all`; totals were read from the raw log files because the rtk proxy swallows `test result:` lines.

## Deviations from Plan

**None.** The plan executed exactly as written. The two branches it provided for contingencies were both resolved in the non-contingent direction and are recorded here as facts rather than deviations:

- **The `gh`-fetch STOP branch was NOT taken** — the fetch succeeded, so nothing was reconstructed.
- **The predicate-mismatch deferred-item branch was NOT taken** — the pin agrees with pmcp exactly, so no deferred item was filed and no production source changed.

One in-scope finding was recorded rather than deferred: the `988-1015` line citation is one line short and omits the rationale comment (§ B.6.1). This is documentation exactness within the plan's own artifact, not a code defect.

## Scope fence — held

- `## Verdict` **not** upgraded; still `PENDING`.
- No requirement checkbox flipped; HTTP-01..09 / CLNT-01/02/05 untouched.
- `.planning/REQUIREMENTS.md` **not** edited — HTTP-08's ⚠ caveat block already states the need this plan fills, and updating that text is 113-28's.
- Steps 1–3 and step 4 of the schema arm **not** modified — the document change is provably 191 insertions / **0 deletions**.
- No pmcp behaviour changed. `supported_flags` is byte-identical to HEAD before this plan.

## Note for 113-28 (wave 5, edits the same file)

113-28 amends step 4 of the re-verification obligation. This plan **deliberately left step 4 alone** and added its own "Landing state (shared)" paragraph *after* Arm 2 instead, so 113-28's structured edit can still target step 4's original text without a conflict. Two candidate follow-ups it may want, both intentionally out of scope here:

1. Widen the `988-1015` citation in `.planning/REQUIREMENTS.md`'s HTTP-08 caveat block to `983-1016`, and mark the "gate needs a second arm" sentence as satisfied by § B.6.
2. Decide whether HTTP-08's requirement prose should say `resources.subscribe` (the capability the predicate actually reads) alongside `resourceSubscriptions` (the `SubscriptionFilter` field it currently names).

## Known Stubs

None. No placeholder values, no empty-collection defaults, no TODO/FIXME markers were introduced.

## Threat Flags

None. This plan added a test and a planning document; it introduced no network endpoint, auth path, file-access pattern or schema change. The threat register's six entries (T-113-155 … T-113-160) are all `mitigate` and all mitigated as planned:

| Threat ID | Mitigated by |
|---|---|
| T-113-155 | the two-armed obligation with its explicit "arm 1 alone is not a run of the gate" statement |
| T-113-156 | § B.6's verbatim pin plus the mutator fallthrough that fails by name (falsification A) |
| T-113-157 | the fetch succeeded and is recorded literally; nothing was reconstructed |
| T-113-158 | the strict parser — proven to FAIL rather than skip by falsification A |
| T-113-159 | `b6_and_b1_record_the_same_conformance_sha` |
| T-113-160 | no production source changed; the pin agreed, so no "fix" was needed or made |

## Self-Check: PASSED

- `tests/v2_conformance_pin.rs` — FOUND
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md` — FOUND, carries all four `#### B.6.x` sub-headings
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-32-SUMMARY.md` — FOUND
- Commit `9be9e4b2` — FOUND
- Commit `5f1089ef` — FOUND
