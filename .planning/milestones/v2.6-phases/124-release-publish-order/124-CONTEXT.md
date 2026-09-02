# Phase 124: Release & Publish Order - Context

**Gathered:** 2026-08-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 124 delivers **PKGR-01's residual work plus the milestone's actual release**: the
release-coverage gate learns to see workspace-**excluded** publishable crates (closing the
blind spot its own header names this phase for), the publish ledgers state the
`pmcp-package`-precedes ordering constraint explicitly and machine-check it, the version
pins between `pmcp-package` and its four consumers are audited as one set, and the v2.6
crate tree actually reaches crates.io under one tag.

**Measured state this discussion worked from (2026-08-26, branch
`feat/v2.6-package-portability`, 239 commits ahead of `main`, 1 behind):**

| Crate | Published (crates.io API) | In-tree |
|---|---|---|
| `pmcp` | 2.19.0 | 2.19.0 — **but root `Cargo.toml` moved `jsonwebtoken` 10.3→11.0 in-tree; `src/` untouched since v2.19.0** |
| `cargo-pmcp` | 0.21.0 | 0.23.0 |
| `pmcp-package` | 0.1.1 (0.2 line never published) | 0.3.0 — **with post-bump in-crate commits (Phase 123 tar-framing rule + golden corpus)** |
| `pmcp-agent` | 0.2.0 | 0.3.0 |
| `pmcp-team-servers` | 0.1.1 | 0.2.0 |
| `pmcp-cfn-renderer` | 0.1.0 | 0.2.0 |
| `pmcp-openapi-server` | 0.1.0 | 0.1.1 |
| `pmcp-server-toolkit` | 0.1.1 | 0.1.2 |
| `pmcp-workbook-server` | 0.1.0 | 0.1.1 |
| `pmcp-tasks` | 0.1.0 | 0.1.1 |
| `mcp-tester` / `mcp-preview` | 0.8.0 / 0.3.1 | same — subject to the D-05 sweep |

