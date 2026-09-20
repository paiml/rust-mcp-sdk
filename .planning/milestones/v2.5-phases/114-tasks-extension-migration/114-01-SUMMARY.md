---
phase: 114-tasks-extension-migration
plan: 01
subsystem: infra
tags: [provenance, vendoring, schema, sha256, tripwire, mcp-tasks, ext-tasks, spec-hold]

# Dependency graph
requires:
  - phase: 112-version-plumbing-spine
    provides: "The `error_codes.rs` PROVENANCE-comment discipline this plan transfers to a whole vendored artifact"
  - phase: 113-stateless-http-multi-round-trip-elicitation
    provides: "113-SPEC-RECHECK.md — the hold-record structure, the binding re-verification shape, and the `hold` / Third Outcome Policy decision this phase inherits"
provides:
  - "schema/vendored/ext-tasks/{schema.ts,schema.json} — the authoritative v2 tasks wire schema pinned at commit 2c1425d9, byte-identical to upstream (proven twice)"
  - "schema/vendored/ext-tasks/PROVENANCE.md — repo/SHA/date/size/SHA256 per file, a reproduce-from-this-file-alone recipe, and the binding re-verification obligation"
  - "tests/vendored_schema_provenance.rs — a runtime-discovery tripwire that fails on any unrecorded edit, with 5 recorded negative controls"
  - "114-SPEC-RECHECK.md — the D-18 hold, the DQ6 both-repos trigger amendment, the three-branch outcome policy, and a 39-row wire-value inventory"
affects: [114-03, 114-05, 114-06, 114-08, 114-09, 114-10, 114-11, 114-12, 114-13, 114-14, 114-18, 114-verification]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Vendored third-party artifact + PROVENANCE record + digest tripwire (the error_codes.rs discipline scaled from a comment block to a directory)"
    - "Maximal-hex-run parsing so a 40-char commit pin is distinguishable from a 64-char digest's prefix"
    - "Loud skip (prints ASSERTED NOTHING) rather than silent pass when an external tool is unavailable"

key-files:
  created:
    - schema/vendored/ext-tasks/schema.ts
    - schema/vendored/ext-tasks/schema.json
    - schema/vendored/ext-tasks/PROVENANCE.md
    - tests/vendored_schema_provenance.rs
    - .planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md
  modified: []

key-decisions:
  - "DQ6 resolved BOTH: the D-18 hold clears only when a versioned (non-draft) schema directory exists in modelcontextprotocol/modelcontextprotocol AND in modelcontextprotocol/ext-tasks — six [~] requirements must not flip on a core-only publication event"
  - "The trigger is a CONDITION, not a date — inherited verbatim from 113, and it cuts both ways: the gate is neither discharged nor due merely because a day passed"
  - "Partial publication (one repo only) lands in STILL-ABSENT, not in a fourth state"
  - "schema/ deliberately NOT added to Cargo.toml [package] exclude — excluding it would break the tripwire for downstream `cargo test` on the published crate, the same failure that forced tests/team_contracts_conformance.rs out when contracts/ was excluded"
  - "The tripwire asserts attribution only, never schema content — wire shapes are asserted by the plans that implement them"
  - "sha256sum added as a fallback digest binary so the tripwire cannot silently skip on a CI image without perl shasum"

patterns-established:
  - "Pattern 1: a vendored artifact carries its own PROVENANCE.md and is guarded by a digest tripwire; editing it without updating the record is a test failure"
  - "Pattern 2: byte-identity to upstream is proven TWICE — SHA256 (post-fetch integrity) and git blob SHA-1 cross-checked against the GitHub contents API at the pinned commit"
  - "Pattern 3: a hold record mirrors 113-SPEC-RECHECK.md's section names verbatim so the two are diff-able"

requirements-completed: [TASK-01, TASK-02, TASK-03, TASK-04]

# Metrics
duration: 16min
completed: 2026-07-28
---

# Phase 114 Plan 01: Vendored ext-tasks Schema & D-18 Hold Record Summary

