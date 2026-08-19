---
phase: 119
plan: 09
subsystem: documentation
tags: [docs, readme, changelog, protocol-era, v2, example-counts]
status: complete

requires:
  - "119-01: HTTP-01..08 + CLNT-01/02/05 flipped to [x]; 113-SPEC-RECHECK PUBLISHED-CONFIRMED — the settled v2 surface this README writes against"
  - "119-02: the `## Agents & Teams` README section this plan extends around"
  - "119-04: tests/docs04_examples_run.rs + tests/docs06_v2_examples_run.rs — the invocations the README must not disagree with"
  - "119-05: pmcp-book ch12-17 — the link target of the new `## Protocol Versions` section"
provides:
  - "README `## Protocol Versions` — the SDK's index-tier dual-era story (first time the README carries one)"
  - "README release header naming the measured published crate (pmcp 2.18.0) with an accurate dual-era claim"
  - "README `## Examples` groups covering all six D-15 examples plus the two v2 Tasks examples, every invocation build-proven"
  - "CHANGELOG headings dated for every published version"
affects:
  - "119-10: the closing gate re-runs `make quality-gate`; this plan left it green"

requirements-completed: []

tech-stack:
  added: []
  patterns:
    - "index-tier points, long-form tier retells — README links Chapter 12.17 rather than restating it"
    - "measure-then-write: every number and version in the diff came from a command run in this worktree"
    - "cite an example in the exact form its own rustdoc header uses"

key-files:
  created:
    - .planning/phases/119-documentation-three-shapes-v2-migration/119-09-SUMMARY.md
  modified:
    - README.md
    - CHANGELOG.md

decisions:
  - "DOCS-04 and DOCS-05 NOT booked — this plan is the README quarter of DOCS-04 only, and DOCS-05 was already earned by 119-05"
  - "The two v2 Tasks examples are cited WITH `--features full`, contradicting the plan's C-4 premise, because measurement showed s51 does not build without it"
  - "The third stale example count (`200+ working examples`) was corrected too — leaving it would have left the file disagreeing with itself"
  - "CHANGELOG `## [2.16.0] - Unreleased` left untouched: 2.16.0 was never published"
  - ".planning/WINDOWS.md deliberately NOT appended — outside this plan's declared file scope in a parallel wave"

metrics:
  duration: ~50 min
  completed: 2026-08-19

actuals:
  tokens: 30900
  tasks: 3
  commits: 4
---

# Phase 119 Plan 09: README Era Story & Measured Counts Summary

The README now carries an era story for the first time — a short `## Protocol Versions` section
that states the per-request dual-version fact, gives the one-line opt-in for each of the three
roles, shows the v1 opt-OUT severance build, and then points into Chapter 12.17 rather than
retelling it — while the release header it sits above stopped advertising a version that was
eighteen minor releases old and a specification-alignment claim this milestone falsifies.

## What Was Built

### Task 1 — `## Protocol Versions` + the refreshed release header (`a1d06406`)

**The new section** (README lines 488–514) carries exactly the four things the plan scoped and
nothing more: the dual-version fact (v1 `2025-11-25` and v2 `2026-07-28`, negotiated **per
request**, one binary, one port); a one-sentence opt-in per role (servers: nothing to do; clients:
one `ClientBuilder::with_protocol_version(...)` call; agents: already done by `pmcp-agent`'s
prefer-v2-fall-back-to-v1 invoker); the v1 opt-OUT command with the reason `--no-default-features`
alone proves nothing; and links to Chapter 12.17 and `docs/v1-sunset-policy.md`. The sunset policy
is linked, not restated.

D-05's vocabulary rule is stated explicitly in the section and applied throughout: eras are `v1`/
`v2` with their date strings, the crate is always name-attached ("pmcp 2.18"), and no bare version
string is used to mean an era.

**The release header** went from `## Latest Release: v2.0.0` to `## Latest Release: pmcp 2.18.0`,
with a body that separates the published release from the in-flight line and replaces the false
bullet with an accurate one.