`./scripts/check-release-coverage.sh` currently exits 0 ("all 24 publishable workspace
members have a publish step") and `pmcp-openapi-server` is present in both CLAUDE.md's
ledger (slot 9b) and `release.yml` (step at :339) — SC1 is a re-verification, not new work.
All four consumer pins on `pmcp-package` are consistent at `^0.3`.

**In scope:** the gate extension + its permanent red self-test; the machine-checked order
assertion; both-ledger prose reconciliation (including the stale `release.yml:98-99`
"NO in-repo consumers" comment); the full version-drift sweep; the pmcp 2.19.1 and
audit-driven `pmcp-package` bumps; the PR → merge → tag v2.19.1 → verified-publication ship.

**Out of scope:** merging `feat/package-172-cli` (Phase 123 deferred it; still owed,
still separate); the MCP-registry OIDC publish fix (open since v2.17; its job must not
gate crates.io publication); any full 25-crate topological order model (D-10 is bounded).

</domain>

<decisions>
## Implementation Decisions

### Gate Extension (coverage of workspace-excluded crates)

- **D-01:** **Discovery is a filesystem scan, not a list.** The gate finds
  workspace-excluded publishable crates by scanning `crates/*/Cargo.toml` for manifests
  carrying their own `[workspace]` table without `publish = false`. No hand-maintained
  array of known-excluded paths — the script's own philosophy ("no allowlist here on
  purpose") extends to the exclusion side, so the next excluded crate is covered the day
  it is created rather than repeating the `pmcp-tasks` silent-skip story.

- **D-02:** **The red direction is a permanent self-test, not a one-off demonstration.**
  A test runs the script against a doctored copy of `release.yml` with `pmcp-package`'s
  publish step removed and asserts a non-zero exit, wired into `make quality-gate`
  (same-commit registration rule per `Makefile` precedent, Phase 122/123). SC2's
  "demonstrated by running it, not asserted in prose" thereby stays proven on every
  future gate run instead of rotting after the phase closes.

### Versions (what ships, at what number)

- **D-03:** **pmcp core patch-bumps to 2.19.1.** The in-tree `jsonwebtoken` 10.3→11.0
  move under an already-published 2.19.0 is exactly the phantom-delta class this phase
  exists to catch; it ships now rather than silently never. **Guard encoded with the
  decision:** execution verifies no `jsonwebtoken` type crosses `pmcp`'s public API
  surface — if one does, the patch axis is wrong and the decision comes back to the user
  before the bump lands.

- **D-04:** **`pmcp-package`'s ship version is audit-driven at execution, not locked
  now.** Diff the crate since its 0.3.0 bump commit (`6430afae`): docs/tests-only →
  ship 0.3.0 as-is; source-visible additive API → 0.4.0 (pre-1.0 minor per the crate's
  own Phase 122 precedent) **with the full nine-emitter one-set sweep** (four consumer
  manifests, `PMCP_PACKAGE_VERSION_REQ`, both pin tripwires — inventory in
  `122-08-SUMMARY.md`). The plan encodes the decision procedure; the number falls out of
  the measurement. Note `85ee222f` also shows in the crate's path log — the audit
  determines whether it actually touched crate source.

- **D-05:** **Full version-drift sweep, machine-measured.** Every publishable crate gets
  the three-way comparison: crates.io API version vs in-tree version vs git diff since
  the last release tag. Catches any other pmcp-style phantom delta (`mcp-tester` at
  0.8.0=0.8.0 is unverified until swept). Gotcha measured this session: the crates.io
  API returns an empty body without a `User-Agent` header — the sweep must send one,
  and must use the API, never `cargo search`/`cargo info` (Phase 122: those report the
  in-tree path override as if published).

### Release Vehicle & Tag

- **D-06:** **The tag is `v2.19.1`** — matches the pmcp core bump (D-03), the
  established convention for releases that include a core SDK move. Every crate with a
  new version rides the same tag; `release.yml` skips already-published versions
  gracefully.

- **D-07:** **The phase owns the full ship, verified.** Phase 124 closes only when the
  tag is pushed, `release.yml` has run, and each expected crate version is confirmed
  live via the crates.io API. Merge and tag-push are **human checkpoints** — the user
  fires them. — **Reversibility:** one-way — publishing to crates.io is permanent
  (versions can be yanked, never unpublished, and the version numbers are consumed
  forever); this is why both outward-facing steps sit behind explicit checkpoints.

- **D-08:** **The PR vehicle is the milestone branch as-is.**
  `feat/v2.6-package-portability` → `main` directly, `.planning/` commits included —
  the v2.17 (PR #302) precedent. No clean-branch cherry-pick port of 239 commits.
  Sync the 1-behind commit from `main` before opening the PR. — **Reversibility:**
  costly — after a squash-merge, new branches must be cut off `upstream/main` or the
  v2.4 squash-divergence trap repeats (recorded in project memory; the mitigation is
  procedural, not structural).

### Ledger Reconciliation (SC4)

- **D-09:** **The constraint is stated in BOTH ledgers.** `release.yml`'s ordering
  comment block gains the explicit "pmcp-package precedes pmcp-cfn-renderer, pmcp-agent,
  pmcp-team-servers, cargo-pmcp" statement and the stale `:98-99` "has NO in-repo
  consumers yet" line is corrected (it has four); CLAUDE.md's items 13/13a/15a prose is
  tightened to name the same constraint in one place rather than scattering it across
  correction notes. The two ledgers have disagreed before and `release.yml` won every
  time — keeping both current is the stated discipline, and the workflow remains the
  authority.

- **D-10:** **Publish-step ORDER is machine-checked, bounded to the pmcp-package
  cluster.** The gate (or a sibling assertion in the same script) verifies
  `pmcp-package`'s publish step precedes its four consumers' steps in `release.yml`.
  Prose order agreement rots — items 2, 12, and PR #303 were all prose-vs-workflow
  order bugs. Scope is deliberately the cluster only, not a full dependency-order
  model of all 25 crates.

### Claude's Discretion

- The matcher form for workspace-excluded publish steps (`cargo publish
  --manifest-path crates/<name>/Cargo.toml` rather than `-p <name>`) and how it slots
  into the existing here-string/no-bash-4-isms discipline.
- Self-test file placement and harness shape (script-level test vs cargo test wrapper),
  provided it registers in `make quality-gate` in the same commit.
- Whether the D-05 sweep script is a committed permanent tool or a phase-local script —
  pick based on how cheap permanence turns out to be.
- Implementation of the D-10 order assertion.
- CHANGELOG/release-notes handling per existing release conventions.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### The gate and its blind spot
- `scripts/check-release-coverage.sh` — the whole script (89 lines): its header names
  this phase for the workspace-excluded blind spot; its failure discipline (explicit
  failures, empty-list-is-failure, comment stripping, here-string SIGPIPE note,
  bash-3.2 constraints) must survive the extension.
- `Makefile` `check-release-coverage` target (`:892-895`) and the quality-gate chain —
  where the extension and the D-02 self-test register.
- `.github/workflows/ci.yml:233` — where the gate runs in CI.

### The publish order (both ledgers)
- `.github/workflows/release.yml` — the AUTHORITY on order. Publish steps:
  `pmcp-package` :440 (via `--manifest-path`), `pmcp-cfn-renderer` :466, `pmcp-agent`
  :484, `pmcp-team-servers` :499, `cargo-pmcp` :520. Stale comment at :98-99
  ("NO in-repo consumers yet") is D-09 work. Ordering comment block at :426-439.
- `CLAUDE.md` (repo root) — "Release & Publish Workflow" section: the prose ledger
  (items 9b, 13, 13a, 14, 15, 15a), the ⚠ ORDERING CONSTRAINT FOR PHASE 124 under
  item 13, the Version Bump Rules, the Release Steps, and the Pre-Flight Checklist.

### Version pins and tripwires (the one-set rule)
- `cargo-pmcp/tests/pmcp_package_pin.rs` — `EXPECTED_PIN` asserts the literal `"0.3"`
  caret pin at `cargo-pmcp/Cargo.toml:88`.
- `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — `EXPECTED_VERSION_LINE` plus
  `pmcp_package_dev_dep_is_path_only` (the CR-01 path-only constraint: this crate
  publishes five steps before `pmcp-package`).
- `cargo-pmcp/src/templates/agent.rs` — `PMCP_PACKAGE_VERSION_REQ`, the
  compiler-invisible ninth emitter (its drift test is its only tripwire; a green build
  does not cover it).
- `.planning/phases/122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b/122-08-SUMMARY.md`
  — the full nine-emitter inventory with each one's guard (D-04's sweep checklist).

### Prior-phase decisions this phase inherits
- `.planning/phases/122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b/122-CONTEXT.md`
  — attestation carriage decisions; Phase 122 published NOTHING and named Phase 124 the
  owner of the release half.
- `.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-CONTEXT.md`
  — the settled verb set (D-01..D-03) and the deferred `feat/package-172-cli` merge
  (explicitly outside this phase too).
- `.planning/ROADMAP.md` — Phase 124 section: goal, reality-check, SC1-SC4.
- `.planning/REQUIREMENTS.md` — PKGR-01 and its measured-drift correction block.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `scripts/check-release-coverage.sh`: the extension edits this script in place — its
  explicit-failure discipline and bash-3.2 constraints are load-bearing, documented
  in-file, and reproduced-by-experiment (the SIGPIPE note). Preserve, don't rewrite.
- The pin-tripwire pattern (`cargo-pmcp/tests/pmcp_package_pin.rs`,
  `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs`): the model for any new
  assertion this phase adds.
- `release.yml`'s skip-if-published step shape: each publish step already tolerates
  already-published versions, which is what makes the single-tag multi-crate release
  safe to re-run.

### Established Patterns
- **Same-commit registration rule**: a new test target must be wired into
  `make quality-gate` in the commit that creates it (Phase 122/123; `Makefile:337-339`
  precedent) — applies to the D-02 self-test.
- **Measured, not asserted**: every claim in this phase's summaries needs a run
  transcript (exit codes, API responses), matching SC2's own wording.
- **One-set version rule**: `pmcp-package` and the three crates pinning it move
  together or not at all (CLAUDE.md item 13; the type-crossing inventory there is why).
- **RUSTFLAGS gotcha**: run `RUSTFLAGS="" make quality-gate` before push (local gate
  blind-spot memory) and `rustup update stable` per the Pre-Flight Checklist.

### Integration Points
- `make quality-gate` → `check-release-coverage` chain (local) and `ci.yml:233` (CI):
  both pick up the extended gate automatically since they call the same script.
- `release.yml`'s `Publish to MCP Registry` job is separate from the crates.io job —
  its known-open OIDC issue (since v2.17) must not gate D-07's publication verification.
- Human checkpoints: PR merge and tag push (`git push upstream v2.19.1`) are the two
  user-fired actions; everything before and after is agent work.

</code_context>

<specifics>
## Specific Ideas

- The crates.io API requires a `User-Agent` header — measured this session: without one
  the versions endpoint returns an empty body and every probe looks like a fetch
  failure. The D-05 sweep must send one.
- SC1 is re-verification on this branch (the gate already exits 0 and
  `pmcp-openapi-server` is in both ledgers) — record the transcript, don't rebuild.
- The phase goal's "the CLI can never ship pinned to a package version it cannot read"
  is operationalized by D-04's one-set procedure plus D-10's order assertion — no
  additional mechanism is expected.

</specifics>

<deferred>
## Deferred Ideas

- **Merging `feat/package-172-cli`** — carried forward from Phase 123 (its D-07);
  still SDK-owed, still explicitly outside this phase. It must precede any
  re-measurement of the CLI verb surface.
- **MCP-registry OIDC publish fix** — open since v2.17.0; its `release.yml` job is
  independent of the crates.io publish job and must not gate this phase's close. Fix
  belongs in its own slice of work.
- **Full 25-crate topological publish-order model** — D-10 deliberately bounds the
  machine-checked order to the `pmcp-package` cluster; a general model is its own tool
  if ever needed.
- **PKGX-F1 / PKGX-F2 live legs** — parked on the pmcp.run backend, unchanged by this
  phase; activate when `getPackageArtifact` / attestation issuance ship.

</deferred>

---

*Phase: 124-release-publish-order*
*Context gathered: 2026-08-26*
