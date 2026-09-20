---
phase: 124-release-publish-order
plan: 06
subsystem: infra
tags: [release, pull-request, ci, merge, human-checkpoint, retroactive-record]

requires:
  - phase: 124-05
    provides: "the authorised version-bump set, the CHANGELOG section release.yml extracts, and the committed 124-expected-release.json this plan's merge carries to main"
provides:
  - "A merged `main` carrying the whole v2.6 milestone plus this phase's release hygiene — PR #348, merge commit 370ac869, 2026-08-27"
  - "The commit that tag v2.19.1 names"
affects: [124-07, release-workflow]

actuals:
  tokens: 0
  tasks: 2
  commits: 0

record_type: retroactive
recorded_at: 2026-09-02
recorded_by: "/gsd-verify-work 125 close-out — reconstructed from git, gh and crates.io evidence, NOT written contemporaneously"
---

# 124-06 SUMMARY — Open the release PR, drive CI green, merge

> ## ⚠ This is a RETROACTIVE record, written 2026-09-02
>
> The work described here happened on 2026-08-26/27. No SUMMARY was written at the time, so
> the ROADMAP row read `124. Release & Publish Order | 0/7 | Planned` for six days while the
> release was in fact live on crates.io. That gap is what made every milestone-scoped GSD verb
> treat v2.6 as unfinished and, on 2026-09-02, made `phase.complete 125` return
> `next_phase: "124"`.
>
> **Everything below is reconstructed from primary evidence** — `git log`, `gh pr list`,
> `gh run list`, and the crates.io API — and each claim names the artifact it rests on. Nothing
> here is recalled or inferred from intent. Where the evidence does not settle something, it
> says so rather than guessing.

## Objective (from the plan)

Open the release pull request and get it merged, with CI proving the extended coverage gate on
the way in. D-08 makes the milestone branch itself the release vehicle —
`feat/v2.6-package-portability` straight to `main`, `.planning/` commits included, no
clean-branch cherry-pick of ~240 commits. D-07 makes the merge a human checkpoint.

## What actually happened

**The milestone reached `main` in TWO pull requests, not one.** The plan anticipated a single
release PR; the branch was merged twice.

| PR | Merged | Head branch | Title |
|---|---|---|---|
| #347 | 2026-08-26 | `feat/v2.6-package-portability` | feat(v2.6): AI-Package portability — config-server packaging, round-trip E2E, attestation carriage (phases 120–122) |
| #348 | 2026-08-27 | `feat/v2.6-package-portability` | release: v2.6 AI-Package Portability (pmcp 2.19.1 + 9 crates) |

Evidence: `gh pr list --repo paiml/rust-mcp-sdk --state merged`. Both carry the same head
branch, so D-08's "milestone branch straight to main, `.planning/` included" was honoured —
there was no cherry-pick onto a synthetic clean branch.

**#348 is the release PR this plan owns.** Its merge commit is `370ac869`, and that is the
commit tag `v2.19.1` names (`git log -1 v2.19.1` → `370ac869 … (#348)`). Its title states the
shipping set as "pmcp 2.19.1 + 9 crates".

**The human merge checkpoint (D-07) was honoured** in the sense that matters: the merges are
recorded against the maintainer's account and no agent performed them. The plan's own
prohibition on an agent-performed merge is consistent with the record.

## What the evidence does NOT settle

- **The CI transcript the plan asked to record was never captured**, and the run is not
  recoverable in the form the plan wanted. The plan's output was "a merged `main` … with a
  recorded CI transcript"; the merge exists, the transcript does not. Recorded as a real gap,
  not waved through.
- **Whether the extended coverage gate (`scripts/check-release-coverage.sh`, added by 124-01)
  actually ran green on the PR** cannot be confirmed from `gh` this long after; the check-run
  detail is no longer listed. The script is present in-tree and chained into
  `make quality-gate`, so it gates every subsequent run, but the specific PR-time verdict is
  unrecovered.
- **The "+ 9 crates" in #348's title does not match the 14 in `124-expected-release.json`.**
  The title is the author's shorthand; the manifest is the contract, and 124-07's verification
  is against the manifest. Noted so a reader does not treat the title as the expected set.

## Deviation from plan

**Two PRs instead of one.** The plan's shape assumed one release PR carrying the milestone.
In practice #347 landed phases 120–122 first and #348 landed the release hygiene on top of the
same branch. This is a deviation in mechanics, not in D-08's substance — the branch was still
the vehicle and `.planning/` still travelled with it.

## Task Commits

None attributable to this plan: its deliverable is a merge, not a commit. The merge commit is
`370ac869` (PR #348).

## Next Phase Readiness

124-07 could proceed and did: `main` at `370ac869` carried the authorised bumps and the
CHANGELOG section, and the tag was pushed against it.

## Self-Check

- [x] Every factual claim names its evidence source (`gh pr list`, `git log -1 v2.19.1`)
- [x] The two-PR deviation is stated rather than smoothed into the plan's expected shape
- [x] The unrecovered CI transcript is recorded as a gap, not asserted as satisfied
- [x] The title-vs-manifest crate-count discrepancy is flagged
- [x] Marked `record_type: retroactive` in frontmatter so no reader mistakes it for a
      contemporaneous account
