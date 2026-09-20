---
phase: 118-conformance-against-the-official-suite
plan: 02
subsystem: conformance-tooling
tags: [conformance, npm-pin, packaging, ci, supply-chain]
requires: []
provides:
  - "conformance/ pinned @modelcontextprotocol/conformance@0.2.0-alpha.11 (lockfile + engine-strict)"
  - "the canonical zero-check policy (conformance/README.md § 7) for plans 118-04, 118-05, 118-08"
  - "the two-pin reconciliation (conformance/README.md § 10) for plan 118-09's wiring test"
  - "Cargo.toml exclude entries for conformance/ and tests/ci_conformance_gate_wiring.rs"
affects:
  - "118-04, 118-05 (apply README § 7 at measurement time)"
  - "118-08 (authors ZERO_CHECK_SCENARIOS, MIN_CHECKS_V1, MIN_CHECKS_V2)"
  - "118-09 (tests/ci_conformance_gate_wiring.rs reads conformance/package-lock.json + README.md)"
tech-stack:
  added:
    - "@modelcontextprotocol/conformance@0.2.0-alpha.11 (npm, dev/CI tooling only, never a crate dep)"
  patterns:
    - "exact npm pin + committed lockfile + engine-strict, mirroring tests/integration/typescript-interop/"
    - "a runtime reader shares the packaging disposition of the path it reads"
key-files:
  created:
    - conformance/package.json
    - conformance/package-lock.json
    - conformance/.npmrc
    - conformance/README.md
  modified:
    - Cargo.toml
decisions:
  - "Pinned 0.2.0-alpha.11 (newest alpha, unchanged from RESEARCH) — latest/0.1.16 has no --requirements flag"
  - "engine-strict=true in conformance/.npmrc, because engines.node alone only WARNS"
  - "Both conformance/ and its future reader excluded from the tarball — ship both or exclude both"
  - "Zero-check policy is EXACT SET EQUALITY plus per-requirement-set check floors, not a blanket ban"
metrics:
  tasks: 3
  commits: 3
  duration_minutes: 95
  completed: 2026-08-09
---

# Phase 118 Plan 02: Pin the Official Conformance Suite Summary

Pinned `@modelcontextprotocol/conformance@0.2.0-alpha.11` with a committed lockfile and an
`engine-strict` Node-22 floor proved by a failing Node-20 install, excluded the manifest and its
future reader from the crates.io tarball, and wrote the single normative statement of the zero-check
policy and the two-pin reconciliation.

## Commits

| Task | Commit | Files |
|------|--------|-------|
| 1 — pin the suite, enforce the Node floor | `341ef604` | `conformance/package.json`, `conformance/package-lock.json`, `conformance/.npmrc` |
| 2 — packaging disposition | `bbfb6acf` | `Cargo.toml` |
| 3 — the README | `0b659f65` | `conformance/README.md` |

## Task 1 — the five legitimacy re-checks, verbatim

Re-checked before use, not taken from RESEARCH on faith. All five agree with RESEARCH; the pin is
unchanged at `0.2.0-alpha.11`.

**1. dist-tags** (`npm view @modelcontextprotocol/conformance dist-tags --json`):

```json
{
  "latest": "0.1.16",
  "alpha": "0.2.0-alpha.11"
}
```

`npm view ... time --json` confirms `0.2.0-alpha.11` was published `2026-08-07T14:01:03.583Z` and is
the newest version overall (the `0.2.0-alpha.*` line runs alpha.0 through alpha.11; nothing newer
exists on either tag). **Chosen pin: `0.2.0-alpha.11` — the same value RESEARCH recorded**, so no
deviation to explain. `latest` remains the 0.1.16 line (2026-03-30), which has no `--requirements`
flag, so D-04 is impossible on it.

**2. `--requirements` support** — `conformance list` on the installed binary reports both requirement
sets. Measured membership (recorded here because 118-04/118-05/118-08 need it):

| Requirement set | Scenarios frozen | Server scored | Server run-but-not-scored |
|-----------------|------------------|---------------|---------------------------|
| `2025-11-25` | 48 | 30 | 3 — `server-session-lifecycle` (added-after-release), `json-schema-2020-12` (pending), `server-sse-polling` (pending) |
| `2026-07-28` | 69 | 37 | 13 — 10 `tasks-*` (extension), `json-schema-2020-12` / `http-header-validation` / `http-custom-header-server-validation` (pending) |

Server-arm totals 33 and 50 reproduce RESEARCH's measured figures exactly.

**3. slopcheck** (`slopcheck install -e npm @modelcontextprotocol/conformance`):

```
  [OK] @modelcontextprotocol/conformance (npm)

==================================================
  scanned 1 packages
  1 OK
```

The `-e npm` is mandatory as RESEARCH warned; the check exits 0.

