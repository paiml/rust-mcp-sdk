# Phase 124: Release & Publish Order - Research

**Researched:** 2026-08-26
**Domain:** Release engineering — Cargo workspace publish ordering, shell-based CI gates, crates.io publication
**Confidence:** HIGH (every load-bearing claim measured in-session against this working tree)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Gate Extension (coverage of workspace-excluded crates)**

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

**Versions (what ships, at what number)**

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

**Release Vehicle & Tag**

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

**Ledger Reconciliation (SC4)**

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

### Deferred Ideas (OUT OF SCOPE)

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
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PKGR-01 | *(original text)* "`pmcp-openapi-server` is added to CLAUDE.md's publish order. It is absent today (zero occurrences) and would silently not publish." **Superseded by the measured-drift correction block in `.planning/REQUIREMENTS.md:42-49`**: the original premise is closed; residual work is (a) the coverage gate's structural blindness to workspace-excluded publishable crates, and (b) stale version targets. | §Summary (SC1 re-verified, transcript recorded); §Architecture Patterns Pattern 1 (D-01 discovery, proven to yield exactly `pmcp-package`); §Pattern 2 (D-02 self-test, with a measured negative baseline proving today's gate exits 0 on a doctored workflow); §Pattern 3 (D-10 order assertion, step line numbers measured); §Common Pitfalls 1–8; §Runtime State Inventory (the out-of-repo release state) |
</phase_requirements>

## Summary

This phase has almost no *technology* risk and a large amount of *measurement* risk. Nothing
new is installed, no library is chosen, and no API is designed. The entire deliverable is
(a) ~40 lines of POSIX-ish shell added to an existing 89-line gate, (b) a fixture-driven
Makefile self-test in a shape this repo has already built twice, (c) two prose ledgers
reconciled against a workflow that is the authority, and (d) a one-way, human-fired
publication to crates.io. The way this phase fails is not "we picked the wrong tool" — it
is "we shipped a version number that was already taken, or we didn't ship one that should
have been."

Three findings from this session change the shape of the plan materially, and all three
are measured rather than inferred. **First, SC2's negative baseline is now proven:** with
`pmcp-package`'s publish step deleted from a copy of `release.yml`, today's gate prints
`release-coverage: all 24 publishable workspace members have a publish step.` and exits
**0**. The blind spot is not theoretical and the plan can assert it with a transcript.
**Second, D-04's audit resolves cleanly to "ship 0.3.0 as-is":** every line
`pmcp-package` gained since its 0.3.0 bump commit `6430afae` is a doc comment or a test
fixture — filtering the diff for added lines that are neither `//!` nor `///` returns
empty for both touched source files. That deletes the nine-emitter sweep from the plan's
critical path (all nine are already consistent at `0.3`, verified). **Third, and most
consequentially, D-05's sweep yields six additional phantom deltas beyond `pmcp`**, one of
them large: `pmcp-workbook-runtime` sits at a published `0.1.0` while carrying a new
public module `reconcile` (590 lines) and five new crate-root re-exports. The CONTEXT
anticipated the sweep finding "any other pmcp-style phantom delta"; it found six, and two
of them need a semver judgement the CONTEXT has not pre-decided.

A fourth finding concerns the release vehicle. `main` moved while this branch was being
written: `c64e2b2b` is a **squash** commit (single parent, not a merge) that landed phases
120–122 from this very branch as PR #347. The branch is now 241 ahead / 1 behind, and the
residual source delta between `main` and HEAD is only Phase 123's work. D-08's "sync the
1-behind commit from `main` before opening the PR" is therefore far more load-bearing than
its one-clause phrasing suggests: it is not a fast-forward, it is a **9-file conflict
resolution** including three production files (`Makefile`,
`cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs`, `graphql_contract.rs`) and two
add/add conflicts — measured with `git merge-tree`. Skipping the sync does not just make a
big PR diff; it makes GitHub compute the diff from merge-base `67758388`, re-presenting
all of 120–123 as if unmerged.

**Primary recommendation:** Sequence the phase as *measure → gate → reconcile → decide
versions → ship*, and put the D-05 sweep **before** the version decisions rather than
alongside them — the sweep's output is the input to how many crates get bumped, and this
session's run shows that number is 10, not the 2 the CONTEXT names. Implement D-01 as a
filesystem scan for *discovery* plus the root gate's own `cargo metadata` `.publish == null`
predicate for *classification*, so one predicate governs both halves of the ledger.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Discovering workspace-excluded publishable crates | Local shell gate (`scripts/check-release-coverage.sh`) | — | The gate already owns "who must publish"; adding a second discovery source keeps one script authoritative rather than splitting the question across two tools |
| Classifying a discovered crate as publishable | Cargo (`cargo metadata --no-deps --manifest-path …`) | — | The root gate's predicate is `.publish == null`; reusing it for the excluded half prevents a filesystem heuristic from disagreeing with Cargo about what "publishable" means |
| Proving the gate fails when it should (D-02) | Make target (`*-guard-selftest`) | — | Two existing precedents in this repo; the self-test must run *before* the gate it proves, as a declared prerequisite |
| Publish ORDER enforcement (D-10) | Same shell gate | — | Order is a property of `release.yml` text, which the gate already reads and comment-strips; a second reader would need to duplicate the comment-stripping discipline |
| Publish ORDER declaration | `.github/workflows/release.yml` (AUTHORITY) | `CLAUDE.md` prose ledger (mirror) | CLAUDE.md states this explicitly and history confirms it: "Where the two have ever disagreed, the workflow was right and the prose was wrong" |
| Version-drift measurement (D-05) | Script over `git` + crates.io API | — | The in-tree path override makes Cargo the wrong oracle; only the registry API knows what is published |
| Actual publication | GitHub Actions (`release.yml`, tag-triggered) | Human (merge + tag push) | One-way and permanent; D-07 puts both outward-facing steps behind human checkpoints |
| Release-notes assembly | `CHANGELOG.md` (awk-extracted by the workflow) | — | `create-release` extracts the `## [X.Y.Z]` section; an absent section yields empty release notes, silently |

## Standard Stack

This phase adds **no dependencies of any kind**. The stack is what the gate already uses,
and the binding constraint is that it must keep running under stock macOS `bash`.

### Core

| Tool | Version (measured) | Purpose | Why Standard |
|------|--------------------|---------|--------------|
| `bash` | 3.2.57(1)-release (arm64-apple-darwin25) | The gate's interpreter locally | `[VERIFIED: /bin/bash --version, this session]` The script header names this explicitly: "No bash-4-isms (`mapfile`, empty-array `"${a[@]}"` under `set -u`): this is chained into the local `make quality-gate`, and stock macOS bash is 3.2." |
| `cargo metadata --no-deps --format-version 1` | cargo 1.9x (repo pins `dtolnay/rust-toolchain@stable` in CI) | Enumerate + classify publishable crates | Already the gate's data source; `--manifest-path` is the only mechanism that reaches a workspace-excluded crate (`Makefile:1345-1349` says exactly this) |
| `jq` | present (gate already depends on it) | `.publish == null` predicate | Existing dependency; failure is already an explicit gate failure |
| `curl` + crates.io API v1 | — | Published-version oracle for D-05/D-07 | `[VERIFIED: this session]` `https://crates.io/api/v1/crates/<name>/versions` returned correct data for 21 crates with a `User-Agent` header |
| `awk` | POSIX | Fixture-driven extractors for guard self-tests | Two precedents: `scripts/named-test-binary-count.awk`, `scripts/deny-allow-entry-count.awk` |
| `make` | — | Gate chaining + self-test registration | `Makefile:892-895` `check-release-coverage`; `quality-gate` calls it second, right after `lint-plans` |
| `git merge-tree --write-tree --messages` | git 2.47.1 | Non-destructive merge conflict preview | `[VERIFIED: /opt/homebrew/bin/git --version]` Requires git ≥ 2.38; available |

### Supporting

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `cargo public-api` (pinned 0.52.0, Phase 112) | Prove no `jsonwebtoken` type crosses `pmcp`'s public API | D-03's guard — the rigorous form of the grep below |
| `cargo semver-checks` (pinned 0.49.0, Phase 112) | Confirm the chosen semver axis for a bumped crate | `pmcp` 2.19.1 (patch) and any crate the D-05 sweep promotes |
| `gh` CLI | PR creation, release inspection | D-08 vehicle; D-07 verification of the GitHub Release |
| `python3` (stdlib `json`) | Parse crates.io API responses in the sweep | Avoids assuming `jq` field paths for the registry schema; `jq` works equally well |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Filesystem scan + `cargo metadata` per excluded crate (recommended) | Filesystem scan + inline TOML grep for `publish = false` | Simpler and dependency-free, but introduces a **second** definition of "publishable" that can drift from Cargo's. Measured edge case: `crates/pmcp-package/Cargo.toml:20` says `publish = true`, and `cargo metadata` reports `publish=null` for it — the two agree *today*, but `publish = ["registry"]` (a private-registry restriction) would be classified differently by the two approaches |
| `cargo metadata` per excluded crate | `cargo metadata` with `--manifest-path` on the root plus manual exclude-list parsing | The root `[workspace].exclude` array lists 11 paths, most of them non-publishable examples; parsing it re-creates the allowlist D-01 forbids |
| Extending `check-release-coverage.sh` (recommended) | A new sibling script | The comment-stripping, here-string SIGPIPE avoidance, empty-list-is-failure and bash-3.2 disciplines are all documented **in-file** and load-bearing; a sibling would have to reproduce all four or silently weaken one |
| Makefile `*-guard-selftest` target (recommended) | A `#[test]` in a Rust crate shelling out to the script | Rust would need a home crate; the workspace-excluded `pmcp-package` is the wrong home and `cargo-pmcp` is unrelated. Both existing precedents are Makefile targets |
| Permanent committed sweep script | Phase-local throwaway | Recommend **permanent** — see Open Question 2; the measured cost is ~40 lines and it is the only tool that can detect a phantom delta, which this session proves recurs |