| Claim in the old header | Status | What replaced it |
|---|---|---|
| `## Latest Release: v2.0.0` | 18 minor versions stale, and a bare version used where the crate was meant | `## Latest Release: pmcp 2.18.0` |
| "PMCP v2.0 — aligned with the MCP TypeScript SDK v2.0 release (2026-03-22)" | describes a release two years of versions ago | "Published 2026-08-16. The in-flight line is pmcp 2.19.0, still unreleased" |
| "**Protocol v2025-11-25**: Full alignment with the latest MCP specification" | **falsified by this milestone** — v1 is no longer the latest specification | "**Two protocol eras, one binary**: … negotiated per request — see [Protocol Versions](#protocol-versions)" |
| "**60+ Examples**" | stale | "**85 examples in `examples/`** … (119 example targets across the whole workspace)" |

Six feature bullets that are still true (MCP Apps, Conformance Suite, Tower Middleware, PMCP
Server, Uniform Constructor DX, MCP Tasks) were kept; the MCP Tasks bullet was corrected — it no
longer calls the feature "experimental" with a DynamoDB-only store, and it now says the v2 shape is
an extension and still provisional, matching Chapter 12.17's own provisionality callout. Two
bullets were added for what the 2.x line actually gained since v2.0 (credential storage from
2.18.0, Agents & Teams). The `**Full changelog**` pointer and both `---` rules are preserved.

### Task 2 — two new `## Examples` groups (`deef905a`)

Added inside the existing fenced block, in the form the Agent Skills group already uses:

```bash
# Agents & Teams — an agent loop, and four reference servers cooperating
cargo run --example s49_sampling_host
cargo run -p pmcp-agent --example s50_standalone_vs_sampled
cargo run -p pmcp-team-servers --example doc_review_team --features runtime

# MCP 2026-07-28 (v2) — start the SERVER first, in its own terminal; each client
# takes the server address as its argument and defaults to where the server binds
cargo run --example s47_v2_stateless_mrtr --features full   # serves on 127.0.0.1:8147
cargo run --example s48_v2_mrtr_client --features full      # then, in another terminal
cargo run --example s53_v2_agent_client --features full     # the pmcp-agent connector, v2 with v1 fallback

# MCP 2026-07-28 (v2) Tasks — same rule: server first, then the agent
cargo run --example s50_v2_tasks_server --features full     # serves on 127.0.0.1:8150
cargo run --example s51_v2_tasks_agent --features full
```

The `-p` flags are load-bearing, not decorative: `s49` appears twice inside root `examples/` and
`s50` appears in both root `examples/` and `crates/pmcp-agent/examples/`. No example was renumbered,
no index page was added, and no example is cited by a bare number.

The two bind addresses in the comments were read from the source constants, not guessed:
`examples/s47_v2_stateless_mrtr.rs:85` and `examples/s50_v2_tasks_server.rs:131` both declare
`DEFAULT_ADDR`, and each client's `DEFAULT_ADDR` (`s48:61`, `s53:100`, `s51:111`) matches its
server's, so the no-argument forms shown do pair up.

### Task 3 — CHANGELOG headings (`235781d6`)

`## [2.18.0] - Unreleased` → `## [2.18.0] - 2026-08-16`, `## [2.17.0] - Unreleased` →
`## [2.17.0] - 2026-07-19`. Content unchanged; `git diff --numstat` reports **2 added / 2 removed**,
which is the cap the plan set.

## Measurements

Every number and version written into the diff was measured in this worktree at execution time.

**`cargo search pmcp --limit 3`**, literal output:

```
pmcp = "2.18.0"          # High-quality Rust SDK for Model Context Protocol (MCP) with full TypeScript SDK compatibility
cargo-pmcp = "0.20.0"    # Production-grade MCP server development toolkit
pmcp-macros = "0.6.1"    # Procedural macros for PMCP SDK - Model Context Protocol
... and 33 crates more (use --limit N to see more)
```

Root `Cargo.toml:3` agrees: `version = "2.18.0"`. RESEARCH correction C-2 confirmed — CONTEXT.md's
2.17.0 was stale.