**The v2 tasks wire schema is now an offline, diff-able, digest-guarded artifact pinned at `2c1425d9`, and the phase's hold is written down with a both-repos trigger, a three-branch outcome policy, and a 39-row inventory of every wire value the next 17 plans will write.**

## Performance

- **Duration:** ~16 min
- **Started:** 2026-07-28T04:57:38Z
- **Completed:** 2026-07-28T05:13:16Z
- **Tasks:** 3
- **Files created:** 5
- **Files modified:** 0 (`Cargo.toml` / `Cargo.lock` byte-unchanged)

## Accomplishments

- **The network is out of every downstream plan's critical path.** Before this, every wire value in `114-RESEARCH.md` was a decaying network finding against a `draft/` directory in a repo whose own description begins *"Status: Experimental"*. Seventeen implementation plans can now read `schema/vendored/ext-tasks/` instead of re-fetching, and a reviewer can diff text against text.
- **Byte-identity to upstream is proven twice, not asserted once.** SHA-256 proves the files have not changed *since* the fetch; that alone cannot prove the fetch was faithful. Git blob SHA-1s cross-checked against the GitHub contents API at the pinned commit close that gap: `2634c47c…` / `d6ccaff7…` match exactly, as do the upstream-reported sizes (9421 / 46903).
- **The measurements match `114-RESEARCH.md` exactly** — 374 lines / 9421 bytes for `schema.ts`, 46,903 bytes for `schema.json` — which independently corroborates that research read the same artifact this plan pinned.
- **The hold is a runnable procedure, not a note.** `114-SPEC-RECHECK.md` carries a 4-step procedure, three landing states, and a 39-row inventory in which every row names the file it lands in and the plan that owns it.
- **The `-32003`/`-32021` upstream disagreement is recorded as its own row with a direction-recheck obligation**, so a contradiction *between two upstream documents* cannot quietly resolve itself in review.

## Task Commits

1. **Task 1: Vendor the ext-tasks draft schema at a pinned commit** — `667144a2` (chore)
2. **Task 2: Write the phase hold record (114-SPEC-RECHECK.md)** — `b5237c06` (docs)
3. **Task 3: Provenance tripwire test** — `40b5171b` (test)

## Files Created

- `schema/vendored/ext-tasks/schema.ts` — 374 lines / 9421 bytes. The authoritative v2 tasks TypeScript schema: `TaskStatus`, `Task`, the five `DetailedTask` variants, `CreateTaskResult = Result & Task`, `UpdateTaskRequest.params.inputResponses`, `TasksExtensionCapability = Record<string, never>`.
- `schema/vendored/ext-tasks/schema.json` — 1834 lines / 46,903 bytes. 24 `$defs` carrying the per-variant `required` arrays that make `result`/`error`/`inputRequests` status-conditional.
- `schema/vendored/ext-tasks/PROVENANCE.md` — the attribution record: source table, per-file digest table, the blob-SHA corroboration, a reproduce-this-fetch recipe, a change protocol, and the pointer to the hold.
- `tests/vendored_schema_provenance.rs` — 5 tests, 399 lines. Runtime file discovery, three named failure modes, and the 40-hex pin assertion.
- `.planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md` — 458 lines. The D-18 hold record.

## The pin

| Field | Value |
|-------|-------|
| Repository | `modelcontextprotocol/ext-tasks` |
| Commit | `2c1425d9a288b9b1f489430fe1e00bb392b47e48` |
| Commit date | 2026-07-15T20:41:09Z |
| Subject | `Bump hono in the npm_and_yarn group across 1 directory (#6)` |
| Fetched | 2026-07-28T05:00:58Z, over HTTPS from `raw.githubusercontent.com` **at the SHA**, never at `main` |
| `schema.ts` SHA256 | `2203cc75469e32a92a60f4b7b4de949577e25f18fafff69aa92ec06773ab70f6` |
| `schema.json` SHA256 | `b17cb4a2534379c214b17770bd5d3d54f69fde16a953bfb542c58235a61274bb` |