**Installation:**

```bash
# Nothing to install. Verify the toolchain matches CI before any release work:
rustup update stable && rustc --version
```

## Package Legitimacy Audit

**Not applicable — this phase installs zero external packages.**

The work is shell, Make, YAML and Markdown edits plus version-number changes to manifests
that already exist. No `cargo add`, no `npm install`, no new `[dependencies]` entry is
contemplated by any decision in CONTEXT.md.

One adjacent item is worth naming so it is not mistaken for a package addition: `pmcp`'s
root `Cargo.toml` moved `jsonwebtoken` from `"10.3"` to `"11.0"`. That change is **already
in the tree** (merged to `main` as PR #332, `5b7eef49`) and is not being made by this
phase; this phase only decides what version number ships it.

| Package | Registry | Change made by this phase | Verdict | Disposition |
|---------|----------|---------------------------|---------|-------------|
| `jsonwebtoken` | crates.io | None — requirement bump already in tree via #332 | n/a | Pre-existing; D-03 ships it under 2.19.1 |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
                       ┌─────────────────────────────────────────┐
                       │  DECLARATION (two ledgers, one authority)│
                       └─────────────────────────────────────────┘
                                        │
            ┌───────────────────────────┴──────────────────────────┐
            │                                                      │
   .github/workflows/release.yml                            CLAUDE.md prose
   (AUTHORITY — executable)                                 (mirror — human-read)
            │                                                      │
            │  publish steps, in order                             │  items 1..17
            │  :440 pmcp-package (--manifest-path)                 │  item 13 = pmcp-package
            │  :466 pmcp-cfn-renderer  (-p)                        │  item 13a, 14, 15, 15a
            │  :484 pmcp-agent         (-p)                        │
            │  :499 pmcp-team-servers  (-p)                        │  D-09 reconciles
            │  :520 cargo-pmcp         (-p)                        │  ← ─ ─ ─ ─ ─ ─ ─┘
            │
            ▼
 ┌────────────────────────────────────────────────────────────────────────────┐
 │  GATE: scripts/check-release-coverage.sh                                    │
 │                                                                             │
 │   ┌── discovery ────────────────────────┐   ┌── discovery (NEW, D-01) ───┐ │
 │   │ cargo metadata --no-deps            │   │ scan crates/*/Cargo.toml    │ │
 │   │   → 28 packages                     │   │   for own [workspace] table │ │
 │   │ jq .publish == null → 24 names      │   │   → 1 hit: pmcp-package     │ │
 │   └─────────────────┬───────────────────┘   └──────────┬──────────────────┘ │
 │                     │                                   │                    │
 │                     │        ┌── classification ────────┘                    │
 │                     │        │ cargo metadata --manifest-path <m>            │
 │                     │        │   → SAME predicate .publish == null           │
 │                     ▼        ▼                                               │
 │            ┌──────────────────────────────┐                                  │
 │            │ comment-strip release.yml    │                                  │
 │            │ then, per crate, here-string │                                  │
 │            │ match (never `| grep -q`)    │                                  │
 │            │  root:     `cargo publish -p <name>( |$)`                       │
 │            │  excluded: `cargo publish --manifest-path <relpath>`  (NEW)     │
 │            └───────────────┬──────────────┘                                  │
 │                            │                                                 │
 │            ┌───────────────▼──────────────┐                                  │
 │            │ ORDER ASSERTION (NEW, D-10)  │                                  │
 │            │ line(pmcp-package step)      │                                  │
 │            │   < line(each of 4 consumers)│                                  │
 │            └───────────────┬──────────────┘                                  │
 └────────────────────────────┼─────────────────────────────────────────────────┘
                              │ exit 0 / exit 1
              ┌───────────────┴────────────────┐
              ▼                                ▼
   make quality-gate (local)        ci.yml:233 "Release-ledger coverage"
   Makefile:892-895                 → gate.needs → blocks merge
              ▲
              │ declared PREREQUISITE (D-02)
   ┌──────────┴──────────────────────────────────────────┐
   │ SELF-TEST: doctored release.yml copy, step removed,  │
   │ assert non-zero exit. Proves the RED direction.      │
   └─────────────────────────────────────────────────────┘

                       ┌─────────────────────────────────┐
                       │  SHIP (one-way, human-gated)    │
                       └─────────────────────────────────┘
   D-05 sweep ──► version decisions ──► CHANGELOG [2.19.1] ──► sync main (9 conflicts)
        │                                                             │
        │ crates.io API                                        ◄──────┘
        │ (User-Agent REQUIRED)                                       │
        ▼                                                             ▼
   published-vs-in-tree-vs-diff                       PR ──[human]──► merge
                                                                      │
                                                        [human] git push upstream v2.19.1
                                                                      │
                                                                      ▼
                                                     release.yml: create-release
                                                       → publish-crates (ordered, skip-if-published)
                                                       → publish-mcp (OIDC, known-broken, NOT a gate)
                                                       → build-tester / build-* binaries
                                                                      │
                                                                      ▼
                                                     D-07 verification: crates.io API per crate
```

### Recommended Project Structure

No new directories. The touched surface is:

```
scripts/
├── check-release-coverage.sh        # EXTEND in place (D-01, D-10) — do not rewrite
├── release-version-sweep.sh         # NEW (D-05) — recommend permanent, see Open Q2
├── named-test-binary-count.awk      # precedent: extractor + its own self-test
└── deny-allow-entry-count.awk       # precedent: extractor + its own self-test
Makefile                             # register self-test as a PREREQUISITE of the gate
.github/workflows/release.yml        # D-09 comment reconciliation (4 stale regions)
CLAUDE.md                            # D-09 prose reconciliation (items 13/13a/15a)
CHANGELOG.md                         # NEW `## [2.19.1]` section (required by create-release)
Cargo.toml + N crate manifests       # D-03/D-05 version bumps
```

### Pattern 1: Two-source discovery, one classification predicate (D-01)

**What:** Discover workspace-excluded crates by filesystem scan; classify them with the
*same* Cargo predicate the root half already uses.

**When to use:** Whenever a gate's enumerator has a structural blind spot and the fix
risks introducing a second, divergent definition of the thing being enumerated.

**Why the second `cargo metadata` call is worth it:** `[VERIFIED: this session]` running
`cargo metadata --no-deps --format-version 1 --manifest-path crates/pmcp-package/Cargo.toml`
took **0.023s total** (`0.01s user 0.01s system`), which is inside the gate's stated
sub-second budget. And it resolves the one real ambiguity: `crates/pmcp-package/Cargo.toml`
declares `publish = true` (line 20), while the root gate's rule is documented as
"`publish` is null for publishable crates and `[]` for `publish = false`". Cargo reports
`pmcp-package publish=null` for the `publish = true` form — measured — so the predicate
transfers exactly. A hand-rolled `grep -q 'publish = false'` would agree today and
diverge the day someone writes `publish = ["internal"]`.

**Example (proven this session — yields exactly one hit):**

```bash
# Source: prototyped and executed against this working tree, 2026-08-26
# bash 3.2-safe: no mapfile, no arrays, no process substitution.
for m in crates/*/Cargo.toml; do
  grep -qE '^\[workspace\]' "$m" || continue          # discovery: own workspace root
  # classification: the SAME predicate as the root half
  meta="$(cargo metadata --no-deps --format-version 1 --manifest-path "$m")" || {
    echo "::error::cargo metadata failed for $m — coverage was NOT checked"; exit 1; }
  name="$(printf '%s' "$meta" | jq -r '.packages[] | select(.publish == null) | .name')" || {
    echo "::error::jq failed over $m — coverage was NOT checked"; exit 1; }
  [ -n "$name" ] || continue                           # publish-restricted → not our problem
  # matcher: --manifest-path form, NOT `-p <name>` (see Pitfall 3)
  if ! grep -qE "cargo publish --manifest-path ${m}( |\$)" <<<"$PUBLISH_LINES"; then
    missing_count=$((missing_count + 1))
    missing_list="${missing_list}  - ${name} (workspace-excluded; needs a --manifest-path step)
"
  fi
done
```

Measured output of the discovery half alone:
`EXCLUDED-PUBLISHABLE: pmcp-package  (crates/pmcp-package/Cargo.toml)` — exactly one hit,
no false positives inside `crates/`.

### Pattern 2: Fixture-driven guard self-test as a declared prerequisite (D-02)

**What:** A `make` target that feeds known inputs to the gate's own logic, asserts the
expected verdicts, *and* asserts the fixture count so a silently-dropped fixture fails.
Declared as a prerequisite of the gate it proves.

**When to use:** Any gate whose failure mode is "passes vacuously." Both existing
instances in this repo were built for exactly that.

**The two precedents, verbatim in shape:**

```makefile
# Source: Makefile:592-593 (and :684)
.PHONY: test-openapi-server-guard-selftest
test-openapi-server-guard-selftest:
	@echo "$(BLUE)Self-testing the named-test-binary count extractor...$(NC)"
	@fail=0; ran=0; \
	check() { \
		fixture="$$1"; expected="$$2"; shift 2; \
		actual=$$(printf '%s\n' "$$@" | awk -v want="$$WANT" -f scripts/named-test-binary-count.awk); \
		ran=$$((ran + 1)); \
		if [ "$$actual" != "$$expected" ]; then \
			echo "$(RED)✗ guard self-test fixture '$$fixture': expected $$expected, actual $$actual$(NC)"; \
			fail=1; \
		fi; \
	}; \
	check real 8 "$$RUN" '' 'running 8 tests' '' \
		'test result: ok. 8 passed; 0 failed; ...'; \
	...
	if [ "$$fail" -ne 0 ]; then exit 1; fi; \
	if [ "$$ran" -ne 6 ]; then \
		echo "$(RED)✗ count extractor self-test executed $$ran fixtures, expected 6 — a fixture was lost$(NC)"; \
		exit 1; \
	fi

# Source: Makefile:684 — the gate declares its own proof as a prerequisite
test-openapi-server: test-openapi-server-guard-selftest

# Source: Makefile:1414 — same shape for the crypto allowlist
no-crypto-check: no-crypto-allowlist-guard-selftest
```

The rationale is stated in-repo at `Makefile:657-658`: the extraction "lives in
`scripts/named-test-binary-count.awk` — one file, read by both this gate and
`test-openapi-server-guard-selftest`, so the gate and the proof of the gate cannot drift."

**Applying it here.** The self-test should assert **at minimum** these fixtures, because
each pins a distinct way the extension can pass vacuously:

| Fixture | Expected | Failure mode it pins |
|---------|----------|----------------------|
| intact workflow | exit 0 | the gate still passes on good input (no false red) |
| `pmcp-package` `--manifest-path` step removed | exit ≠ 0 | **SC2's headline** — today this returns 0 (measured, below) |
| a root crate's `-p` step removed | exit ≠ 0 | the original half still works after the edit |
| `--manifest-path` step present but COMMENTED OUT | exit ≠ 0 | the comment-strip discipline extends to the new matcher |
| order inverted: `pmcp-package` step moved after `cargo-pmcp` | exit ≠ 0 | D-10's assertion is live, not decorative |
| workflow file absent | exit ≠ 0 | pre-existing `[ -f "$WORKFLOW" ]` guard survives |

**The measured negative baseline (this is SC2's "demonstrated by running it"):**

```
$ /usr/bin/grep -v 'cargo publish --manifest-path crates/pmcp-package/Cargo.toml' \
    .github/workflows/release.yml > /tmp/release-doctored.yml
$ wc -l   # doctored: 658 vs original: 659  — exactly the publish line removed
$ ./scripts/check-release-coverage.sh /tmp/release-doctored.yml
release-coverage: all 24 publishable workspace members have a publish step.
EXIT=0
```

The gate is not merely silent about `pmcp-package` — it affirmatively reports full
coverage while the crate this milestone bumps has no publish step at all.

### Pattern 3: Line-ordinal order assertion, bounded to a named cluster (D-10)

**What:** Extract the line number of each cluster member's publish step from the
comment-stripped workflow and assert `line(pmcp-package) < line(consumer)` for each of the
four consumers.

**When to use:** When prose order and executable order have historically disagreed and the
executable is the authority. CLAUDE.md documents three separate instances (items 2, 12,
PR #303).

**Measured current state — the assertion passes today, so this is a lock, not a fix:**

| Step | `release.yml` line | Form |
|------|--------------------|------|
| `pmcp-package` | **440** | `cargo publish --manifest-path crates/pmcp-package/Cargo.toml` |
| `pmcp-cfn-renderer` | 466 | `cargo publish -p pmcp-cfn-renderer` |
| `pmcp-agent` | 484 | `cargo publish -p pmcp-agent` |
| `pmcp-team-servers` | 499 | `cargo publish -p pmcp-team-servers` |
| `cargo-pmcp` | 520 | `cargo publish -p cargo-pmcp` |

**Critical implementation note:** the order assertion must read the **comment-stripped**
text, and must derive line numbers from that same stripped text — or, better, match on the
`- name: Publish <crate>` step headers *after* stripping. Matching raw line numbers against
`grep -n` on the unstripped file will mis-order the moment a prose comment mentioning a
crate is added above a step. The existing script already computes `PUBLISH_LINES` once;
reuse it.

```bash
# Source: shape derived from the existing script's discipline, this session
step_line() {  # $1 = the exact publish command fragment
  printf '%s\n' "$PUBLISH_LINES" | grep -nF "$1" | head -1 | cut -d: -f1
}
pkg_line="$(step_line 'cargo publish --manifest-path crates/pmcp-package/Cargo.toml')"
[ -n "$pkg_line" ] || { echo "::error::pmcp-package publish step not found"; exit 1; }
for consumer in pmcp-cfn-renderer pmcp-agent pmcp-team-servers cargo-pmcp; do
  cl="$(step_line "cargo publish -p ${consumer}")"
  [ -n "$cl" ] || { echo "::error::${consumer} publish step not found"; exit 1; }
  if [ "$pkg_line" -ge "$cl" ]; then
    echo "::error::pmcp-package publishes AFTER ${consumer} — ${consumer} pins pmcp-package,"
    echo "::error::so cargo publish will fail with 'no matching package named pmcp-package'."
    exit 1
  fi
done
```

Note the "not found" arms are **explicit failures**, matching the script's stated
discipline ("a gate that cannot see must say so, never pass"). A `head -1`-into-empty
silently yielding `""` and then `[ "" -ge "$cl" ]` would be a bash syntax error under
`set -e`, which is a failure — but an *unreadable* one; name it instead.

### Pattern 4: Registry-truth version sweep (D-05)

**What:** Three-way comparison per crate: in-tree version, crates.io published version,
and source diff since **the tag that published that version**.

**The base-selection subtlety, which this session got wrong twice before getting it right:**

| Base choice | Problem |
|-------------|---------|
| Latest tag (`v2.19.0..HEAD`) | Misses drift introduced between an *earlier* tag and `v2.19.0`. A crate unchanged since `v2.19.0` but changed at `v2.18.0` was already skipped as "already published" at the `v2.19.0` release, so its delta is still unshipped. This base reported only 1 phantom delta. |
| The crate's own version-bump commit | Over-reports: changes between the bump commit and the tag that published it **are in the published artifact**. This base reported 6 phantom deltas, several spurious. |
| **The earliest `v*` tag CONTAINING the version-bump commit** ✅ | Correct: that is the release at which the current version actually went to crates.io. |

```bash
# Source: executed against this working tree, 2026-08-26
base=$(git log -1 --format=%h -L '/^version = /,+1:'"$manifest" | head -1)
tag=$(git tag --list 'v*' --contains "$base" --sort=creatordate | head -1)
[ -z "$tag" ] && tag="UNRELEASED"     # version bump not yet in any tag → ships next tag
git diff --shortstat "$tag"..HEAD -- "$crate_dir"
```

**The crates.io probe — `User-Agent` is mandatory:**

```bash
# Source: crates.io crawler policy; confirmed working this session for 21 crates
curl -s -H "User-Agent: pmcp-release-audit (guy@mlguy.us)" \
  "https://crates.io/api/v1/crates/${name}/versions" \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['versions'][0]['num'])"
