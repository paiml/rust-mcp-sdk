---
phase: 124-release-publish-order
plan: 02
subsystem: infra
tags: [git, merge, release, squash-divergence, quality-gate, cargo, manifest-inventory]

requires:
  - phase: 124-01
    provides: the extended release-coverage gate + 8-fixture guard self-test that this merge had to preserve through a Makefile and check-release-coverage.sh conflict
  - phase: 123
    provides: the save/load/pull seam work in pmcp_run/graphql*.rs and the ANSI-escaping inspect renderer that both sides of the merge contended over
provides:
  - "A topological merge commit (11cce7c1) making upstream/main c64e2b2b an ancestor of the milestone branch, so the release PR diffs from the post-squash base"
  - "A hunk-by-hunk conflict-resolution transcript for all 12 conflicted paths, each with the winning side and the reason"
  - "A semantic manifest version inventory tool (package version + every dependency requirement) proving no version literal moved"
  - "Four WINDOWS.md ledger entries: two defective acceptance criteria, one RESEARCH Pitfall 4 correction, one worktree-isolation process defect"
affects: [124-05, 124-06, 124-07, release-workflow]

actuals:
  tokens: 1750
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "Semantic manifest inventory over grep: parse every tracked Cargo.toml with tomllib and emit crate name + [package].version + EVERY dependency requirement (normal/dev/build/target/workspace), then diff before-vs-after — a `^[+-]version = ` grep is blind to `pmcp = { version = \"...\" }` lines that are equally release-critical"
    - "Superset-proof conflict resolution: before taking a side, diff stage-3 against stage-2 and read ONLY the lines the losing side uniquely carries — a `--stat` alone cannot distinguish 'ours refactored this' from 'ours dropped this'"
    - "Topological-merge verification: identical before/after tree SHAs prove a sync changed zero bytes, which is a stronger no-regression proof than any per-file check"

key-files:
  created:
    - .planning/phases/124-release-publish-order/124-02-SUMMARY.md
  modified:
    - .planning/WINDOWS.md

key-decisions:
  - "All 12 conflicts resolved to OURS — verified per-file by reading the losing side's unique lines, not assumed from diff direction"
  - "inspect.rs resolved to ours on security grounds: main renders attestation fields raw, ours routes every one through untrusted() which escapes control chars; the deleted truncate() is strictly subsumed by untrusted()"
  - "The merge is purely topological — the post-merge tree is byte-identical to the pre-merge tree (same tree SHA fd43383f). Its entire value is ancestry, not content"
  - "Two plan acceptance criteria were measured defective rather than silently passed over, and recorded in WINDOWS.md"

patterns-established:
  - "Abort-not-improvise on a live checkout: when the merge and several git verbs were blocked by the permission classifier, the merge was handed back rather than synthesized via read-tree/patch — which would have produced wrong ancestry while looking successful"

requirements-completed: [PKGR-01]

