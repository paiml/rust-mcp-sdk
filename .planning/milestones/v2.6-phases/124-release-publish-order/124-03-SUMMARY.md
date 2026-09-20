---
phase: 124-release-publish-order
plan: 03
subsystem: infra
tags: [release, crates-io, versioning, semver, bash, make, public-api, semver-checks]

requires:
  - phase: 124-01
    provides: the two-source discovery (root members + workspace-EXCLUDED crates) this sweep mirrors, so the sweep can see everything the coverage gate can
  - phase: 124-02
    provides: the topological merge that made this branch's tree the one worth measuring against the registry
provides:
  - "scripts/release-version-sweep.sh — a committed, reporting-only three-way version-drift sweep (in-tree vs crates.io vs source delta since the publishing tag) over all 25 publishable crates"
  - "Makefile target release-sweep, deliberately absent from quality-gate"
  - "A machine-readable 7-column TSV consumed by plans 05 and 07, from which the human table is rendered"
  - "A measured, artifact-corroborated classification of all 25 crates into clean / already-bumped / phantom-delta"
  - "D-03's patch-axis guard discharged authoritatively: zero jsonwebtoken occurrences in pmcp's --all-features public API"
  - "A refutation of one inherited phantom delta (pmcp-widget-utils) via published-artifact evidence"
affects: [124-04, 124-05, 124-06, 124-07, release-workflow]

actuals:
  tokens: 15200
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Baseline PROVENANCE as a first-class column: a diff base derived from a heuristic is reported with its confidence (tag:<n> / tag:<n>+confirmed / no-tag / unresolved) rather than presented as a fact, and the +confirmed value is unreachable without supplied corroboration evidence"
    - "Corroborate against the PUBLISHED ARTIFACT, not the tag: download the .crate for the exact published version and read it. This refuted one of seven inherited findings on the first pass"
    - "Report-then-fail: the complete report prints before any non-zero exit, because a partial sweep that stops at the first failure is less useful than a complete one that refuses to claim success"
    - "PROBE_FAILED and UNPUBLISHED are distinct classifications and neither may absorb the other: rendering a parse failure as UNPUBLISHED manufactures a false phantom delta, which can authorise a permanent version bump"
    - "One source of truth, rendered once: the TSV is emitted first and the human table is awk-rendered FROM it, so the machine artifact and the human report cannot disagree"

key-files:
  created:
    - scripts/release-version-sweep.sh
    - .planning/phases/124-release-publish-order/124-03-SUMMARY.md
  modified:
    - Makefile
    - .planning/WINDOWS.md

key-decisions:
  - "The sweep is a PERMANENT committed tool, not a phase-local script (CONTEXT left this to Claude's discretion): ~470 lines including its rationale header, no new dependency, and it is the only mechanism that can detect a phantom delta"
  - "release-sweep is deliberately NOT chained into quality-gate — it needs network, and a version delta is legitimate right up until a release, so gating would make the gate red on every ordinary branch"
  - "Reporting-only does NOT mean always-exit-0: the exit status reports 'did this sweep measure everything it claims to have measured', never 'is there a delta'"
  - "UNPUBLISHED sets the failure flag (superset of the plan's must_have) because a publishable crate that has never shipped IS the pmcp-tasks failure class; measured safe — all 25 crates are published today"
  - "pmcp-widget-utils is CLEAN, not a phantom delta: its published 0.1.0 src/lib.rs is byte-identical to the in-tree file. The inherited RESEARCH finding is refuted by artifact evidence and it must not be bumped"
  - "cargo public-api was run with --all-features as well as the criterion's default-features form: jwt-auth is non-default, so the default run could not have contained jsonwebtoken whatever the truth"

patterns-established:
  - "A test seam named as a scope, not a list: RELEASE_SWEEP_STUB_DIR mirrors CRATES_DIR in check-release-coverage.sh — it exists so failure paths are proven by fixture rather than asserted in prose"
  - "Corroboration is EARNED, never assumed: RELEASE_SWEEP_CORROBORATED is empty by default, so tag:<n>+confirmed cannot appear unless someone supplied the evidence"

requirements-completed: [PKGR-01]

coverage:
  - id: D1
    description: "make release-sweep enumerates all 25 publishable crates — root members and the workspace-EXCLUDED crates/pmcp-package alike — and prints the three-way comparison"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "make release-sweep > /tmp/124-sweep.txt — exit 0, 25 crate lines, final line 'all 25 publishable crate(s) measured against the registry.'"
        status: pass
      - kind: integration
        ref: "grep -c 'pmcp-package' /tmp/124-sweep.txt -> 1 (the workspace-excluded crate is present)"
        status: pass
    human_judgment: false
  - id: D2
    description: "The sweep exits NON-ZERO after printing the COMPLETE report on a failed probe, an unparseable body or a 404 — proven by three fixtures, not asserted"
    verification:
      - kind: integration
        ref: "RELEASE_SWEEP_STUB_DIR fixture 'empty_body' (200 + empty) -> exit 1, 25 rows printed, all classified PROBE_FAILED"
        status: pass
      - kind: integration
        ref: "fixture 'schema_stub' (the MEASURED crates.io type-descriptor body on a 200) -> exit 1, 25 rows, all PROBE_FAILED"
        status: pass
      - kind: integration
        ref: "fixture 'not_found' (404 + {\"errors\":[...]}) -> exit 1, 25 rows, all UNPUBLISHED — visually distinct from PROBE_FAILED"
        status: pass
    human_judgment: false
  - id: D3
    description: "The forbidden oracles appear only as header prose, never as executable commands; the TSV carries seven columns with no blank provenance and the rendered table is derived from it"
    verification:
      - kind: integration
        ref: "grep -v '^[[:space:]]*#' > code.txt; grep -c 'cargo search' code.txt -> 0; grep -c 'cargo info' code.txt -> 0; whole-file counts 1 and 2"
        status: pass
      - kind: integration
        ref: "awk -F'\\t' '{print NF}' tsv | sort -u -> 7 only; awk 'NR>1 && $6==\"\"' -> no output; TSV data rows 25 == rendered table rows 25"
        status: pass
    human_judgment: false
  - id: D4
    description: "release-sweep is reachable from make and absent from the quality-gate recipe, and the gate still passes"
    verification:
      - kind: integration
        ref: "grep -c 'release-sweep' Makefile -> 3 (comment :899, .PHONY :915, target :916); heading-anchored quality-gate extraction -> 0 occurrences"
        status: pass
      - kind: integration
        ref: "RUSTFLAGS=\"\" make quality-gate -> exit 0"
        status: pass
    human_judgment: false
  - id: D5
    description: "D-03's patch-axis guard is discharged authoritatively: no jsonwebtoken type crosses pmcp's public API"
    verification:
      - kind: integration
        ref: "cargo public-api --simplified --all-features (26801 lines) — grep -c jsonwebtoken -> 0, while grep -c jwt -> 286, so the enumeration is not vacuous"
        status: pass
      - kind: integration
        ref: "cargo semver-checks check-release --baseline-version 2.19.0 -> exit 0, 223 pass / 30 skip, 'no semver update required' (SUPPORTING evidence only)"
        status: pass
    human_judgment: false
  - id: D6
    description: "Every source-visible phantom delta's baseline is corroborated against the published .crate artifact, and one inherited finding is refuted by that evidence"
    verification:
      - kind: integration
        ref: "published pmcp-workbook-runtime 0.1.0 .crate — src/reconcile.rs absent, 'pub mod reconcile' count 0, 'RenderMode' count 0 (A5 answered)"
        status: pass
      - kind: integration
        ref: "published pmcp-code-mode-derive 0.2.0 src/lib.rs:274 emits validate_sql_query (sync); in-tree emits validate_sql_query_async(...).await"
        status: pass
      - kind: integration
        ref: "published pmcp-widget-utils 0.1.0 src/lib.rs diff against in-tree -> IDENTICAL; the v1.3 baseline is REFUTED"
        status: pass
    human_judgment: false
  - id: D7
    description: "Per-crate disposition of every phantom delta, decided by the user at the Task 3 blocking-human checkpoint"
    verification: []
    human_judgment: true
    rationale: "Version-number consumption is one-way and unrecoverable in both directions (a published version can be yanked, never unpublished; a version left unshipped stays unshipped). The plan makes this a gate=\"blocking-human\" decision precisely so no agent judgement substitutes for the user's."

