---
phase: 124-release-publish-order
plan: 05
subsystem: infra
tags: [release, crates-io, versioning, semver, cargo, publish-order, halted]

requires:
  - phase: 124-03
    provides: "the closed four-crate authorised bump list and the mcp-tester prohibition this plan executes against"
  - phase: 124-04
    provides: "the caret exception for PATCH bumps and the one-set rule, both of which this plan's blocker turns on"
provides:
  - "A measured refutation that plan 03's authorised bump list is executable as written: pmcp-workbook-runtime 0.1.0 -> 0.2.0 is semver-INCOMPATIBLE with four in-workspace requirement sites, one of which belongs to a crate that cannot ship a change"
  - "D-04 discharged by measurement: pmcp-package ships 0.3.0 AS-IS (docs + tests + fixtures only since 6430afae), with all nine emitters verified mutually consistent and both pin tripwires green"
  - "A latent published-artifact defect in pmcp-server-toolkit 0.1.2, independent of this phase's bumps"
  - "Four WINDOWS entries (#49-#52), three of them defective acceptance criteria in this plan"
affects: [124-05, 124-06, 124-07, release-workflow]

actuals:
  tokens: 79000
  tasks: 4
  commits: 6

tech-stack:
  added: []
  patterns:
    - "Probe the consequence before consuming the version: temporarily apply the bump, run a FULL cargo resolution (not --no-deps, which does not resolve and returned a false green here), read the error, restore. The blocker is then measured rather than argued"
    - "Partition a per-manifest keyed diff into [package].version rows and dependency-requirement rows — they answer to different authorities (the closed authorised list vs the discovered consequence set) and conflating them makes the assertion unsatisfiable"
    - "Classify a source diff by counting ADDED lines that are neither doc comment nor blank; 0 is the docs-only verdict, and it is a number rather than a reading"

key-files:
  created:
    - .planning/phases/124-release-publish-order/124-05-SUMMARY.md
  modified:
    - .planning/WINDOWS.md

key-decisions:
  - "HALTED before any version edit rather than consuming pmcp-workbook-dialect 0.1.1, which plan 03's closed list does not authorise. Plan 05 Task 2 Step C2 prescribes exactly this ('stop and return to the user rather than consuming unauthorised version numbers') and CONTEXT D-07 makes version consumption a one-way door"
  - "pmcp-package ships 0.3.0 as-is (D-04 Step B). Both source files touched since 6430afae are 100% doc-comment additions: 0 added non-doc non-blank lines in each"
  - "No partial commit of the authorised set. Three of the four bumps are independently safe, but plan 05's must_have requires all authorised version changes in ONE commit, so committing 3 of 4 would create precisely the half-moved state the one-set rule exists to prevent"
  - "Recorded the three defective acceptance criteria rather than working around them silently — one of them (CR-01) fails at the UNMODIFIED base and would lead an executor to 'fix' a compliant manifest"

patterns-established:
  - "A closed authorised list is only closed with respect to what its author could see; an executor that discovers a mechanically-forced fifth member must escalate, not extend it"

requirements-completed: [PKGR-01]

coverage:
  - id: D1
    description: "D-04's audit procedure executed: pmcp-package's ship version determined by measurement to be 0.3.0 as-is, with the second path-log commit (85ee222f) checked for real source content rather than inferred"
    verification:
      - kind: integration
        ref: "git diff --numstat 6430afae..HEAD -- crates/pmcp-package -> 21 files; only src/oci/mod.rs (+154) and src/package/server.rs (+22/-5) are under src/; ADDED lines that are neither doc-comment nor blank = 0 in BOTH"
        status: pass
    human_judgment: false
  - id: D2
    description: "All nine pmcp-package version emitters verified mutually consistent on the 0.3 line, with row 9 path-only per CR-01"
    verification:
      - kind: integration
        ref: "cargo test -p cargo-pmcp --test pmcp_package_pin -> exit 0, 'test result: ok. 1 passed'"
        status: pass
      - kind: integration
        ref: "cargo test -p cargo-pmcp --lib emitted_package_requirement_matches_workspace_major_minor_line -> exit 0, '1 passed; 524 filtered out' (NON-ZERO test count asserted)"
        status: pass
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test pmcp_package_pin -> 2 passed (pmcp_package_dev_dep_is_path_only)"
        status: pass
    human_judgment: false
  - id: D3
    description: "The blocker is measured, not argued: bumping pmcp-workbook-runtime to 0.2.0 with the four requirement sites unmoved fails cargo resolution"
    verification:
      - kind: integration
        ref: "temporary bump + `cargo metadata --format-version 1 --offline` -> EXIT 101, 'failed to select a version for the requirement pmcp-workbook-runtime = \"^0.1.0\" ... required by package pmcp-server-toolkit v0.1.2'; control run at the unmodified tree exits 0; manifest restored, `git status --short` empty"
        status: pass
    human_judgment: false
  - id: D4
    description: "The authorised bump set is NOT executable as recorded — pmcp-workbook-dialect needs a version number plan 03's closed list does not authorise"
    verification: []
    human_judgment: true
    resolution: "RESOLVED round 3 — Option A. See ROUND 3."
    rationale: "Consuming a crates.io version number is permanent and one-way (CONTEXT D-07). Plan 03 reserved this class of decision to the user at a gate=\"blocking-human\" checkpoint, and plan 05's own Task 2 Step C2 instructs the executor to stop rather than extend the list. Three distinct resolutions exist with different permanent costs; choosing among them is the user's call, not the executor's."
  - id: D5
    description: "Tasks 1 (bumps), 3 (CHANGELOG) and 4 (release manifest) are NOT delivered"
    verification: []
    human_judgment: true
    resolution: "RESOLVED round 3 — Tasks 1, 3 and 4 all delivered. See ROUND 3."
    rationale: "Blocked on D4. The CHANGELOG section must name every crate that ships and the release manifest's expected_new set is the authorised list plus consequence bumps — both are unwritable until the bump set is final."
  - id: D6
    description: "Round 2: the amended axis (pmcp-workbook-runtime 0.1.1) is refuted by the published-baseline semver verdict — render_xlsx changed arity, so the crate needs a breaking bump"
    verification:
      - kind: integration
        ref: "cargo semver-checks check-release -p pmcp-workbook-runtime --baseline-version 0.1.0 -> EXIT 100, function_parameter_count_changed: render_xlsx 2 -> 3 params at src/render/mod.rs:270; 'semver requires new major version: 1 major and 0 minor checks failed'"
        status: pass
      - kind: integration
        ref: "pmcp EXIT 0 '(patch change)' 223 pass/30 skip; pmcp-workbook-compiler EXIT 0 'no semver update required'; pmcp-code-mode-derive EXIT 101 'no library target' (proc-macro, not checkable)"
        status: pass
    human_judgment: false
  - id: D7
    description: "Round 2: a third compiler-invisible version emitter (PMCP_VERSION in the workbook-server scaffold) enumerated and proven guarded, by a name-anchored sweep over all tracked files rather than by trusting a plan's files_modified"
    verification:
      - kind: integration
        ref: "negative control: constant left at 2.19.0 with root at 2.19.1 -> `cargo build -p cargo-pmcp` EXIT 0 while `cargo test -p cargo-pmcp --lib emitted_pmcp_version_matches_workspace_pin` EXIT 101; restored"
        status: pass
      - kind: integration
        ref: "pin-necessity check with positive control: compiler 0/0 and dialect 0/0 new-symbol uses vs pmcp-server-toolkit 3 imports / 10 qualified refs"
        status: pass
    human_judgment: false
  - id: D8
    description: "Round 2: row 2 of the authorised set is OPEN again; options A and B remain, C is eliminated"
    verification: []
    human_judgment: true
    resolution: "RESOLVED round 3 — Option A; runtime ships 0.2.0."
    rationale: "Option C was chosen on a premise measurement refutes. Restoring 0.2.0 restores the round-1 collision, which needs the user's call on whether pmcp-workbook-dialect 0.1.1 is authorised. Version-number consumption is one-way (CONTEXT D-07)."

  - id: D9
    description: "Round 3: the five authorised versions and all ten of their emitters land in ONE commit, with the [package].version axis matching the authorised map exactly and the requirement axis matching the Cargo-forced consequence set exactly"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "per-manifest keyed table -> 9 rows: 5 [package].version (the authorised map) + 4 pmcp-workbook-runtime requirement rows; cross-check confirms exactly {pmcp, pmcp-workbook-runtime, pmcp-code-mode-derive, pmcp-workbook-compiler, pmcp-workbook-dialect} moved vs HEAD"
        status: pass
      - kind: integration
        ref: "cargo metadata --offline full resolve EXIT 0; five scaffold/pin tripwires EXIT 0 with non-zero test counts; RUSTFLAGS='' make quality-gate EXIT 0 via status file, 13214-line log, banner present, 0 truncation markers"
        status: pass
      - kind: integration
        ref: "semver-checks per crate vs crates.io baseline: pmcp EXIT 0 'patch change'; pmcp-workbook-runtime EXIT 0 'major change'; pmcp-workbook-compiler EXIT 0; pmcp-workbook-dialect EXIT 0"
        status: pass
    human_judgment: false
  - id: D10
    description: "Round 3 Task 3: the 2.19.1 CHANGELOG section exists AND release.yml can actually extract it — the extractor was broken and is fixed"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "workflow's ORIGINAL awk -> 0 bytes for v2.19.1, v2.19.0 and v2.18.0 (undefined \\[ escape in -v makes [2.19.1] a character class under BSD awk 20200816)"
        status: pass
      - kind: integration
        ref: "FIXED extractor (index(), bare version, fail-closed guard) -> v2.19.1 4499 bytes/67 lines; v2.19.0 8112; v2.18.0 6365; negative control 9.9.9 -> 0 bytes; boundary: '## [2.19.0]' count 0 inside the extraction"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/CHANGELOG.md gains '## [0.3.0]'; headings now 0.3.0 / 0.2.0 / 0.1.0"
        status: pass
    human_judgment: false
  - id: D11
    description: "Round 3 Task 4: 124-expected-release.json is committed, parses, and its two crate sets are disjoint and complete"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "plan verify: tag == v2.19.1, expected_new 14, expected_skips 11, disjoint, union 25; every expected_skips entry carries a non-empty reason"
        status: pass
      - kind: integration
        ref: "cross-check vs on-disk manifests: 25/25 crates match the version the JSON claims, and exactly the five authorised crates differ from HEAD"
        status: pass
    human_judgment: false
  - id: D12
    description: "pmcp-code-mode-derive 0.2.1's semver axis is UNVERIFIABLE by tooling and is recorded as such rather than left silent"
    verification: []
    human_judgment: true
    rationale: "cargo semver-checks exits 101 with 'no library target' — a proc-macro has no API surface to compare, and the change is in emitted code. The patch axis rests on plan 03's reasoning alone, and this phase twice found that class of reasoning wrong about an axis. A human should confirm the axis before the tag if it matters."
  - id: D13
    description: "The historical blast radius of the release.yml awk bug on CI is unmeasured"
    verification: []
    human_judgment: true
    rationale: "Only BSD awk 20200816 was measured (0 bytes for three past releases). CI runs ubuntu-latest/mawk; neither mawk nor gawk is installed here, so whether past GitHub Releases actually shipped empty notes is not established. The fix is implementation-independent; the historical question needs a Linux runner. WINDOWS #56."

