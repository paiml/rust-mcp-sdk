---
phase: 124-release-publish-order
verified: 2026-09-02T17:10:00Z
status: passed
score: 4/4 ROADMAP success criteria verified
behavior_unverified: 0
overrides_applied: 0
record_type: retroactive
human_verification: []
---

# Phase 124: Release & Publish Order — Verification Report

**Phase Goal:** What this milestone builds actually ships. The publish ledger names every
crate it must, the machine-checked half of that ledger covers the crate this milestone bumps,
and the version pins between `pmcp-package` and `cargo-pmcp` move together so the CLI can
never ship pinned to a package version it cannot read.

**Verified:** 2026-09-02T17:10:00Z
**Status:** passed
**Re-verification:** No — initial verification

> ## ⚠ Retroactive verification, written 2026-09-02
>
> Phase 124 executed 2026-08-26/27 and shipped, but no verification report was written at the
> time — `verification_status` read `missing` for six days while the release was live on
> crates.io. This report verifies the phase's four ROADMAP Success Criteria against the tree
> and the registry as measured on 2026-09-02, and against the state at tag `v2.19.1` where the
> two differ. It is marked `record_type: retroactive` so no reader mistakes it for a
> contemporaneous account. Companion records: `124-06-SUMMARY.md`, `124-07-SUMMARY.md`.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `./scripts/check-release-coverage.sh` exits 0, and `pmcp-openapi-server` appears in both CLAUDE.md's publish order and `release.yml` | ✓ VERIFIED | Ran it: **exit 0**, `release-coverage: all 25 publishable workspace members have a publish step.` Occurrence counts: `CLAUDE.md` 9, `.github/workflows/release.yml` 9 |
| 2 | The coverage gate is extended to workspace-EXCLUDED publishable crates so `crates/pmcp-package` is covered | ✓ VERIFIED | The gate reports **25** members. `pmcp-package` carries its own `[workspace]` table so `cargo metadata --no-deps` cannot see it (measured at scoping: 28 packages, not among them); the gate's filesystem-scan discovery is what raises the count to 25 and brings it in scope. PKGR-01's stated residual (a) is closed |
| 3 | `pmcp-package` ships a new version, and `cargo-pmcp`'s caret pin plus the `pmcp_package_pin.rs` tripwire move in the same change | ✓ VERIFIED | `pmcp-package` **0.3.0 confirmed live on crates.io** (registry API, `User-Agent` set). Pin/tripwire consistency holds now at the 0.4 line: `cargo-pmcp/Cargo.toml` `pmcp-package = { version = "0.4", … }` and `cargo-pmcp/tests/pmcp_package_pin.rs` `EXPECTED_PIN = "0.4"`, against `crates/pmcp-package/Cargo.toml:10` `version = "0.4.0"` — all three agree. See the drift note below |
| 4 | CLAUDE.md and `release.yml` agree that `pmcp-package` precedes `pmcp-cfn-renderer`, `pmcp-agent` and `cargo-pmcp`, with the constraint stated in the ledger | ✓ VERIFIED | `release.yml` step order measured: `pmcp-package` **:497** → `pmcp-cfn-renderer` **:524** → `pmcp-agent` **:542** → `cargo-pmcp` **:578`. CLAUDE.md states it as an explicit "⚠ AUTHORITATIVE ORDERING CONSTRAINT" block under item 13, not implicitly by step order, and `check-release-coverage.sh` machine-checks it (D-10) |

**Score:** 4/4 ROADMAP success criteria verified.

### Version-drift note on criterion 3

The criterion was written against "the next bump from HEAD". At tag `v2.19.1` that was
`pmcp-package` **0.3.0** with `cargo-pmcp` pinning `"0.3"` and `EXPECTED_PIN = "0.3"` — the
set moved together, which is what the criterion demands. The tree has since advanced to
**0.4.0 / "0.4" / "0.4"** through post-v2.6 work (`pmcp-package` 0.3.1 and 0.4.0 are both live
on crates.io). Both snapshots are internally consistent, so the criterion holds at the tag and
holds today; the invariant it protects — pin, tripwire and package version moving as one set —
has not been violated in between.

## Phase outcome beyond the criteria

**The release shipped.** All 14 `expected_new` versions in `124-expected-release.json` are
confirmed live on crates.io (re-measured 2026-09-02 via the registry API; `cargo search` /
`cargo info` deliberately not used, per CLAUDE.md, because they report the in-tree path
override as published state). Tag `v2.19.1` sits on `370ac869`, the merge commit of PR #348.

**It did not ship cleanly, and that is recorded rather than smoothed.** Release workflow run
`#33122173433` has conclusion **`failure`**: a crates.io ownership 403 on `pmcp-team-servers`
(CI's token is `noahgift`; that crate and `pmcp-tasks` were owned by `guyernest` alone). The
job's per-crate guard tolerates only "already published", so the 403 exited 1 and skipped every
later crate — 11 of 14 published, with `cargo-pmcp` 0.23.0 and `pmcp-tasks` 0.1.1 as
collateral. All three were recovered by hand. The partial state was incomplete, not corrupt:
the 11 published crates were mutually consistent and the 3 stragglers stayed at their previous
self-consistent versions. Full account in `124-07-SUMMARY.md`.

## Gaps carried forward (none blocking)

1. **Ownership is an unchecked publish precondition.** `check-release-coverage.sh` verifies a
   publish STEP exists per crate; it is blind to whether the CI token may use it. This is the
   precondition that actually failed. The pre-tag probe that would catch it is recorded in
   `124-07-SUMMARY.md`. Not a criterion of this phase, so not scored — but it is the single
   most valuable thing the phase learned.
2. **The contemporaneous CI transcript for PR #348 was never captured** and the check-run
   detail is no longer retrievable. Recorded as a real gap in `124-06-SUMMARY.md`.
3. **The plan's closeout PR never ran** for plans 06/07's own artefacts; this report and the
   two SUMMARYs perform that closeout six days late.

## Verdict

Phase 124 achieved its goal: the milestone shipped, the publish ledger is complete and
machine-checked over 25 members including the workspace-excluded one, the ordering constraint
is stated authoritatively and enforced, and the `pmcp-package` / `cargo-pmcp` pin set moves as
a unit. The one-way door was opened deliberately and the result verified per crate against the
registry rather than against a workflow status — which is precisely why the 403's silent
collateral was caught at all.

---

_Verified: 2026-09-02T17:10:00Z (retroactive)_
_Verifier: Claude, during `/gsd-verify-work 125` milestone close-out_