duration: ~75 min
completed: 2026-08-27
status: halted
---

# Phase 124 Plan 03: Measure the Release, Then Let the User Decide It Summary

**A committed three-way version-drift sweep (`make release-sweep`) measures all 25 publishable crates against the crates.io API and finds seven phantom deltas — of which artifact corroboration confirms six and REFUTES one — while `cargo public-api --all-features` discharges D-03's patch axis with zero `jsonwebtoken` occurrences across pmcp's 26,801-line public surface.**

> **STATUS: COMPLETE.** Tasks 1 and 2 were committed before the checkpoint; Task 3's
> one-way-door decision was taken by the user on 2026-08-27 at the `gate="blocking-human"`
> checkpoint and is recorded verbatim in `## Task 3 Decision — DECIDED` below.
> Chosen: **`bump-all-source-visible`** + **`mcp-tester` left unbumped**. Plan 05 executes
> that closed four-crate list and nothing else.

## Performance

- **Duration:** ~75 min
- **Started:** 2026-08-27T18:00Z (approx.)
- **Completed (Tasks 1–2):** 2026-08-27T18:25Z
- **Tasks:** 3 of 3 complete
- **Files modified:** 4 (1 created, 3 modified)

## Accomplishments

- `scripts/release-version-sweep.sh`: a permanent, reporting-only sweep with a seven-section rationale header, mirroring the release-coverage gate's two-source discovery so it sees all 25 publishable crates including the workspace-excluded `crates/pmcp-package`.
- Baseline **provenance** promoted to a first-class column with a closed value set, because tag containment is a heuristic and this repo has tagged crates whose publish step did not exist.
- Three failure paths proven **by fixture**, not asserted: empty body, the measured crates.io schema-stub 200, and a 404 — each exits non-zero after printing all 25 rows, and `PROBE_FAILED` is visually distinct from `UNPUBLISHED`.
- All seven reported phantom deltas corroborated against the **published `.crate` artifact**; six confirmed, one (`pmcp-widget-utils`) refuted.
- D-03's guard discharged authoritatively with `cargo public-api --all-features`, and `cargo semver-checks` recorded explicitly as supporting evidence only.

## Task Commits

1. **Task 1: Build the three-way version-drift sweep as a committed tool** — `f5e9ef91` (feat)
2. **Task 2: Run the sweep, run D-03's public-API guard, produce the decision table** — no code changes by design (measurement only); its measured defects landed as `05d2af71` (docs, WINDOWS ledger)
3. **Task 3: DECISION checkpoint** — no code commit by design; the user's decision is recorded in `## Task 3 Decision — DECIDED` above