duration: ~165 min (three rounds)
completed: 2026-08-27
status: complete
---

# Phase 124 Plan 05: Consume the Authorised Versions — COMPLETE (after two measured halts)

**ROUND 2 (Option C): `cargo semver-checks` refutes Option C's premise — `render_xlsx` changed arity from 2 to 3 parameters, so `pmcp-workbook-runtime` needs a BREAKING bump (0.2.0 on a 0.x line), not 0.1.1; shipping 0.1.1 would silently break every `^0.1.0` consumer. Row 2 is open again and options A/B remain. ROUND 1: the list is not executable as recorded — `pmcp-workbook-runtime` 0.1.0 -> 0.2.0 is semver-INCOMPATIBLE with four in-workspace requirement sites (measured: `cargo metadata` exits 101), and moving them forces a fifth crate — `pmcp-workbook-dialect` — to consume a version number nobody authorised. No version literal was edited. Separately, D-04 is fully discharged by measurement: `pmcp-package` ships 0.3.0 as-is, all nine emitters consistent, all three tripwires green.**

> **STATUS: HALTED at a `gate="blocking-human"` decision checkpoint.** Task 2's audit half
> is complete and needs no user input. Task 1, Task 3 and Task 4 are blocked on one
> decision. The working tree is byte-identical to the base commit — `git status --short`
> is empty and the per-manifest keyed table reports NO DIFFERING ROWS.

---

# ROUND 3 — resumed under Option A: COMPLETE

> Rounds 1 and 2 are preserved below. Read this section for what shipped; read those for how
> the number was arrived at, which is the part worth keeping.

**Option A.** `pmcp-workbook-runtime` returns to **0.2.0** — the number plan 03 originally
chose, now for the demonstrated reason rather than the convention — and
`pmcp-workbook-dialect` **0.1.0 → 0.1.1** is authorised as the fifth row. All four tasks are
complete.

## The final set, as consumed

| # | Crate | From | To | `cargo semver-checks` (baseline built from crates.io) |
|---|---|---|---|---|
| 1 | `pmcp` | 2.19.0 | **2.19.1** | EXIT 0 — "(patch change)", 223 checks: 223 pass / 30 skip |
| 2 | `pmcp-workbook-runtime` | 0.1.0 | **0.2.0** | EXIT 0 — **"(major change)"**; 0.1.1 refuted at EXIT 100 |
| 3 | `pmcp-code-mode-derive` | 0.2.0 | **0.2.1** | EXIT 101 — **UNVERIFIABLE** (see below) |
| 4 | `pmcp-workbook-compiler` | 0.1.0 | **0.1.1** | EXIT 0 — "no semver update required" |
| 5 | `pmcp-workbook-dialect` | 0.1.0 | **0.1.1** | EXIT 0 — pure re-pin, no source change |

**`pmcp-code-mode-derive` remains UNVERIFIED and the other four rows' green verdicts do not
launder it.** It is a proc-macro with no library target, so `semver-checks` exits 101 —
there is no API surface to compare, and its change is in *emitted* code, which no API check
can see. The patch axis rests on plan 03's reasoning alone (the macro's own API does not
move). This phase has now twice found that class of reasoning wrong about an axis, so the
honest record is "unverified, and here is why it cannot be verified".

## I re-derived the edit set rather than replaying it — and it is TEN, not eight

The instruction to re-derive was worth following: restoring 0.2.0 adds not only the dialect
`[package].version` but **two further Cargo-forced pin moves** (`pmcp-workbook-compiler:41`
and `pmcp-workbook-dialect:25`) that 0.1.1 did not require. Round 2's seven became **ten**,
not eight.

| Group | Count | Edits |
|---|---|---|
| `[package].version` | 5 | `Cargo.toml:3`; `pmcp-workbook-runtime:3`; `pmcp-code-mode-derive:3`; `pmcp-workbook-compiler:3`; `pmcp-workbook-dialect:3` |
| `pmcp-workbook-runtime` requirement pins | 4 | `pmcp-server-toolkit:81`, `:202`; `pmcp-workbook-compiler:41`; `pmcp-workbook-dialect:25` — all → `"0.2.0"` |
| Compiler-invisible constant | 1 | `cargo-pmcp/src/templates/workbook_server.rs:53` `PMCP_VERSION` → `"2.19.1"` |

### Two of the four pins are necessary; two are superstitious-looking but Cargo-forced

`^0.1.0` does not admit `0.2.0`, so Cargo forces all four (round 1 measured `cargo metadata`
EXIT 101 otherwise). But *why* they move differs, and the distinction is the difference
between a necessary bound and a superstitious one:

| Site | Moves because | new-symbol imports / qualified refs |
|---|---|---|
| `pmcp-server-toolkit/Cargo.toml:81` | **CODE** — its source needs the new API | **3 / 10** |
| `pmcp-server-toolkit/Cargo.toml:202` | **CODE** — same | same |
| `pmcp-workbook-compiler/Cargo.toml:41` | Cargo-forced only | **0 / 0** |
| `pmcp-workbook-dialect/Cargo.toml:25` | Cargo-forced only | **0 / 0** |

The 0/0 readings were taken with `pmcp-server-toolkit` as a **positive control**, so the
enumeration is not vacuous — and only after catching a false positive in my own first grep,
which reported a spurious **9** for the compiler because `grep -n` prefixes each line with its
path and the pattern matched `reconcile` in `.../src/reconcile/drift.rs:17:` rather than in
the import list. The compiler has its own crate-local `reconcile` module, which is what made
the wrong number plausible. Re-run with `grep -h` (path prefix stripped) it is 0. A reader who
knows the first grep was wrong has grounds to trust the second: the control is what exposed
it, since 9-vs-3 against a control known to be *larger* was the wrong shape.

`pmcp-server-toolkit`'s move also fixes the latent publish defect found in round 1: 0.1.2
requested `^0.1.0` while `src/workbook/handler.rs` used `RenderMode`, `reconcile_reference`
and `ReconcileReport`, none of which exist in the published 0.1.0. `workbook` is not a default
feature (`default = ["code-mode"]`), so a publish-verification build would not have caught it.

### Deliberately unchanged, asserted line by line

`cargo-pmcp:68` and `crates/mcp-tester:21` still pin `pmcp = "2.19.0"` (caret exception,
patch-only). All four `mcp-tester` before-publish pins still read `0.8.0`. `cargo-pmcp:75`
(`pmcp-workbook-compiler ^0.1.0`) and `pmcp-workbook-compiler:45`
(`pmcp-workbook-dialect ^0.1.0`) stay — both already admit their crate's 0.1.1.
`pmcp-package` 0.3.0 and all nine of its emitters stay. `PMCP_AGENT_VERSION`,
`PMCP_PACKAGE_VERSION_REQ` and `TOOLKIT_VERSION` stay, all three guarded and green.

## Task 3 — the CHANGELOG, and the extractor that could never have read it

The plan asks for the section **and** for proof that `release.yml` can extract it. I ran the
workflow's own program. It returned **zero bytes**.

**The bug, and it is not specific to this release.** `release.yml:34` built the matcher as
`awk -v ver="## \[$VERSION_NO_V\]"`. awk applies string-literal escape processing to `-v`
assignments, and `\[` is an **undefined** escape. BSD awk 20200816 (measured) drops the
backslash, so `ver` arrived as `## [2.19.1]` and `$0 ~ ver` read `[2.19.1]` as a **character
class** matching one of `{2,.,1,9}`. The heading has `[` in that position, so it never
matched. Proof it is the matcher and not the heading: the literal regex `/^## \[2.19.1\]/`
matches the same line.

```
workflow awk for v2.19.1 -> 0 bytes
workflow awk for v2.19.0 -> 0 bytes
workflow awk for v2.18.0 -> 0 bytes
awk -v ver="## \\[2.19.1\\]" 'BEGIN{print ver}'   ->   ## [2.19.1]
```