**Published-version set**, from the local crates.io sparse-index cache
(`~/.cargo/registry/index/index.crates.io-*/.cache/pm/cp/pmcp`), most recent ten:

```
2.8.1, 2.9.0, 2.10.0, 2.11.0, 2.12.0, 2.13.0, 2.14.0, 2.15.0, 2.17.0, 2.18.0
```

**2.16.0 is absent.** No `v2.16.0` tag exists on `origin` or `upstream` either
(`git ls-remote --tags upstream` returns `v2.15.0`, `v2.17.0`, `v2.18.0` for that range). That
version was skipped, so its `## [2.16.0] - Unreleased` heading is accurate and was left alone —
this is the "if the measurement shows a version is in fact unpublished" branch the plan asked to be
recorded.

**Release dates**, from the upstream GitHub releases (the local clone has no `v2.18.0` tag object):

| Version | Source | Date written |
|---|---|---|
| 2.18.0 | `gh api repos/paiml/rust-mcp-sdk/releases/tags/v2.18.0 --jq .published_at` → `2026-08-16T09:17:32Z` | `2026-08-16` |
| 2.17.0 | `gh api …/v2.17.0 --jq .published_at` → `2026-07-19T19:43:08Z` | `2026-07-19` |

The `v2.17.0` tag commit date (`git log -1 --format=%ad --date=short v2.17.0` → `2026-07-19`) agrees
independently. Format matches the file's own dated entries (`## [2.15.0] - 2026-07-10`).

**Example counts.** Two different numbers exist here and the README now says which it means:

| Figure | Command | Value |
|---|---|---|
| Root example **files** | `ls examples/*.rs \| wc -l` | **85** |
| Workspace example **targets** | `cargo metadata --no-deps` over all packages | **119** across **14** packages |

The README's `## Examples` intro and its `[Examples](examples/)` link both now read "85 … in
`examples/`" with the 119 workspace figure in parentheses, so the two are distinguishable rather
than conflated. Note for future readers: `make test-examples` builds **87** targets across 3 trees —
a third number again, and the reason this phase removed an over-claim from the Makefile. The README
cites neither the 87 nor an unqualified total.

## Verification

| # | Check | Result |
|---|---|---|
| 1 | `cargo build --features full --example s50_v2_tasks_server --example s51_v2_tasks_agent` | ✅ exit 0 |
| 2 | `cargo build --features full --example s47_v2_stateless_mrtr --example s48_v2_mrtr_client --example s53_v2_agent_client` | ✅ exit 0 |
| 3 | `cargo build --example s49_sampling_host` | ✅ exit 0 |
| 4 | `cargo build -p pmcp-agent --example s50_standalone_vs_sampled` | ✅ exit 0 |
| 5 | `cargo build -p pmcp-team-servers --example doc_review_team --features runtime` | ✅ exit 0 |
| 6 | `grep -c '^## Protocol Versions' README.md` | ✅ `1` |
| 7 | `grep -c '^## Agents & Teams' README.md` | ✅ `1` — 119-02's section extended, not duplicated |
| 8 | `grep -c '^## Latest Release: v2.0.0' README.md` / `grep -c '^## Latest Release' README.md` | ✅ `0` / `1` |
| 9 | `grep -cF 'Full alignment with the latest MCP specification' README.md` | ✅ `0` |
| 10 | `grep -cF '60+ Examples'` / `grep -cF 'includes 60+ comprehensive examples'` | ✅ `0` / `0` |
| 11 | `grep -c 'full-v2' README.md` | ✅ `2` |
| 12 | `grep -c 'ch12-17-migrating-to-mcp-2026-07-28' README.md` | ✅ `1` |
| 13 | `grep -c 'v1-sunset-policy.md' README.md` | ✅ `1` |
| 14 | `grep -c '^## Examples' README.md` | ✅ `1` |
| 15 | `grep -c '^## \[2.19.0\] - Unreleased' CHANGELOG.md` | ✅ `1` — in-flight line untouched |
| 16 | `grep -c '^## \[2.18.0\] - Unreleased'` / `'^## \[2.17.0\] - Unreleased'` | ✅ `0` / `0` |
| 17 | `git diff --numstat CHANGELOG.md` | ✅ `2  2` — headings only |
| 18 | `cargo fmt --all -- --check` | ✅ exit 0 |
| 19 | `make quality-gate` | ✅ **ALL TOYOTA WAY QUALITY CHECKS PASSED** |