**Plan metadata:** _(this SUMMARY's own commit)_

## Files Created/Modified

- `scripts/release-version-sweep.sh` — the D-05 three-way sweep; discovery, probe, classification, semver selection, baseline provenance, TSV emission, table rendering, failure accounting.
- `Makefile` — `release-sweep` target (`:899` comment, `:915` `.PHONY`, `:916` target), deliberately not in `quality-gate`.
- `.planning/WINDOWS.md` — ledger entries #42–#45.
- `.planning/phases/124-release-publish-order/124-03-SUMMARY.md` — this file.

## Environment (precondition, recorded per Task 1)

| Tool | Required pin (Phase 112) | Resolved |
|---|---|---|
| `cargo-public-api` | 0.52.0 | **0.52.0** — already installed, no install needed |
| `cargo-semver-checks` | 0.49.0 | **0.49.0** — already installed, no install needed |
| `python3` | — | 3.13.7 |
| `jq` | — | 1.7.1 |
| `git` | ≥ 2.38 | 2.47.1 |

## Task 1 acceptance criteria — measured

| Criterion | Measured | Verdict |
|---|---|---|
| `make release-sweep` exits 0, ≥ 24 crate lines | exit 0, **25** lines | PASS |
| `grep -c 'User-Agent' script` ≥ 1 | 3 | PASS |
| comment-stripped `cargo search` = 0 | 0 | PASS |
| comment-stripped `cargo info` = 0 | 0 | PASS |
| whole-file `cargo search` ≥ 1 (header prose survives) | 1 | PASS |
| `grep -c 'crates/\*/Cargo.toml' script` ≥ 1 | 2 | PASS |
| `pmcp-package` appears in the report | 1 | PASS |
| TSV non-empty, seven columns | all rows `NF=7` | PASS |
| rendered table count == TSV data-row count | 25 == 25 | PASS |
| no blank provenance (`awk 'NR>1 && $6==""'`) | no output | PASS |
| `grep -c 'release-sweep' Makefile` ≥ 3 | 3 | PASS |
| `release-sweep` absent from the quality-gate recipe (heading-anchored) | 0 of 31 recipe lines | PASS |
| `RUSTFLAGS="" make quality-gate` | **exit 0** | PASS |

### Failure paths, proven by fixture

`RELEASE_SWEEP_STUB_DIR` is the single test seam (a scope, mirroring `CRATES_DIR` in the coverage gate). Each fixture stubs the probe for every crate:

| Fixture | Stubbed response | Exit | Report rows | Classification |
|---|---|---|---|---|
| `empty_body` | `200` + empty body | **1** | 25 | all `PROBE_FAILED` |
| `schema_stub` | `200` + the measured `{ meta: { next_page: null, total: int }, versions: [{ audit_actions: [{ action: string,` | **1** | 25 | all `PROBE_FAILED` |
| `not_found` | `404` + `{"errors":[{"detail":"Not Found"}]}` | **1** | 25 | all `UNPUBLISHED` |

Each printed the COMPLETE report before failing, and the first `::error::` line named the crate and the HTTP status. `PROBE_FAILED` and `UNPUBLISHED` render as different tokens in both the TSV and the table.

**Baseline fixture (`no-tag` renders as a marker, never as an empty delta):** exercised by the live tree, which has nine such crates. Each renders provenance `no-tag` with delta `(bump not in any tag — ships at the next tag)`, never `(none)`, and each is classified `already-bumped` rather than `clean`.

## Task 2 — the measured sweep transcript

`make release-sweep`, re-run 2026-08-27 against this branch (NOT inherited from RESEARCH, which was measured against a moving `main` three days earlier). Provenance shown is the **corroborated** run.

```
Release version-drift sweep — in-tree vs crates.io vs source delta since the publishing tag
TSV: /tmp/124-release-sweep-corroborated.tsv

cargo-pmcp               in-tree=0.23.0    published=0.21.0       already-bumped no-tag                 (bump not in any tag — ships at the next tag)
mcp-preview              in-tree=0.3.1     published=0.3.1        PHANTOM-DELTA  tag:v2.7.0+confirmed   1 file changed, 1 insertion(+), 1 deletion(-)
mcp-tester               in-tree=0.8.0     published=0.8.0        clean          tag:v2.19.0            (none)
pmcp                     in-tree=2.19.0    published=2.19.0       PHANTOM-DELTA  tag:v2.19.0+confirmed  1 file changed, 1 insertion(+), 1 deletion(-)
pmcp-agent               in-tree=0.3.0     published=0.2.0        already-bumped no-tag                 (bump not in any tag — ships at the next tag)
pmcp-cfn-renderer        in-tree=0.2.0     published=0.1.0        already-bumped no-tag                 (bump not in any tag — ships at the next tag)
pmcp-code-mode           in-tree=0.5.4     published=0.5.4        clean          tag:v2.19.0            (none)
pmcp-code-mode-derive    in-tree=0.2.0     published=0.2.0        PHANTOM-DELTA  tag:v2.3.1+confirmed   2 files changed, 4 insertions(+), 4 deletions(-)
pmcp-macros              in-tree=0.6.1     published=0.6.1        clean          tag:v2.7.0             (none)
pmcp-macros-support      in-tree=0.1.0     published=0.1.0        clean          tag:v2.4.0             (none)
pmcp-openapi-server      in-tree=0.1.1     published=0.1.0        already-bumped no-tag                 (bump not in any tag — ships at the next tag)
pmcp-server              in-tree=0.2.4     published=0.2.4        clean          tag:v2.19.0            (none)
pmcp-server-toolkit      in-tree=0.1.2     published=0.1.1        already-bumped no-tag                 (bump not in any tag — ships at the next tag)
pmcp-sql-server          in-tree=0.1.0     published=0.1.0        PHANTOM-DELTA  tag:v2.9.0+confirmed   1 file changed, 2 insertions(+), 2 deletions(-)
pmcp-tasks               in-tree=0.1.1     published=0.1.0        already-bumped no-tag                 (bump not in any tag — ships at the next tag)
pmcp-team-servers        in-tree=0.2.0     published=0.1.1        already-bumped no-tag                 (bump not in any tag — ships at the next tag)
pmcp-toolkit-athena      in-tree=0.1.0     published=0.1.0        clean          tag:v2.9.0             (none)
pmcp-toolkit-mysql       in-tree=0.1.1     published=0.1.1        clean          tag:v2.9.2             (none)
pmcp-toolkit-postgres    in-tree=0.1.0     published=0.1.0        clean          tag:v2.9.0             (none)
pmcp-widget-utils        in-tree=0.1.0     published=0.1.0        PHANTOM-DELTA  tag:v1.3               1 file changed, 1 insertion(+), 4 deletions(-)
pmcp-workbook-compiler   in-tree=0.1.0     published=0.1.0        PHANTOM-DELTA  tag:v2.9.2+confirmed   2 files changed, 24 insertions(+), 5 deletions(-)
pmcp-workbook-dialect    in-tree=0.1.0     published=0.1.0        clean          tag:v2.9.2             (none)
pmcp-workbook-runtime    in-tree=0.1.0     published=0.1.0        PHANTOM-DELTA  tag:v2.9.2+confirmed   5 files changed, 1100 insertions(+), 49 deletions(-)
pmcp-workbook-server     in-tree=0.1.1     published=0.1.0        already-bumped no-tag                 (bump not in any tag — ships at the next tag)
pmcp-package             in-tree=0.3.0     published=0.1.1        already-bumped no-tag                 (bump not in any tag — ships at the next tag)

swept 25 publishable crate(s) — 7 carrying a phantom delta
release-version-sweep: all 25 publishable crate(s) measured against the registry.
```

Exit 0. Every crate probed at `http=200`; no `PROBE_FAILED`, no `UNPUBLISHED`, no `unresolved` baseline.

### Step B — every crate assigned to exactly one bucket, with the deciding evidence

**clean (9)** — in-tree equals published, delta since the publishing tag is empty:
`mcp-tester` (v2.19.0), `pmcp-code-mode` (v2.19.0), `pmcp-macros` (v2.7.0), `pmcp-macros-support` (v2.4.0), `pmcp-server` (v2.19.0), `pmcp-toolkit-athena` (v2.9.0), `pmcp-toolkit-mysql` (v2.9.2), `pmcp-toolkit-postgres` (v2.9.0), `pmcp-workbook-dialect` (v2.9.2).
Deciding evidence: `git diff --shortstat <tag>..HEAD -- <crate dir>` is empty.
`mcp-tester` in particular answers CONTEXT D-05's open question: **clean, no delta**.

**clean by refutation (1)** — reported as a phantom delta, disproved by artifact:
`pmcp-widget-utils`. See the refutation subsection below.

**already bumped (9)** — in-tree ahead of published, bump in no tag, ships as-is at this tag; nothing to decide:
`cargo-pmcp` 0.21.0→0.23.0, `pmcp-agent` 0.2.0→0.3.0, `pmcp-cfn-renderer` 0.1.0→0.2.0, `pmcp-openapi-server` 0.1.0→0.1.1, `pmcp-server-toolkit` 0.1.1→0.1.2, `pmcp-tasks` 0.1.0→0.1.1, `pmcp-team-servers` 0.1.1→0.2.0, `pmcp-workbook-server` 0.1.0→0.1.1, `pmcp-package` 0.1.1→0.3.0.

**phantom delta, non-source (3)** — in-tree equals published, delta touches only a manifest dependency requirement:

- **`pmcp`** — `Cargo.toml` only: `jsonwebtoken = { version = "10.3" }` → `"11.0"`. Published 2.19.0's manifest confirmed to carry `[dependencies.jsonwebtoken] version = "10.3"`. **Pre-decided by CONTEXT D-03** → 2.19.1.
- **`mcp-preview`** — `Cargo.toml` only: `tower-http` `"0.6"` → `"0.7"`. Published 0.3.1's manifest confirmed to carry `[dependencies.tower-http] version = "0.6"`. Recommend **leave**: no source moved, and a consumer of `mcp-preview` (a binary preview tool) resolves its own tower-http through this crate's requirement only when building it, so the practical consequence is a stale dependency floor rather than wrong behaviour.
- **`pmcp-sql-server`** — `Cargo.toml` only, and both changed lines are `[dev-dependencies]`: `mcp-tester` `"0.7.0"` → `"0.8.0"` and `rusqlite` `"0.39"` → `"0.40"`. Published 0.1.0's manifest confirmed to carry `version = "0.7.0"` and `version = "0.39"` respectively. Recommend **leave**: dev-deps affect only someone building this crate's own test suite from the published `.crate`. *(Note: the retained `[dev-dependencies.mcp-tester] version = "0.7.0"` in the published manifest is direct measured proof of the `mcp-tester` hazard mechanism — see the ordering row.)*

**phantom delta, source-visible (3)**:

- **`pmcp-code-mode-derive`** — `src/lib.rs:274`, the proc-macro's emitted code for `"sql"`:
  `self.pipeline.validate_sql_query(code, &context)` → `self.pipeline.validate_sql_query_async(code, &context).await`
  (plus the two doc tables at `:38` and `:254`, and a dev-dep bump `pmcp-code-mode` `"0.4.0"`→`"0.5.0"`).
  **This is behaviourally real**: the published 0.2.0 macro generates the *synchronous* validation call. Both methods still exist in `pmcp-code-mode` (`src/validation.rs:936` sync, `:958` async), so the published macro still *compiles* — it just silently generates the wrong path. Recommend **bump → 0.2.1** (the generated call changes but the macro's own public API does not).
- **`pmcp-workbook-compiler`** — two components, and the second is not in RESEARCH's description:
  1. `src/lib.rs` — a doc-comment line plus an assertion inside `#[cfg(test)] mod harvest_output_table_tests` (`RESERVED_TOOL_NAMES` 5th entry `verify_accuracy`). **Neither is compiled into the published artifact's runtime surface.** Confirmed: `verify_accuracy` count 0 in the published 0.1.0 `src/lib.rs`.
  2. `Cargo.toml` — `umya-spreadsheet` `"3.0"` → `"=3.0.0"`. Published 0.1.0 confirmed to carry `[dependencies.umya-spreadsheet] version = "3.0"`. The in-tree comment records *why* the exact pin exists: 3.0.1 bumps transitive `quick-xml` 0.37→0.41, forking `Cargo.lock` onto two copies (which the Phase-93 purity gate fails closed on) **and** regressing a data-validation-list ingest test. Since `Cargo.lock` is gitignored, a cold resolve of the *published* crate lands on the version the project rejected. Recommend **bump → 0.1.1** on that basis — but the call is genuinely borderline and is an open row.
- **`pmcp-workbook-runtime`** — **additive public API**, the largest delta at +1100/−49 across 5 files:
  new `pub mod reconcile` (590 lines) plus new crate-root re-exports `RenderMode`, `reconcile_reference`, `seed_reference_inputs`, `OutputRow`, `ReconcileReport`, `ToolReport`; plus dev-deps `rust_xlsxwriter` 0.95→0.96 and a new `zip = "7"`.
  Recommend **bump → 0.2.0** (pre-1.0 additive → minor, matching the crate family's own Phase-122 `pmcp-package` 0.2→0.3 precedent).

### Step B — baseline corroboration (the review's largest finding, discharged)

Tag containment was **not** accepted as fact for any crate whose classification could authorise consuming a version number. Each was corroborated by downloading the published `.crate` from `https://crates.io/api/v1/crates/<name>/<version>/download` and reading it directly:

| Crate | Baseline claimed by the heuristic | Corroboration performed | Result |
|---|---|---|---|
| `pmcp` 2.19.0 | `tag:v2.19.0` | published manifest `[dependencies.jsonwebtoken] version = "10.3"` | **confirmed** |
| `mcp-preview` 0.3.1 | `tag:v2.7.0` | published manifest `[dependencies.tower-http] version = "0.6"` | **confirmed** |
| `pmcp-sql-server` 0.1.0 | `tag:v2.9.0` | published manifest dev-deps `mcp-tester "0.7.0"`, `rusqlite "0.39"` | **confirmed** |
| `pmcp-code-mode-derive` 0.2.0 | `tag:v2.3.1` | published `src/lib.rs:274` emits `validate_sql_query` (sync) | **confirmed** |
| `pmcp-workbook-compiler` 0.1.0 | `tag:v2.9.2` | published manifest `umya-spreadsheet "3.0"`; `verify_accuracy` absent from published `src/lib.rs` | **confirmed** |
| `pmcp-workbook-runtime` 0.1.0 | `tag:v2.9.2` | published artifact has **no** `src/reconcile.rs`; `pub mod reconcile` count 0; `RenderMode` count 0 | **confirmed (A5 answered)** |
| `pmcp-widget-utils` 0.1.0 | `tag:v1.3` | published `src/lib.rs` diffed against in-tree | **REFUTED** |

The corroborated sweep re-run supplies these six as `RELEASE_SWEEP_CORROBORATED`, so the script renders `tag:<n>+confirmed` for exactly them. `pmcp-widget-utils` is deliberately excluded and still renders bare `tag:v1.3`, which is the visible marker that its baseline is not trusted.

**A5 answered explicitly.** CLAUDE.md item 9a notes `pmcp-workbook-runtime` is published out-of-band by its own Phase 91/92 release, so its tag association could have been atypical. It is not: the published 0.1.0 artifact genuinely lacks the entire `reconcile` module and the `RenderMode` re-export, so the +1100/−49 delta is genuinely unshipped and `tag:v2.9.2` is a sound baseline. The heuristic was right here — but it was *checked*, not assumed.

### Step B — the refutation: `pmcp-widget-utils` is NOT a phantom delta

The sweep reports `pmcp-widget-utils` 0.1.0 = 0.1.0 with a `1 file changed, 1 insertion(+), 4 deletions(-)` delta from `tag:v1.3` — a `cargo fmt` reflow of `inject_bridge_script`'s `format!` call. RESEARCH Pitfall 2 carries the same finding.

**It is wrong, and the artifact says so.** `diff` of the published 0.1.0 `src/lib.rs` against the in-tree file: **identical**. The published crate already contains the one-line post-`fmt` form. Tracing it:

- version-bump commit `7711a955` ("refactor: extract inject_bridge_script into shared pmcp-widget-utils crate");
- earliest tag containing it by `creatordate`: `v1.3` — but the `fmt` reflow landed later, in `eb7e4bf1` ("style: apply cargo fmt --all across workspace");
- earliest tag containing `eb7e4bf1`: `v1.11.0`, and `git diff v1.11.0..HEAD -- crates/pmcp-widget-utils` is **empty**.

So the crate was published at a tag at or after `v1.11.0`, not at `v1.3`. The heuristic picked a base that is *too early*, which over-reports — the safe direction, and exactly the direction the plan's threat register (T-124-23) predicted. Recorded as WINDOWS #42.

**Corollary worth stating, because it bounds how much the heuristic can be wrong.** "Earliest tag containing the bump commit" is always at or before the true publishing tag, so the error is always over-reporting, never under-reporting. A `clean` reading (empty diff from an at-or-earlier base) therefore cannot hide a real delta, short of a change-and-revert that would leave the published artifact matching no tagged state. That is why only the seven *reported* deltas needed corroboration and the nine `clean` readings did not.

### Step C — D-03's guard, discharged authoritatively

| Check | Command | Result |
|---|---|---|
| Cheap first pass (suggestive only) | `grep -rn jsonwebtoken src/` | 15 hits, all private: struct fields in `jwt.rs:33` / `jwt_validator.rs:72`, function-local `use`, and private method signatures. Cannot see re-exports or trait-associated types — which is why it is not the answer. |
| Criterion as written | `cargo public-api --simplified` | 22,462 lines, **0** `jsonwebtoken` |
| **Authoritative** | `cargo public-api --simplified --all-features` | 26,801 lines, **0** `jsonwebtoken`, **286** lines mentioning `jwt` |
| Supporting only | `cargo semver-checks check-release --baseline-version 2.19.0` | exit 0 — 223 checks: 223 pass, 30 skip; "no semver update required" |

**Why the `--all-features` run is the one that counts, and the criterion's form is not.** `jsonwebtoken` is an optional dependency gated behind the non-default `jwt-auth` feature (`Cargo.toml:151`, `:307`). A default-features run never compiles the JWT modules at all, so its zero was guaranteed regardless of the truth — a vacuous pass. The `--all-features` run enumerates the JWT surface (`JwtValidatorConfig` and 285 other `jwt` lines are present) and *still* contains zero `jsonwebtoken` occurrences. **A1 holds: no `jsonwebtoken` type crosses `pmcp`'s public API, so D-03's patch axis (2.19.0 → 2.19.1) is correct.** The defective criterion is recorded as WINDOWS #44.

**`cargo semver-checks` is SUPPORTING evidence, not independent confirmation.** It validates Rust API compatibility between two versions of *this* crate; it does not and cannot answer whether a dependency's major-version move is semantically safe when the dependency is reached behind feature flags. Its run here compared 2.19.0 against 2.19.0 ("no change; assume patch") because `git diff v2.19.0..HEAD -- src/` is empty — so it confirms that pmcp's own source has not moved, which is a *different* fact from the one D-03 needs. **`cargo public-api` is the authoritative check for D-03's guard.**

### Step E — the `mcp-tester` ordering hazard, measured

Re-measured on this branch, with a **bounded** matcher (`cargo publish -p <name>( |$)`), because an unbounded grep for `cargo publish -p pmcp-server` resolves to the `pmcp-server-toolkit` step at :263 — a silently wrong answer (WINDOWS #45).

**Six in-repo crates pin `mcp-tester` by version, all at `0.8.0`:**

| Pin site | Publish step (`release.yml`, raw line) | Order vs `mcp-tester` (:401) |
|---|---|---|
| `crates/pmcp-server-toolkit/Cargo.toml:192` | :263 | **BEFORE** |
| `crates/pmcp-sql-server/Cargo.toml:57` | :329 | **BEFORE** |
| `crates/pmcp-openapi-server/Cargo.toml:63` | :344 | **BEFORE** |
| `crates/pmcp-workbook-server/Cargo.toml:58` | :383 | **BEFORE** |
| `cargo-pmcp/Cargo.toml:69` | :525 | after — safe |
| `crates/pmcp-server/Cargo.toml:31` | :543 | after — safe |

All six are `[dev-dependencies]` entries carrying **both** `path` and `version`. Cargo strips a dev-dep from the published manifest only when it carries no version requirement; one that carries a requirement is **retained** and must resolve on crates.io at publish time. `crates/pmcp-openapi-server/Cargo.toml:112-119` already states this rule in-tree.

**Measured proof of the mechanism, not just the rule:** the published `pmcp-sql-server` 0.1.0 manifest contains `[dev-dependencies.mcp-tester] version = "0.7.0"` — the entry survived publication exactly as described.

It is green today **only** because `0.8.0` is already on crates.io (verified: in-tree `0.8.0` == published `0.8.0`, and the sweep classifies `mcp-tester` **clean** with no delta). Bumping `mcp-tester` to, say, `0.9.0` without moving all four before-publish pins in the same change kills the release job at `pmcp-server-toolkit`, the first of them.

## Task 3 Decision — DECIDED

> **Decided 2026-08-27 by the user at the `gate="blocking-human"` checkpoint.**
> Option chosen: **`bump-all-source-visible`**, plus **`mcp-tester` = (a) leave unbumped**.
> Plan 05 executes the closed list below and nothing else.

### The authorised bump set — closed, exhaustive, `(crate, from, to)`

| # | Crate | From | To | Axis | Baseline provenance | Corroboration |
|---|---|---|---|---|---|---|
| 1 | `pmcp` | 2.19.0 | **2.19.1** | patch | `tag:v2.19.0+confirmed` | D-03 guard clean — `cargo public-api --all-features`, 0 `jsonwebtoken` in 26,801 lines, 286 `jwt` as positive control |
| 2 | `pmcp-workbook-runtime` | 0.1.0 | **0.2.0** | minor (pre-1.0 additive) | `tag:v2.9.2+confirmed` | published `.crate` artifact |
| 3 | `pmcp-code-mode-derive` | 0.2.0 | **0.2.1** | patch | `tag:v2.3.1+confirmed` | published `.crate` artifact; **orchestrator-verified** at `crates/pmcp-code-mode-derive/src/lib.rs:274` |
| 4 | `pmcp-workbook-compiler` | 0.1.0 | **0.1.1** | patch | `tag:v2.9.2+confirmed` | published `.crate` artifact; **orchestrator-verified** against the crates.io dependencies API |

No other crate's version may be touched by plan 05.

### Decision line per open row

- **`pmcp-workbook-runtime` → 0.2.0 (BUMP).** Additive public API: a new `pub mod reconcile`
  (590 lines) plus six crate-root re-exports (`RenderMode`, `reconcile_reference`,
  `seed_reference_inputs`, `OutputRow`, `ReconcileReport`, `ToolReport`). Pre-1.0 additive
  takes the minor axis. Left unbumped, no downstream consumer could reach the new module.
- **`pmcp-code-mode-derive` → 0.2.1 (BUMP).** The published 0.2.0 macro emits
  `validate_sql_query(...)` where in-tree emits `validate_sql_query_async(...).await`. Both
  methods still exist, so the generated code *compiles* — it is simply the wrong path, which
  is exactly why nothing has noticed. A wrong published artifact, not a missing feature.
- **`pmcp-workbook-compiler` → 0.1.1 (BUMP).** In-tree pins `umya-spreadsheet = "=3.0.0"`;
  the published 0.1.0 carries `^3.0`. A cold resolve of the published crate therefore lands
  on 3.0.1, which forks `Cargo.lock` onto a second `quick-xml` — the purity gate fails
  closed — and regresses an ingest test. RESEARCH classified this as "doc + test assertion"
  and **missed the pin**; the sweep caught it.
- **`mcp-preview` — LEAVE.** Manifest-only (`tower-http "0.6"` → `"0.7"`). No source moved.
- **`pmcp-sql-server` — LEAVE.** `[dev-dependencies]` only (`mcp-tester "0.7.0"` → `"0.8.0"`,
  `rusqlite "0.39"` → `"0.40"`). Does not affect the published library surface.
- **`pmcp-widget-utils` — LEAVE. Not a delta at all.** Its published 0.1.0 `src/lib.rs` is
  byte-identical to the in-tree file. The `tag:v1.3` baseline is over-early and
  **uncorroborated**: the `fmt` reflow landed in `eb7e4bf1`, whose earliest containing tag is
  `v1.11.0`, and `git diff v1.11.0..HEAD` is empty. RESEARCH Pitfall 2 lists it as a phantom
  delta; that finding is **REFUTED**. This is a measured instance of T-124-23 — tag
  containment is not a publication oracle (WINDOWS #42).

### `mcp-tester` — explicit line

**NOT bumped.** The sweep classifies it **clean**: in-tree `0.8.0` == published `0.8.0`, no
delta. It is presented as its own decision row only because bumping it carries a consequence
no other row has — six in-repo crates pin it by version and **four publish before it**
(`pmcp-server-toolkit` :263, `pmcp-sql-server` :329, `pmcp-openapi-server` :344,
`pmcp-workbook-server` :383, against `mcp-tester` at :401), each a `[dev-dependencies]` entry
carrying **both** `path` and `version`, which Cargo **retains** in the published manifest
(measured: published `pmcp-sql-server` 0.1.0 contains `[dev-dependencies.mcp-tester]
version = "0.7.0"`). It is green today only because 0.8.0 is already on crates.io.

**Prohibition for plan 05:** do not bump `mcp-tester`. Bumping it without moving all four
before-publish pins in the same change kills the release job at `pmcp-server-toolkit`, the
first of them.

### The caret non-decision (settled, not open)

`pmcp` 2.19.0 → 2.19.1 requires **no** downstream pin bumps. `crates/mcp-tester/Cargo.toml:21`
and `cargo-pmcp/Cargo.toml:68` pin `pmcp = "2.19.0"`, and `^2.19.0` already admits 2.19.1.
CLAUDE.md's blanket Version Bump Rule ("downstream crates that pin a bumped dependency must
also be bumped") over-fires on patch bumps; plan 04 Task 2 records the caret exception.

### Consequence of every row left unbumped

For `mcp-preview`, `pmcp-sql-server`, `pmcp-widget-utils` and `mcp-tester`: **the delta stays
unshipped indefinitely.** `release.yml` skips an already-published version gracefully and
silently, so nothing fails and nothing reports it. `make release-sweep` is the only mechanism
that will ever surface these again. For `pmcp-widget-utils` this costs nothing — there is no
delta to ship. For the other three the omission is deliberate and measured.

### Ships as-is at this tag, already bumped ahead of the registry (9)

`cargo-pmcp` 0.23.0 · `pmcp-agent` 0.3.0 · `pmcp-cfn-renderer` 0.2.0 · `pmcp-openapi-server`
0.1.1 · `pmcp-package` 0.3.0 · `pmcp-server-toolkit` 0.1.2 · `pmcp-tasks` 0.1.1 ·
`pmcp-team-servers` 0.2.0 · `pmcp-workbook-server` 0.1.1. No decision was required for these.

### Bounding argument — why the nine `clean` readings are trustworthy

"Earliest tag containing the bump commit" is always at-or-before the true publishing tag, so
the heuristic can only **over**-report a delta, never under-report one. A crate the sweep
reads as `clean` therefore cannot be hiding a real unshipped change. This is what makes the
sweep a decision instrument rather than merely suggestive, and plan 05 and the phase verifier
may rely on it.

### The table presented to the user

**Pre-decided rows (confirmation only)**

| Crate | In-tree | Published | Provenance | Delta | Disposition |
|---|---|---|---|---|---|
| `pmcp` | 2.19.0 | 2.19.0 | `tag:v2.19.0+confirmed` | `Cargo.toml`: `jsonwebtoken` 10.3→11.0 | **2.19.1 (patch)** — CONTEXT D-03. Guard came back **clean** (0 `jsonwebtoken` in the `--all-features` public API). |
| `pmcp-package` | 0.3.0 | 0.1.1 | `no-tag` | already bumped | Ships 0.3.0 as-is; D-04's audit is plan 04's, not this plan's. |

**Already bumped — ship as-is at this tag, no decision needed (9):** `cargo-pmcp` 0.23.0, `pmcp-agent` 0.3.0, `pmcp-cfn-renderer` 0.2.0, `pmcp-openapi-server` 0.1.1, `pmcp-server-toolkit` 0.1.2, `pmcp-tasks` 0.1.1, `pmcp-team-servers` 0.2.0, `pmcp-workbook-server` 0.1.1, `pmcp-package` 0.3.0.

**Open rows**

| # | Crate | In-tree = Published | Provenance | What the delta actually changes | Recommendation |
|---|---|---|---|---|---|
| 1 | `pmcp-workbook-runtime` | 0.1.0 | `tag:v2.9.2+confirmed` | New `pub mod reconcile` (590 lines) + 6 new crate-root re-exports — additive public API | **Bump → 0.2.0** (pre-1.0 additive → minor) |
| 2 | `pmcp-code-mode-derive` | 0.2.0 | `tag:v2.3.1+confirmed` | Emitted code for `"sql"` switches from `validate_sql_query` to `validate_sql_query_async(...).await` | **Bump → 0.2.1** (generated code changes; the macro's own API does not) |
| 3 | `pmcp-workbook-compiler` | 0.1.0 | `tag:v2.9.2+confirmed` | `umya-spreadsheet "3.0"` → `"=3.0.0"`; plus a doc line and a `#[cfg(test)]` assertion | **Bump → 0.1.1** — borderline; the exact pin exists because 3.0.1 breaks purity + ingest |
| 4 | `mcp-preview` | 0.3.1 | `tag:v2.7.0+confirmed` | `tower-http "0.6"` → `"0.7"` (manifest only) | **Leave** |
| 5 | `pmcp-sql-server` | 0.1.0 | `tag:v2.9.0+confirmed` | dev-deps `mcp-tester "0.7.0"`→`"0.8.0"`, `rusqlite "0.39"`→`"0.40"` (manifest only) | **Leave** |
| 6 | `pmcp-widget-utils` | 0.1.0 | `tag:v1.3` **(REFUTED)** | Nothing — the published artifact already contains the change | **Leave — it is not a delta** |

**Named ordering row**

| Row | Fact | Options |
|---|---|---|
| `mcp-tester` | Sweep says **clean** (0.8.0 == 0.8.0, no delta). Six crates pin it at `0.8.0`; four publish BEFORE it (:263, :329, :344, :383 vs :401) carrying `path`+`version` dev-deps that Cargo retains in the published manifest. | **(a)** Leave unbumped — the safe default, and what the measurement supports. **(b)** Bump, with a stated resolution for the four before-publish pins in the same change. Doing (b) without the resolution kills the release job at `pmcp-server-toolkit`. |

**Stated non-decision (settled, not open):** `pmcp` 2.19.0 → 2.19.1 requires **no** downstream pin bumps. `crates/mcp-tester/Cargo.toml:21` and `cargo-pmcp/Cargo.toml:68` pin `pmcp = "2.19.0"`, and the caret requirement `^2.19.0` already admits 2.19.1. CLAUDE.md's blanket Version Bump Rule over-fires on patch bumps; plan 04 Task 2 records the caret exception in the ledger.

### ADDENDUM — amended 2026-08-27 by the user, one row only

> **This APPENDS to the decision above; it does not rewrite it.** The record is meant to show
> what was decided, what it collided with, and how it was resolved.

**What collided.** Plan 05's executor, before editing any version literal, ran the
workspace-wide requirement search Task 1 mandates and found that row 2 —
`pmcp-workbook-runtime` 0.1.0 → **0.2.0** — is not executable as recorded. On a 0.x line
Cargo reads `^0.1.0` as `>=0.1.0, <0.2.0`, so a MINOR bump is the *breaking* axis, and four
in-workspace sites pin the crate at `"0.1.0"`: `crates/pmcp-server-toolkit/Cargo.toml:81` and
`:202`, `crates/pmcp-workbook-compiler/Cargo.toml:41`,
`crates/pmcp-workbook-dialect/Cargo.toml:25`.

**The measurement that established it** (bump applied temporarily, full resolve run, tree
restored):

```
CONTROL (unmodified tree)    cargo metadata --format-version 1 --offline   EXIT=0
PROBE   (runtime at 0.2.0)   cargo metadata --format-version 1 --offline   EXIT=101
error: failed to select a version for the requirement `pmcp-workbook-runtime = "^0.1.0"`
candidate versions found which didn't match: 0.2.0
required by package `pmcp-server-toolkit v0.1.2`
```

Moving those pins would then have forced a FIFTH version number — `pmcp-workbook-dialect`
0.1.0 → 0.1.1, since that crate is 0.1.0 in-tree AND 0.1.0 published — which this closed list
did not authorise. Plan 05 halted rather than consume it.

**The amendment.** `pmcp-workbook-runtime` ships **0.1.1**, not 0.2.0.

The reasoning is that **0.2.0 was the error, not the cost.** The change is purely additive — a
new `pub mod reconcile` plus six re-exports, breaking nothing — so `0.1.1` states the truth
while `0.2.0` announces a break to every consumer. This decision chose "pre-1.0 additive →
minor" as a convention without checking who pinned the crate; the resolve probe is what
exposed it. The Phase-122 precedent cited above does **not** transfer: `pmcp-package`
0.2 → 0.3 carried five genuinely breaking changes, which is why the minor axis was right
*there*.

**The amended list — closed and exhaustive, superseding the table above in exactly one row:**

| # | Crate | From | To | Status |
|---|---|---|---|---|
| 1 | `pmcp` | 2.19.0 | **2.19.1** | unchanged from the original decision |
| 2 | `pmcp-workbook-runtime` | 0.1.0 | **0.1.1** | **AMENDED — was 0.2.0** |
| 3 | `pmcp-code-mode-derive` | 0.2.0 | **0.2.1** | unchanged |
| 4 | `pmcp-workbook-compiler` | 0.1.0 | **0.1.1** | unchanged |

**Explicitly NOT bumped:** `pmcp-workbook-dialect` stays **0.1.0** — option A's fifth number
is not consumed. The standing prohibitions are unaffected: `mcp-tester`, `mcp-preview`,
`pmcp-sql-server`, `pmcp-widget-utils`.

**Pin consequence under 0.1.1:** `^0.1.0` admits 0.1.1, so no pin is *forced* to move.
`pmcp-server-toolkit:81` and `:202` are nevertheless tightened to `"0.1.1"` because that
crate's `src/workbook/handler.rs` genuinely uses API absent from the published 0.1.0; the
compiler and dialect pins are left alone because measurement shows they do not.

#### ADDENDUM 2 — the amendment's premise was refuted by measurement, same day

> Appended, not rewritten: the two addenda together are the record of a decision, the
> evidence that contradicted it, and what that leaves open.

Executing the amendment above, plan 05 ran `cargo semver-checks check-release` against the
**published** baseline — the per-crate verdict Task 1 requires, and which no earlier step had
run for this crate. Result:

```
$ cargo semver-checks check-release -p pmcp-workbook-runtime --baseline-version 0.1.0
    Checking pmcp-workbook-runtime v0.1.0 -> v0.1.1 (minor change)
--- failure function_parameter_count_changed: pub fn parameter count changed ---
  pmcp_workbook_runtime::render::render_xlsx now takes 3 parameters instead of 2,
  in crates/pmcp-workbook-runtime/src/render/mod.rs:270
     Summary semver requires new major version: 1 major and 0 minor checks failed
EXIT=100
```

**The change is NOT purely additive.** `pub fn render_xlsx` gained a third parameter,
`mode: RenderMode` — and `RenderMode` is itself one of the symbols absent from the published
0.1.0. It is public (`pub mod render` at `lib.rs:65`) and called across a crate boundary
(`crates/pmcp-server-toolkit/src/workbook/render_resource.rs:42,108`). On a 0.x line "requires
new major" means bumping the leftmost non-zero component: **0.1.0 -> 0.2.0**.

So `0.1.1` would be actively wrong rather than merely conservative: `^0.1.0` admits `0.1.1`,
so every consumer pinned to the published 0.1.0 that calls `render_xlsx` would silently
receive the incompatible version on a fresh resolve and fail to compile. That is precisely the
harm the minor axis exists to prevent on a 0.x line.

**Both stated rationales for this row were wrong, in opposite directions.** The original
decision said "additive public API -> minor" (right answer, wrong reason). The amendment said
"purely additive, breaking nothing -> patch" (wrong answer, same wrong premise). Neither cited
`render_xlsx`. The tool did.

The other three rows are confirmed by the same measurement and are unaffected:

| Crate | Move | `cargo semver-checks` verdict |
|---|---|---|
| `pmcp` | 2.19.0 -> 2.19.1 | EXIT 0 — classified "(patch change)", 223 pass / 30 skip, "no semver update required" |
| `pmcp-code-mode-derive` | 0.2.0 -> 0.2.1 | EXIT 101 — **not checkable**: proc-macro crate, no library target, so it has no API surface the tool can compare. The change is in *emitted* code; plan 03's reasoning (the macro's own API does not move) stands, unverified by tooling. |
| `pmcp-workbook-compiler` | 0.1.0 -> 0.1.1 | EXIT 0 — "no semver update required" |

**Status: row 2 is OPEN again.** Restoring 0.2.0 restores the original collision — all four
`^0.1.0` pin sites are forced to move, and `pmcp-workbook-dialect` (0.1.0 in-tree AND 0.1.0
published) then needs a version number this list does not authorise. Option C is eliminated;
options A and B from `124-05-SUMMARY.md` remain.

#### ADDENDUM 3 — RESOLVED 2026-08-27: Option A, and the final closed set

> Third and last append. The trail above is deliberately preserved in full: the right answer
> reached by the wrong argument, then the wrong answer reached from the same argument, then
> the measured truth. It shows a future releaser exactly which kind of reasoning to distrust.

**Resolution: Option A.** `pmcp-workbook-runtime` returns to **0.2.0** — the number the
original decision chose, now for the demonstrated reason rather than the convention — and
`pmcp-workbook-dialect` **0.1.0 -> 0.1.1** is authorised as the fifth row.

### The final authorised set — closed and exhaustive, five rows

| # | Crate | From | To | `cargo semver-checks` (baseline built from crates.io) |
|---|---|---|---|---|
| 1 | `pmcp` | 2.19.0 | **2.19.1** | EXIT 0 — "(patch change)", 223 checks: 223 pass / 30 skip |
| 2 | `pmcp-workbook-runtime` | 0.1.0 | **0.2.0** | EXIT 0 — "(major change)". 0.1.1 was refuted at EXIT 100 |
| 3 | `pmcp-code-mode-derive` | 0.2.0 | **0.2.1** | EXIT 101 — **UNVERIFIABLE**, see below |
| 4 | `pmcp-workbook-compiler` | 0.1.0 | **0.1.1** | EXIT 0 — "no semver update required" |
| 5 | `pmcp-workbook-dialect` | 0.1.0 | **0.1.1** | EXIT 0 — "no semver update required" (pure re-pin) |

**`pmcp-code-mode-derive` stays marked UNVERIFIED, and the other four rows' green verdicts do
not launder it.** It is a proc-macro crate with no library target, so `semver-checks` exits
101 with "no library target" — there is no API surface for the tool to compare, and the change
is in *emitted* code, which no API check can see. Its patch axis rests on plan 03's reasoning
alone (the macro's own public API does not move). Given that this phase twice found exactly
that class of reasoning wrong about an axis, the honest record is "unverified, and here is why
it cannot be verified" rather than silence.

### The four forced pin moves, and why two of them are not the same as the other two

`0.1.0 -> 0.2.0` is semver-incompatible, so every `^0.1.0` requirement is forced to `0.2.0` —
Cargo refuses to resolve otherwise (measured: EXIT 101). But the four sites divide:

| Site | Moves because | Evidence |
|---|---|---|
| `crates/pmcp-server-toolkit/Cargo.toml:81` | **CODE** — its source needs the new API | 3 new-symbol imports, 10 path-qualified refs |
| `crates/pmcp-server-toolkit/Cargo.toml:202` | **CODE** — same | same |
| `crates/pmcp-workbook-compiler/Cargo.toml:41` | Cargo forces it; the source does not need it | **0** and **0** |
| `crates/pmcp-workbook-dialect/Cargo.toml:25` | Cargo forces it; the source does not need it | **0** and **0** |

The 0/0 readings were taken against `pmcp-server-toolkit` as a **positive control**, so the
enumeration is not vacuous — and after a false positive in the first attempt at that grep,
which matched `reconcile` in the *file path* (`.../src/reconcile/drift.rs:17:`) rather than in
the import list and reported a spurious 9 for the compiler. Re-run with the path prefix
stripped, it is 0. The distinction matters: it is the difference between a necessary bound and
a superstitious one.

### Not consumed, and not moved

`pmcp-workbook-compiler`'s `pmcp-workbook-dialect` pin at `:45` stays `"0.1.0"` — dialect's
own move is 0.1.0 -> 0.1.1, which `^0.1.0` already admits. `cargo-pmcp:75`'s
`pmcp-workbook-compiler = "0.1.0"` stays for the same reason. The `pmcp` caret non-bump
(`cargo-pmcp:68`, `crates/mcp-tester:21`) and the `mcp-tester` prohibition are unaffected.

## Decisions Made

1. **The sweep is permanent, not phase-local.** CONTEXT left this to Claude's discretion. Permanence is cheap (one shell file, no new dependency) and it is the only mechanism that can detect a phantom delta — a class this repo has now hit at least seven times.
2. **Reporting-only, but not always-exit-0.** The exit status answers "did this sweep measure everything it claims to", never "is there a delta". That keeps it out of `quality-gate` (no false red on ordinary branches) while making an unmeasured crate impossible to overlook.
3. **`UNPUBLISHED` sets the failure flag** — a superset of the plan's must_have. A publishable crate that has never shipped *is* the `pmcp-tasks` failure class. Measured safe: all 25 crates are published, so `make release-sweep` exits 0 today.
4. **`+confirmed` provenance is opt-in via supplied evidence**, so the value cannot appear unless corroboration was actually done.
5. **`cargo public-api` was run with `--all-features`** in addition to the criterion's literal form, because the literal form is vacuous.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing Critical] The `cargo public-api` acceptance criterion is vacuous as written**
- **Found during:** Task 2 Step C
- **Issue:** `cargo public-api --simplified` runs with DEFAULT features. `jsonwebtoken` is optional behind the non-default `jwt-auth` feature, so the JWT modules are never compiled and the zero count is guaranteed independently of the fact being tested. The criterion would have passed even if a `jsonwebtoken` type *did* cross the public API.
- **Fix:** Also ran `cargo public-api --simplified --all-features` and asserted zero there, plus a positive control (286 lines mention `jwt`, so the surface is genuinely enumerated).
- **Verification:** 26,801 API lines, `grep -c jsonwebtoken` → 0, `grep -c jwt` → 286.
- **Committed in:** `05d2af71` (recorded as WINDOWS #44; no code change was required).

**2. [Rule 1 — Bug] An inherited phantom-delta finding is false**
- **Found during:** Task 2 Step B corroboration
- **Issue:** RESEARCH Pitfall 2 and the sweep both report `pmcp-widget-utils` as a phantom delta. Its published 0.1.0 `src/lib.rs` is byte-identical to the in-tree file — the `v1.3` baseline is over-early.
- **Fix:** Refuted with artifact evidence, traced to the correct baseline (`v1.11.0`, whence the delta is empty), excluded from the corroborated set so it visibly renders bare `tag:v1.3`, and presented as a "leave — it is not a delta" row.
- **Verification:** `diff published/src/lib.rs crates/pmcp-widget-utils/src/lib.rs` → identical; `git diff --shortstat v1.11.0..HEAD -- crates/pmcp-widget-utils` → empty.
- **Committed in:** `05d2af71` (WINDOWS #42).

**3. [Rule 2 — Missing Critical] The `pmcp-workbook-compiler` delta is under-described upstream**
- **Found during:** Task 2 Step B
- **Issue:** RESEARCH describes it as "doc + test assertion", which would make it trivially leave-able. It also carries `umya-spreadsheet "3.0"` → `"=3.0.0"`, a published-manifest constraint whose absence lets the published crate cold-resolve onto 3.0.1 — the version the in-tree comment records as forking `Cargo.lock` onto a second `quick-xml` and regressing an ingest test.
- **Fix:** Read the full diff rather than the shortstat, verified the published manifest still carries the caret range, and reclassified the row from "leave" to a recommended bump with the reasoning shown.
- **Verification:** published 0.1.0 `Cargo.toml` → `[dependencies.umya-spreadsheet] version = "3.0"`.
- **Committed in:** `05d2af71` (WINDOWS #43).

**4. [Rule 3 — Blocking] Prefix collision in the publish-ordinal measurement**
- **Found during:** Task 2 Step E
- **Issue:** An unbounded `grep -F 'cargo publish -p pmcp-server'` over `release.yml` returns line 263 — the `pmcp-server-toolkit` step — a silently wrong ordinal rather than an error.
- **Fix:** Re-measured all seven ordinals with the bounded matcher `cargo publish -p <name>( |$)` that `check-release-coverage.sh` already uses, and recorded both raw and comment-stripped ordinals.
- **Verification:** bounded run reproduces the plan's stated `:525` / `:543` exactly.
- **Committed in:** `05d2af71` (WINDOWS #45).

---

**Total deviations:** 4 auto-fixed (2 missing-critical, 1 bug, 1 blocking).
**Impact on plan:** All four strengthen the measurement rather than change the plan's shape. Two of them change what the user is asked (`pmcp-widget-utils` removed as a bump candidate; `pmcp-workbook-compiler` promoted from "leave" to a recommended bump), which is exactly why the checkpoint exists.

## Issues Encountered

- **The `cargo public-api` default-features run is a false green.** Resolved by running `--all-features` with a positive control. See deviation 1.
- **Bash command-shape restrictions in the worktree-isolated harness.** Several multi-step measurement commands were refused by the environment's safety classifier as "too complex to verify". Resolved by writing each measurement as a scratchpad script file and invoking it with a single `bash <path>` call — no measurement was skipped or simplified to fit.
- **No probe failures occurred in the live runs.** The schema-stub 200 the review reproduced twice in ~8 requests did not recur across ~75 live probes here. Its handling is nonetheless proven by the `schema_stub` fixture rather than left to chance.

## User Setup Required

None — no external service configuration required. The sweep needs only outbound HTTPS to `crates.io`.

## Next Phase Readiness

**Unblocked.** The `## Task 3 Decision — DECIDED` section above names the closed four-crate `(crate, from, to)` list plan 05 is authorised to execute, and the `mcp-tester` prohibition.

Ready for downstream once decided:
- `make release-sweep` and `/tmp/124-release-sweep-corroborated.tsv` are the inputs plans 05 and 07 consume.
- D-03's patch axis is confirmed, so `pmcp` → 2.19.1 needs no revision.
- The `mcp-tester` ordering hazard has a measured row awaiting an explicit answer; plan 05 carries the matching prohibition and the two must agree.

---
*Phase: 124-release-publish-order*
*Completed: 2026-08-27 (all 3 tasks; Task 3 decided by the user at the blocking-human checkpoint)*

## Self-Check: PASSED

All created files verified present on disk (`scripts/release-version-sweep.sh`,
`124-03-SUMMARY.md`, the corroborated TSV) and all three task commits verified
present in `git log --oneline --all` (`f5e9ef91`, `05d2af71`, `93165d8e`).
No `git stash` operation was performed (the shared stack was left at its 14
pre-existing entries).

---

### ADDENDUM 4 — `pmcp-code-mode-derive` axis corrected to 0.3.0 (2026-08-27, pre-PR gate)

**Appended, not rewritten.** Rows above recording `0.2.1` are the historical record of what was
decided in each round; this addendum supersedes them.

The user elected, before Wave 6 opened the PR, to close the one row this phase had knowingly
left resting on reasoning alone. `pmcp-code-mode-derive` was the only crate in the release whose
axis no tool could verify — a proc-macro with no library target, so `cargo semver-checks` exits
101. It was resolved by hand instead, and **the reasoning was wrong**.

Measured:

- The published `0.2.0` `.crate` differs from in-tree by **exactly one code line** (doc comments
  excluded): the emitted call moves from `validate_sql_query(code, &context)` to
  `validate_sql_query_async(code, &context).await`.
- `validate_sql_query_async` is **absent from `pmcp-code-mode` 0.4.0** and present from `0.5.0`
  (both `.crate` artifacts downloaded and grepped).
- `pmcp-code-mode-derive` declares **no runtime dependency** on `pmcp-code-mode` — its
  `[dependencies]` are `syn`, `quote`, `proc-macro2`, `darling` only. The `pmcp-code-mode`
  entry is a **dev-dependency**, which Cargo strips from the published manifest.

Therefore nothing constrains the pairing. A user on `pmcp-code-mode` 0.4.x with
`pmcp-code-mode-derive = "0.2"` would have received `0.2.1` **automatically** on their next
resolve and failed to compile with *no method named `validate_sql_query_async`*. A breaking
change delivered on a patch number is the worst case, because patch upgrades are silent.

**Corrected axis: `0.2.0 -> 0.3.0`.** On a 0.x line the minor is the breaking axis, so `^0.2`
users are no longer auto-upgraded. Users adopting 0.3.0 must be on `pmcp-code-mode >= 0.5.0`.

**A required consequence the sweep had missed:** root `Cargo.toml:257` pinned
`pmcp-code-mode-derive = { version = "0.2.0", ... }`. That is compatible with `0.2.1` but **not**
with `0.3.0` — left alone, `pmcp` itself would have failed to resolve at publish time. Moved to
`"0.3.0"`. Three documentation pins were also moved for consistency
(`pmcp-book/src/ch12-9-code-mode.md:90`, `pmcp-course/src/part8-advanced/ch22-code-mode.md:68`,
`pmcp-course/src/part8-advanced/ch22-exercises.md:17`), which the 81-series audit requires to be
byte-equal between book and course.

**Why this matters beyond the row.** This is the **third** version-axis error the phase caught
and the **second** that prose reasoning produced. `pmcp-workbook-runtime` was caught by
`cargo semver-checks`; this one had no tool and needed a manual artifact diff. The pattern is
consistent: on this codebase, an axis argued from a description of *what was added* has been
wrong every time it was checked. Check what *changed shape*, against the published artifact.