Every past release, not just this one. This is threat **T-124-08** realised exactly as the
plan describes it: an empty-notes GitHub Release created at exit 0, silently.

**Caveat, stated because it bounds the claim.** Only **one** awk was measured. CI runs
`ubuntu-latest`, whose `awk` is mawk; neither mawk nor gawk is installed here. So whether CI
has actually been shipping empty release notes is **not established** — only that the
construction is unsafe and fails on the implementation I could test. Recorded as WINDOWS #56
with that residual named.

**The fix** (deviation Rule 1) passes the bare version — no backslashes, so nothing to escape
— and matches with `index()` rather than a regex, plus a fail-closed guard:

```awk
CHANGELOG=$(awk -v ver="$VERSION_NO_V" '
  index($0, "## [" ver "]") == 1 {p=1; next}
  /^## \[/ && p {exit}
  p {print}
' CHANGELOG.md)
if [ -z "$CHANGELOG" ]; then
  echo "::error::CHANGELOG.md has no '## [$VERSION_NO_V]' section - refusing to create a release with empty notes" >&2
  exit 1
fi
```

Verified with the fixed program, including a negative control and a boundary check:

| Input | Result |
|---|---|
| `v2.19.1` | **4,499 bytes / 67 lines** — `test -s` PASS |
| `v2.19.0` | 8,112 bytes — the fix repairs the past too |
| `v2.18.0` | 6,365 bytes |
| `9.9.9` (negative control) | **0 bytes** |
| `## [2.19.0]` inside the 2.19.1 extraction | **0** — stops at the next heading |

> **My own first verification was a false green, and it is worth recording.** I wrapped the
> extraction as `CHANGELOG=$(awk ...)` then `printf '%s\n' "$CHANGELOG"`, which writes a
> 1-byte newline when the extraction is empty — so `test -s` PASSED on zero content and
> reported "1 byte, non-empty: YES". The plan's own `<verify>` for Task 3 uses both the broken
> awk construction and a bare `test -s`, so it could not have caught the bug it exists to
> catch. WINDOWS #57. Extraction checks must write raw output and assert a realistic byte
> count, not merely non-emptiness.

**`crates/pmcp-package/CHANGELOG.md`** gained its missing `## [0.3.0]` section. It stopped at
`[0.2.0] - Unreleased` while the crate ships 0.3.0, so the attestation-carriage line's five
source-breaking changes had **no entry at all** — including that `PackageError` is not
`#[non_exhaustive]`, so its two new variants break downstream exhaustive matches. The retained
0.2.0 entry now carries a note that crates.io went 0.1.1 → 0.3.0 and that line never
published.

## Task 4 — the committed release manifest

`.planning/phases/124-release-publish-order/124-expected-release.json`, generated from this
wave's measurements and **committed**, so it travels into the release PR and is readable from
merged `main` — unlike the plan summaries plan 07 would otherwise have to read, which are
written after the events they describe.

| Set | Count | Contents |
|---|---|---|
| `expected_new` | **14** | the 5 authorised bumps + the 9 already ahead of the registry |
| `expected_skips` | **11** | in-tree == published; `release.yml` skips them deliberately, each with a `reason` |
| union | **25** | every publishable crate the plan-03 sweep enumerated; the sets are disjoint |

Also carries `registry_baselines` with per-crate provenance (including the deliberately
**uncorroborated** `pmcp-widget-utils` baseline, so its refutation stays visible),
`semver_verdicts` with `pmcp-code-mode-derive` marked UNVERIFIABLE, and
`authorised_exceptions` naming the `mcp-tester` disposition, the caret non-bump, the dialect
re-pin authorisation, and the code-vs-Cargo-forced pin distinction.

**Cross-checked rather than asserted:** every crate named in either set exists in the
workspace with exactly the version the JSON claims (**25/25**), and the set of crates whose
`[package].version` differs from `HEAD` is **exactly** the five authorised.

## Verification

**Per-manifest keyed table**, partitioned per WINDOWS #51 — 9 rows, and each axis matches its
own authority exactly:

```
[package].version rows (5) — EQUAL the authorised map, no extras, no omissions:
  Cargo.toml                                2.19.0 -> 2.19.1
  crates/pmcp-workbook-runtime/Cargo.toml    0.1.0 -> 0.2.0
  crates/pmcp-code-mode-derive/Cargo.toml    0.2.0 -> 0.2.1
  crates/pmcp-workbook-compiler/Cargo.toml   0.1.0 -> 0.1.1
  crates/pmcp-workbook-dialect/Cargo.toml    0.1.0 -> 0.1.1
dependency-requirement rows (4) — EQUAL the forced consequence set:
  pmcp-server-toolkit    dependencies.pmcp-workbook-runtime      0.1.0 -> 0.2.0
  pmcp-server-toolkit    dev-dependencies.pmcp-workbook-runtime  0.1.0 -> 0.2.0
  pmcp-workbook-compiler dependencies.pmcp-workbook-runtime      0.1.0 -> 0.2.0
  pmcp-workbook-dialect  dependencies.pmcp-workbook-runtime      0.1.0 -> 0.2.0
```

| Check | Result |
|---|---|
| `cargo metadata --format-version 1 --offline` (full resolve) | EXIT 0 |
| `cargo test -p cargo-pmcp --lib emitted_pmcp_version_matches_workspace_pin` | EXIT 0, `1 passed; 524 filtered out` |
| `cargo test -p cargo-pmcp --lib emitted_toolkit_version_matches_workspace_pin` | EXIT 0, `1 passed; 524 filtered out` |
| `cargo test -p cargo-pmcp --lib emitted_package_requirement_matches_workspace_major_minor_line` | EXIT 0, `1 passed; 524 filtered out` |
| `cargo test -p cargo-pmcp --test pmcp_package_pin` | EXIT 0, `1 passed` |
| `cargo test -p pmcp-openapi-server --test pmcp_package_pin` | EXIT 0, `2 passed` |
| CR-01: non-comment `pmcp-package =` lines carrying a version key | **0** |
| `RUSTFLAGS="" make quality-gate` | **EXIT 0** |

**The gate result is trustworthy on three independent conditions**, per WINDOWS #47 — a
redirect alone is not sufficient, because the output proxy truncates a capture *and*
substitutes its own status:

1. status file, written by the same process that ran `make`: **0**
2. `ALL TOYOTA WAY QUALITY CHECKS PASSED` present: **1 occurrence**
3. **13,214 lines / 861,784 bytes**, `lines truncated` marker count **0**, log ends in the
   success banner

> **One caveat on that green.** The gate's `1. FUZZ Testing validation` leg is **vacuous on a
> stable toolchain**: it emits `error: the option Z is only accepted on the nightly compiler`
> (cargo-fuzz needs `-Zsanitizer=address`) and the gate still reports PASSED. Pre-existing and
> out of scope, but the banner should not be read as fuzz coverage. WINDOWS #58.

## Task Commits

| # | Task | Commit | What |
|---|---|---|---|
| 1 | — | `d0ccccde` | `docs(124-03)`: addendum 3 — the Option A resolution record |
| 2 | **Task 1** | `f1216928` | `chore(124-05)`: **all ten version edits in ONE commit** (Step G one-set rule) |
| 3 | **Task 3** | `1b4bb9ae` | `docs(124-05)`: the 2.19.1 CHANGELOG, the pmcp-package 0.3.0 section, and the release.yml awk fix |
| 4 | **Task 4** | `363b100f` | `docs(124-05)`: `124-expected-release.json` |

Round 1 `8f1c2dee` and round 2 `e6c9e2b7` recorded the two halts.

**Task 2 produced no commit by design** — its audit verdict is "ship `pmcp-package` 0.3.0
as-is", so there was nothing to change. Its findings are in round 1 below and stand
unmodified.

## Files Created/Modified in round 3

- `Cargo.toml`, `crates/pmcp-workbook-runtime/Cargo.toml`,
  `crates/pmcp-code-mode-derive/Cargo.toml`, `crates/pmcp-workbook-compiler/Cargo.toml`,
  `crates/pmcp-workbook-dialect/Cargo.toml` — the five `[package].version` bumps.
- `crates/pmcp-server-toolkit/Cargo.toml` — two runtime pins → `0.2.0` (code grounds).
- `cargo-pmcp/src/templates/workbook_server.rs` — `PMCP_VERSION` → `2.19.1`.
- `CHANGELOG.md` — new `## [2.19.1] - 2026-08-27` section.
- `crates/pmcp-package/CHANGELOG.md` — new `## [0.3.0]` section.
- `.github/workflows/release.yml` — the extractor fix plus a fail-closed empty-notes guard.
- `.planning/phases/124-release-publish-order/124-expected-release.json` — **new**.
- `.planning/phases/124-release-publish-order/124-03-SUMMARY.md` — addendum 3.
- `.planning/WINDOWS.md` — entries #56–#58.

## Round-3 deviations

**1. [Rule 1 — Bug] `release.yml`'s changelog extractor never matched any heading**
- **Found during:** Task 3, running the workflow's own program as the plan requires
- **Issue / Fix / Verification:** as above. Fixed in `.github/workflows/release.yml`.
- **Committed in:** `1b4bb9ae` · **Recorded in:** WINDOWS #56 (with the mawk residual)

**2. [Rule 1 — Bug] My own first extraction check was a false green**
- **Issue:** `printf '%s\n' "$CHANGELOG"` turns an empty extraction into a 1-byte file, so
  `test -s` passes on zero content. The plan's `<verify>` has the same shape.
- **Fix:** write raw output; assert a realistic byte count and a negative control.
- **Recorded in:** WINDOWS #57