```

### Anti-Patterns to Avoid

- **`printf ... | grep -q` for workflow matching:** the script documents a REPRODUCED
  SIGPIPE bug — "`grep -q` exits the instant it matches, and once the workflow outgrows
  the ~64 KiB pipe capacity the still-writing `printf` takes SIGPIPE and returns 141 —
  pipefail propagates that, `if !` inverts it, and the gate reports a crate that
  demonstrably HAS a publish step as missing." Use the here-string. **The file is now
  24,914 bytes and grows ~18 lines per crate** — this phase adds lines to it.
- **Rewriting `check-release-coverage.sh`:** four separate disciplines are documented
  in-file and each was earned by a real failure. Extend, don't replace.
- **Bumping only some of a version-emitter set:** CLAUDE.md item 13 documents that
  `PMCP_PACKAGE_VERSION_REQ` is invisible to `cargo build` — reverting only it leaves
  `cargo build --workspace` at exit 0 while `cargo test -p cargo-pmcp --lib` goes to exit
  101. (Not triggered this phase — see D-04's resolution — but the rule stands.)
- **Adding a `version` key to `crates/pmcp-openapi-server`'s `pmcp-package` dev-dep:**
  the Phase 121 CR-01 constraint. That crate publishes at `:339`, five steps before
  `pmcp-package` at `:440`. Currently correct: `pmcp-package = { path = "../pmcp-package" }`
  with no version key.
- **Trusting `cargo search` / `cargo info` for published state:** they report the in-tree
  path override. CLAUDE.md records the exact incident: "during Phase 122 `cargo info
  pmcp-package` printed `version: 0.3.0 (from ./crates/pmcp-package)`, which is the
  workspace path dep and not a published fact."

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| "Is this crate publishable?" | A TOML grep for `publish = false` | `cargo metadata --manifest-path <m>` + `.publish == null` | Measured: `publish = true` → `publish=null`; the grep and Cargo agree today but the grep cannot represent `publish = ["registry"]` |
| "Which crates are workspace-excluded?" | Parse root `[workspace].exclude` | Filesystem scan for a nested `[workspace]` table | The exclude array holds 11 entries, mostly non-publishable examples; parsing it recreates the allowlist D-01 forbids |
| "What version is published?" | `cargo search` / `cargo info` | crates.io API v1 + `User-Agent` | In-tree path override masquerades as published state (CLAUDE.md, Phase 122) |
| "Does the merge conflict?" | Actually merging to find out | `git merge-tree --write-tree --messages` | Non-destructive; gives the exact conflicted-path list without touching the working tree |
| "Does the publish order hold?" | Reading the prose ledger | Line-ordinal assertion over comment-stripped `release.yml` | Three recorded prose-vs-workflow order bugs; the workflow won every time |
| "Did the release actually publish?" | Reading the Actions log | crates.io API per expected `(crate, version)` | The `publish-mcp` job is known-broken since v2.17 and would make a green-eyed reading of the run misleading |
| Semver axis for a bumped crate | Eyeballing the diff | `cargo semver-checks` (pinned 0.49.0) | Already pinned in this repo; `constructible_struct_adds_field` has bitten this project before (D-113-D) |

**Key insight:** every hand-rolled shortcut in this domain fails in the *quiet* direction —
it reports success. That is why the repo's existing gates all carry an explicit
empty/zero-reading failure arm, and why D-02 asks for a red-direction proof rather than a
green one.

## Runtime State Inventory

> This is a release/migration phase. A grep audit finds files; it does not find what is
> already published, tagged, or registered outside this repository.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| **Stored data (registry state)** | crates.io holds, per the API this session: `pmcp` 2.19.0, `pmcp-package` **0.1.1** (the entire 0.2 line was never published), `cargo-pmcp` 0.21.0, `pmcp-agent` 0.2.0, `pmcp-team-servers` 0.1.1, `pmcp-cfn-renderer` 0.1.0, `pmcp-openapi-server` 0.1.0, `pmcp-server-toolkit` 0.1.1, `pmcp-workbook-server` 0.1.0, `pmcp-tasks` 0.1.0, `mcp-tester` 0.8.0, `mcp-preview` 0.3.1, `pmcp-server` 0.2.4, `pmcp-code-mode` 0.5.4, `pmcp-widget-utils` 0.1.0, `pmcp-workbook-runtime` 0.1.0, `pmcp-workbook-compiler` 0.1.0, `pmcp-sql-server` 0.1.0, `pmcp-code-mode-derive` 0.2.0, `pmcp-macros` 0.6.1, `pmcp-toolkit-mysql` 0.1.1 | **Permanent and one-way.** Every version number consumed is consumed forever. D-07's human checkpoints exist for this. Re-verify immediately before the tag push — this table ages. |
| **Live service config (GitHub)** | `release.yml` triggers on `push: tags: v*` and on `workflow_dispatch`. `create-release` extracts release notes by awk-matching `## \[<version-without-v>\]` in `CHANGELOG.md`. Latest tag is `v2.19.0` (`cfee41f9`, 2026-08-20). | A `## [2.19.1]` section **must exist in CHANGELOG.md before the tag is pushed** or the GitHub Release is created with empty notes — silently, since the awk yields an empty string rather than failing. |
| **Live service config (MCP registry)** | `publish-mcp` job exists and its OIDC publish has been broken since v2.17.0. | **None.** Explicitly deferred by CONTEXT; it is a separate job from `publish-crates` and must not gate D-07's verification. Note it will show as a red job on a run that otherwise succeeded — do not read that as a failed release. |
| **OS-registered state** | None — this project registers nothing with the OS. | None. Verified: no launchd/systemd/Task Scheduler artifacts in the release path. |
| **Secrets / env vars** | `CARGO_REGISTRY_TOKEN` (repo secret, consumed by every publish step); `GITHUB_TOKEN` (create-release). Local: `RUSTFLAGS` is exported in CI but absent locally. | No key renames. **Run `RUSTFLAGS="" make quality-gate` before push** — recorded local-gate blind spot #9. |
| **Build artifacts** | Nothing stale that a version bump invalidates: no egg-info analogue, no baked version constants beyond `PMCP_PACKAGE_VERSION_REQ` (verified `= "0.3"`, consistent). | None, given D-04's resolution to ship 0.3.0 as-is. |
| **Git topology (release-blocking)** | `main` is at `c64e2b2b`, a **squash** commit (`git cat-file -p` shows a single `parent 67758388`) that landed phases 120–122 from this branch as PR #347. Branch is 241 ahead / 1 behind. | D-08's sync is a **9-file conflict resolution**, not a fast-forward. See Pitfall 1. |