No book file was touched, so no `mdbook build` was required from this plan.

### Cross-check: every README invocation appears verbatim elsewhere

Task 2's acceptance criterion requires each added invocation to match, character-for-character in
its example-name-and-flags portion, a book chapter or a run test. All eight do:

| README line | Matching source |
|---|---|
| `cargo run --example s49_sampling_host` | `tests/docs04_examples_run.rs:185` (`claim:` string) |
| `cargo run -p pmcp-agent --example s50_standalone_vs_sampled` | `tests/docs04_examples_run.rs:136` |
| `cargo run -p pmcp-team-servers --example doc_review_team --features runtime` | `tests/docs04_examples_run.rs:246` (`rebuild:` string) |
| `cargo run --example s47_v2_stateless_mrtr --features full` | `pmcp-book/src/ch12-17-…md:96` |
| `cargo run --example s48_v2_mrtr_client --features full` | `pmcp-book/src/ch12-17-…md:106` |
| `cargo run --example s53_v2_agent_client --features full` | `examples/s53_v2_agent_client.rs:12` (rustdoc header) |
| `cargo run --example s50_v2_tasks_server --features full` | `examples/s50_v2_tasks_server.rs:6` |
| `cargo run --example s51_v2_tasks_agent --features full` | `examples/s51_v2_tasks_agent.rs:12` |