**3. [Rule 2 — Missing Critical] `crates/pmcp-package/CHANGELOG.md` had no 0.3.0 entry**
- **Fix:** added, with the five breaking changes and the never-published-0.2.0 note.
- **Committed in:** `1b4bb9ae`

**4. [Rule 1 — Bug] A false positive in my own pin-necessity grep**
- **Issue:** `grep -n` path prefixes made `reconcile` in a *file path* look like a new-symbol
  import, reporting 9 for `pmcp-workbook-compiler` instead of 0.
- **Fix:** re-run with `grep -h`; a positive control is what made the wrong shape visible.
- **Recorded in:** the round-3 narrative above, deliberately, so the corrected number is
  trustworthy rather than merely asserted.

**5. [Rule 2 — Missing Critical] The gate's fuzz leg is vacuous on stable**
- **Fix:** none — pre-existing and out of scope. Documented so the banner is not over-read.
- **Recorded in:** WINDOWS #58

**Total round-3 deviations:** 5 (3 Rule 1, 2 Rule 2). One is a real workflow bug fixed; two
are corrections to verification method, including one of my own; one is an out-of-scope
finding documented rather than fixed.

## Next Phase Readiness

**Plan 05 is complete and plan 06 is unblocked.**

- The tag to push is **`v2.19.1`**, matching the CHANGELOG heading with the leading `v`
  removed, and matching `124-expected-release.json`'s `tag`.
- Release notes are non-empty and were **read before the tag exists** — 4,499 bytes, proven
  with the workflow's own (now working) extractor.
- `124-expected-release.json` is committed, so plans 06 and 07 verify against an artifact that
  survives the merge.
- Carried forward for plan 07's pre-tag check: `expected_new` and `expected_skips` are
  **separate** sets. A crate in `expected_skips` having an unchanged version is CORRECT, not a
  defect — that was the flaw in the original pre-tag criterion.