## Common Pitfalls

### Pitfall 1: The `main` sync is a 9-file conflict, not a fast-forward

**What goes wrong:** D-08 reads as a one-line chore ("Sync the 1-behind commit from `main`
before opening the PR"). Executed naively it stalls the phase mid-flight with conflicts in
production source, at the exact moment the plan expects to be opening a PR.

**Why it happens:** `main`'s single commit ahead is a **squash** of this branch's own
120–122 work. Git's merge base is therefore `67758388` (before either side's changes), so
the three-way merge sees the same content arriving from two directions, and every file
Phase 123 further modified after the squash point conflicts.

**Measured — `git merge-tree --write-tree --messages HEAD main`, exit 1, 9 conflicts:**

```
CONFLICT (content): .planning/REQUIREMENTS.md
CONFLICT (content): .planning/ROADMAP.md
CONFLICT (content): .planning/STATE.md
CONFLICT (content): .planning/WINDOWS.md
CONFLICT (content): Makefile
CONFLICT (content): cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs
CONFLICT (content): cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs
CONFLICT (add/add): crates/pmcp-package/tests/common/mod.rs
CONFLICT (add/add): docs/design/package-portability-pmcp-run-handoff.md
```

Note `cargo-pmcp/Cargo.toml`, `crates/pmcp-package/src/oci/mod.rs` and
`crates/pmcp-package/src/package/server.rs` auto-merged cleanly — the conflicts are
concentrated, not diffuse.

**How to avoid:** Budget the sync as its own task with a full `make quality-gate` after it,
placed **before** any version bump (a conflict resolution that silently reverts a bumped
manifest is the worst possible ordering). Resolve `Makefile` and the two `graphql*.rs`
files with care — those are the three that carry real behaviour.

**Warning signs:** A PR diff that shows all of phases 120–123 rather than just 123+124
means the sync was skipped and GitHub is diffing from `67758388`.

### Pitfall 2: The D-05 sweep finds six more phantom deltas than the CONTEXT anticipated

**What goes wrong:** The plan budgets two version bumps (D-03's `pmcp`, D-04's
`pmcp-package`) and discovers at ship time that four more crates carry unreleased source.
Because `release.yml` skips already-published versions gracefully, those crates *silently
do not ship* — the same failure class as the unpublished `pmcp-tasks`, one layer up.