The order `--example NAME --features full` (rather than the run tests' `--features full --example
NAME`) was chosen because it is simultaneously the README's own house style
(`cargo run --example s16_typed_tools --features schema-generation`, pre-existing), Chapter 12.17's
style, and every one of these examples' own rustdoc headers. Flag order is semantically irrelevant
to cargo, and all five v2 invocations were built in that exact form.

## Deviations from Plan

### 1. [Rule 1 - Bug] The v2 Tasks examples DO need a feature flag — the plan's C-4 premise is false

- **Found during:** Task 2, running the plan's own `<verify>` command.
- **Issue:** the plan (RESEARCH correction C-4) asserts `s50_v2_tasks_server` and
  `s51_v2_tasks_agent` "have NO `[[example]]` declaration — they are auto-discovered and build
  under DEFAULT features, so citing them with a feature flag would be wrong", and turns that into
  two acceptance criteria requiring the README to carry no flag on either. Half of it is right and
  half is not. The `[[example]]` blocks are indeed absent (confirmed in `Cargo.toml`), but the
  conclusion does not follow — **an absent `[[example]]` block means cargo cannot apply
  `required-features`, not that none are required.** Measured:

  ```
  $ cargo build --example s50_v2_tasks_server --example s51_v2_tasks_agent
  error[E0433]: cannot find `testing` in `pmcp`
     --> examples/s51_v2_tasks_agent.rs:592:15
      |
  note: found an item that was configured out
     --> src/lib.rs:63:9
   62 | #[cfg(any(test, feature = "testing"))]
   63 | pub mod testing;
  ```

  `s50_v2_tasks_server` alone does build under defaults; `s51_v2_tasks_agent` does not, and the
  plan's verify command builds both together, so the command as written **fails**.
- **Fix:** both are cited with `--features full`, which is what their own rustdoc headers document
  (`examples/s50_v2_tasks_server.rs:6`, `examples/s51_v2_tasks_agent.rs:12`) and what makes the
  command run. `cargo build --features testing --example s51_v2_tasks_agent` was also confirmed
  green, so `testing` is the minimal addition; `full` was chosen for consistency with the adjacent
  s47/s48/s53 group and with the examples' own documentation.
- **Why this overrides the acceptance criteria:** the plan's controlling instruction in the same
  paragraph is "Every command must be one that runs… do not drop [a feature flag] from an example
  that needs it", and D-10 requires full runnable invocations. Threat T-119-46 is exactly "a README
  command that does not run — a feature flag on an undeclared example, **or a missing one**". The
  criteria `grep -c 's51_v2_tasks_agent --features' README.md == 0` were derived from the false
  premise and, if honoured, would have shipped the very defect the threat register names.
- **Files modified:** `README.md`. **Commit:** `deef905a`.
- **Caveat worth recording:** `s50_v2_tasks_server` building under "default features" is itself
  partly an artifact — it imports `pmcp::server::streamable_http_server`, which is not a default
  feature. It compiles because `cargo build --example` for the root package also builds
  dev-dependencies, and the `pmcp-agent` dev-dep unifies `streamable-http` back on. The same
  unification applies to `cargo run --example`, so a reader copying either form gets a working
  build; but nothing in the manifest *guarantees* it, which is a second reason to cite the explicit
  `--features full`.

### 2. [Rule 1 - Bug] A third stale example count, not in the plan's list

- **Found during:** Task 1, while locating the two counts the plan named.
- **Issue:** `README.md` carried `- **[Examples](examples/)** - 200+ working examples` in the
  Documentation section — a third instance of the same falsehood, and further from the truth than
  either of the two the plan scoped (measured: 85 root files, 119 workspace targets).
- **Fix:** corrected to `85 working examples in examples/ (119 example targets workspace-wide)`,
  using the same measured pair as the other two locations.
- **Rationale:** threat T-119-48 is "correcting one stale example count and leaving the other", and
  its mitigation is that all locations must agree on one measured value. Fixing two of three would
  have left the file self-contradictory in precisely the way the threat describes.
- **Files modified:** `README.md`. **Commit:** `a1d06406`.

### 3. [Clarification] The `cargo pmcp` verb criterion is unsatisfiable as literally written

- **Issue:** Task 1's criterion says `grep -ohE 'cargo pmcp [a-z-]+ [a-z-]+' README.md | sort -u`
  must print only lines within {`agent new`, `agent dev`, `team dev`}. On the **base** README that
  grep already prints 22 distinct verbs (`cargo pmcp new my-server`, `cargo pmcp test conformance`,
  `cargo pmcp deploy logs`, `cargo pmcp workbook compile`, …), all pre-existing and all legitimate.
  Satisfying the criterion literally would mean deleting most of the README's CLI documentation.
- **Reading applied:** the plan's `<prohibitions>` entry — "No CLI verb is named beyond `cargo pmcp
  agent new`, `cargo pmcp agent dev` and `cargo pmcp team dev`" — constrains what **this plan
  adds**, not the whole file. Enforced by differential check instead:

  ```
  $ diff <(git show HEAD~3:README.md | grep -ohE 'cargo pmcp [a-z-]+ [a-z-]+' | sort -u) \
         <(grep -ohE 'cargo pmcp [a-z-]+ [a-z-]+' README.md | sort -u)
  Files are identical
  ```

  Zero new verbs introduced. The only `cargo pmcp` mention added anywhere in this diff is
  `cargo pmcp agent new`, inside the `## Protocol Versions` agent bullet.

### 4. [Rule 3 - Blocking] Disk exhaustion corrupted the incremental cache mid-gate

- **Found during:** final verification.
- **Issue:** the first `make quality-gate` run died with `ENOSPC: no space left on device`;
  `df -h /System/Volumes/Data` showed **903Gi used, 1.1Gi available, 100% capacity** — three sibling
  wave-4 executors building in parallel worktrees. Two knock-on symptoms followed, both of which
  look like code regressions and are not:
  1. `shared::sse_parser::tests::take_utf8_prefix_cost_grows_linearly_not_quadratically` failed
     (`grew 10.3x for a 4x input step; the ceiling is 8.0x`) — a minimum-of-5 **timing ratio**
     assertion, run on a machine under heavy parallel build load. It passes in isolation:
     `cargo test --lib take_utf8_prefix_cost_grows_linearly_not_quadratically` → `1 passed`.
  2. `error: cached cgu … should have an object file, but doesn't` — incremental-compilation
     artifacts truncated by the out-of-space write.
- **Fix:** waited for sibling load to clear (56Gi free), removed the corrupted cache with
  `rm -rf target/debug/incremental` (gitignored build output inside this worktree only — no `git
  clean`, no `git reset`), and re-ran. `make quality-gate` then printed
  **`✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`**.
- **Attribution:** neither symptom is attributable to this plan. The complete diff is
  `README.md | 63 +++-` and `CHANGELOG.md | 4 +-` — **zero Rust files touched**, and
  `cargo fmt --all -- --check` is clean.
- **Files modified:** none.

## Requirements Ledger

`requirements-completed: []` — **deliberately empty, and neither ID in this plan's frontmatter was
booked.**

- **DOCS-04** requires book chapters **and** runnable examples **and** README **and** course. This
  plan is the **README quarter only**. Plans 119-06 (course) and 119-07 (book chapter) were
  executing in parallel with this one in the same wave and neither had merged at the time this
  SUMMARY was written, so DOCS-04 is not satisfiable from here. Booking it would have repeated the
  defect 119-01 hit and reverted.
- **DOCS-05** was already `[x]`, earned by 119-05. It was not touched.

No `state.*` verb was run and `STATE.md` / `ROADMAP.md` / `REQUIREMENTS.md` were not modified — the
orchestrator owns those writes after the wave merges, per the parallel-execution contract.

## Known Stubs

None. No placeholder text, no TODO/FIXME, no unwired data path — the plan writes prose and command
lines, all of which are backed by a command that was run.

## Broken-Windows Ledger

**Not appended, deliberately.** `.planning/WINDOWS.md` is outside this plan's declared file scope
(`README.md` and `CHANGELOG.md` only) in a four-way parallel wave, it was edited by 119-05 in the
previous wave, and 119-10's disclosure tripwire keys on its citation surface. Appending from a
worktree that merges concurrently with three siblings would risk a conflict in the exact file the
phase's closing gate depends on. The four deviations above are recorded here instead, in full, and
none is a stub, a skipped test, or an unrun `<verify>` — every `<verify>` in this plan was executed,
and deviation 1 is a *corrected* verify, not a skipped one.

## Threat Flags

None. This plan modified two markdown files. It added no network endpoint, no auth path, no file
access pattern and no schema. No package was installed (`T-119-SC` disposition `accept` holds
unchanged).

Mitigation status for the register this plan carried:

| Threat | Disposition | Outcome |
|---|---|---|
| T-119-44 stale "full alignment" claim | mitigate | ✅ removed; `grep -cF` → 0 |
| T-119-45 unpublished version named | mitigate | ✅ 2.18.0 measured against crates.io **and** the root manifest **and** the upstream release |
| T-119-46 a README command that does not run | mitigate | ✅ all eight built; **this threat actually fired** — see deviation 1 |
| T-119-47 bare version read as an era | mitigate | ✅ stale header removed; name-attached convention stated and applied |
| T-119-48 one count fixed, another left | mitigate | ✅ all **three** locations corrected to one measured pair |
| T-119-49 changelog content smuggled in | mitigate | ✅ `2 2` numstat |

## Commits

| Task | Commit | Subject |
|---|---|---|
| 1 | `a1d06406` | `docs(119-09): add README Protocol Versions section and refresh release header` |
| 2 | `deef905a` | `docs(119-09): add Agents & Teams and v2 example groups to README` |
| 3 | `235781d6` | `docs(119-09): date the two mislabelled published CHANGELOG headings` |

## Self-Check: PASSED

- `README.md` — present, modified, `## Protocol Versions` at line 488, `## Latest Release: pmcp
  2.18.0` at line 518, new example groups at lines 631–643.
- `CHANGELOG.md` — present, both headings dated.
- `.planning/phases/119-documentation-three-shapes-v2-migration/119-09-SUMMARY.md` — this file.
- Commits `a1d06406`, `deef905a`, `235781d6` — all present in `git log --oneline`, all on
  `worktree-agent-aa3d605cdbf7ee682`, all descended from base `7b939ee2`.
- `git status --short --untracked-files=all` — clean (`.pmat/*` cache churn from `make
  quality-gate` restored with `git checkout -- .pmat/`).