**4. `scripts.postinstall`** — `npm view @modelcontextprotocol/conformance@0.2.0-alpha.11
scripts.postinstall` prints **nothing** (empty). The full `scripts` object contains only author-side
entries (`build`, `test`, `lint`, `prepack`, `prepare`, …); `prepare` runs only for git/local
installs, never for a registry tarball, and `--ignore-scripts` covers it regardless.

**5. Upstream repository and commit** — `gitHead` is **present**, so no fallback to a release tag was
needed:

```
repository: git+https://github.com/modelcontextprotocol/conformance.git
gitHead:    c321dd32035556e6769d3724a8ee97d87c3faaac
```

That commit is upstream `chore: bump version to 0.2.0-alpha.11 (#448)`, dated `2026-08-07T13:58:45Z`.

Supply-chain extras observed while checking: the published tarball carries an npm provenance
attestation (`https://slsa.dev/provenance/v1`) and a registry signature, and the lockfile records
`integrity: sha512-imPK9tx5gQsL6ZKQq4MrsyDYfSaIwpRmX6+ogjbeAXs9LGvxkBxWcY7KcS7TvwaBk/ZiVWl6b/naF4q83UwDRA==`.

## Task 1 — the Node-20 refusal transcript (negative control (c))

`rm -rf conformance/node_modules` then, with Node 20 first on `PATH`:

```
== node/npm under control ==
v20.20.0
10.8.2
== npm ci --prefix conformance --ignore-scripts (Node 20) ==
npm error code EBADENGINE
npm error engine Unsupported engine
npm error engine Not compatible with your version of node/npm: pmcp-conformance-suite@0.0.0
npm error notsup Not compatible with your version of node/npm: pmcp-conformance-suite@0.0.0
npm error notsup Required: {"node":">=22"}
npm error notsup Actual:   {"npm":"10.8.2","node":"v20.20.0"}
NODE20_CI_EXIT=1
== node_modules present? ==
no node_modules (install refused)
```

The refusal is real, not a warning, and no tree was created. This also settles an open question about
the mechanism: `npm ci --prefix conformance` **does** read `conformance/.npmrc` — the `--prefix` sets
the local prefix that npm resolves the project config from. Under Node v22.22.2 the identical command
exits 0 and installs 117 packages.

For completeness, the failure the `.npmrc` prevents was also reproduced directly by running the
installed binary under Node 20:

```
SyntaxError: The requested module 'fs' does not provide an export named 'globSync'
    at ModuleJob._instantiate (node:internal/modules/esm/module_job:213:21)
Node.js v20.20.0
```

## Task 1 — `conformance list` after a `--ignore-scripts` install (evidence `--ignore-scripts` is viable)

After `rm -rf conformance/node_modules && npm ci --prefix conformance --ignore-scripts` under Node
v22.22.2:

```
== conformance --version ==
0.2.0-alpha.11
version-exit=0
== conformance list ==
Server scenarios (test against a server):
  - server-initialize [2025-06-18,2025-11-25]
  - server-session-lifecycle [2025-03-26,2025-06-18,2025-11-25]
  - server-stateless [2026-07-28]
  ...
  - tasks-status-notifications [extension]
  ...
Client scenarios (test against a client):
  - initialize [2025-06-18,2025-11-25]
  ...
list-exit=0
```

Both requirement sets (`2025-11-25`, `2026-07-28`) appear in the bracketed applicability lists, and
`conformance list --server --requirements <rev>` works for both. No fallback to a scripts-enabled
`npm ci` was needed, so no `hasInstallScript` audit was required.

Other Task-1 acceptance evidence:

| Check | Result |
|-------|--------|
| `npm ci --prefix conformance --ignore-scripts` from clean `node_modules` | exit 0 |
| `test -x conformance/node_modules/.bin/conformance` | pass |
| `jq -e` installed version == manifest pin | `true`, exit 0 |
| `npm ls --prefix conformance @modelcontextprotocol/conformance` | `@modelcontextprotocol/conformance@0.2.0-alpha.11` |
| pin string contains `^`/`~`/`>`/`*` | no — bare `0.2.0-alpha.11` |
| `jq -r '.engines.node'` | `>=22` |
| `jq -r '.private'` | `true` |
| `grep -c 'engine-strict=true' conformance/.npmrc` | `1` |
| lockfile `integrity` | starts `sha512-` |
| `git status --porcelain conformance/node_modules` | empty (never staged) |

## Task 2 — packaging disposition

