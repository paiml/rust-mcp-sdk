---
phase: 124-release-publish-order
plan: 07
subsystem: infra
tags: [release, crates-io, tag, publish, verification, ownership-403, retroactive-record]

requires:
  - phase: 124-05
    provides: "124-expected-release.json — the 14 expected_new / 11 expected_skips manifest this plan verifies against"
  - phase: 124-06
    provides: "merged main at 370ac869, the commit tag v2.19.1 names"
provides:
  - "v2.6 published: all 14 expected_new crate versions confirmed live on crates.io"
  - "A measured instance of the release job's non-graceful 403 failure mode — 11 of 14 published, then the job exited 1 and skipped the rest"
  - "The crates.io OWNERSHIP precondition, absent from every in-repo release check"
affects: [release-workflow, v2.7]

actuals:
  tokens: 0
  tasks: 4
  commits: 0

record_type: retroactive
recorded_at: 2026-09-02
recorded_by: "/gsd-verify-work 125 close-out — reconstructed from git, gh and the crates.io API, NOT written contemporaneously"
---

# 124-07 SUMMARY — Tag, publish, verify per crate

> ## ⚠ This is a RETROACTIVE record, written 2026-09-02
>
> See the same banner in `124-06-SUMMARY.md`. The publish happened 2026-08-27; the record is
> six days late. Every claim below is re-measured from primary sources on 2026-09-02, not
> recalled.

## Objective (from the plan)

Ship it, then prove it shipped. Re-verify registry state at the last reversible moment, hand
the tag push to the user, confirm each expected crate version live on crates.io — per crate,
against the registry API, not against the workflow run's status.

## Outcome: the phase's goal was ACHIEVED, but not on the first attempt

**All 14 `expected_new` versions are live.** Re-verified 2026-09-02 against
`https://crates.io/api/v1/crates/<name>/versions` with a `User-Agent` header (the CLAUDE.md
Pre-Flight oracle; `cargo search`/`cargo info` are forbidden here because they report the
in-tree path override as published state):

| crate | expected | live | | crate | expected | live |
|---|---|---|---|---|---|---|
| pmcp | 2.19.1 | ✓ | | pmcp-cfn-renderer | 0.2.0 | ✓ |
| pmcp-workbook-runtime | 0.2.0 | ✓ | | pmcp-openapi-server | 0.1.1 | ✓ |
| pmcp-code-mode-derive | 0.3.0 | ✓ | | pmcp-package | 0.3.0 | ✓ |
| pmcp-workbook-compiler | 0.1.1 | ✓ | | pmcp-server-toolkit | 0.1.2 | ✓ |
| pmcp-workbook-dialect | 0.1.1 | ✓ | | pmcp-tasks | 0.1.1 | ✓ |
| cargo-pmcp | 0.23.0 | ✓ | | pmcp-team-servers | 0.2.0 | ✓ |
| pmcp-agent | 0.3.0 | ✓ | | pmcp-workbook-server | 0.1.1 | ✓ |

14/14. D-07's closing condition — each expected crate version confirmed live, per crate,
against the registry API — is satisfied.

## The tag, and the run that failed

