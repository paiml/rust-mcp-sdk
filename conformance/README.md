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

### 8a. What the blocking gate CLAIMS (widened by Phase 118.1 plan 14)

The repository's claim must be exactly the claim it can defend. Measured on this pin, from one
process, five full runs across plans 118.1-13 and 118.1-14 with identical results:

| Requirement set | Passed / failed | Checks executed | Scored scenarios | Scored failing | Suite exit |
|-----------------|-----------------|-----------------|------------------|----------------|------------|
| `2025-11-25` | 72 / 2 | 74 | 30 | **1** | 1 |
| `2026-07-28` | 142 / 36 | 178 | 37 | **0** | **0** |

`scripts/run-conformance-suite.sh` therefore blocks on all of the following, and nothing weaker:

1. **`2026-07-28`: the ENTIRE scored set green, and the suite's own exit status 0.** Universally
   quantified over the scored set (`FULLY_SCORED_GREEN_REVISIONS`), so no entry can be deleted from
   it to turn a red run green. Guarded against vacuity by a floor on the scored-scenario count.
2. **`2025-11-25`: 29 named scenarios present and entirely green** (`BLOCKING_GREEN_SCENARIOS`,
   floor `MIN_BLOCKING_GREEN_SCENARIOS`). These are 29 of that leg's 30 scored scenarios. This is an
   **inclusion list of claims, not a known-fail allowlist** — adding an entry can only make the gate
   stricter, and no entry can ever be added to silence a failure, because a failing scenario cannot
   satisfy "entirely green". Only deletion could weaken it, which is what the floor closes.
3. The MRTR surface, the two check floors, and both zero-check set equalities, exactly as before.

**Why `2025-11-25` is not on list 1.** Its 30th scored scenario, `tools-call-with-logging`, FAILS:
the SDK has no handler-facing emitter for `notifications/message`. That is the last of the nine
gaps, it is owned by **Phase 118.2**, and it is stated here and printed by the script on every run
rather than being tolerated anywhere. When it lands, move `2025-11-25` into
`FULLY_SCORED_GREEN_REVISIONS` and delete its now-redundant entries from `BLOCKING_GREEN_SCENARIOS`.

**No pass-count is asserted, deliberately.** Plan 118.1-13 measured the `2026-07-28` total
oscillating 141↔142 at roughly 3 failures per 14 fresh server processes. The cause is
`ServerAcceptsWhitespaceHeaderValue`, which calls an *arbitrary* tool from `tools/list` with no
arguments and requires a non-error response — so any tool that legitimately rejects an argument-free
call fails it. It lives in `http-header-validation`, which is **not scored** at `2026-07-28`, so it
cannot disturb clause 1. A pass-count floor would have imported that flake straight into a blocking
gate. The same measurement **refuted** a suspected RFC 9110 OWS-trimming defect: in 14 of 14 fresh
processes the server trimmed the whitespace and routed correctly.

## 9. What is FORBIDDEN

No `--expected-failures` baseline. No known-fail allowlist of any shape. If a scenario genuinely
does not apply to this SDK, that is a conversation at re-pin time — a reviewed commit with a stated
reason — not a standing exemption that accumulates silently.

**How this is ENFORCED, and how it must NOT be.** Do not add a raw
`grep -rn -- '--expected-failures' scripts/ .github/workflows/` and expect it to come back empty:
it matches **on the clean tree**, because the token appears in the comments that FORBID it — today
at `scripts/run-conformance-suite.sh:38`, `.github/workflows/ci.yml:586` and line 178 of this file.
A fence satisfied by the documentation explaining it is a fence that can only ever fail, and a
guaranteed-failing check gets deleted, taking the real one with it.

The enforcement is structural and already exists:
`tests/ci_conformance_gate_wiring.rs::no_known_fail_allowlist_reaches_a_conformance_command` strips
`#` comment lines with `commands_only`, asserts NON-VACUITY (the stripped view still contains
`--requirements`), and only then asserts that every entry of `CONFORMANCE_FORBIDDEN_FLAGS`
(`--expected-failures`, `--spec-version`, `--suite`, `--all-features`) is absent from the COMMANDS.
`no_status_masking_reaches_an_era_matrix_command` does the same for the era-matrix script over
`|| true`, `|| :` and `continue-on-error`. If a NEW suppression shape appears, add it to those
constants — never add a second grep.

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

> **The `compare` command above stopped working on 2026-08-17** and a substitute is recorded here so
> a future re-pin is not blocked by it. It now returns `404 Not Found` for these two SHAs. That is
> **not** a statement about the pins: both SHAs still resolve individually via
> `gh api repos/modelcontextprotocol/conformance/commits/<sha>`, the repo is public and unarchived,
> and `compare/main...main` on the *same* repo returns `identical` — so it is neither an access nor a
> reachability problem. The same 404 reproduces on an unrelated public repo, i.e. it is upstream of
> this project. Re-derive the verdict from the ancestry listing instead, which needs no compare:
>
> ```bash
> gh api "repos/modelcontextprotocol/conformance/commits?sha=<package-gitHead>&per_page=30" \
>   --jq '.[].sha' | grep -n '<repository-pin-sha>'
> ```
>
> The 1-based line number *N* it prints is the repository pin's position walking back from the
> package pin, so the package is `N - 1` commits ahead; the pin is **crossed** only if the SHA does
> not appear at all (then run the same query in the other direction before concluding). Measured
> 2026-08-17: the repository pin appears at line **15**, i.e. `ahead_by: 14`, `behind_by: 0` —
> identical to the compare endpoint's last working answer.

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