**Measured sweep (base = the tag that published each crate's current version):**

| Crate | In-tree | Published | Publish tag | Delta since publish tag | Nature |
|---|---|---|---|---|---|
| `pmcp` | 2.19.0 | 2.19.0 | v2.19.0 | 1 file, +1/−1 | `Cargo.toml` only: `jsonwebtoken` 10.3→11.0 — **D-03** |
| `pmcp-widget-utils` | 0.1.0 | 0.1.0 | v1.3 | 1 file, +1/−4 | `src/lib.rs` — **`cargo fmt` reflow only**, behaviourally identical |
| `pmcp-code-mode-derive` | 0.2.0 | 0.2.0 | v2.3.1 | 2 files, +4/−4 | **BEHAVIOURAL**: generated code for `"sql"` changed from `validate_sql_query(code, &context)` to `validate_sql_query_async(code, &context).await` |
| `mcp-preview` | 0.3.1 | 0.3.1 | v2.7.0 | 1 file, +1/−1 | `Cargo.toml` dep bump only |
| `pmcp-sql-server` | 0.1.0 | 0.1.0 | v2.9.0 | 1 file, +2/−2 | `Cargo.toml` dep bumps only |
| `pmcp-workbook-compiler` | 0.1.0 | 0.1.0 | v2.9.2 | 2 files, +24/−5 | `RESERVED_TOOL_NAMES` gained a 5th entry `verify_accuracy` (doc + test assertion) |
| `pmcp-workbook-runtime` | 0.1.0 | 0.1.0 | v2.9.2 | 5 files, **+1100/−49** | **ADDITIVE PUBLIC API**: new `pub mod reconcile` (590 lines) plus new root re-exports `RenderMode`, `reconcile_reference`, `seed_reference_inputs`, `OutputRow`, `ReconcileReport`, `ToolReport` |
| `mcp-tester` | 0.8.0 | 0.8.0 | v2.19.0 | **(none)** | Clean — CONTEXT's open question answered |
| `pmcp-macros`, `pmcp-macros-support`, `pmcp-code-mode`, `pmcp-server`, `pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`, `pmcp-toolkit-athena`, `pmcp-workbook-dialect` | = published | = | various | **(none)** | Clean |
| `pmcp-agent` 0.3.0, `pmcp-tasks` 0.1.1, `pmcp-cfn-renderer` 0.2.0, `pmcp-server-toolkit` 0.1.2, `pmcp-team-servers` 0.2.0, `pmcp-workbook-server` 0.1.1, `pmcp-openapi-server` 0.1.1, `cargo-pmcp` 0.23.0, `pmcp-package` 0.3.0 | > published | | UNRELEASED | bump not yet in any tag | Already bumped — ship as-is at this tag |

**How to avoid:** Run the sweep as an early task and route its output to the user as a
decision, not as an execution detail. `pmcp-workbook-runtime` in particular is a
pre-1.0 additive-API change, which by the crate-family precedent (Phase 122's
`pmcp-package` 0.2→0.3) is a **minor** bump to 0.2.0, not a patch.

**Warning signs:** A crate whose in-tree version equals its published version *and* whose
`git diff <publish-tag>..HEAD` is non-empty. That conjunction is the entire signature.

### Pitfall 3: The `--manifest-path` matcher must not be pattern-matched like the `-p` form

**What goes wrong:** Reusing the existing regex shape
`cargo publish --manifest-path ${crate}( |$)` fails, because the workflow line ends in
`Cargo.toml 2>&1) && echo "$OUTPUT" || {` — there is no path-terminal boundary the way
there is for `-p <name>`, and the *crate name* does not appear in the command at all.

**Why it happens:** The root half matches on crate **name**; the excluded half must match
on manifest **path**. `[VERIFIED: .github/workflows/release.yml:445]` the literal is:

```
        OUTPUT=$(cargo publish --manifest-path crates/pmcp-package/Cargo.toml 2>&1) && echo "$OUTPUT" || {
```

**How to avoid:** Match the manifest path with a trailing-space boundary
(`--manifest-path ${m}( |\$)`), and derive `${m}` from the scan (`crates/<name>/Cargo.toml`)
so the matcher and the discovery cannot disagree about the path spelling. Prefer `grep -F`
for the fixed portion where possible — a path contains `.` and `/`, which are regex-live.

**Warning signs:** The extended gate reports `pmcp-package` missing while the step is
plainly present, or reports it present when the self-test's doctored copy removed it.

### Pitfall 4: The rtk shell proxy corrupts `grep -v` and `git diff`

**What goes wrong:** Verification commands appear to succeed while producing nonsense.
Reproduced this session: `grep -v '<pattern>' release.yml > doctored.yml` produced a
**5-line** file instead of 658 — the proxy answered with a search result rather than
performing the inverse match. A self-test built on that output would "prove" whatever the
corruption produced.

**How to avoid:** Use absolute binary paths (`/usr/bin/grep`, `/opt/homebrew/bin/git`) in
any command whose *output* is load-bearing evidence. Inside `make` this is not an issue
(recipes run under `/bin/sh` directly), so the Makefile self-test is safe — the hazard is
the agent's own verification transcripts.

**Warning signs:** Line counts that differ wildly from expectation; `git diff` output that
does not match `git diff --stat`.

### Pitfall 5: Four crates carry a `mcp-tester` dev-dep with BOTH `path` and `version`, publishing before it

**What goes wrong:** Not this phase — but it is one `mcp-tester` bump away from being this
phase. `cargo` strips a dev-dep from the published manifest **only** when it carries no
version requirement; one that carries a requirement is retained and must resolve on
crates.io at publish time.

**Measured — the four at-risk crates, all publishing before `mcp-tester` (`:396`):**

| Crate | Pin | Publish step |
|---|---|---|
| `crates/pmcp-server-toolkit/Cargo.toml:192` | `mcp-tester = { version = "0.8.0", path = "../mcp-tester" }` | :258 |
| `crates/pmcp-sql-server/Cargo.toml:57` | same | :324 |
| `crates/pmcp-openapi-server/Cargo.toml:63` | same | :339 |
| `crates/pmcp-workbook-server/Cargo.toml:58` | same | :378 |

(`crates/pmcp-server` `:31` and `cargo-pmcp` `:69` carry it too, but publish *after*
`mcp-tester`, so they are safe by construction.)

This is tracked as deferred item **D12** in
`crates/pmcp-openapi-server/tests/pmcp_package_pin.rs`, which states the rule: "a
`[dev-dependencies]` entry carrying BOTH `path` and `version` is safe only while that
version is already on crates.io."

**How to avoid:** The D-05 sweep found `mcp-tester` clean at 0.8.0, so it needs no bump and
the four stay green. **If** a later decision bumps `mcp-tester`, those four pins must move
in the same change or the release job fails at the first of them. Worth a one-line note in
the phase summary so the next releaser inherits it.

### Pitfall 6: `release.yml`'s own ordering comment contradicts its own step order

**What goes wrong:** D-09 is scoped in CONTEXT as fixing "the stale `:98-99` line." There
are **four** stale regions, and the most misleading one is a full prose ordering list that
disagrees with the executable steps below it in the same file.

**Measured stale regions in `.github/workflows/release.yml`:**

| Lines | Stale content | Truth |
|---|---|---|
| 96–102 | The `# Publish order:` list ends `-> cargo-pmcp -> pmcp-server -> pmcp-package`, i.e. `pmcp-package` **LAST** | `pmcp-package` publishes at `:440`, **before** `cargo-pmcp` `:520` and `pmcp-server` `:538` |
| 98–99 | "`has NO in-repo consumers yet`, so it publishes LAST … Move it earlier only once a shipped crate actually pins it." | It has **four** in-repo consumers, and it *was* already moved earlier |
| 428–429 | "pins all four (`pmcp-package = "0.1"`, `pmcp-cfn-renderer = "0.1"`, `pmcp-agent = "0.1"`, `pmcp-team-servers = "0.1"`)" | Measured: `pmcp-package = "0.3"`, `pmcp-cfn-renderer = "0.2"`, `pmcp-agent = "0.3"`, `pmcp-team-servers = "0.2"` |
| 459–460 | "`pmcp-package = "0.1"`, needs the 0.1.1 `[auth.cognito]` promotion" | `crates/pmcp-cfn-renderer/Cargo.toml:10` reads `pmcp-package = { version = "0.3", path = "../pmcp-package" }` |

**How to avoid:** Treat D-09 as "reconcile every crate-version and order claim in
`release.yml`'s comments," not as a single-line edit. D-10's assertion protects the *order*
claim mechanically; the *pin-version* claims in comments have no guard and will rot again —
consider stating them by reference ("see CLAUDE.md item 13") rather than by value.

### Pitfall 7: `CHANGELOG.md` has no `[2.19.1]` section and the workflow fails silently without one

**What goes wrong:** The GitHub Release is created with empty notes and nobody notices
until someone reads the release page.

**Why it happens:** `[VERIFIED: .github/workflows/release.yml:29-38]` the extraction is:

```yaml
CHANGELOG=$(awk -v ver="## \\[$VERSION_NO_V\\]" '
  $0 ~ ver {p=1; next}
  /^## \[/ && p {exit}
  p {print}
' CHANGELOG.md)
```

An unmatched version yields an empty string. The step exits 0; `gh release create --notes ""`
succeeds.

**How to avoid:** Add the `## [2.19.1] - <date>` section as a plan task with an explicit
verification that re-runs that exact awk against the file and asserts non-empty output.
The current head of the file is `## [2.19.0] - 2026-08-20`.

### Pitfall 8: The `publish-mcp` job will be red on a successful release

**What goes wrong:** D-07 asks the phase to close only when "`release.yml` has run" — a
reading of the Actions UI shows a failed job and the phase looks blocked.

**How to avoid:** D-07's verification must be **per-crate against the crates.io API**, not
against the workflow run's overall status. CONTEXT already scopes the OIDC failure out;
encode that in the verification command so the executor does not re-litigate it.

## Code Examples

### Extending the gate: the complete new block, bash-3.2 safe

```bash
# Source: composed from scripts/check-release-coverage.sh's own conventions,
# prototyped and executed against this working tree 2026-08-26.
#
# Placement: AFTER the existing root-member while-loop, BEFORE the
# `if [ "$missing_count" -gt 0 ]` reporting block, so both halves report together.

# Workspace-EXCLUDED publishable crates. cargo metadata --no-deps cannot see
# them (they carry their own [workspace] table), which is why they need their
# own discovery. Discovery is a SCAN, not a list: the next excluded crate is
# covered the day it is created.
excluded_seen=0
for m in crates/*/Cargo.toml; do
  [ -f "$m" ] || continue
  grep -qE '^\[workspace\]' "$m" || continue
  excluded_seen=$((excluded_seen + 1))

  # Classification uses the SAME predicate as the root half, from cargo itself,
  # so a filesystem heuristic can never disagree with cargo about what
  # "publishable" means. Measured cost: 0.023s.
  EX_META="$(cargo metadata --no-deps --format-version 1 --manifest-path "$m")" || {
    echo "::error::cargo metadata failed for $m — release-ledger coverage was NOT checked"
    exit 1
  }
  EX_NAME="$(printf '%s' "$EX_META" | jq -r '.packages[] | select(.publish == null) | .name')" || {
    echo "::error::jq failed over $m — release-ledger coverage was NOT checked"
    exit 1
  }
  [ -n "$EX_NAME" ] || continue   # publish-restricted → correctly not our problem

  total=$((total + 1))
  # NOTE the matcher differs from the root half: an excluded crate publishes via
  # --manifest-path, and the crate NAME never appears in the command.
  if ! grep -qE "cargo publish --manifest-path ${m}( |\$)" <<<"$PUBLISH_LINES"; then
    missing_count=$((missing_count + 1))
    missing_list="${missing_list}  - ${EX_NAME} (workspace-EXCLUDED: needs a \`cargo publish --manifest-path ${m}\` step)
"
  fi
done

# An empty scan is a FAILURE, mirroring the empty-crate-list rule above: this
# repo has had exactly one workspace-excluded publishable crate since Phase 108,
# so zero hits means the glob or the cwd is wrong, not that coverage holds.
if [ "$excluded_seen" -eq 0 ]; then
  echo "::error::found ZERO crates/*/Cargo.toml carrying their own [workspace] table —"
  echo "::error::this repo has at least one (crates/pmcp-package); refusing to pass a check that verified nothing"
  exit 1
fi
```

### D-03's guard: proving no `jsonwebtoken` type crosses `pmcp`'s public API

```bash
# Cheap first pass (measured this session — 2 files, 16 references, all internal):
grep -rn "jsonwebtoken" src/
#   src/server/auth/jwt.rs:33            keys: HashMap<String, jsonwebtoken::DecodingKey>,   ← private field
#   src/server/auth/jwt_validator.rs:330 async fn get_key(...) -> Result<jsonwebtoken::DecodingKey>  ← private fn
#   … remaining 14 are `use` statements inside fn bodies and error-kind matches

# Authoritative pass — this is what the plan should verify against:
cargo public-api --simplified > /tmp/pmcp-public-api.txt
grep -c "jsonwebtoken" /tmp/pmcp-public-api.txt   # MUST be 0

# And confirm the patch axis:
cargo semver-checks check-release --baseline-version 2.19.0
```

The grep shows no `pub` item names a `jsonwebtoken` type — line 330's `get_key` has no
`pub` qualifier and line 33's `keys` is a private struct field — so the patch axis in D-03
looks correct. `[ASSUMED]` until `cargo public-api` confirms it; the grep cannot see
re-exports or trait-associated types.

### D-05 sweep, complete

```bash
#!/usr/bin/env bash
# Source: executed against this working tree 2026-08-26; output in Pitfall 2.
set -uo pipefail
UA="pmcp-release-audit (guy@mlguy.us)"

# Root workspace publishable members …
cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.publish == null) | "\(.name)\t\(.version)\t\(.manifest_path)"' > /tmp/pubs.tsv
# … plus the workspace-excluded ones (same discovery as the gate)
for m in crates/*/Cargo.toml; do
  grep -qE '^\[workspace\]' "$m" || continue
  cargo metadata --no-deps --format-version 1 --manifest-path "$m" \
    | jq -r '.packages[] | select(.publish == null) | "\(.name)\t\(.version)\t\(.manifest_path)"' >> /tmp/pubs.tsv
done

while IFS=$'\t' read -r name ver manifest; do
  dir=$(dirname "$manifest"); rel=${dir#"$PWD"/}; [ "$rel" = "$PWD" ] && rel="."
  mrel=${manifest#"$PWD"/}
  published=$(curl -s -H "User-Agent: $UA" \
    "https://crates.io/api/v1/crates/$name/versions" \
    | python3 -c "import sys,json;print(json.load(sys.stdin)['versions'][0]['num'])" 2>/dev/null \
    || echo UNPUBLISHED)
  # base = the tag that actually PUBLISHED the current in-tree version
  bump=$(git log -1 --format=%h -L '/^version = /,+1:'"$mrel" 2>/dev/null | head -1)
  tag=$(git tag --list 'v*' --contains "$bump" --sort=creatordate 2>/dev/null | head -1)
  if [ "$rel" = "." ]; then paths="src/ Cargo.toml"; else paths="$rel"; fi
  if [ -z "$tag" ]; then delta="(bump not in any tag → ships at next tag)";
  else delta=$(git diff --shortstat "$tag"..HEAD -- $paths | tr -d '\n'); [ -z "$delta" ] && delta="(none)"; fi
  printf '%-26s in-tree=%-9s published=%-9s tag=%-10s %s\n' "$name" "$ver" "$published" "${tag:-UNRELEASED}" "$delta"
done < /tmp/pubs.tsv
```

### D-07 verification: confirm each expected version went live

```bash
# Source: shape confirmed working this session for 21 crates.
UA="pmcp-release-audit (guy@mlguy.us)"
fail=0
while IFS=' ' read -r name want; do
  got=$(curl -s -H "User-Agent: $UA" "https://crates.io/api/v1/crates/$name/versions" \
        | python3 -c "import sys,json;print(' '.join(v['num'] for v in json.load(sys.stdin)['versions']))")
  case " $got " in
    *" $want "*) echo "OK   $name $want" ;;
    *)           echo "MISS $name expected $want, registry has: $got"; fail=1 ;;
  esac
done <<'EOF'
pmcp 2.19.1
pmcp-package 0.3.0
pmcp-agent 0.3.0
pmcp-team-servers 0.2.0
pmcp-cfn-renderer 0.2.0
pmcp-server-toolkit 0.1.2
pmcp-openapi-server 0.1.1
pmcp-workbook-server 0.1.1
pmcp-tasks 0.1.1
cargo-pmcp 0.23.0
EOF
exit $fail
```

*(The list above is this session's measurement of what is already bumped and awaiting
publication; extend it with whatever the D-05 decision adds.)*

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hand-maintained prose publish ledger only (CLAUDE.md) | `release.yml` is the AUTHORITY; prose is a mirror | PR #303 (post-v2.17), stated in CLAUDE.md | Prose-vs-workflow disagreements are resolved in the workflow's favour by rule, not by argument |
| No machine check that a crate has a publish step | `scripts/check-release-coverage.sh` in `make quality-gate` + `ci.yml:233` | 2026-08-21 (after `pmcp-tasks` was found unpublished) | A missing publish step is now a build failure — for root members |
| Gates asserted correct in prose | Gates carry fixture-driven `*-guard-selftest` prerequisites | Phases 121/122 | Two precedents; D-02 is the third instance of an established pattern |
| `cargo search` for published state | crates.io API v1 with `User-Agent` | Phase 122 (after `cargo info` reported the path override) | The registry is the only oracle for what shipped |
| `npx cdk synth` for deploys | `pmcp-cfn-renderer` pure-Rust synthesis | PR #313 / v0.20.0 | Explains why `cargo-pmcp` pins `pmcp-cfn-renderer` and hence the cluster order |

**Deprecated/outdated:**
- `release.yml:96-102`'s publish-order comment — contradicted by its own steps (Pitfall 6).
- `release.yml:428-429`, `:459-460` pin versions (`"0.1"`) — all four are now `0.2`/`0.3`.
- `.planning/REQUIREMENTS.md:37` PKGR-01's original text — superseded by the correction
  block immediately below it at `:42-49`. Plans must read the correction, not the bullet.

## Project Constraints (from CLAUDE.md)

Directives that bind this phase specifically:

| Directive | Source | Binding effect here |
|---|---|---|
| `make quality-gate` before any commit; zero clippy warnings; pre-commit hook enforces | "Quality Gate Enforcement" | Every task ends with the gate. It now includes the extended `check-release-coverage`. |
| Run `RUSTFLAGS="" make quality-gate` before push | project memory, local-gate blind spot #9 | RUSTFLAGS is exported in CI, absent locally |
| `rustup update stable` first — "local/CI version mismatch is the #1 cause of CI failures" | "Pre-Flight Checklist" step 1 | Do this before the release PR, not after CI goes red |
| Check crates.io versions before bumping | Pre-Flight step 2 — **but it prescribes `cargo search`** | **Contradiction:** item 13's own Phase-122 note forbids `cargo search`. Resolve in favour of the API; see Open Question 4 |
| "Downstream crates that pin a bumped dependency must also be bumped" | "Version Bump Rules" | The one-set rule. Not triggered by D-04's resolution, but binds any crate the D-05 sweep promotes |
| `release.yml` is the AUTHORITY on publish order | Workspace Crates preamble | D-09 reconciles prose *toward* the workflow, never the reverse |
| Item 9b CR-01: `pmcp-openapi-server`'s `pmcp-package` dev-dep must stay path-only | Item 9b | Do not "restore a pin" while editing pins elsewhere. Verified currently correct |
| Item 13 ⚠ ORDERING CONSTRAINT explicitly names Phase 124 as owner | Item 13 | This phase must check the three consumers publish carrying `^0.3` — verified consistent |
| Zero SATD comments; cognitive complexity ≤ 25 | Toyota Way section | Applies to any Rust added; this phase adds shell, so mainly the SATD rule |
| Contract-first: update contract YAML, `pmat comply check` | "Contract-First Development" | No contract surface changes here (no public API moves under D-04's resolution) |
| ALWAYS requirements (fuzz/property/unit/example) for new **features** | "ALWAYS Requirements" | This phase ships no feature; the analogue is D-02's self-test, which the plan must treat as non-optional |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | No `jsonwebtoken` type crosses `pmcp`'s public API, so 2.19.1 (patch) is the right axis | Code Examples / D-03 | If a re-export or trait associated type exposes one, a major-dependency bump becomes a breaking change shipped as a patch. **Mitigation is already in the plan: run `cargo public-api` — do not ship on the grep alone.** |
| A2 | `pmcp-workbook-runtime`'s new public surface warrants 0.2.0 (pre-1.0 minor) rather than 0.1.1 | Pitfall 2 | Under-bumping publishes additive API as a patch. Precedent is `pmcp-package` 0.2→0.3 for source-breaking changes; additive-at-pre-1.0 → minor is the conventional reading, but not a decision the user has made |
| A3 | `pmcp-widget-utils`, `mcp-preview` and `pmcp-sql-server` need no bump (fmt/dep-manifest only) | Pitfall 2 | A dep-requirement change does alter the published manifest; leaving it unshipped means consumers keep resolving the old requirement. Low impact, but it is a judgement, not a measurement |
| A4 | The `publish-mcp` OIDC job is still broken | Runtime State Inventory | If it now works, nothing breaks — the phase just gains a green job. Sourced from project memory (open since v2.17), not re-tested this session |
| A5 | `git tag --list --contains --sort=creatordate \| head -1` correctly identifies the publishing release for every crate | Pattern 4 | A crate published out-of-band (CLAUDE.md says `pmcp-workbook-runtime` was) may have no correct tag, making its delta over- or under-reported. `pmcp-workbook-runtime` is exactly the crate with the largest reported delta — worth a manual confirmation |
| A6 | crates.io `versions[0]` is the newest version | Sweep / D-07 | The API returns newest-first in practice (consistent across 21 crates this session) but this is not a documented guarantee; sorting by semver would be safer |

## Open Questions

1. **What happens to the six phantom deltas the sweep found?**
   - What we know: measured, classified, and listed in Pitfall 2. Two are behaviourally
     real (`pmcp-code-mode-derive`'s async SQL validation switch; `pmcp-workbook-runtime`'s
     new `reconcile` module and five re-exports); three are manifest/format-only; one
     (`pmcp-workbook-compiler`) is a doc + test-assertion change tracking a runtime const.
   - What's unclear: CONTEXT pre-decided only `pmcp` (D-03) and `pmcp-package` (D-04). D-05
     mandates the *measurement* but not the *disposition*.
   - Recommendation: put a `checkpoint:human-verify` after the sweep task presenting this
     exact table, with a recommended disposition per crate (bump
     `pmcp-code-mode-derive` → 0.2.1; `pmcp-workbook-runtime` → 0.2.0; leave the three
     manifest-only crates; leave `pmcp-workbook-compiler` unless its `RESERVED_TOOL_NAMES`
     assertion is load-bearing for a published consumer). Do not let the plan decide this
     silently — each one is a permanent, unrecoverable version-number consumption.

2. **Permanent sweep script or phase-local?** (explicitly Claude's discretion)
   - What we know: the working implementation is ~40 lines of shell with no dependencies
     beyond what the repo already uses, and it is the only mechanism that can detect a
     phantom delta.
   - What's unclear: whether it should *gate* (fail CI when a phantom delta exists) or
     merely *report*. Gating would be wrong — a delta is legitimate right up until a
     release — but the release Pre-Flight Checklist has no machine-checked step at all today.
   - Recommendation: commit it as `scripts/release-version-sweep.sh`, **reporting only**,
     invoked by a `make release-sweep` target that is *not* in `quality-gate`, and add one
     line to CLAUDE.md's Pre-Flight Checklist pointing at it (replacing the `cargo search`
     step — see Q4). Cheap permanence, no false CI failures.

3. **Does the D-02 self-test doctor a real copy of `release.yml`, or use inline fixtures?**
   - What we know: both existing self-tests use **inline fixtures** fed to an extracted
     `awk` script, precisely so "the gate and the proof of the gate cannot drift."
   - What's unclear: the coverage gate's logic is not extracted into a separate awk file,
     so the inline-fixture pattern does not transfer directly.
   - Recommendation: doctor a real copy in a temp dir (`mktemp -d`), since the script
     already takes the workflow path as `$1` — that argument exists and is the natural seam.
     Assert on **exit codes plus the crate name appearing in the error output**, not on the
     full message text, so wording changes do not break the self-test.

4. **CLAUDE.md's Pre-Flight Checklist prescribes `cargo search`, which item 13 forbids.**
   - What we know: Pre-Flight step 2 says "Check crates.io versions: `cargo search pmcp
     --limit 5`". Item 13's Phase-122 note says verify "with the crates.io API … NOT with
     `cargo search`/`cargo info`, which report the in-tree path override."
   - What's unclear: nothing factual — this is an unreconciled internal contradiction in
     the very document D-09 is reconciling.
   - Recommendation: fold it into D-09's scope. It is one edit, in the file already being
     edited, closing a contradiction that would otherwise mislead the next releaser.

5. **Is `pmcp-workbook-runtime`'s measured +1100/−49 delta genuinely unpublished?**
   - What we know: crates.io reports exactly one version, `0.1.0`; in-tree is `0.1.0`; the
     diff from `v2.9.2` (the tag containing its version-bump commit) is 5 files.
   - What's unclear: CLAUDE.md item 9a notes this crate "is published out-of-band by its
     own Phase 91/92 workbook-runtime release" — so its tag association may be atypical (A5).
   - Recommendation: confirm by `cargo package --manifest-path crates/pmcp-workbook-runtime/Cargo.toml
     --list` against the published `.crate`, or simply by checking whether
     `src/reconcile.rs` exists in the published 0.1.0 (`cargo download` / docs.rs).
     docs.rs for `pmcp-workbook-runtime` 0.1.0 will show whether `pub mod reconcile` is there.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `bash` | The gate, locally | ✓ | 3.2.57(1) arm64-apple-darwin25 | — (constraint, not a gap) |
| `git` | Sweep, merge preview, tags | ✓ | 2.47.1 | — (`merge-tree --write-tree` needs ≥ 2.38 ✓) |
| `cargo` / `rustc` | metadata, build, publish | ✓ | workspace builds | — |
| `jq` | gate predicate | ✓ | gate already runs green | — |
| `curl` + network to crates.io | D-05, D-07 | ✓ | 21 crates queried successfully | none — D-07 cannot be verified offline |
| `python3` | sweep JSON parsing | ✓ | stdlib `json` used successfully | `jq` |
| `make` | gate chaining | ✓ | `make quality-gate` is the repo's standard | — |
| `gh` CLI | PR + release inspection | ✓ (assumed — repo workflow depends on it) | — | web UI |
| `cargo public-api` | A1's authoritative check | ✗ (not verified installed this session) | pinned 0.52.0 per Phase 112 | `cargo install cargo-public-api@0.52.0` |
| `cargo semver-checks` | semver axis confirmation | ✗ (not verified installed this session) | pinned 0.49.0 per Phase 112 | `cargo install cargo-semver-checks@0.49.0` |
| `pmat` | CI complexity gate | n/a locally (CI-only per D-07 of Phase 75) | 3.15.0 in CI | — |

**Missing dependencies with no fallback:** none.

**Missing dependencies with fallback:** `cargo public-api` and `cargo semver-checks` are
not confirmed present locally; both are `cargo install`-able at their pinned versions, and
A1's guard depends on the first. The plan should include the install as an explicit step
rather than assuming availability.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (libtest) for Rust; **`make` targets with inline shell fixtures** for gate self-tests — the relevant framework for this phase |
| Config file | `Makefile` (targets + chaining); `.github/workflows/ci.yml:233` for CI |
| Quick run command | `make check-release-coverage` |
| Full suite command | `RUSTFLAGS="" make quality-gate` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PKGR-01 / SC1 | Gate exits 0 on the real workflow; `pmcp-openapi-server` present in both ledgers | smoke | `./scripts/check-release-coverage.sh && /usr/bin/grep -c 'pmcp-openapi-server' CLAUDE.md .github/workflows/release.yml` | ✅ (re-verification; passes today, transcript recorded above) |
| PKGR-01 / SC2 | Gate covers `crates/pmcp-package`; deleting its step makes the gate FAIL | unit (fixture) | `make check-release-coverage-guard-selftest` | ❌ Wave 0 — new target + new fixtures |
| PKGR-01 / SC2 | Discovery finds excluded crates without an allowlist | unit (fixture) | same target, fixture: a synthetic `crates/<tmp>/Cargo.toml` with `[workspace]` | ❌ Wave 0 |
| PKGR-01 / SC3 | `pmcp-package` version + all consumer pins move as one set | unit | `cargo test -p cargo-pmcp --test pmcp_package_pin` and `make test-openapi-server` | ✅ both tripwires exist and pass |
| PKGR-01 / SC3 | Scaffold emitter matches the crate version | unit | `cargo test -p cargo-pmcp --lib emitted_package_requirement_matches_workspace_major_minor_line` | ✅ exists |
| PKGR-01 / SC4 | `pmcp-package` publish step precedes its four consumers | unit (fixture) | `make check-release-coverage-guard-selftest` (inverted-order fixture) | ❌ Wave 0 |
| D-03 | No `jsonwebtoken` type on `pmcp`'s public API | integration | `cargo public-api --simplified \| grep -c jsonwebtoken` → `0` | ❌ Wave 0 (tool install) |
| D-05 | Every publishable crate's three-way comparison is recorded | smoke | `make release-sweep` (proposed) | ❌ Wave 0 |
| D-07 | Each expected `(crate, version)` is live on crates.io | manual-gated | the D-07 script above | ❌ Wave 0 — **cannot run before the tag is pushed** |

### Sampling Rate

- **Per task commit:** `make check-release-coverage` (sub-second) plus the new self-test.
- **Per wave merge:** `RUSTFLAGS="" make quality-gate`.
- **Phase gate:** full `make quality-gate` green on the synced branch **before** the PR is
  opened; then the D-07 registry verification **after** the tag push.

### Wave 0 Gaps

- [ ] `Makefile`: `check-release-coverage-guard-selftest` target — covers SC2, SC4
- [ ] `Makefile`: declare it a **prerequisite** of `check-release-coverage` (the
      `test-openapi-server: test-openapi-server-guard-selftest` shape at `Makefile:684`)
- [ ] `scripts/release-version-sweep.sh` + `make release-sweep` — covers D-05
- [ ] Tool install: `cargo install cargo-public-api@0.52.0 cargo-semver-checks@0.49.0` — covers A1
- [ ] `CHANGELOG.md`: `## [2.19.1]` section — required by `create-release`, verified by
      re-running the workflow's own awk

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No auth surface changes; `jsonwebtoken` moves version but no code changes |
| V3 Session Management | no | — |
| V4 Access Control | **yes** | `CARGO_REGISTRY_TOKEN` is a repo secret consumed only by `publish-crates`; the tag trigger means anyone who can push a `v*` tag can publish. D-07's human checkpoint on the tag push is the control. |
| V5 Input Validation | **yes (gate inputs)** | The gate parses `release.yml` and crate manifests. Both are trusted in-repo files, but the comment-stripping rule is a validation control: it prevents a *commented* publish step from counting as coverage |
| V6 Cryptography | no | `pmcp-package`'s no-crypto boundary is enforced by `make no-crypto-check`, unchanged by this phase — **do not add a crypto dep while touching that crate's manifest** |
| V14 Configuration | **yes** | Supply-chain: the version pins between `pmcp-package` and four consumers. A published `cargo-pmcp` resolving two semver-incompatible `pmcp-package` copies is the failure CLAUDE.md item 13's ORDERING CONSTRAINT describes |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| A crate silently never publishes | Denial (of the release) | The coverage gate — this phase's subject |
| Published CLI pinned to an unreadable package version | Tampering (semantic) | One-set version rule + D-10 order assertion + the two pin tripwires |
| Two semver-incompatible copies of `pmcp-package` in one binary | Tampering | CLAUDE.md item 13's constraint; verified consistent at `^0.3` this session |
| Dev-dep with `path` + `version` retained in a published manifest, publishing before its target | Denial | Path-only rule (CR-01), enforced by `pmcp_package_dev_dep_is_path_only`; four `mcp-tester` instances remain as accepted risk (D12) |
| Vacuously-green gate | Repudiation (of the check) | Empty-list-is-failure + explicit tool-failure arms + the D-02 red-direction self-test |
| Unrecoverable version consumption | — (safety, not security) | D-07's human checkpoints on merge and tag push |

## Sources

### Primary (HIGH confidence — measured in this working tree, 2026-08-26)

- `scripts/check-release-coverage.sh` (89 lines, read in full) — the gate, its four
  documented disciplines, and its self-declared Phase-124 blind spot
- `.github/workflows/release.yml` — publish steps and line numbers (`:98-102`, `:222`,
  `:339`, `:396`, `:426-439`, `:440`, `:445`, `:466`, `:484`, `:499`, `:520`, `:538`),
  `create-release` awk extraction (`:29-38`), tag trigger (`:4-6`)
- `Makefile` — `check-release-coverage` (`:892-895`), `quality-gate` chain,
  `pmcp-package-gate`, `test-openapi-server-guard-selftest` (`:592-593`),
  `no-crypto-allowlist-guard-selftest` (`:1375-1376`), `PURITY_NO_CRYPTO_CRATES` (`:1349`)
- `.github/workflows/ci.yml:215-218` — "Release-ledger coverage" step
- `Cargo.toml` — root `[workspace] members` / `exclude` (`crates/pmcp-package` excluded),
  `jsonwebtoken = { version = "11.0", optional = true }`
- `crates/pmcp-package/Cargo.toml` — own `[workspace]` (`:6`), `name` (`:9`),
  `publish = true` (`:20`), `version = "0.3.0"`
- `cargo-pmcp/tests/pmcp_package_pin.rs` — `EXPECTED_PIN = "0.3"`
- `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — `EXPECTED_VERSION_LINE = "0.3."`
  (`:89`), `EXPECTED_DEP_PATH = "../pmcp-package"` (`:92`), the CR-01 rationale and D12
- `cargo-pmcp/src/templates/agent.rs:67` — `const PMCP_PACKAGE_VERSION_REQ: &str = "0.3";`
- Command transcripts recorded inline above: gate run (exit 0, 24 members); doctored-gate
  run (exit 0 — the blind-spot proof); `git merge-tree` (exit 1, 9 conflicts);
  `git cat-file -p c64e2b2b` (single parent → squash); the D-05 sweep (3 base variants);
  `cargo metadata --manifest-path` timing (0.023s) and `publish=null` reading;
  `/bin/bash --version`; the D-01 scan prototype (1 hit)
- crates.io API v1 `/crates/<name>/versions` — 21 crates queried with a `User-Agent` header

### Secondary (MEDIUM confidence)

- `CLAUDE.md` "Release & Publish Workflow" — items 1–17, the ⚠ ORDERING CONSTRAINT under
  item 13, Version Bump Rules, Release Steps, Pre-Flight Checklist. Authoritative as
  project instruction; its *factual* claims about pin versions were cross-checked against
  the manifests (and its Pre-Flight `cargo search` step contradicts item 13 — Open Q4)
- `.planning/REQUIREMENTS.md` — PKGR-01 (`:37`) and its measured-drift correction (`:42-49`)
- `.planning/phases/124-release-publish-order/124-CONTEXT.md` — D-01..D-10
- `.planning/STATE.md` — milestone position, prior-phase decision log
- Project memory — rtk `grep`/`git diff` corruption; local-gate blind spot #9 (RUSTFLAGS);
  the v2.4 squash-divergence trap; nextest `test()` vs `binary()` selector hazard

### Tertiary (LOW confidence)

- The crates.io API's newest-first version ordering (A6) — consistent across 21 responses
  this session but not verified against documentation
- `publish-mcp` OIDC breakage still open (A4) — from memory, not re-tested

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — no new dependencies; every tool version measured locally
- Architecture: **HIGH** — all four patterns have in-repo precedents read this session, and
  Patterns 1 and 2 were prototyped and executed
- Pitfalls: **HIGH** — 1, 2, 3, 4, 6, 7 measured directly; 5 read from source; 8 from memory
- Version dispositions: **MEDIUM** — the *measurements* are HIGH, but the *disposition* of
  the six phantom deltas is a user decision that has not been made (Open Q1)
- D-03's patch axis: **MEDIUM** — grep supports it, `cargo public-api` has not confirmed it

**Research date:** 2026-08-26
**Valid until:** **3 days.** This is unusually short and deliberate: the registry state,
the branch divergence (241/1) and the merge-conflict set are all measured against a moving
`main`. Re-run the sweep and `git merge-tree` at execution time rather than trusting these
numbers.