Worth carrying forward: the pinned commit's own subject is a **dependabot bump**, not a schema change. The schema content at this pin has been stable since at least 2026-07-15, and `pushed_at` (2026-07-15T20:42:26Z) says nothing has been pushed to the repository since.

## Measured state of the D-18 trigger (2026-07-28 — the date the final spec was due)

```
$ gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft

$ gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'
draft
```

Both exit 0; neither is UNAVAILABLE. **The condition is unmet in both repositories**, so the hold remains correctly engaged. Note the asymmetry a re-runner should carry: `ext-tasks` has **never** published a versioned directory, so there is no precedent there for what a release looks like.

## Negative controls — 5 run, all reverted

The plan mandated two. Five were run, because two of the three named failure modes would otherwise have been proven only as a side effect of another control, and because the plan's own acceptance-criterion command turned out to pass vacuously.

| # | Control | Mandated? | Result |
|---|---------|-----------|--------|
| 1 | Append one byte (`X`) to `schema.ts` | yes | **FAILURE MODE 1 fired**, naming `schema/vendored/ext-tasks/schema.ts`, printing computed `b4f3adf7…` against the recorded set. Mode 3 also fired (the old digest became stale) — correct and expected. |
| 2 | Delete `schema.json`'s recorded digest row from `PROVENANCE.md` | yes | **FAILURE MODE 1 fired ALONE**, naming `schema.json`; mode 3 PASSED. Controls 1 and 2 are not redundant — they fail different sets. |
| 3 | Remove `PROVENANCE.md` entirely | supplementary | **FAILURE MODE 2 fired** on 4 of 5 tests: *"THE VENDORED ARTIFACT IS UNATTRIBUTED"*. The anti-vacuity test correctly still PASSED (the files are still there). |
| 4 | Remove `schema.json` from disk | supplementary | **FAILURE MODE 3 fired ALONE and by name** — *"A STALE ENTRY: PROVENANCE RECORDS A DIGEST FOR A FILE THAT NO LONGER EXISTS"*, printing the orphaned `b17cb4a2…`. The anti-vacuity floor also fired (`found only 1 file(s)`). Mode 1 PASSED, which is the orthogonality proof: mode 3 is not a shadow of mode 1. |
| 5 | Strip every **maximal** 40-hex run from `PROVENANCE.md`, keeping both SHA-256 digests | supplementary | **The load-bearing one.** The commit-SHA test FAILED — while `grep -Eo '[0-9a-f]{40}'` on the same file **still returned two matches** (`2203cc75…` and `b17cb4a2…`, the 40-char *prefixes* of the two SHA-256 digests). See "Deviations" #1. Re-run after the `needless_collect` refactor and it still fired. |

After every control: `git diff --stat -- schema/` is **empty**, and the full 5-test suite re-runs green.

```
Summary [0.038s] 5 tests run: 5 passed, 2314 skipped
```

## Verification

| Gate | Result |
|------|--------|
| `cargo nextest run --features full -E 'test(/vendored_schema/)'` | **5 passed, 0 failed** (plan required ≥3) |
| `git diff --stat -- Cargo.toml Cargo.lock` | **empty** — zero-new-deps constraint held |
| `make fmt-check` | exit 0 |
| `make lint` | exit 0, zero warnings (pedantic + nursery, `--lib --tests`) |
| `114-SPEC-RECHECK.md` `## Verdict` | **PENDING** |
| `git check-ignore -v schema/vendored/ext-tasks/schema.ts` | prints nothing, exit 1 — not ignored |
| `python3 -c "json.load(...)"` on `schema.json` | exit 0 |
| Upstream blob SHA-1 cross-check | both match |

## Decisions Made

