# The pinned MCP conformance suite

**READ THIS LIKE A SPEC, NOT LIKE CONFIG.** This directory is the external referee for CONF-01.
Section 7 below is *normative*: plans 118-04, 118-05 and 118-08 all apply it, and it is stated here
once so they cannot drift apart.

Pinned version: **`@modelcontextprotocol/conformance@0.2.0-alpha.11`**
(published 2026-08-07; upstream commit `c321dd32035556e6769d3724a8ee97d87c3faaac`).

---

## 1. What this directory is

`conformance/` pins the official `@modelcontextprotocol/conformance` CLI — the suite the MCP project
publishes to grade an implementation against a spec revision. It contains **no source code**. It is
CI tooling, so `conformance/` is listed in the root `Cargo.toml` `exclude` array and ships in no
crate; the exclusion was measured, not assumed (`cargo package -p pmcp --list --allow-dirty` went
from 517 to 514 entries when it was added, and the three removed entries were exactly this
directory's files).

Everything here is three files plus this document:

| File | Role |
|------|------|
| `package.json` | the exact pin, the Node floor, `private: true`, and deliberately **no** `scripts` block |
| `package-lock.json` | `lockfileVersion: 3` with the `sha512-` integrity hash that makes the install reproducible |
| `.npmrc` | `engine-strict=true` — the line that turns the Node floor from a warning into a refusal |

The `package.json` carries no `scripts` block on purpose: the real invocation lives in
`scripts/run-conformance-suite.sh` (plan 118-08) so a wiring test can pin the commands **as data**
rather than chase them through npm indirection.

## 2. Why a lockfile and not `npx`

`npx @modelcontextprotocol/conformance` resolves at run time. A yanked version, a re-published
tarball, or a moved dist-tag silently changes the referee — and a referee that changes without a
commit cannot be reviewed. A committed `package-lock.json` records a `resolved` URL **and** a
`sha512-` integrity hash, so `npm ci` either reproduces the exact bytes that were graded before or
fails loudly.

This is not hypothetical in this repo. The Purity Gate bit-rotted from exactly this failure mode:
an unpinned CLI plus a gitignored lockfile, drifting under a green check. Pin it or it rots.

## 3. Why this exact version and not `latest`

The `latest` dist-tag is the **0.1.16** line (2026-03-30). It ships **no** `requirements/` directory
and **no** `--requirements` flag, so the two-run shape this phase needs is impossible on it. The
`alpha` dist-tag is `0.2.0-alpha.11`, which is what is pinned here. Any re-pin must stay at
`0.2.0-alpha.11` or later for the same reason.

The pin is exact — no caret, no tilde. The in-repo analog
(`tests/integration/typescript-interop/package.json`) uses `^1.17.2`; a range is wrong here because
a range reintroduces exactly the drift the lockfile exists to remove.

## 4. Why Node >= 22, and why `.npmrc`

The suite imports `globSync` from `node:fs` at module scope, which lands in Node 22, and the package
declares **no** `engines` of its own. On Node 20 it fails at load with:

```
SyntaxError: The requested module 'fs' does not provide an export named 'globSync'
```

`engines.node` in `package.json` is only a declaration. Without `engine-strict`, npm prints an
`EBADENGINE` **warning** and installs anyway, so the wrong-Node install "succeeds" and the failure
surfaces later, at first invocation, as the error above. `engine-strict=true` in `conformance/.npmrc`
is what converts the declaration into an enforcement.

Proved by execution (plan 118-02 Task 1, negative control (c)) under Node v20.20.0:

```
npm error code EBADENGINE
npm error engine Unsupported engine
npm error notsup Not compatible with your version of node/npm: pmcp-conformance-suite@0.0.0
npm error notsup Required: {"node":">=22"}
npm error notsup Actual:   {"npm":"10.8.2","node":"v20.20.0"}
```

Exit status 1, and no `node_modules` tree was created. The same command under Node v22.22.2 exits 0.

## 5. Why `--ignore-scripts`

`npm ci` runs lifecycle scripts by default, and it runs them for the **whole dependency tree**, not
just the package you named. Confirming that the direct package's `scripts.postinstall` is empty is
therefore necessary but nowhere near sufficient. Every install in this repo is:

```bash
npm ci --prefix conformance --ignore-scripts
```

Disabling scripts is only a real mitigation if the tool still works afterwards, so that was proved
rather than assumed: after a scripts-disabled install, `conformance --version` prints
`0.2.0-alpha.11` and `conformance list` prints the full scenario inventory including both requirement
sets. If a future pin breaks under `--ignore-scripts`, that is a **finding** — audit every locked
package's `hasInstallScript` flag and record the audit before relaxing this.

## 6. What the two runs prove

One server process, two negotiations:

```bash
conformance server --url http://127.0.0.1:<port>/ \
  --requirements 2025-11-25 -o target/conformance-results/v1

conformance server --url http://127.0.0.1:<port>/ \
  --requirements 2026-07-28 -o target/conformance-results/v2
```

The milestone headline is "one pmcp server binary transparently serves both eras". A single v2 run
would leave the official referee silent about half of that claim, and two separate processes would
prove "pmcp can serve v1" and "pmcp can serve v2" as two independent facts — not that **one** binary
does both. One process also catches cross-era state bleed that isolated processes hide.

The two requirement sets overlap but are not nested. A scenario that appears in both must be run
once under each; neither run covers the other. Measured on this pin:

| Requirement set | Scenarios frozen | Server arm: scored | Server arm: run but not scored |
|-----------------|------------------|--------------------|-------------------------------|
| `2025-11-25` | 48 | 30 | 3 (`server-session-lifecycle`, `json-schema-2020-12`, `server-sse-polling`) |
| `2026-07-28` | 69 | 37 | 13 (10 `tasks-*` extension + 3 `pending`) |

`conformance list --server --requirements <revision>` prints the exact membership.

## 7. THE ZERO-CHECK POLICY (canonical — plans 118-04, 118-05 and 118-08 all apply this)

A scenario can report `0 passed, 0 failed` and still render as a green tick. A build that is green
because nothing was asserted is the defect this phase exists to prevent. The rule, in six parts:

1. **Zero-check scenarios exist.** Measured on this pin: `server-sse-polling` and
   `tasks-status-notifications` both report `0 passed, 0 failed` and render green. This is a
   property of the SUITE, not of pmcp. **Correction recorded at execution time (118-02):** on
   `0.2.0-alpha.11` neither of those two is *scored* — `conformance list --server --requirements
   2025-11-25` places `server-sse-polling` under `pending`, and `--requirements 2026-07-28` places
   `tasks-status-notifications` under `extension`. Both therefore sit outside D-03's scored boundary
   today. Nothing in the suite prevents a **scored** scenario from reporting zero checks, so the
   question stays open until it is measured, in both directions.
2. **A blanket ban is not the rule.** "No scored scenario may report zero checks" was the pre-review
   criterion in 118-04/118-05 and is replaced here. Two reasons: on this pin it is aimed at
   scenarios that are not scored at all, and if a scored scenario ever does report zero checks a
   blanket ban converts a suite property into a permanent red build with nowhere to record why.
3. **The rule is EXACT SET EQUALITY.** `scripts/run-conformance-suite.sh` (plan 118-08) carries a
   committed, closed list `ZERO_CHECK_SCENARIOS`, one entry per `<requirement-set>:<scenario>` pair
   with a one-line reason, populated from the measurements recorded by plans 118-04 and 118-05. The
   driver compares the OBSERVED set of zero-check scored scenarios against that list and fails on
   ANY difference. A new zero-check scenario means something stopped being exercised; a listed
   scenario that starts reporting checks means the list is stale. Both directions fail, exactly like
   the era baseline. An empty list is a legitimate — and the strongest — outcome.
4. **Plus a floor on total checks.** Independently of rule 3, the driver asserts the TOTAL executed
   check count per requirement set is at least a hard-coded floor: `MIN_CHECKS_V1` for
   `--requirements 2025-11-25` and `MIN_CHECKS_V2` for `--requirements 2026-07-28`. Both are
   recorded at pin time from the JSON the suite writes under `target/conformance-results/`. A floor
   is NEVER lowered; a re-pin either raises it or restates it, with the new measurement recorded in
   the re-pin commit.
5. **This is NOT a known-fail allowlist** and it does not violate D-03. Every entry on the list
   PASSES. The list records only *where the referee ran no assertions*, and because rule 3 is
   bidirectional it cannot be used to hide a regression — adding an entry to silence a failure is
   impossible, since a failing scenario is not a zero-check scenario.
6. **118-04 and 118-05 apply the same rule at measurement time.** A scored scenario reporting zero
   checks is recorded as a FINDING in that plan's SUMMARY unless it is one of the measured entries.
   The final list is AUTHORED in 118-08 from those recorded measurements — never invented.

## 8. What can and cannot fail the job

With `--requirements`, the suite exits 1 **iff a SCORED scenario has a FAILURE check**. Entries in
the "run and reported, but never scored" buckets — reasons `extension`, `pending`,
`added-after-release` — run, report, and cannot fail the job. That scored set is D-03's boundary,
per D-14. The Tasks extension (all 10 `tasks-*` scenarios) is NOT implemented in the target example,
which is why those are `extension` and why that is fine.

Results are written with `-o target/conformance-results/` and uploaded as a CI artifact, so
not-scored outcomes stay reviewable at re-pin time — that visibility is what the no-allowlist stance
in section 9 actually protects. The path lives under `target/` because `.gitignore` already ignores
`target/` and does **not** ignore a top-level `results` directory; writing there would dirty the
worktree and show up as spurious diff noise on every run.

## 9. What is FORBIDDEN

No `--expected-failures` baseline. No known-fail allowlist of any shape. If a scenario genuinely
does not apply to this SDK, that is a conversation at re-pin time — a reviewed commit with a stated
reason — not a standing exemption that accumulates silently.

## 10. THE TWO PINS, reconciled

This repo carries **two independent pins to the same upstream project**. Neither subsumes the other,
and a reader must not assume it does.

| Pin | Where | Value | What it fixes |
|-----|-------|-------|---------------|
| Repository pin | `tests/v2_conformance_pin.rs` (recorded in `113-SPEC-RECHECK.md` § B.1) | `a865118206d4d8cc8dbc5f5201607839281d0c3b` (2026-07-23) | HTTP-08's grading, which comes from a TypeScript predicate with no specification sentence behind it |
| Package pin | `conformance/package-lock.json` | `0.2.0-alpha.11`, built from upstream `c321dd32035556e6769d3724a8ee97d87c3faaac` (2026-08-07) | the referee binary this phase actually executes |

**Verdict: the two commits are NOT the same.** Measured with
`gh api repos/modelcontextprotocol/conformance/compare/a865118206d4d8cc8dbc5f5201607839281d0c3b...c321dd32035556e6769d3724a8ee97d87c3faaac`:
status `ahead`, `ahead_by: 14`, `behind_by: 0`. The package pin is therefore a **strict descendant**
of the repository pin — 14 commits newer, on the same line of history, with no divergence.

What that implies, and what to do:

- **The package is newer (today's case).** The predicate `113-SPEC-RECHECK.md` § B.1 quotes may have
  moved in the 14 commits this phase runs. Action: when a `request-metadata`-adjacent scenario
  changes verdict, re-read the predicate at `c321dd3…` before blaming pmcp, and refresh § B.1's sha
  and quoted predicate in the same commit that changes the outcome.
- **The commits are equal.** Nothing to do; note the agreement at re-pin time so a future reader
  knows it was checked rather than overlooked.
- **The package is OLDER, or the two have diverged (`behind_by > 0`).** Stop. The repository pin is
  grading against code the executed referee does not contain. Re-pin the package forward, or move
  the § B.1 pin back, but do not leave them crossed.

Finally: `--requirements <revision>` freezes which scenarios are **SCORED**, not their
**implementations**. `requirements/2026-07-28.yaml` is anchored at `0.2.0-alpha.10` upstream; the
scenario *code* still comes from whichever package version is installed, and its checks can change
between releases. That is precisely why the lockfile and the requirement set are complementary
rather than redundant — one freezes membership, the other freezes bytes.

## 11. How to re-pin

```bash
export PATH="$HOME/.nvm/versions/node/v22.22.2/bin:$PATH"   # any Node >= 22

npm view @modelcontextprotocol/conformance dist-tags --json  # pick the new version (>= 0.2.0-alpha.11)
npm view @modelcontextprotocol/conformance@<new> scripts.postinstall   # must print nothing
npm view @modelcontextprotocol/conformance@<new> gitHead               # feeds section 10
slopcheck install -e npm @modelcontextprotocol/conformance             # must report "1 OK"

npm install --prefix conformance --save-exact --ignore-scripts \
  @modelcontextprotocol/conformance@<new>

rm -rf conformance/node_modules
npm ci --prefix conformance --ignore-scripts        # must reproduce from the lockfile
make test-conformance                               # run both requirement sets locally
```

The `-e npm` on `slopcheck` is mandatory: without it, auto-detection picks crates.io and reports a
spurious `registry unreachable`.

Then, in the **same commit** as the version bump:

1. update the pinned version and `gitHead` in sections 1, 3 and 10 of this file;
2. re-run the § 10 `gh api ... compare` and restate the verdict;
3. re-derive `ZERO_CHECK_SCENARIOS` and both check floors from the fresh run;
4. review the diff in `target/conformance-results/` — a re-pin that changes not-scored outcomes is
   exactly the thing that artifact exists to make visible.

## 12. How to run it locally

`make test-conformance` (plan 118-08) is the single local spelling. Do not memorise the raw CLI
invocation; the Makefile target and `scripts/run-conformance-suite.sh` are the reviewed copy.

The **blocking** enforcement lives in `.github/workflows/ci.yml`, not in the Makefile. A green
`make test-conformance` on a laptop is evidence, not a gate.