coverage:
  - id: D1
    description: "upstream/main c64e2b2b is an ancestor of the milestone branch, so the release PR will diff against the post-squash base rather than re-presenting phases 120-123"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "git merge-base --is-ancestor c64e2b2b3a8c58d22b92d5b11b18648abf673ad8 HEAD -> exit 0"
        status: pass
      - kind: integration
        ref: "git cat-file -p HEAD | grep '^parent' -> two parents: c494f808 and c64e2b2b"
        status: pass
    human_judgment: false
  - id: D2
    description: "All 12 conflicts resolved with no conflict markers left in the tree and no unresolved merge paths"
    verification:
      - kind: integration
        ref: "grep -c '^<<<<<<<' over all 12 conflicted files -> 0 for each; git diff --name-only --diff-filter=U -> empty"
        status: pass
    human_judgment: false
  - id: D3
    description: "No version literal moved during the conflict resolution — neither a [package].version nor any dependency version requirement"
    verification:
      - kind: integration
        ref: "diff /tmp/124-manifest-inventory-before.txt /tmp/124-manifest-inventory-after.txt -> exit 0, 780 lines each across 62 manifests"
        status: pass
      - kind: integration
        ref: "git rev-parse c494f808^{tree} == git rev-parse 11cce7c1^{tree} -> both fd43383fb34ab121435402b3aa0fbb2c173a8505 (zero-byte merge)"
        status: pass
      - kind: integration
        ref: "secondary: grep -c '^[+-]version = ' /tmp/124-manifest-diff.txt -> 0"
        status: pass
    human_judgment: false
  - id: D4
    description: "Plan 01's release-coverage gate and its 8-fixture guard self-test survived the Makefile and check-release-coverage.sh conflicts"
    requirement: "PKGR-01"
    verification:
      - kind: integration
        ref: "make check-release-coverage-guard-selftest -> exit 0, 'release-coverage gate self-test passed (8 fixtures)'"
        status: pass
      - kind: integration
        ref: "make check-release-coverage -> exit 0, 'all 25 publishable workspace members have a publish step'"
        status: pass
      - kind: integration
        ref: "grep -c 'check-release-coverage: check-release-coverage-guard-selftest' Makefile -> 1"
        status: pass
    human_judgment: false
  - id: D5
    description: "The merged tree passes the full project quality gate"
    verification:
      - kind: integration
        ref: "RUSTFLAGS=\"\" make quality-gate -> exit 0 (captured to /tmp/124-gate-exit.txt, unpiped)"
        status: pass
    human_judgment: false

duration: 50min
completed: 2026-08-27
status: complete
---

# Phase 124 Plan 02: Sync `main` into the milestone branch Summary

**A purely topological merge of the PR #347 squash (`c64e2b2b`) into `feat/v2.6-package-portability`: 12 conflicts all resolved to ours after per-file superset proofs, leaving the tree byte-identical (same tree SHA) while making `main` an ancestor so the release PR diffs from the post-squash base.**

## Performance

- **Duration:** ~50 min (first measurement 17:04:09Z → gate green 17:51:42Z), including two checkpoint round-trips and one permission hand-back
- **Started:** 2026-08-27T17:04:09Z
- **Completed:** 2026-08-27T17:53:00Z
- **Tasks:** 2
- **Files modified:** 1 committed by this plan (`.planning/WINDOWS.md`) + the merge commit itself

## Accomplishments

- **`c64e2b2b` is now an ancestor of the milestone branch** (`git merge-base --is-ancestor` exit 0; merge commit `11cce7c1` has both parents). This is the one property D-08 actually needs.
- **All 12 conflicts resolved with a recorded rationale**, each backed by reading the *losing* side's unique lines rather than trusting diff direction.
- **Proved no version literal moved two independent ways** — an empty 780-line semantic manifest inventory diff, and an identical before/after tree SHA.
- **Plan 01's gate survived** the two conflicts that could have silently deleted it (`Makefile`, `scripts/check-release-coverage.sh`).
- **Caught two defective acceptance criteria** in this plan rather than passing over them, plus a correction to RESEARCH Pitfall 4.

## Task Commits

1. **Task 1: Re-measure the conflict set** — no commit (measurement-only task by design; evidence in `/tmp/124-main-sha.txt`, `/tmp/124-merge-tree.txt`)
2. **Task 2: Perform the sync** — `11cce7c1` (merge)

**Plan metadata:** see the `docs(124-02)` commit carrying this SUMMARY and the WINDOWS.md entries.

## Measurement record (Task 1)

**Resolved evidence binaries** — `command -v`, per the plan's Step 0, with one correction:

| Binary | `command -v` result | Resolved absolute path |
|---|---|---|
| `git` | `/opt/homebrew/bin/git` | `/opt/homebrew/bin/git` |
| `grep` | **`grep` — a shell FUNCTION**, not a path | `/usr/bin/grep` (via `which -a grep`) |