`cargo package -p pmcp --list --allow-dirty` was captured to a file (the `>` redirect preserves
cargo's exit status) and only then asserted against, per D-19.

| Measurement | Value |
|-------------|-------|
| Entries BEFORE the exclude edit | **517**, of which **3** matched `^conformance/` (`.npmrc`, `package.json`, `package-lock.json`) |
| Entries AFTER the exclude edit | **514** |
| `grep -cE '^conformance/'` after | `0` |
| `grep -c 'ci_conformance_gate_wiring'` after | `0` |
| `cargo package` exit status | `0` on both runs |

The 517 → 514 delta is exactly the three files, so no surprise mass-exclusion occurred. The
pre-change count is not a hypothetical: **without this task the Node manifest would have shipped to
every downstream consumer**, which is precisely the `115-REVIEW.md` CR-01 failure.

Other Task-2 verdicts:

- **`make purity-check` exits 0** with the Node manifest in the tree. Full run: Layer 1 clean,
  Phase-92/93/95 cones clean, Layer 2 `cargo-deny [bans] ok`, final line
  `purity-check PASSED`.
- **`course.yml` `package-content` is unaffected.** `grep -n 'pmcp-course' .github/workflows/course.yml`
  plus reading the job body confirms the tar globs are exactly `pmcp-course/src/` and
  `pmcp-course/metadata.toml`, with two `--transform` rules scoped to `pmcp-course/`. `conformance/`
  is outside those globs; nothing to change.
- The inline comment contains the literal `ship both or exclude both` and names both rejected
  dispositions (`ci_severance_gate_wiring.rs`, `v2_conformance_pin.rs`) with the reason each was
  rejected.

## Task 3 — README verification

| Acceptance check | Result |
|------------------|--------|
| Exists, ≥ 60 lines | **250 lines** |
| Plan's `<automated>` conjunction | exit 0 |
| Required literals | all present: `--requirements 2025-11-25` (2), `--requirements 2026-07-28` (3), `globSync` (2), `--expected-failures` (1), `slopcheck install -e npm` (1), `engine-strict` (3), `--ignore-scripts` (5), `ZERO_CHECK_SCENARIOS` (2), `MIN_CHECKS_V1` (1), `MIN_CHECKS_V2` (1), `target/conformance-results` (5), `a865118206d4d8cc8dbc5f5201607839281d0c3b` (2) |
| § 7 fragments | `EXACT SET EQUALITY` (1), `is NOT a known-fail allowlist` (1); all six rules numbered |
| Pinned version matches the manifest | verified BY COMMAND, not by eye: `PIN=$(jq -r '.dependencies["@modelcontextprotocol/conformance"]' conformance/package.json); grep -qF -- "$PIN" conformance/README.md` → exit 0 (`PIN=0.2.0-alpha.11`, 7 occurrences) |
| `results/` audit | 5 occurrences total, **all** inside `target/conformance-results/`; `grep -n 'results/' … \| grep -v 'target/conformance-results/'` returns nothing. Audited lines: 103, 106, 151, 170, 241. No bare `results/` path is recommended anywhere — § 8 makes the point using the words "a top-level `results` directory" precisely so the audit stays clean |
| `make quality-gate` | **exit 0** (final line `ALL TOYOTA WAY QUALITY CHECKS PASSED`), which includes `make check-todos` → `✓ No technical debt comments` |

### § 10 verdict — do the two pinned commits agree?

**No — and the answer is a measurement, not an assumption.**

```
gh api repos/modelcontextprotocol/conformance/compare/\
a865118206d4d8cc8dbc5f5201607839281d0c3b...c321dd32035556e6769d3724a8ee97d87c3faaac
→ {"status":"ahead","ahead_by":14,"behind_by":0,"total_commits":14}
```

The npm package pin (`c321dd3…`, 2026-08-07) is a **strict descendant** of the Phase-113 repository
pin (`a865118…`, 2026-07-23): 14 commits newer, zero behind, no divergence. The 14 commits include
scenario fixes (`server-stateless` elicitation capability, `sse-multiple-streams` negotiated-version
POSTs, `everything-server` sampling routing) — i.e. the predicate `113-SPEC-RECHECK.md` § B.1 quotes
sits in a tree that has moved. README § 10 records the verdict and gives the reader a concrete action
for each of the three possible relations (package newer / equal / older-or-diverged).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The plan's `git check-ignore -q` acceptance command cannot produce a verdict**

- **Found during:** Task 1 acceptance.
- **Issue:** `git check-ignore -q conformance/package.json conformance/package-lock.json conformance/.npmrc`
  exits **128** with `fatal: --quiet is only valid with a single pathname`. The plan reads a non-zero
  exit as "nothing matched", so a hard error would have been recorded as a PASS — the same D-19 class
  of false-green the plan was written to eliminate, reproduced inside its own acceptance criteria.
- **Fix:** ran `git check-ignore -q` **per file** and interpreted the tri-state (0 = ignored/BAD,
  1 = not ignored/good, other = error). Result: all three files report `1`, i.e. not ignored.
- **Files modified:** none (verification method only).
- **Commit:** n/a — the corrected method is recorded here so 118-04/05/08 do not copy the broken form.

**2. [Rule 1 — Bug] The `.npmrc` comment made its own acceptance criterion fail**

- **Found during:** Task 1 step 2.
- **Issue:** the first draft explained the setting using the literal string `engine-strict=true` in
  prose, so `grep -c 'engine-strict=true' conformance/.npmrc` returned `2` where the criterion
  requires `1`.
- **Fix:** reworded the comment to say "engine-strict" without the `=true` suffix, leaving exactly one
  literal occurrence — the directive itself.
- **Commit:** `341ef604`.

**3. [Rule 1 — Bug] The plan's zero-check premise is contradicted by the pinned suite**

- **Found during:** Task 3 § 7, while measuring rather than restating.
- **Issue:** the plan instructs § 7 rule 1 to state that "some **scored** scenarios report
  `0 passed, 0 failed`", naming `server-sse-polling` and `tasks-status-notifications`, and rule 2 to
  declare a blanket no-zero-check rule **UNSATISFIABLE**. On `0.2.0-alpha.11` neither scenario is
  scored: `conformance list --server --requirements 2025-11-25` puts `server-sse-polling` under
  `pending`, and `--requirements 2026-07-28` puts `tasks-status-notifications` under `extension`.
  Both therefore sit outside D-03's scored boundary, and writing "UNSATISFIABLE" into the phase's
  normative document would have made the canonical policy open with a false statement.
- **Fix:** § 7 rule 1 states the measurement and labels the correction explicitly; rule 2 replaces the
  blanket ban for two stated reasons (it is aimed at scenarios that are not scored, and a blanket ban
  leaves nowhere to record a legitimate suite property) rather than by asserting impossibility. The
  rule itself — EXACT SET EQUALITY plus check floors, bidirectional — is **unchanged**, because
  nothing in the suite prevents a scored scenario from reporting zero checks; only 118-04/118-05's
  measurements can settle that. § 7 rule 3 now also states that an empty `ZERO_CHECK_SCENARIOS` list
  is a legitimate and indeed the strongest outcome.
- **Commit:** `0b659f65`.

### Environment (not a code deviation)

**Host disk exhaustion (ENOSPC) failed `make quality-gate` twice before it passed.**
The volume hit 93% with ~1.0 GiB free while four sibling worktree agents each built a full workspace.
Runs 1 and 2 died with `ld: write() failed, errno=28`, `rustc-LLVM ERROR: IO failure on output
stream`, and `failed to create work product index … (os error 28)` — all of which read as compiler
errors. This is the failure mode recorded in the `project_disk_exhaustion_fake_test_failures` ledger
entry: check `df -h /` before believing a build regression. Resolved without touching any shared
artifact: removed this worktree's own `target/debug` (gitignored build output; never `git clean`,
never the main checkout's 74 GB `target/`) and re-ran with `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`. Neither setting affects whether a check
passes; both cut the disk footprint enough for the gate to complete. Run 3: **exit 0**.

### Authentication gates

None.

## Threat Flags

None. The three files added are declarative configuration plus prose; they open no network endpoint,
no auth path and no schema at a trust boundary. The `Cargo.toml` change strictly *narrows* what
leaves the repository, and T-118-08's mitigation is now measured rather than asserted.

## Notes for downstream plans

- **118-04 / 118-05:** apply `conformance/README.md` § 7 verbatim. Record, per requirement set, the
  zero-check scored scenarios you actually observe **and** the total executed check count — 118-08
  needs both, and § 7 rule 6 forbids inventing either. Do not copy the plan text's "no scored
  scenario may report zero checks" criterion; it was replaced (deviation 3 above).
- **118-08:** `MIN_CHECKS_V1` / `MIN_CHECKS_V2` are the names § 7 rule 4 fixes; the scored-set sizes
  measured here (30 server scored at 2025-11-25, 37 at 2026-07-28) are the membership those floors sit
  under. Write results to `target/conformance-results/` — `.gitignore` covers `target/` and does not
  cover a top-level `results` directory.
- **118-09:** `tests/ci_conformance_gate_wiring.rs` is **already excluded** in `Cargo.toml`; do not
  re-add it, and do not make it path-tolerant. Its § 10 assertion has a literal to bind to:
  `a865118206d4d8cc8dbc5f5201607839281d0c3b` appears twice in `conformance/README.md`.
- **Any re-pin:** `conformance/README.md` § 11 is the procedure, and § 10 must be re-measured, not
  copied forward.

## Self-Check: PASSED

Created files verified present on disk:

- `conformance/package.json` — FOUND
- `conformance/package-lock.json` — FOUND
- `conformance/.npmrc` — FOUND
- `conformance/README.md` — FOUND

Commits verified present in `git log`:

- `341ef604` — FOUND
- `bbfb6acf` — FOUND
- `0b659f65` — FOUND