- **Open, and named rather than left silent:** `pmcp-code-mode-derive` 0.2.1's axis is
  unverifiable by tooling; and whether CI's mawk shares BSD awk's escape behaviour — i.e. how
  many past releases actually shipped empty notes — is unmeasured (WINDOWS #56).

---


# ROUND 2 — resumed under Option C, halted again: Option C's premise is false

> **Read this before the round-1 write-up below.** The user chose **Option C**
> (`pmcp-workbook-runtime` -> **0.1.1** rather than 0.2.0) on the stated grounds that the
> change is "purely additive — a new `pub mod reconcile` plus six re-exports, breaking
> nothing". I applied the amended set, then ran the per-crate `cargo semver-checks` verdict
> Task 1 requires. **It refutes that premise.** The tree has been returned to a clean state
> and no version literal is currently changed.

## The refutation

```
$ cargo semver-checks check-release -p pmcp-workbook-runtime --baseline-version 0.1.0
    Checking pmcp-workbook-runtime v0.1.0 -> v0.1.1 (minor change)
--- failure function_parameter_count_changed: pub fn parameter count changed ---
  pmcp_workbook_runtime::render::render_xlsx now takes 3 parameters instead of 2,
  in crates/pmcp-workbook-runtime/src/render/mod.rs:270
     Summary semver requires new major version: 1 major and 0 minor checks failed
EXIT=100
```

`pub fn render_xlsx` gained a third parameter, `mode: RenderMode` — and `RenderMode` is itself
one of the symbols absent from the published 0.1.0, so the arity change and the new surface
arrived together:

```rust
pub fn render_xlsx(
    layout: &LayoutDescriptor,
    run: &RunResult,
    mode: RenderMode,                      // <- new third parameter
) -> Result<Vec<u8>, RenderError>
```

- **Public and reachable:** `pub mod render;` at `crates/pmcp-workbook-runtime/src/lib.rs:65`.
- **Called across a crate boundary:** `crates/pmcp-server-toolkit/src/workbook/render_resource.rs:42`
  (`use pmcp_workbook_runtime::render::render_xlsx;`) and `:108` (three-argument call).
- **Baseline is the real artifact:** semver-checks built `pmcp-workbook-runtime v0.1.0
  (baseline)` from crates.io, not from a tag.

On a 0.x line, "requires new major" means bumping the leftmost non-zero component — **0.2.0**.

**Why 0.1.1 would be actively wrong, not merely conservative.** `^0.1.0` admits `0.1.1`. Every
consumer pinned to the published 0.1.0 that calls `render_xlsx` would silently receive the
incompatible version on a fresh resolve and fail to compile. On a 0.x line the minor is the
breaking axis precisely to prevent that, and `pmcp-server-toolkit` is an in-repo instance of
the consumer shape in question.

**Both stated rationales for this row were wrong, in opposite directions.** Plan 03 said
"additive public API -> minor" — right answer, wrong reason. Option C said "purely additive ->
patch" — wrong answer, same wrong premise. RESEARCH described the delta as `pub mod reconcile`
plus six re-exports and nobody re-derived it. **No document in this phase cited `render_xlsx`
before now.** The per-crate semver-checks run is the check that catches this class, and Task 1
is the first step that required it for this crate.

## The other three rows are confirmed and unaffected

| Crate | Move | `cargo semver-checks` | Verdict |
|---|---|---|---|
| `pmcp` | 2.19.0 -> 2.19.1 | EXIT **0** — classified "(patch change)", 223 checks: 223 pass / 30 skip, "no semver update required" | **patch correct** |
| `pmcp-workbook-runtime` | 0.1.0 -> 0.1.1 | EXIT **100** — 1 major, 0 minor failed | **PATCH WRONG — needs 0.2.0** |
| `pmcp-code-mode-derive` | 0.2.0 -> 0.2.1 | EXIT **101** — *not checkable*: proc-macro, no library target, so there is no API surface to compare. Its change is in **emitted** code. | patch defensible on plan 03's reasoning; **unverifiable by tooling** — recorded as such rather than claimed as verified |
| `pmcp-workbook-compiler` | 0.1.0 -> 0.1.1 | EXIT **0** — "no semver update required" | **patch correct** |

## What this leaves open — the original collision returns

Restoring 0.2.0 restores round 1's blocker exactly: `^0.1.0` does not admit 0.2.0, so all four
pin sites are forced to move (measured in round 1: `cargo metadata` EXIT 101), and
`pmcp-workbook-dialect` — 0.1.0 in-tree **and** 0.1.0 published — then needs a version number
the authorised list does not contain.

**Option C is eliminated.** Options **A** and **B** from round 1 remain, unchanged in
substance:

| | Option | Consumes | Cost |
|---|---|---|---|
| **A** | Authorise `pmcp-workbook-dialect` 0.1.0 -> 0.1.1; move all four pins to `0.2.0` | the four **+ dialect 0.1.1** | One extra permanent number. Fully consistent published tree, no second runtime copy anywhere. What CLAUDE.md's Version Bump Rules prescribe literally. |
| **B** | Move all four pins to `0.2.0`, leave `pmcp-workbook-dialect` at 0.1.0 | the four | Dialect's edit never publishes; published dialect 0.1.0 keeps `^0.1.0` forever, so downstream trees can carry two runtime copies. Measured in round 1 to still *compile* (no runtime type crosses the dialect boundary; 0 workspace imports of its re-exports). Violates the crate family's "re-export, don't re-declare" keystone. |

A third possibility the measurement now makes visible, offered as fact rather than
recommendation: **`pmcp-workbook-dialect`'s cost under A is small.** Its entire public surface
names no runtime type, its only non-test runtime use is one re-export line, and nothing in the
workspace imports through it — so 0.1.1 would be a pure re-pin release. That is an argument
for A being cheap, not an argument that I should choose it.

## Work completed in round 2 (all of it re-verified, none of it committed as version changes)

### The full emitter enumeration — HOW, not just what

Method, recorded because the coordinator asked for it: over every **tracked** file
(`git ls-files`, so `target/` and build output are structurally excluded — 4,333 files), find
lines mentioning the crate by **either spelling** (hyphen or underscore) **and** containing a
semver-ish literal. Name-anchored rather than manifest-anchored, so it reaches constants,
scaffold templates, book/course prose and tests. For `pmcp` the crate name is too common to
anchor on, so that hunt is version-anchored on the literal `2.19.0` instead. Cross-checked
with a targeted sweep for `const [A-Z_]*VERSION[A-Z_]*: &str = "[0-9]` across `cargo-pmcp/`,
`crates/` and `src/`, and for `drifted from` to enumerate the guards.

**The find that no plan's `files_modified` contains:**
`cargo-pmcp/src/templates/workbook_server.rs:53` — `const PMCP_VERSION: &str = "2.19.0";`,
emitted into projects created by `cargo pmcp new --kind workbook-server`, and guarded by
**exact equality** against the root `[package].version` in
`emitted_pmcp_version_matches_workspace_pin`.

Negative control, the Phase-122 experiment re-run for this constant:

```
reverted constant to "2.19.0" while the root reads 2.19.1
cargo build -p cargo-pmcp                                          EXIT=0
cargo test -p cargo-pmcp --lib emitted_pmcp_version_matches...     EXIT=101
  assertion `left == right` failed: the scaffold's hardcoded pmcp version `2.19.0`
  drifted from the workspace-root pin `2.19.1` — bump PMCP_VERSION in workbook_server.rs
restored
```

A green build does not prove a complete bump — measured again, on a new instance.

**Complete emitter table for the amended set** (each row's action and the guard that enforces
it). Rows marked UNCHANGED are the ones a checklist silently drops.

| Crate | Emitter | Value | Action | Guard |
|---|---|---|---|---|
| `pmcp` | `Cargo.toml:3` `[package].version` | `2.19.0` | -> `2.19.1` | the bump itself |
| `pmcp` | `cargo-pmcp/src/templates/workbook_server.rs:53` `PMCP_VERSION` | `2.19.0` | -> `2.19.1` | **`emitted_pmcp_version_matches_workspace_pin` only — invisible to `cargo build`** |
| `pmcp` | `cargo-pmcp/Cargo.toml:68` | `"2.19.0"` | **UNCHANGED** — caret exception | none needed; `^2.19.0` admits 2.19.1 |
| `pmcp` | `crates/mcp-tester/Cargo.toml:21` | `"2.19.0"` | **UNCHANGED** — caret exception | same |
| `pmcp` | ~30 prose/`#[deprecated(since)]`/CHANGELOG occurrences | `2.19.0` | **UNCHANGED** — historical statements ("as of pmcp 2.19.0", "before 2.19.0") that describe when something happened | n/a |
| `pmcp-workbook-runtime` | `crates/pmcp-workbook-runtime/Cargo.toml:3` | `0.1.0` | **OPEN** (0.2.0 vs 0.1.1) | the four pins fail `cargo metadata` |
| `pmcp-workbook-runtime` | `crates/pmcp-server-toolkit/Cargo.toml:81` (optional dep) | `"0.1.0"` | -> the shipped version, **required** | `cargo metadata`; and the published-artifact defect below |
| `pmcp-workbook-runtime` | `crates/pmcp-server-toolkit/Cargo.toml:202` (dev-dep) | `"0.1.0"` | -> the shipped version, **required** | same |
| `pmcp-workbook-runtime` | `crates/pmcp-workbook-compiler/Cargo.toml:41` | `"0.1.0"` | **UNCHANGED under 0.1.1; forced to move under 0.2.0** | `cargo metadata` under 0.2.0 |
| `pmcp-workbook-runtime` | `crates/pmcp-workbook-dialect/Cargo.toml:25` | `"0.1.0"` | **UNCHANGED under 0.1.1; forced to move under 0.2.0** | same |
| `pmcp-code-mode-derive` | `crates/pmcp-code-mode-derive/Cargo.toml:3` | `0.2.0` | -> `0.2.1` | the bump itself |
| `pmcp-code-mode-derive` | root `Cargo.toml:257` dev-dep | `"0.2.0"` | **UNCHANGED** — `^0.2.0` admits 0.2.1 | none |
| `pmcp-code-mode-derive` | `pmcp-book/src/ch12-9-code-mode.md:90`, `pmcp-course/src/part8-advanced/ch22-code-mode.md:68`, `ch22-exercises.md:17` | `"0.2.0"` | **UNCHANGED** — doc-quoted caret floors that stay correct; tightening an unnecessary bound is its own defect | none (unguarded) |
| `pmcp-code-mode-derive` | `CHANGELOG.md:926` | `0.2.0` | **UNCHANGED** — historical record of the 0.2.0 publish | n/a |
| `pmcp-workbook-compiler` | `crates/pmcp-workbook-compiler/Cargo.toml:3` | `0.1.0` | -> `0.1.1` | the bump itself |
| `pmcp-workbook-compiler` | `cargo-pmcp/Cargo.toml:75` | `"0.1.0"` | **UNCHANGED** — `^0.1.0` admits 0.1.1 | none |

Also enumerated and confirmed **not** to move: `cargo-pmcp/src/templates/agent.rs:49`
`PMCP_AGENT_VERSION = "0.3.0"` (tracks `pmcp-agent`, not bumped), `:67`
`PMCP_PACKAGE_VERSION_REQ = "0.3"` (tracks `pmcp-package`, ships 0.3.0 as-is), and
`workbook_server.rs:59` `TOOLKIT_VERSION = "0.1.2"` (tracks `pmcp-server-toolkit`, not
bumped) — all three guarded, all three verified green.

### The pin question the coordinator asked me to prove rather than assume

Do `pmcp-workbook-compiler` / `-dialect` actually use the runtime API that is new since the
published 0.1.0? Measured two ways, with `pmcp-server-toolkit` as a positive control so the
enumeration is not vacuous:

| Crate | `use pmcp_workbook_runtime::{...}` lines naming a new symbol | `pmcp_workbook_runtime::<new>` path-qualified refs |
|---|---|---|
| `pmcp-workbook-compiler` | **0** | **0** |
| `pmcp-workbook-dialect` | **0** | **0** |
| `pmcp-server-toolkit` (control) | **3** | **10** |

New-symbol set: `reconcile`, `RenderMode`, `ReconcileReport`, `ToolReport`, `OutputRow`,
`seed_reference_inputs`, `reconcile_reference`. The control's three lines are
`handler.rs:32`, `render_resource.rs:43`, `render_uri.rs:42`, all importing `RenderMode`.

**Conclusion: neither the compiler's nor the dialect's pin should be tightened on code
grounds.** They move under 0.2.0 only because Cargo forces them to, not because their source
needs the new surface. `pmcp-server-toolkit`'s two pins move on code grounds and would move
under either axis.

> **A false positive of my own, recorded because it nearly stuck.** My first version of this
> check reported "9 new-symbol imports" for `pmcp-workbook-compiler`. All nine were `grep -n`
> output lines whose **file path** contained `reconcile`
> (`crates/pmcp-workbook-compiler/src/reconcile/drift.rs:17:...`) — the pattern matched the
> path prefix, not the import list. The compiler has its own crate-local `reconcile` module
> (`src/lib.rs:58`), which is what made the collision plausible. Re-run with the path prefix
> stripped (`grep -h`), the count is 0. A positive control is what made the difference
> visible: 9-vs-3 against a control known to be smaller was the wrong shape.

### The per-manifest keyed table under the amended set (before reverting)

Applied, measured, then reverted. Partitioned as WINDOWS #51 prescribes:

```
6 differing row(s):
  [package].version rows (4) — equal the amended authorised map EXACTLY:
    Cargo.toml                                  2.19.0 -> 2.19.1
    crates/pmcp-workbook-runtime/Cargo.toml      0.1.0 -> 0.1.1
    crates/pmcp-code-mode-derive/Cargo.toml      0.2.0 -> 0.2.1
    crates/pmcp-workbook-compiler/Cargo.toml     0.1.0 -> 0.1.1
  dependency-requirement rows (2) — the discovered consequence set:
    crates/pmcp-server-toolkit/Cargo.toml  dependencies.pmcp-workbook-runtime      0.1.0 -> 0.1.1
    crates/pmcp-server-toolkit/Cargo.toml  dev-dependencies.pmcp-workbook-runtime  0.1.0 -> 0.1.1
```

No extra rows, no missing rows on either axis. `cargo metadata` (full resolve) EXIT 0, and all
three scaffold drift tests green with non-zero counts (`1 passed; 524 filtered out` each).

### Current tree state

**Clean of version changes.** All seven edits were reverted; `git status --short` shows only
the two `.planning/` documents this round writes. The keyed table reports `NO DIFFERING ROWS`
and `cargo metadata` EXIT 0. Nothing half-moved is committed or left behind — the one-set rule
is preserved by there being no version commit at all. Re-applying is seven line-anchored
`sed` substitutions, scripted and reproduced above.

`RUSTFLAGS="" make quality-gate` was **not** run this round: with the tree byte-identical to
the base on every non-`.planning` path, it would measure nothing. It must be run by whichever
run lands the version edits, and its log must be confirmed to end in
`ALL TOYOTA WAY QUALITY CHECKS PASSED` (~11.5k–12k lines) rather than a truncation marker.

### Carried forward from round 1, unchanged and not re-litigated

`pmcp-package` ships **0.3.0 as-is** (docs + tests + fixtures only since `6430afae`: 0 added
non-doc, non-blank lines in both touched source files). All nine emitters mutually consistent
on the 0.3 line with row 9 path-only. Three tripwires green. `mcp-tester` unchanged at all
four before-publish pin sites. The caret non-bump of `crates/mcp-tester` and `cargo-pmcp`
against `pmcp` holds.

### Still not delivered

Tasks 3 (CHANGELOG `## [2.19.1]`, plus the `crates/pmcp-package/CHANGELOG.md` gap at
`[0.2.0] - Unreleased` while shipping 0.3.0) and 4 (`124-expected-release.json`). Both must
name the final shipped versions, so both remain blocked on row 2.

### New ledger entries this round

| # | Kind | Subject |
|---|---|---|
| 53 | deviation | `render_xlsx` arity change makes `pmcp-workbook-runtime` a breaking bump; three documents called it additive; semver-checks per crate against the published baseline is the check that catches it |
| 54 | deviation | A third compiler-invisible emitter (`PMCP_VERSION`) that no plan's `files_modified` lists, with the build-0/test-101 negative control |
| 55 | stub | Eight unguarded stale pmcp-family version floors across five scaffold templates — found, deliberately not changed, recorded so they stop being invisible |

---

## Performance

- **Duration:** ~55 min
- **Started:** 2026-08-27T19:00Z (approx.)
- **Halted:** 2026-08-27T19:20Z
- **Tasks:** 1 of 4 complete (Task 2's audit); 3 blocked
- **Files modified:** 1 (`.planning/WINDOWS.md`) + this SUMMARY. **Zero version literals changed.**

---

## THE BLOCKER — `pmcp-workbook-runtime` 0.2.0 forces an unauthorised fifth bump

### What plan 03 authorised

| # | Crate | From | To | Axis |
|---|---|---|---|---|
| 1 | `pmcp` | 2.19.0 | 2.19.1 | patch |
| 2 | `pmcp-workbook-runtime` | 0.1.0 | **0.2.0** | minor (pre-1.0 additive) |
| 3 | `pmcp-code-mode-derive` | 0.2.0 | 0.2.1 | patch |
| 4 | `pmcp-workbook-compiler` | 0.1.0 | 0.1.1 | patch |

"No other crate's version may be touched by plan 05."

### What the workspace-wide requirement search found

Task 1's action mandates a per-bumped-crate search of every workspace manifest rather than
recollection. Run over all 61 tracked `Cargo.toml` files, parsing `[dependencies]`,
`[dev-dependencies]`, `[build-dependencies]`, `[workspace.*]` and `[target.*]`:

| Bumped crate | Move | In-workspace requirement sites | Verdict |
|---|---|---|---|
| `pmcp` 2.19.0 -> 2.19.1 | patch | 36 sites (16 versioned, 20 path-only) — highest floor `2.19.0` at `cargo-pmcp:68` and `crates/mcp-tester:21` | **all COMPATIBLE.** The caret exception holds exactly as plan 04 recorded it. No pin moves. |
| `pmcp-code-mode-derive` 0.2.0 -> 0.2.1 | patch | 1 site: root `Cargo.toml` `[dev-dependencies]` req `0.2.0` | **COMPATIBLE.** No pin move. |
| `pmcp-workbook-compiler` 0.1.0 -> 0.1.1 | patch | 1 site: `cargo-pmcp/Cargo.toml:75` req `0.1.0` (+1 path-only in `fuzz/`) | **COMPATIBLE.** No pin move. |
| **`pmcp-workbook-runtime` 0.1.0 -> 0.2.0** | **minor, pre-1.0 = INCOMPATIBLE** | **4 versioned sites in 3 manifests** | **ALL FOUR MUST MOVE.** |

The four sites, all requiring `"0.1.0"`:

| Site | Table | Owning crate | Owner's version status |
|---|---|---|---|
| `crates/pmcp-server-toolkit/Cargo.toml:81` | `[dependencies]`, `optional` | `pmcp-server-toolkit` | 0.1.2 in-tree vs 0.1.1 published — **already carries a new number** |
| `crates/pmcp-server-toolkit/Cargo.toml:202` | `[dev-dependencies]` | same | same |
| `crates/pmcp-workbook-compiler/Cargo.toml:41` | `[dependencies]` | `pmcp-workbook-compiler` | authorised 0.1.0 -> **0.1.1** |
| `crates/pmcp-workbook-dialect/Cargo.toml:25` | `[dependencies]` | `pmcp-workbook-dialect` | **0.1.0 in-tree == 0.1.0 published — no new number, and none authorised** |

(`crates/pmcp-workbook-compiler/fuzz/Cargo.toml` also depends on the runtime but is
path-only and is not a workspace member, so it is unaffected.)

### The pins MUST move — measured, not argued

Applied the bump temporarily and ran a FULL resolution, then restored:

```
=== CONTROL: full resolve at the UNMODIFIED tree
CONTROL_EXIT=0

=== PROBE: runtime bumped to 0.2.0, consumers still require "0.1.0"
PROBE_EXIT=101
error: failed to select a version for the requirement `pmcp-workbook-runtime = "^0.1.0"`
candidate versions found which didn't match: 0.2.0
location searched: .../crates/pmcp-workbook-runtime
required by package `pmcp-server-toolkit v0.1.2 (.../crates/pmcp-server-toolkit)`

restored; working-tree clean for that manifest: yes
```

> **Method note (a false green worth recording).** The plan's own Task 1 `<verify>` uses
> `cargo metadata --no-deps`. `--no-deps` does **not resolve**, so it returned **EXIT=0**
> against the very tree that a full resolve rejects with 101. The `--no-deps` form cannot
> detect this class of breakage at all.

### Why moving the pins forces a fifth version number

CLAUDE.md's *Version Bump Rules* — as amended by plan 04's caret exception, which covers
**PATCH bumps only** — require a crate that pins a bumped dependency to be bumped itself when
the move is semver-incompatible. `0.1.0 -> 0.2.0` on a pre-1.0 line is incompatible. So:

| Pinning crate | Needs its own bump? | Satisfied? |
|---|---|---|
| `pmcp-server-toolkit` | yes | **already** — 0.1.2 is unpublished and ships new at this tag |
| `pmcp-workbook-compiler` | yes | **already** — authorised 0.1.1 |
| `pmcp-workbook-dialect` | yes | **NO** — it is 0.1.0 == published 0.1.0, and plan 03's closed list does not name it |

`pmcp-workbook-dialect` 0.1.0 -> 0.1.1 is the unauthorised number. Plan 05's prohibition is
explicit ("Must not bump any crate that was not explicitly authorised at plan 03's
checkpoint... Every published version number is consumed permanently") and Task 2 Step C2
prescribes the response ("stop and return to the user rather than consuming unauthorised
version numbers"). CONTEXT D-07 makes it one-way. **Halted.**

### Severity if `pmcp-workbook-dialect` is left at 0.1.0 with its pin moved anyway

Its manifest edit would never publish (`release.yml` skips an already-published version
silently), so crates.io would keep `pmcp-workbook-dialect` 0.1.0 requiring
`pmcp-workbook-runtime ^0.1.0`, while its sibling consumers require `^0.2.0` — a published
tree carrying **two semver-incompatible copies of `pmcp-workbook-runtime`**. That is
verbatim the failure class CLAUDE.md item 13's ⚠ ORDERING CONSTRAINT describes for the
`pmcp-package` cluster, and it is what the crate family's own keystone doc forbids
(`crates/pmcp-workbook-compiler/src/lib.rs:18-24`, *"Re-export, don't re-declare"* — "A
second copy of `Manifest`/`ChangeClass`/`WHITELIST` would make the served loader and the
`diff_version` tool read a DIFFERENT definition than the compiler emits").

**Measured mitigation, offered as fact rather than as a recommendation:** in *this* tree the
two copies would still compile, because no runtime type crosses the dialect boundary.

- `pmcp-workbook-dialect`'s entire public surface — `WHITELIST`, `BASELINE_DIALECT_VERSION`,
  `SUPPORTED_DIALECT_VERSION`, `CandidateRole`, `DialectRules`, and the three methods
  `whitelist() -> &[&str]`, `sheet_layer_prefixes() -> &[String]`,
  `candidate_role(..) -> Option<CandidateRole>` — names **no** runtime type.
- Its only non-test runtime use is the pure re-export at `src/lib.rs:21`
  (`pub use pmcp_workbook_runtime::finding::{LintFinding, LintReport, Severity}`); the other
  two runtime references sit at `:204-205`, inside the `#[cfg(test)]` block that opens at
  `:169`.
- **Nothing in the workspace imports those types through dialect:** a grep for
  `pmcp_workbook_dialect::.*(LintFinding|LintReport|Severity)` across `crates/`,
  `cargo-pmcp/`, `examples/` and `src/` returns **0 hits**. `pmcp-workbook-compiler` takes
  them from the runtime directly (`src/dialect/mod.rs:27`) and takes only
  `CandidateRole`/`DialectRules`/`WHITELIST` from dialect (`:23`).

So the cost of leaving it is a permanently stale published crate and a silent second runtime
copy in downstream trees — not, today, a compile error. Whether that is acceptable is a
judgement about permanent registry state, which is why it is the user's.

### An independent finding: `pmcp-server-toolkit` 0.1.2 is already broken for publication

This is **not** caused by anything Phase 124 does; it is a defect the pin move happens to fix,
and it is why "leave all the pins alone" is not an available option.

`crates/pmcp-server-toolkit/Cargo.toml:81` pins `pmcp-workbook-runtime = "0.1.0"`, while
`crates/pmcp-server-toolkit/src/workbook/handler.rs` makes **42** references to runtime API
that does not exist in the **published** 0.1.0: `RenderMode` (`:32`, `:629-640`, `:1387`,
`:1402`), `pmcp_workbook_runtime::reconcile_reference` (`:718`),
`pmcp_workbook_runtime::reconcile::TOL` (`:723`), `pmcp_workbook_runtime::ReconcileReport`
(`:782-804`). Plan 03 corroborated against the published `.crate` that runtime 0.1.0 has
**no** `src/reconcile.rs`, `pub mod reconcile` count 0, `RenderMode` count 0.

The in-tree `path` dep hides this completely. And `workbook` is **not** a default feature
(`crates/pmcp-server-toolkit/Cargo.toml:93` — `default = ["code-mode"]`), so `cargo publish`'s
default-feature verification build would **not** compile `handler.rs`: 0.1.2 would publish
green and fail to build for any consumer enabling `workbook`. Recorded as WINDOWS #50.

---

## THE DECISION REQUIRED

Three resolutions, with their permanent costs. **All four requirement sites must end up
mutually consistent under every option** — that part is forced by `cargo`, not chosen.

| # | Option | Version numbers consumed | Cost |
|---|---|---|---|
| **A** | **Authorise `pmcp-workbook-dialect` 0.1.0 -> 0.1.1** and move all four pins to `0.2.0` | the authorised four **+ `pmcp-workbook-dialect` 0.1.1** | One extra number, permanently. Fully consistent published tree; no second runtime copy anywhere. Follows CLAUDE.md's Version Bump Rules literally. |
| **B** | Move all four pins to `0.2.0`, leave `pmcp-workbook-dialect` at 0.1.0 | the authorised four only | The dialect manifest edit never ships. Published `pmcp-workbook-dialect` 0.1.0 keeps `^0.1.0` forever, so downstream trees carry two runtime copies. Compiles today (measured above); violates the crate family's keystone invariant; joins the "delta stays unshipped indefinitely" set the user already accepted for `mcp-preview`/`pmcp-sql-server`. |
| **C** | **Revise `pmcp-workbook-runtime` to 0.1.1 (patch) instead of 0.2.0** | the authorised four, one at a revised number | **Zero pin moves, zero consequence bumps** — `^0.1.0` admits 0.1.1 — and it still fixes the `pmcp-server-toolkit` defect above, because published 0.1.1 would carry `reconcile`/`RenderMode`. But it puts +590 lines of additive public API on a PATCH, contradicting the axis plan 03 chose and the crate family's own Phase-122 precedent (`pmcp-package` 0.2 -> 0.3 for additive). Semver-sloppy but mechanically clean. |

**No recommendation is offered.** Plan 03 reserved version-number consumption to the user at a
`gate="blocking-human"` checkpoint precisely so no agent judgement substitutes for theirs, and
the three options differ in *which* permanent cost is paid, not in correctness.

**Whichever is chosen, plan 03's authorised list must be amended in writing** (a fifth row for
A, an explicit "pin moves without a version bump" row for B, or a revised axis for row 2 under
C) before plan 05 is re-run — otherwise the same halt recurs.

---

## Task 2 — COMPLETE. `pmcp-package` ships 0.3.0 AS-IS (D-04)

This half needed no user input and is fully discharged.

### Step A — the audit, with evidence

`crates/pmcp-package/Cargo.toml`'s version line was last changed by **`6430afae`**
(`feat(122-08): move pmcp-package to 0.3.0 and every in-repo emitter with it`, 2026-08-25) —
confirmed with `git log -L '/^version = /,+1:crates/pmcp-package/Cargo.toml'` rather than
assumed. The plan's second candidate, **`85ee222f`** (`fix(cargo-pmcp): honor [server]
memory_mb/timeout_seconds on pmcp-run`, 2026-08-26), **did** touch crate source —
`src/package/server.rs`, +22/−5 — so the plan was right to require checking rather than
inferring from the path log.

`git diff --shortstat 6430afae..HEAD -- crates/pmcp-package` → **21 files changed, 379
insertions(+), 5 deletions(-)**. Only two are under `src/`:

| File | Δ | Content of the added lines |
|---|---|---|
| `src/oci/mod.rs` | +154/−0 | 100% `//!` module docs — the normative artifact-tar framing rule (`a7ec5c59`, `docs(123-04)`) |
| `src/package/server.rs` | +22/−5 | 100% `///` docs on the `[server]` section explaining that `memory_mb`'s `Option` now carries meaning |

The other 19 files are `tests/common/mod.rs` and `tests/golden_fixtures/artifact_tar_v1/*`.

**The classification is a number, not a reading.** Added lines that are neither a `//`-comment
nor blank:

```
ADDED non-doc, non-blank lines, src/oci/mod.rs        -> 0
ADDED non-doc, non-blank lines, src/package/server.rs -> 0
```

**Verdict: docs + tests + fixtures only → ship 0.3.0 as-is** (D-04's first branch). No emitter
moves. No consequence bumps. No `pmcp-package` row in the release manifest's `expected_new`
beyond the already-bumped 0.3.0 that rides this tag regardless.

### Step B/D — the nine emitters, verified mutually consistent

The table has nine rows whether or not the audit says bump. It says ship-as-is, so before ==
after everywhere.

| # | Emitter | Before | After | Guard that would catch it being left behind |
|---|---|---|---|---|
| 1 | `crates/pmcp-package/Cargo.toml:10` `[package].version` | `0.3.0` | `0.3.0` (UNCHANGED) | the four manifest pins fail `cargo build` |
| 2 | `cargo-pmcp/Cargo.toml:88` | `"0.3"` | `"0.3"` (UNCHANGED) | `cargo build` |
| 3 | `crates/pmcp-agent/Cargo.toml:18` | `"0.3"` | `"0.3"` (UNCHANGED) | `cargo build` |
| 4 | `crates/pmcp-team-servers/Cargo.toml:24` | `"0.3"` | `"0.3"` (UNCHANGED) | `cargo build` |
| 5 | `crates/pmcp-cfn-renderer/Cargo.toml:10` | `"0.3"` | `"0.3"` (UNCHANGED) | `cargo build` |
| 6 | `cargo-pmcp/src/templates/agent.rs:67` `PMCP_PACKAGE_VERSION_REQ` | `"0.3"` | `"0.3"` (UNCHANGED) | **INVISIBLE to `cargo build`** — only `cargo test -p cargo-pmcp --lib emitted_package_requirement_matches_workspace_major_minor_line` |
| 7 | `cargo-pmcp/tests/pmcp_package_pin.rs:39` `EXPECTED_PIN` | `"0.3"` | `"0.3"` (UNCHANGED) | `cargo test -p cargo-pmcp --test pmcp_package_pin` |
| 8 | `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:89` `EXPECTED_VERSION_LINE` | `"0.3."` | `"0.3."` (UNCHANGED) | `cargo test -p pmcp-openapi-server --test pmcp_package_pin` |
| 9 | `crates/pmcp-openapi-server/Cargo.toml:124` dev-dep | `{ path = "../pmcp-package" }` | **UNCHANGED / path-only / CR-01** | `pmcp_package_dev_dep_is_path_only` |

Tripwires, with **non-zero test counts asserted** (a filter selecting zero tests passes
vacuously — this project's memory records that whole class of false green):

```
EXIT(pin-tripwire-cargo-pmcp)=0
EXIT(scaffold-drift-lib-test)=0
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 524 filtered out
cargo test -p pmcp-openapi-server --test pmcp_package_pin -> 2 passed (1 suite)
```

### Step C2 — the row-C1 consequence set

`pmcp-package` does not move, so the move is trivially compatible with every `^0.3`
requirement and **no consumer needs its own `[package].version` bump on its account**.
`cargo-pmcp` 0.23.0, `pmcp-agent` 0.3.0, `pmcp-team-servers` 0.2.0 and `pmcp-cfn-renderer`
0.2.0 ride this tag at the numbers they already carry, exactly as plan 03 recorded.

### Step D — CR-01 confirmed intact (and its criterion is defective)

`crates/pmcp-openapi-server`'s `pmcp-package` dev-dep at `:124` is
`{ path = "../pmcp-package" }` — path-only, no `version` key. Untouched.

The plan's criterion for this (`grep -n pmcp-package ... > file; grep -c version file` must
be **0**) returns **1** at the unmodified base. The match is line **100**, a *comment*
(``# `crates/pmcp-package/Cargo.toml`'s own version field stays on the 0.3 line``), not the
dependency. Restricted to non-comment `pmcp-package =` lines the count is **0**, and the real
tripwire passes 2/2. An executor trusting the literal form would "fix" a compliant manifest
and break the publish order. Recorded as WINDOWS #52.

### Step E — no-crypto boundary

Not exercised: `crates/pmcp-package/Cargo.toml` was not edited (byte-identical to HEAD), so
its dependency surface cannot have changed. `make no-crypto-check` is deferred to the run that
actually edits the manifest.

### Step F — `mcp-tester` requirements: UNCHANGED at all four before-publish sites

Verified by the keyed manifest table below (no row for any of them) and consistent with plan
03's explicit decision (**leave unbumped**):

| Site | Requirement | Publish step vs `mcp-tester` (`:401`) |
|---|---|---|
| `crates/pmcp-server-toolkit/Cargo.toml:192` | `0.8.0` (unchanged) | `:263` BEFORE |
| `crates/pmcp-sql-server/Cargo.toml:57` | `0.8.0` (unchanged) | `:329` BEFORE |
| `crates/pmcp-openapi-server/Cargo.toml:63` | `0.8.0` (unchanged) | `:344` BEFORE |
| `crates/pmcp-workbook-server/Cargo.toml:58` | `0.8.0` (unchanged) | `:383` BEFORE |

### The caret non-bump, confirmed

`crates/mcp-tester/Cargo.toml:21` and `cargo-pmcp/Cargo.toml:68` both still pin
`pmcp = "2.19.0"`, untouched. The requirement search above classifies both COMPATIBLE with
2.19.1. Neither crate is bumped on account of the `pmcp` move, per plan 04's recorded caret
exception. (Moot at present, since `pmcp` itself was not bumped either.)

---

## The per-manifest keyed before/after table (Task 1's replacement criterion)

Built by parsing every tracked `Cargo.toml` at `HEAD` and in the working tree — crate name,
`[package].version`, and every dependency requirement across `[dependencies]`,
`[dev-dependencies]`, `[build-dependencies]`, `[workspace.*]` and `[target.*]` — and diffing
the keyed field sets. Deliberately not a diff line count.

```
NO DIFFERING ROWS
```

Zero rows on both the `[package].version` axis and the dependency-requirement axis. The
authorised map has four entries and **none of them landed**, which is the halt.

> **The criterion as written is unsatisfiable** and is recorded as WINDOWS #51. It requires
> the differing-row set to equal EXACTLY the authorised `(crate, version)` map — but the same
> task's `<action>` mandates discovering and moving downstream requirement pins, and those
> rows appear in no `(crate, version)` map. The correct form partitions the rows:
> `[package].version` rows must equal the authorised map exactly; dependency-requirement rows
> must equal the discovered consequence set, recorded and justified. Under `pmcp-workbook-
> runtime` 0.2.0 that consequence set is non-empty and forced, so the literal criterion could
> never have been met.

---

## Task Commits

1. **Task 2 (audit half) + the blocker analysis** — this SUMMARY and `.planning/WINDOWS.md`, committed as the plan's only commit. **No production code or manifest was modified.**

_No `feat`/`chore` version commit exists by design: plan 05's must_have requires all
authorised version changes in ONE commit, so committing the three independently-safe bumps
while the fourth is blocked would create exactly the half-moved state the one-set rule
forbids._

## Files Created/Modified

- `.planning/phases/124-release-publish-order/124-05-SUMMARY.md` — this file.
- `.planning/WINDOWS.md` — entries #49–#52.

## Broken-windows ledger entries

| # | Kind | Subject |
|---|---|---|
| 49 | deviation | Plan 03's authorised list is incomplete for `pmcp-workbook-runtime` 0.2.0 — four forced pin moves, one unauthorised consequence bump |
| 50 | deviation | `pmcp-server-toolkit` 0.1.2 pins runtime `^0.1.0` while using 42 references to API absent from published 0.1.0; `workbook` is non-default so `cargo publish` verification would not catch it |
| 51 | deviation | Plan 05 Task 1's per-manifest keyed criterion is unsatisfiable — it forbids the pin moves the same task mandates |
| 52 | deviation | Plan 05 Task 2's CR-01 grep returns a false failure at the unmodified base by counting a comment |

## Decisions Made

1. **Halt before any version edit.** Plan 05 Task 2 Step C2 prescribes it, plan 05's
   prohibitions forbid the alternative, and CONTEXT D-07 makes version consumption one-way.
2. **`pmcp-package` ships 0.3.0 as-is**, on a measured docs-only classification (0 added
   non-doc, non-blank lines in both touched source files).
3. **No partial commit of the authorised set** — see the Task Commits note.
4. **Record the defective criteria rather than route around them.** Three of this plan's
   acceptance criteria are wrong in ways that would mislead a re-run; one of them fails at the
   unmodified base.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Task 1's `<verify>` command cannot detect the failure it needed to detect**
- **Found during:** Task 1, the consequence probe
- **Issue:** `cargo metadata --no-deps --format-version 1 --offline` does not resolve the
  dependency graph. Against a tree with `pmcp-workbook-runtime` at 0.2.0 and three consumers
  requiring `^0.1.0`, it exits **0**; the full form exits **101**. The plan's verify would
  have certified a tree that cannot build.
- **Fix:** Ran the full `cargo metadata --format-version 1 --offline` with a control run at
  the unmodified tree, and restored the probe edit (`git status --short` empty afterwards).
- **Verification:** CONTROL_EXIT=0, PROBE_EXIT=101 with the requirement-selection error quoted above.
- **Committed in:** n/a (measurement correction)
- **Recorded in:** implicit in WINDOWS #49

**2. [Rule 2 - Missing Critical] The authorised bump list omits a mechanically-forced consequence bump**
- **Found during:** Task 1, the workspace-wide requirement search
- **Issue:** See THE BLOCKER above.
- **Fix:** Halted at a `gate="blocking-human"` checkpoint with three costed options rather
  than consuming an unauthorised version number.
- **Committed in:** n/a
- **Recorded in:** WINDOWS #49

**3. [Rule 1 - Bug] Two of this plan's acceptance criteria are defective**
- **Found during:** Tasks 1 and 2
- **Issue:** (a) the per-manifest keyed criterion forbids the pin moves the same task
  mandates; (b) the CR-01 grep counts a comment and fails at the unmodified base.
- **Fix:** Applied the partitioned form of (a) and the non-comment-restricted form of (b);
  both recorded so a re-run is not misled.
- **Verification:** (a) NO DIFFERING ROWS on both axes; (b) non-comment count 0, tripwire 2/2.
- **Committed in:** n/a
- **Recorded in:** WINDOWS #51, #52

**4. [Rule 2 - Missing Critical] A latent published-artifact defect outside this phase's scope**
- **Found during:** Task 1, assessing whether the runtime pins could be left alone
- **Issue:** `pmcp-server-toolkit` 0.1.2 would publish green and break under `workbook`.
- **Fix:** Not fixed — fixing it requires the blocked pin move. Documented so the option
  space is honest: "leave the pins alone" is not available.
- **Committed in:** n/a
- **Recorded in:** WINDOWS #50

---

**Total deviations:** 4 (2 Rule 1 bugs, 2 Rule 2 missing-critical). Three are corrections to
the plan's *verification method*; one is the halt itself.
**Impact on plan:** Task 2's audit half is delivered complete. Tasks 1, 3 and 4 are blocked on
one user decision. No scope creep — zero production files touched.

## Issues Encountered

- **The output proxy truncates and substitutes its own exit status** (WINDOWS #47, plan 04).
  Every gate-class measurement here was captured inside a script that writes `$?` itself.
- **`RUSTFLAGS="" make quality-gate` was NOT run.** The working tree is byte-identical to the
  base commit — no source, manifest or workflow file was modified — so there is nothing this
  run could gate that plan 04's green run at the same base did not already cover. Claiming a
  fresh green would be a measurement of nothing. It must be run by whichever run actually
  lands the version edits.
- **Bash command-shape restrictions in the worktree-isolated harness**, as plan 03 recorded.
  Every multi-step measurement was written to a scratchpad script and invoked with a single
  `bash <path>`; no measurement was skipped or simplified to fit.

## User Setup Required

None.

## Next Phase Readiness

**BLOCKED.** One decision gates Tasks 1, 3 and 4 of this plan, and therefore plans 06 and 07:

- Choose **A**, **B** or **C** above for `pmcp-workbook-runtime`.
- Amend plan 03's authorised list in writing to reflect the choice.
- Re-run plan 05 from Task 1. Task 2's audit result (`pmcp-package` ships 0.3.0 as-is, nine
  emitters consistent, three tripwires green) carries forward and needs no re-litigation.

Ready and unaffected by the decision:
- The `pmcp` 2.19.1 caret analysis (36 sites, all compatible, no pin moves).
- The `pmcp-code-mode-derive` 0.2.1 and `pmcp-workbook-compiler` 0.1.1 analyses (1 versioned
  site each, both compatible).
- The `mcp-tester` prohibition, verified intact at all four before-publish sites.

---
*Phase: 124-release-publish-order*
*Halted: 2026-08-27 at a `gate="blocking-human"` decision checkpoint*

## Self-Check (round 1): PASSED

- Artifacts on disk: `.planning/phases/124-release-publish-order/124-05-SUMMARY.md` FOUND;
  `.planning/WINDOWS.md` FOUND with entries #49–#52.
- Working tree contains **no** production-file modification: `git status --short` empty before
  writing this SUMMARY, and the keyed manifest table reports NO DIFFERING ROWS.
- Every verification command reported above was actually executed; none is asserted from
  reading. The two probe edits were restored and the restoration verified.
- No stubs and no skipped tests. Three unrun/deferred verifications are named explicitly with
  their reasons: `make quality-gate` (nothing changed to gate), `make no-crypto-check`
  (manifest unedited), and Tasks 3/4 (blocked).
- No `git stash` operation was performed.

---

## Self-Check (final, round 3): PASSED

- **Artifacts on disk:** all eight FOUND — `124-expected-release.json`, `124-05-SUMMARY.md`,
  `124-03-SUMMARY.md`, `WINDOWS.md`, `CHANGELOG.md`, `crates/pmcp-package/CHANGELOG.md`,
  `.github/workflows/release.yml`, `cargo-pmcp/src/templates/workbook_server.rs`.
- **Commits:** all six FOUND in `git log --oneline --all` — `8f1c2dee`, `e6c9e2b7`,
  `d0ccccde`, `f1216928`, `1b4bb9ae`, `363b100f`.
- **The five shipped versions read back from disk**, not from these tables: `pmcp` 2.19.1,
  `pmcp-workbook-runtime` 0.2.0, `pmcp-code-mode-derive` 0.2.1, `pmcp-workbook-compiler`
  0.1.1, `pmcp-workbook-dialect` 0.1.1.
- **Plan `<verification>` re-run at close:** `## [2.19.1] - ` count 1; extraction **4,499
  bytes**; CR-01 non-comment version-key count **0**; the two `pmcp` caret pins still read
  `2.19.0`; `pmcp-package` still 0.3.0.
- **One self-check assertion was itself wrong, and is recorded rather than quietly adjusted.**
  "`mcp-tester` before-publish pins still 0.8.0" expected 4 and measured **5**. The fifth is
  `crates/pmcp-openapi-server/Cargo.toml:117` — a **comment** quoting the pin form as
  documentation, not a dependency. The four real sites are `pmcp-server-toolkit:192`,
  `pmcp-sql-server:57`, `pmcp-openapi-server:63`, `pmcp-workbook-server:58`, all at `0.8.0`
  and all unchanged, matching plan 03's table exactly. This is the fourth instance this phase
  of a grep that cannot distinguish a declaration from prose describing one (WINDOWS #46, #48,
  #52), and it happened in my own check — which is the point of writing it down.
- Every verification reported above was executed; none is asserted from reading. All
  temporary probe edits were restored and each restoration verified.
- No stubs, no skipped tests. Deferred verifications are named with reasons: `make
  no-crypto-check` (not run — `crates/pmcp-package/Cargo.toml` was never edited, so its
  dependency surface cannot have changed), `pmcp-code-mode-derive`'s semver axis
  (unverifiable — proc-macro, no library target), and CI's awk behaviour (unmeasurable here —
  no mawk/gawk installed).
- The gate's fuzz leg is vacuous on stable and is recorded as such (WINDOWS #58); the
  `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner should not be read as fuzz coverage.
- Working tree carries only pre-existing `.pmat/*` tool caches, which were already modified at
  session start and were never staged.
- No `git stash` operation was performed at any point across the three rounds.