## 13. Re-pin review log

Section 11 says how to re-pin. This section records **when the question was last asked and what
the answer was**, because "we checked and there was nothing to move to" and "nobody looked" are
different facts that a silent, unchanged pin cannot tell apart.

### 2026-08-11 — reviewed, HELD at `0.2.0-alpha.11`. There is no newer release.

Phase 118.1 plan 14 ran section 11's investigation steps and stopped before its install steps,
because the investigation returned no target:

```
$ npm view @modelcontextprotocol/conformance dist-tags --json
{ "latest": "0.1.16", "alpha": "0.2.0-alpha.11" }
```

- The `alpha` dist-tag points at **the version already pinned**.
- `npm view … versions --json` ends at `0.2.0-alpha.11`; there is nothing after it.
- The registry's own `modified` timestamp is `2026-08-07T14:01:04.026Z`, equal to that version's
  publish time to the second, so nothing has been published since.
- `latest` is the `0.1.x` line, which section 3 rules out: it ships no `requirements/` directory
  and no `--requirements` flag, so pinning to it would invalidate the scored-set methodology this
  whole gate rests on. Section 3's floor — "any re-pin must stay at `0.2.0-alpha.11` or later" —
  leaves exactly one admissible version, and it is the one already here.

**Legitimacy evidence, recorded even though the pin did not move:**

| Check | Result |
|---|---|
| `scripts.postinstall` | empty — prints nothing |
| `gitHead` | `c321dd32035556e6769d3724a8ee97d87c3faaac`, unchanged from sections 1 and 10 |
| `slopcheck install -e npm` | `1 OK` |
| publisher | `GitHub Actions <npm-oidc-no-reply@github.com>`, npm **trusted publisher** OIDC — not a personal token |
| provenance | SLSA v1 attestation plus a registry signature |
| maintainers | the `modelcontextprotocol` org, including two `@anthropic.com` addresses |
| cadence | `alpha.10` 2026-07-27 → `alpha.11` 2026-08-07 — regular, no unexplained gap |
| `dist.integrity` | `sha512-imPK9tx5gQsL6ZKQq4MrsyDYfSaIwpRmX6+ogjbeAXs9LGvxkBxWcY7KcS7TvwaBk/ZiVWl6b/naF4q83UwDRA==`, **byte-identical to `package-lock.json`** |

No takeover indicator on any of them.

**Section 11's reproduction proof was still run**, because it is worth proving whether or not a
version moved: `rm -rf conformance/node_modules` then `npm ci --prefix conformance
--ignore-scripts` reinstalled from the committed lockfile with `package.json` md5
`40950a081b77e054dba9a5006e5d87e0` and `package-lock.json` md5
`d1ce39fff5939230dd42ecd23668d31b` **unchanged before and after**, and the installed CLI reports
`0.2.0-alpha.11` and its full scenario inventory under `--ignore-scripts` (section 5).
`npm install --save-exact` was deliberately NOT run: it is the command that would write a new pin.

**Section 10's comparison was re-run and its verdict is unchanged:**
`gh api repos/modelcontextprotocol/conformance/compare/a865118206d4d8cc8dbc5f5201607839281d0c3b...c321dd32035556e6769d3724a8ee97d87c3faaac`
→ `status: ahead`, `ahead_by: 14`, `behind_by: 0`. The package pin remains a strict descendant of
the repository pin, so the two are not crossed and nothing needs moving.

**The consequence for attribution is a strengthening, not a weakness.** D-08 held this pin all
phase so that every measured delta would be attributable to the SDK. With no bump available, the
bump delta is **nil by construction** and the fix delta is therefore the *whole* delta — there is
no suite-version confound to separate out at all.

**Re-extraction was still performed.** Every check name, probe body and expected code quoted in
the 118.1 research is pinned to this version, so an unchanged pin cannot have moved them — but
that is an argument, not a measurement. All 16 named expectations were looked up in the installed
bundle (md5 `f3c6b1db650114b62456ef6dac028a3c`, 809 888 bytes) and every one returned a nonzero
hit count, so no in-tree test encoding them needed updating.

**When to ask again:** whenever `npm view @modelcontextprotocol/conformance dist-tags --json`
reports an `alpha` (or a `>= 0.2.0` `latest`) newer than what section 1 names. Then follow section
11 in full and add the outcome here.

### 2026-08-17 — reviewed again, HELD at `0.2.0-alpha.11`. Still no newer release.

Phase 118.2 plan 12 asked the question a second time, six days after the entry above, because D-14
requires the pin to be moved to whatever is newest as the **final act of the phase** — so that the
bump's delta can be reported separately from the delta produced by the phase's SDK fixes. The
investigation returned no target, so the two deltas are not merely separable, they are separated by
construction: **the bump delta is nil.**