Tag `v2.19.1` was pushed against `370ac869` (PR #348's merge). The release workflow run it
triggered is **`#33122173433`, conclusion `failure`** (`gh run list --workflow release.yml`).

**The phase's central prediction came true.** The plan's objective names exactly this risk:
"`release.yml`'s already-published skip means a stale version number produces a *silent*
non-publication — the failure this whole phase exists to make visible." The actual failure was
adjacent but worse-behaved, and the plan's per-crate registry verification is what caught it —
the run's own status would only have said "failure" without saying what shipped.

**Root cause: crates.io ownership, not code and not publish order.** CI's
`CARGO_REGISTRY_TOKEN` belongs to `noahgift`; `pmcp-team-servers` and `pmcp-tasks` were owned
by `guyernest` alone, having been first published by hand from that account. The job died on:

```
error: failed to publish pmcp-team-servers v0.2.0 to registry at https://crates.io
Caused by:
  the remote server responded with an error (status 403 Forbidden): this crate
  exists but you don't seem to be an owner.
```

**The failure is not graceful.** `release.yml`'s per-crate guard tolerates only "already
published"; a 403 takes the `::error::` branch and exits 1, so every crate later in the order
is skipped. **11 of 14 published; `pmcp-team-servers` 0.2.0 failed, and `cargo-pmcp` 0.23.0
and `pmcp-tasks` 0.1.1 were skipped as collateral** — neither had a problem of its own.

**The partial state was safe, not corrupt.** The 11 published crates were internally coherent
(`pmcp-agent` 0.3.0 and `pmcp-cfn-renderer` 0.2.0 both pin `pmcp-package "0.3"`, which had
shipped), and the 3 that did not publish simply stayed at their previous, self-consistent
versions. Incomplete, not broken.

**Recovery** was by hand, in dependency order — `cargo-pmcp` pins
`pmcp-team-servers = "0.2"` (`cargo-pmcp/Cargo.toml:83`), so team-servers had to reach the
registry before cargo-pmcp; `pmcp-tasks` has no in-repo dependents and could go any time. The
2026-09-02 re-verification above confirms all three landed.

## The gap no in-repo check covers

`scripts/check-release-coverage.sh` (added by 124-01) verifies a publish STEP exists for every
publishable crate. **It is blind to whether the token can actually use that step.** Ownership
is a precondition nothing in the repo tests, and it is the one that failed.

The pre-tag check that would have caught it, for the next releaser:

```bash
for c in <every crate in the publish order>; do
  echo "$c: $(curl -s -H 'User-Agent: <you>' https://crates.io/api/v1/crates/$c/owners \
    | jq -r '[.users[].login] | join(", ")')"
done
```

Every crate must list the CI token's account. `cargo owner --add <ci-account> <crate>` sends an
INVITATION the recipient must accept, so it is not instant — do it days before a tag, not at
tag time.

## Scope note: v2.19.2 and v2.19.3 are NOT this phase

Both tags postdate v2.6 and belong to later work, recorded here so no reader folds them into
this milestone:

- **v2.19.2** (2026-08-28, PR #350) — `fix(pmcp-package): gate the CONFIG -> SLOT direction`.
- **v2.19.3** (2026-08-30, PR #352) — `feat(R1): config slots declare who fills them
  (supplied_by)`; the tag itself sits on `0c8900c5`, a dependabot CI bump.

Phase 124's tag is `v2.19.1`, matching `124-expected-release.json`'s `"tag": "v2.19.1"`.

## What the plan asked for and did NOT get

- **The closeout PR delivering the planning record to `upstream/main` never happened for this
  plan's own artefacts.** `.planning/` commits rode the milestone branch (D-08), but
  `124-06-SUMMARY.md` and `124-07-SUMMARY.md` were never written, so the phase record stayed
  open. This file and its sibling close that six days late — the closeout is being performed
  now, retroactively, rather than having been performed then.
- **The per-crate verification transcript was not committed at the time.** The verification
  was re-run on 2026-09-02 and its result is the table above; the contemporaneous transcript
  does not exist.

## Deviations from plan

1. The release run FAILED rather than succeeding; the phase goal was reached only after manual
   recovery of three crates. The plan did not contemplate a mid-order authorization failure.
2. The tag-time verification and the closeout were separated by six days, the second half
   performed retroactively.

## Next Phase Readiness

v2.6 is shippable-complete: 14/14 crate versions live, tag `v2.19.1` pushed and merged. With
this record and `124-06-SUMMARY.md` in place, phase 124 reads 7/7 and the milestone can be
closed via `/gsd-complete-milestone v2.6`.

## Self-Check

- [x] Every published version re-measured against the crates.io API on 2026-09-02, not recalled
- [x] The forbidden oracles (`cargo search`/`cargo info`) were NOT used
- [x] The run failure is stated plainly with its exit mechanism, not smoothed into a success
- [x] Collateral skips named individually (`cargo-pmcp`, `pmcp-tasks`) so neither is blamed
- [x] v2.19.2 / v2.19.3 explicitly excluded from this milestone's scope
- [x] Both unmet plan outputs (closeout PR, contemporaneous transcript) recorded as gaps
- [x] Marked `record_type: retroactive` in frontmatter