No hard-coded `/opt/homebrew/` path was used in any acceptance command; both were resolved at runtime.

- **`MAIN_SHA`:** `c64e2b2b3a8c58d22b92d5b11b18648abf673ad8` (40 chars). Re-confirmed unchanged three times: at measurement, immediately before the merge, and at commit.
- **`upstream`:** `git@github.com:paiml/rust-mcp-sdk.git` (precondition satisfied; `git fetch upstream main` exit 0)
- **Ahead/behind:** 254 ahead, 1 behind
- **Squash confirmed:** `git cat-file -p c64e2b2b | grep -c '^parent'` = **1** (single parent → squash, not a merge)
- **Merge base:** `67758388b16716cd1dd65ecbb70be39d4b120de2`
- **Conflict count: 12** (RESEARCH recorded 9 — measurement was correctly re-taken rather than trusted)

## Conflict resolution — one line per file

Every resolution went to **ours**. In each case this was *proved* by diffing stage 3 (theirs) against stage 2 (ours) and reading only the lines main uniquely carries — a `--stat` alone cannot tell "ours refactored this" from "ours dropped this".

| # | File | Bucket | Winner | Why |
|---|---|---|---|---|
| 1 | `.planning/REQUIREMENTS.md` | planning | ours | Main carries the pre-Phase-123 verb wording (`pack\|unpack\|export\|import`) and `PKGX-02 \| Pending`. Ours has the settled `save\|load\|pull` set and the detailed traceability row. Superseded, not unique. |
| 2 | `.planning/ROADMAP.md` | planning | ours | Main's unique lines are all `Not started` / `TBD` / old verb names for phases 123–124. Strictly stale. |
| 3 | `.planning/STATE.md` | planning | ours | Main is the 2026-08-25 phase-120 snapshot; ours is the `c494f808` phase-124 bookkeeping commit. (This was the one authorized STATE.md write, as a conflict resolution only.) |
| 4 | `.planning/WINDOWS.md` | planning | ours | Main's unique lines are only older counters (`open_count: 24`, `total_count: 33`). Ours: 28/37. |
| 5 | `Makefile` | behavioural | ours | All four main-unique lines are superseded by *extended* versions in ours: `test-examples:` → `test-examples: build-cargo-pmcp-examples`; **`check-release-coverage:` → `check-release-coverage: check-release-coverage-guard-selftest`** (plan 01's edge, :1025); `REQUIRED_TEST_BINARIES` 4 → 8 entries. Selftest target intact at :948. |
| 6 | `cargo-pmcp/src/commands/package/inspect.rs` | behavioural (**unbudgeted**) | ours | **Security.** Main renders package-supplied strings raw (`field("Subject", &attestation.subject.claimed)`); ours routes all of them — plus the `candidates` list in the unknown-kind error — through `untrusted()`. Taking main would reintroduce the ANSI terminal-forgery hole fixed at `1339fc45`. |
| 7 | `cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs` | behavioural | ours | Main's unique lines (a local `static CLIENT`, a single `timeout(300)`, a `.clone()`) are superseded by ours' shared `http_client()` + `CONNECT_TIMEOUT` + per-request `ARTIFACT_TRANSFER_TIMEOUT`/`GRAPHQL_REQUEST_TIMEOUT`. `extract_s3_error` retained. |
| 8 | `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` | behavioural | ours | Main's unique lines are the narrower `decode_verify_attestation_response` / `required_response_str(payload, key)`. Ours has both, with `required_response_str` generalized to take an `operation` argument, plus the three decode tests. Nothing lost. |
| 9 | `cargo-pmcp/tests/package_inspect.rs` | behavioural (**unbudgeted**) | ours | Pure superset: theirs→ours is **+120/−0**. |
| 10 | `crates/pmcp-package/tests/common/mod.rs` | add/add | ours | Pure superset: theirs→ours is **+16/−0**. |
| 11 | `docs/design/package-portability-pmcp-run-handoff.md` | add/add | ours | Only one line differs. Ours contains main's sentence **verbatim** inside a `~~strikethrough~~` plus the supersession note — a content superset by construction. |
| 12 | `scripts/check-release-coverage.sh` | behavioural (**unbudgeted**) | ours | Main's *only* unique content is the 3-line `KNOWN BLIND SPOT` comment that plan 01 deliberately deleted **because it closed that blind spot**. Ours is +230/−3 over main. |

**Nothing was found that main uniquely carries.** Every apparent deletion was a line ours had replaced with a superset.

## The merge is topological — zero bytes changed

`git rev-parse c494f808^{tree}` and `git rev-parse 11cce7c1^{tree}` are both `fd43383fb34ab121435402b3aa0fbb2c173a8505`, and `git diff --name-only c494f808 11cce7c1` returns **0 files**.

This is the expected outcome and worth stating plainly: `main`'s tip is a squash of this branch's *own* 120–122 work, so everything it carried, the branch already had — and more. The sync's entire value is the ancestry record. It is also the strongest available no-regression proof: a byte-identical tree cannot have reverted a manifest, dropped a hunk, or moved a version.

## Verification results

| Check | Result |
|---|---|
| `RUSTFLAGS="" make quality-gate` | **exit 0** — run unpiped, status captured to its own file (`/tmp/124-gate-exit.txt`), never read from a pipeline |
| `make check-release-coverage-guard-selftest` | **exit 0** — "release-coverage gate self-test passed (8 fixtures)" |
| `make check-release-coverage` | **exit 0** — "all 25 publishable workspace members have a publish step" |
| `grep -c 'check-release-coverage: check-release-coverage-guard-selftest' Makefile` | **1** |
| `git merge-base --is-ancestor c64e2b2b HEAD` | **exit 0** |
| Manifest inventory before vs after | **identical** — 780 lines, 62 manifests, `diff` exit 0 |
| Secondary `grep -c '^[+-]version = '` over the manifest diff | **0** |
| Conflict markers across all 12 files | **0 each** |
| Unresolved merge paths | **none** |

The toolchain was already current (`rustc 1.98.0`, `rustup update stable` → "unchanged"), matching CI's `dtolnay/rust-toolchain@stable`.

## Deviations from Plan

### 1. [Rule 3 - Blocking] Conflict set was 12, not the 9 RESEARCH recorded — halted as instructed

- **Found during:** Task 1
- **Issue:** Three unbudgeted conflicts appeared: `scripts/check-release-coverage.sh` (on the plan's *named halt list*), `cargo-pmcp/src/commands/package/inspect.rs`, `cargo-pmcp/tests/package_inspect.rs`.
- **Fix:** Halted and reported rather than proceeding, exactly as Task 1's action specifies. Resolution was authorized at a blocking-human gate, with the `inspect.rs` verdict independently verified on security grounds before I resolved it.
- **Verification:** All three resolved to ours with superset proofs (rows 6, 9, 12 above).

### 2. [Rule 1 - Bug] Two acceptance criteria in this plan are defective

- **Found during:** Task 1 and Task 2
- **Issue:**
  - `grep -c 'Cargo.toml' /tmp/124-merge-tree.txt` returns 0 — **measured 1**, a false positive: it greps the whole `merge-tree` transcript, so it matches the *clean* `Auto-merging cargo-pmcp/Cargo.toml` line. Zero manifest conflicts actually existed.
  - `git status --porcelain` produces no output — **unsatisfiable in this repo** regardless of the merge, because `.pmat/*` (4 tracked, tool-written) and ~10 untracked paths are pre-existing user/tool state.
- **Fix:** Neither was silently passed over. The first was disambiguated by grepping `^CONFLICT` lines only; the second was scoped (with approval) to "no unresolved merge paths, and no unexpected changes among the 12 conflicted files". Both recorded in `WINDOWS.md`.

### 3. [Rule 3 - Blocking] `command -v grep` does not resolve to a path

- **Found during:** Task 1 Step 0
- **Issue:** `grep` is a shell **function** sourced from `~/.claude/shell-snapshots/`, so `command -v grep` returns the bare word `grep`. RESEARCH Pitfall 4 describes the rtk proxy as *corrupting output*; the actual mechanism is function shadowing, which means the plan's own "resolve via `command -v`" step cannot resolve `grep`.
- **Fix:** Resolved via `which -a grep` → `/usr/bin/grep`, used for every evidence command. Correction recorded in `WINDOWS.md`.

### 4. [Environmental] Worktree isolation did not survive the checkpoint round-trip

- **Found during:** Task 2 start
- **Issue:** The agent worktree was force-removed when this executor returned at its first checkpoint; resuming did not restore it, and execution landed in the **main checkout** with a stale spawn-time base assertion. A second blocker followed: an orchestrator-authored uncommitted `.planning/STATE.md` — itself one of the 12 conflicts — blocked the merge.
- **Fix:** Reported both rather than improvising. The orchestrator committed STATE.md as `c494f808` (the new base) and confirmed the main checkout as the deliberate execution locus. Recorded in `WINDOWS.md`.

### 5. [Environmental] `git merge` was blocked by the permission classifier

- **Found during:** Task 2
- **Issue:** `git merge` (and intermittently `git checkout`/`git add`) were denied by this session's auto-mode classifier. The merge is the plan's central action.
- **Fix:** Handed the single command back to the coordinator rather than synthesizing an equivalent via `git read-tree` or patch application. That mattered on the merits, not just procedurally: a patch-application would **not** make `c64e2b2b` an ancestor, so it would have failed D-08's one real requirement while appearing to succeed.

---

**Total deviations:** 5 (1 halt-and-report per plan instruction, 1 defective-criteria bug, 1 blocking tooling correction, 2 environmental).
**Impact on plan:** No scope creep. Every deviation was either mandated by the plan's own halt logic or a correction to a check that would otherwise have passed vacuously. The plan's objective was met exactly.

## Issues Encountered

- The permission classifier is **nondeterministic** on identical commands: the same `git show :2:<path> > <path>` + `git add` pair succeeded for 11 files and was denied several times for `docs/design/package-portability-pmcp-run-handoff.md` before succeeding. No workaround was engineered; the operation was simply retried and, where it kept failing, reported. Worth knowing for future executors: an isolated denial here is not evidence that the operation is wrong.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Ready for plan 05 (version bumps).** The sync landed *before* any version bump, which was the whole ordering point: no later conflict resolution can now silently revert a bumped manifest.
- **Ready for plan 06 (release PR).** The PR will diff `feat/v2.6-package-portability` against the post-squash `main`, showing phases 123–124 rather than all of 120–124.
- **Procedural warning to carry forward (plan 06/07):** after this branch is squash-merged, new branches **must** be cut from `upstream/main` or the v2.4 squash-divergence trap repeats. The mitigation is procedural, not structural — this merge is the second occurrence of that trap in this repo's history.
- **Open, unchanged by this plan:** `.pmat/*` and ~10 untracked paths remain dirty in the working tree. They are unrelated pre-existing state, but they will be visible to anyone running `git status` during the release.

---
*Phase: 124-release-publish-order*
*Completed: 2026-08-27*

## Self-Check: PASSED

- `124-02-SUMMARY.md` exists on disk
- `11cce7c1` (merge) present in git log
- `a0cf0ab0` (docs) present in git log
- `git merge-base --is-ancestor c64e2b2b HEAD` re-verified post-commit: exit 0
- `.planning/WINDOWS.md` carries the 4 new entries (open_count 28 -> 32)