- **DQ6 → BOTH repositories.** The hold clears only when a versioned (non-`draft`) schema directory exists in `modelcontextprotocol/modelcontextprotocol` **and** in `modelcontextprotocol/ext-tasks`. Phase 113's condition was written before tasks moved to a separate repo (SEP-2663); read literally against 113's wording, a core-only release would have released six `[~]` requirements while saying nothing about five of them. An extension-only release is equally insufficient — `resultType` and the `-3202x`/`-32602` codes are graded by the core schema.
- **Partial publication lands in `STILL-ABSENT`.** Written explicitly so a half-published pair cannot be walked through steps 2–3 and called confirmed.
- **`schema/` stays OUT of `Cargo.toml`'s `[package] exclude`.** 56,324 bytes is immaterial against the crates.io limit, and excluding it would break `tests/vendored_schema_provenance.rs` for anyone running `cargo test` on the published crate — the exact failure that forced `tests/team_contracts_conformance.rs` out of the package when `contracts/` was excluded (`Cargo.toml:41-45`). Keeping it in also means `Cargo.toml` stays byte-unchanged, satisfying threat T-114-SC.
- **The tripwire asserts attribution only.** No schema content is parsed or validated here. Wire shapes belong to the plans that implement them, and mixing the two would make a wire-shape change look like a provenance failure.
- **`inputResponses`, not `inputs`.** Recorded as inventory row 26 with an explicit instruction: if a re-verifier finds `inputs` in the published schema that is DRIFT and reopens the phase, **not** a reason to "restore" the older name from the v2.5 research pack.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] The plan's own 40-hex acceptance criterion passes vacuously**

- **Found during:** Task 3 (Provenance tripwire test)
- **Issue:** The plan specified *"assert `PROVENANCE.md` contains a 40-hex-character commit SHA, so the record can never degrade into 'fetched from main'"*, and Task 1's acceptance criterion used `grep -Eo '[0-9a-f]{40}'`. **A 64-character SHA-256 digest contains a 40-character hex prefix.** A record that recorded digests but no commit pin would therefore satisfy that check while pinning nothing — precisely the degradation the assertion exists to prevent.
- **Fix:** The test extracts **maximal** runs of lowercase hex and requires one of length *exactly* 40, so a 64-char digest cannot satisfy it. The reason is written into the assertion's own failure message so it is not re-litigated.
- **Verification:** Negative control 5. With every maximal 40-hex run stripped and both SHA-256 digests intact, `grep -Eo '[0-9a-f]{40}'` still returned two matches while the test correctly FAILED.
- **Files modified:** `tests/vendored_schema_provenance.rs`
- **Committed in:** `40b5171b`

**2. [Rule 2 - Missing critical functionality] `sha256sum` fallback so the tripwire cannot silently skip in CI**

- **Found during:** Task 3
- **Issue:** The plan mandated shelling out to `shasum -a 256` and skipping if absent. `shasum` is a perl script; a CI image without perl would make this tripwire skip on every run — reporting green while asserting nothing about the artifact it exists to protect. A tripwire that can silently disable itself in the environment that matters most is not a tripwire.
- **Fix:** `DIGEST_BINARIES` tries `shasum -a 256` first (the canonical form, and what `PROVENANCE.md` documents) then `sha256sum` (GNU coreutils). The skip path additionally prints `SKIPPED (ASSERTED NOTHING)` and *"Treat this as UNVERIFIED, not as a pass"*, and names every binary tried.
- **Verification:** `make lint` + the 5-test suite green on macOS via `shasum`. The skip probe deliberately runs against `Cargo.toml` rather than `PROVENANCE.md`, so a **missing** `PROVENANCE.md` reports as FAILURE MODE 2 instead of being mistaken for a missing tool.
- **Files modified:** `tests/vendored_schema_provenance.rs`
- **Committed in:** `40b5171b`

**3. [Rule 3 - Blocking] Test names had to contain `vendored_schema` for the plan's verify command to select anything**

- **Found during:** Task 3
- **Issue:** The plan's verification command is `-E 'test(/vendored_schema/)'`. nextest's `test()` predicate matches **test names**, not binary names. With descriptive names like `every_vendored_files_digest_is_recorded_in_provenance_md`, the run reported `Starting 0 tests across 82 binaries` and `error: no tests to run` — a green-looking command that executed nothing.
- **Fix:** All five tests renamed to carry a `vendored_schema_` prefix. The plan's command now selects all five unchanged.
- **Verification:** `5 tests run: 5 passed`.
- **Files modified:** `tests/vendored_schema_provenance.rs`
- **Committed in:** `40b5171b`