```
$ npm view @modelcontextprotocol/conformance dist-tags --json
{ "latest": "0.1.16", "alpha": "0.2.0-alpha.11" }

$ npm view @modelcontextprotocol/conformance versions --json   # tail
… "0.2.0-alpha.9", "0.2.0-alpha.10", "0.2.0-alpha.11" ]
```

- The `alpha` dist-tag still points at **the version already pinned**, and the version list still
  ends there — nothing has been published after it.
- The registry's `modified` timestamp is still `2026-08-07T14:01:04.026Z`, equal to `alpha.11`'s
  publish time (`2026-08-07T14:01:03.583Z`) to the second.
- `latest` is still the `0.1.x` line, which section 3 rules out. Note that `npm view … version`
  reports `0.1.16` because it reads `dist-tags.latest`; that is **older** than the pin, and taking
  it would be a downgrade, not a re-pin. Section 3's floor — "at `0.2.0-alpha.11` or later" — still
  leaves exactly one admissible version, and it is the one already here.

**Legitimacy evidence, re-run rather than copied forward** (every value identical to 2026-08-11):

| Check | Result |
|---|---|
| `scripts.postinstall` | empty — prints nothing |
| `gitHead` | `c321dd32035556e6769d3724a8ee97d87c3faaac`, unchanged |
| `slopcheck install -e npm` | `1 OK`, exit 0 |
| publisher | `GitHub Actions <npm-oidc-no-reply@github.com>`, npm **trusted publisher** OIDC (`oidcConfigId oidc:a5486a44-…`) — not a personal token |
| maintainers | the `modelcontextprotocol` org; three `@anthropic.com` and two `@modelcontextprotocol.io` addresses |
| `dist.integrity` | `sha512-imPK9tx5gQsL6ZKQq4MrsyDYfSaIwpRmX6+ogjbeAXs9LGvxkBxWcY7KcS7TvwaBk/ZiVWl6b/naF4q83UwDRA==`, **byte-identical to `package-lock.json` line 32** |

**Run `slopcheck` from a neutral directory.** It literally executes `npm install <pkg>` in the
current working directory, and `npm install` walks *up* the tree for a manifest. Invoked from this
repository — which has no root `package.json` — it reached `~/package.json` and audited that
project's 1 927 packages instead, printing npm's output and no `1 OK` verdict. Nothing was modified
(that manifest already listed the package and its mtime did not change), but the check silently
measured the wrong thing. `cd /tmp && slopcheck install -e npm …` gives the real answer.

**Reproduction proof, again produced by execution rather than argument.** The measurement run below
performs section 11's `npm ci --prefix conformance --ignore-scripts` itself, and it reinstalled 117
packages from the committed lockfile with **both manifests byte-unchanged** —
`package.json` md5 `40950a081b77e054dba9a5006e5d87e0` and `package-lock.json` md5
`d1ce39fff5939230dd42ecd23668d31b`, the same two digests recorded on 2026-08-11.
`git status --short conformance/` is clean. `npm install --save-exact` was again deliberately NOT
run — it is the command that would write a new pin.

**Section 10's comparison was re-run and its verdict is unchanged** (`ahead_by: 14`,
`behind_by: 0`), but the prescribed `gh api … compare` command now 404s and the verdict had to be
re-derived another way. See the note in section 10 for the diagnosis and the substitute command;
this is a change in the GitHub API's behaviour, not in either pin.

**The measurement.** One run, at the unchanged pin, against the gate as plan `118.2-11` hardened it
(`target/118.2-12-conf-newpin.log`): `2025-11-25` 73 passed / 1 failed, 74 checks, 30 scored, 0
scored failures, suite exit 0; `2026-07-28` 142 passed / 36 failed, 178 checks, 37 scored, 0 scored
failures, suite exit 0; `CONF-01 gates PASSED`, script exit 0. Every number is identical to the
held-pin measurement, which is the expected consequence of an unchanged pin and is recorded because
"identical" and "not re-measured" are different facts.

**Signed off:** 2026-08-17, developer **approved** the outcome at plan `118.2-12`'s blocking
checkpoint — the pin stays at `0.2.0-alpha.11` because it is already the newest published version,
and the bump delta stands recorded as nil. Approving a *nil* bump is not a formality: it is the
record that the question was asked and answered against the registry on this date, which is what
distinguishes "we checked and there was nothing to move to" from "nobody looked". The approval
covers the held pin and the nil delta **only** — it closes no defect. Three findings were explicitly
kept open at sign-off and are not resolved by these green numbers: the `2025-11-25:tools-call-elicitation`
flake (`.planning/WINDOWS.md` entry 9, **not** claimed fixed by further green runs),
`ServerAcceptsWhitespaceHeaderValue` (still **unscored**, exposure unchanged, nothing pre-emptively
softened for it per section 9), and SEP-2575 on v2 plus the client request-lifecycle deadlock
(`.planning/WINDOWS.md` entries 5 and 6). No floor moved, and section 9's prohibitions were neither
weakened nor exempted.

**When to ask again:** unchanged from the entry above.