**4. [Rule 1 - Bug] `make lint` rejected `needless_collect`**

- **Found during:** Task 3, verification
- **Issue:** The commit-SHA test collected filtered runs into a `Vec` and tested `is_empty()`. `clippy::needless_collect` (in `clippy::all`, `-D`) rejects it. `make lint` runs `--lib --tests`, so the new test file is linted like production code.
- **Fix:** Replaced with `.iter().any(|run| run.len() == 40)`.
- **Verification:** `make lint` exit 0. Negative control 5 was **re-run after the refactor** and still fired, confirming the assertion's semantics did not change.
- **Files modified:** `tests/vendored_schema_provenance.rs`
- **Committed in:** `40b5171b`

**5. [Rule 2 - Missing critical functionality] Two extra failure-mode controls plus an anti-vacuity floor**

- **Found during:** Task 3
- **Issue:** The plan mandated three failure modes but only two negative controls, and neither exercised FAILURE MODE 2 or FAILURE MODE 3 *in isolation*. Mode 3 fired only as a side effect of control 1, which cannot distinguish "mode 3 works" from "mode 3 is a shadow of mode 1". Separately, nothing stopped the digest tests passing over an empty directory.
- **Fix:** Controls 3 and 4 added (see table above), each firing exactly one mode by name. A `MINIMUM_VENDORED_FILES` floor was added as a deliberate *minimum* rather than a manifest, so vendoring a third file needs no test edit.
- **Verification:** Control 3 → mode 2 on 4 tests, anti-vacuity test correctly PASSES. Control 4 → mode 3 alone, mode 1 PASSES.
- **Files modified:** `tests/vendored_schema_provenance.rs`
- **Committed in:** `40b5171b`

---

**Total deviations:** 5 auto-fixed (4 × Rule 2, 1 × Rule 1, 1 × Rule 3 — #1 counted once)
**Impact on plan:** No scope creep. Four of the five make the tripwire non-vacuous — a tripwire that can pass while asserting nothing is worse than none, because it manufactures confidence. #4 is a house-standard lint fix. Zero source files outside `tests/` were touched; `Cargo.toml`/`Cargo.lock` are byte-unchanged.

## Issues Encountered

**The plan's Task-1 acceptance criteria were satisfiable in a weaker way than intended.** Two of them (`grep -Eo '[0-9a-f]{40}'`, and `test(/vendored_schema/)` selecting ≥3 tests) turned out to be satisfiable-or-vacuous for reasons only visible on execution. Both were tightened rather than worked around, and both tightenings carry a recorded negative control. Recorded here because it is a *plan-authoring* lesson, not an implementation defect: a hex-length assertion needs anchoring, and a nextest filterset constrains test **naming**.

**No production defect was found and none was fixed.** This plan changed zero files under `src/`.

## Known Stubs

None. Every artifact is complete and load-bearing today.

## Threat Flags

None. This plan adds no network endpoint, no auth path, no schema at a trust boundary, and no file access beyond reading two vendored files and one markdown record from `CARGO_MANIFEST_DIR`. The threat register's four entries are all discharged as planned:

| Threat ID | Disposition | Evidence |
|-----------|-------------|----------|
| T-114-01 (tampering with vendored files) | mitigated | SHA-256 per file in `PROVENANCE.md`, re-checked every test run; proven by controls 1–4 |
| T-114-02 (repudiation / attribution) | mitigated | Full 40-char SHA + fetch date recorded and **asserted present**; fetch pinned to the SHA; proven by control 5 |
| T-114-03 (spoofed upstream fetch) | accepted | HTTPS from `raw.githubusercontent.com` at a `gh`-resolved SHA; additionally corroborated by matching git blob SHA-1s from the GitHub API |
| T-114-SC (supply chain) | accepted | Zero packages installed, zero dependencies added; `Cargo.toml`/`Cargo.lock` byte-unchanged (verified empty `git diff --stat`) |

## Next Phase Readiness

**Wave 1 is complete and everything downstream is unblocked.**

- Plans 114-02…114-20 can read wire values from `schema/vendored/ext-tasks/` offline. **No downstream plan needs `gh` or network access to establish a wire shape.**
- `114-SPEC-RECHECK.md` § Wire-Value Inventory is the checklist the phase's verification and the eventual re-verification run both walk. Rows already name owning plans, so a plan that lands a value not in the table is a visible gap.
- Two rows are flagged for the plans that own them:
  - **Row 23 (`inputRequests` required on `InputRequiredTask`)** is the highest-severity row. Phase 113's `own_reserved_result_fields` silently deletes that key. Owned by **114-10 (DQ2)**, which must land before any plan depends on v2 `tasks/get`.
  - **Row 34 (`Mcp-Name` = `params.taskId`)** must use a *separate* name-key table. `logical_name_key` and `mrtr_eligible` both derive from `MRTR_METHODS`, so a row there would make `tasks/update` MRTR-eligible and `splice_mrtr_params` would delete its entire payload. Owned by **114-06 (DQ4)**.

**Carried forward for the re-verification run (not blockers for this phase):**
- Core PR **#2678** (`SEP-2678`, proposing `-32000`/`-32001`/`-32002`) — inherited from `113-SPEC-RECHECK.md`. Re-check at every run; it would contradict the reserved-codes rule TASK-03 relies on.
- The **`-32003` vs `-32021`** upstream disagreement — its own inventory row with a direction-recheck obligation.
- `114-RESEARCH.md` **A2** (extension versions independently of core) is recorded as a MEDIUM-risk assumption inside § Trigger Condition so a re-runner can falsify it rather than inherit it.

**No requirement checkbox was flipped and `.planning/REQUIREMENTS.md` has a 0-byte diff.** Stated precisely, because the distinction matters to a verifier: TASK-01…TASK-06 currently read **`[ ]`** (`REQUIREMENTS.md:84-89`, traceability rows `Pending`), *not* `[~]`. This plan's `files_modified` does not include `REQUIREMENTS.md` and Task 2's acceptance criteria never asked for an edit there — its job was to **record** the hold, not to apply the booking. Applying the `[~]` *implemented; pending final schema* booking belongs to the phase's closing plan, once there is an implementation to book.

`gsd-sdk query requirements.mark-complete` was **deliberately NOT run** despite this plan's `requirements: [TASK-01…TASK-04]` frontmatter. Running it would flip four checkboxes to `[x]`, directly contradicting the `## Verdict: PENDING` this plan committed and the D-18 hold it exists to record. `## Verdict` is `PENDING`.

---
*Phase: 114-tasks-extension-migration*
*Completed: 2026-07-28*

## Self-Check: PASSED

**Files claimed created — all verified present:**
- `FOUND: schema/vendored/ext-tasks/schema.ts` (374 lines, ≥300 required)
- `FOUND: schema/vendored/ext-tasks/schema.json` (1834 lines, ≥100 required)
- `FOUND: schema/vendored/ext-tasks/PROVENANCE.md` (contains `SHA256`)
- `FOUND: tests/vendored_schema_provenance.rs` (contains `read_dir`)
- `FOUND: .planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md` (contains `Third Outcome Policy`)

**Commits claimed — all verified in `git log --oneline --all`:**
- `FOUND: 667144a2` — chore(114-01): vendor ext-tasks draft schema at a pinned commit
- `FOUND: b5237c06` — docs(114-01): record the D-18 hold with a three-branch outcome policy
- `FOUND: 40b5171b` — test(114-01): provenance tripwire over the vendored ext-tasks schema

**Key link verified:** `tests/vendored_schema_provenance.rs` → `schema/vendored/ext-tasks/PROVENANCE.md` via recomputed SHA-256 compared against the recorded digests (pattern `PROVENANCE` present in both).
