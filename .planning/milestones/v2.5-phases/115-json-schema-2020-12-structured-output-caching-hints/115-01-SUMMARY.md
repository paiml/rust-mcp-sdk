---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 01
subsystem: testing
tags: [json-schema, provenance, vendoring, tripwire, mcp-spec-2026-07-28, cacheable-result, nextest]

# Dependency graph
requires:
  - phase: 114-tasks-extension-migration
    provides: "schema/vendored/ext-tasks/ + tests/vendored_schema_provenance.rs — the vendoring format of record and the single-tree tripwire this plan generalizes; inject_v2_result_envelope, which already supplies CacheableResult's third required key resultType"
provides:
  - "schema/vendored/core-2026-07-28/{schema.ts,schema.json} — the published MCP 2026-07-28 core schema, byte-pinned to upstream commit 271ecc9accafdd9b83a3c869fa67c22953b2af80"
  - "schema/vendored/core-2026-07-28/PROVENANCE.md — fetch attribution, both SHA256 digests, both git blob SHA-1 corroborations, a runnable reproduce block and a change protocol"
  - "A provenance tripwire that scans EVERY tree under schema/vendored/ rather than one hardcoded path, with a MINIMUM_VENDORED_TREES anti-vacuity floor"
  - "tests/v2_core_schema_facts.rs — 8 runtime assertions re-deriving every Phase 115 wire fact from the pinned bytes"
  - "MEASURED: ttlMs is type \"integer\" with minimum 0 in the generated JSON Schema (the .ts says `number`) — u64 is now a measured mapping, not an inference"
  - "MEASURED: exactly SIX $defs extend CacheableResult (DiscoverResult is the sixth), confirmed independently from schema.json and schema.ts"
affects: [115-03, 115-04, 115-05, 115-06, 115-07, 115-08, 115-09, 115-10, 115-11, "any future re-vendoring of an MCP schema", "any plan choosing a Rust type for ttlMs or cacheScope"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Vendored-tree provenance: one PROVENANCE.md per immediate subdirectory of schema/vendored/, discovered by runtime read_dir, never by a hardcoded path"
    - "Content tripwire as the complement of an attribution tripwire: provenance proves the bytes are what was fetched; a separate suite proves what those bytes say"
    - "Digests known BEFORE the fetch, so the fetch is a hard precondition rather than a post-hoc record"

key-files:
  created:
    - schema/vendored/core-2026-07-28/schema.ts
    - schema/vendored/core-2026-07-28/schema.json
    - schema/vendored/core-2026-07-28/PROVENANCE.md
    - tests/v2_core_schema_facts.rs
  modified:
    - tests/vendored_schema_provenance.rs

key-decisions:
  - "Phase 115 test 4 renamed to v2_core_schema_facts_exactly_six_defs_extend_cacheable_result — the plan's own name contained the word the plan's own acceptance grep forbids; the grep (which enforces the wrong-JSON-pointer-spelling ban) was treated as load-bearing and the name yielded"
  - "The ext-tasks READ-ONLY bullet naming tests/vendored_schema_provenance.rs as the only test consumer was RESTATED rather than copied verbatim: this tree has a second consumer (v2_core_schema_facts.rs) that DOES assert content, and copying the bullet would have made the record lie about itself"
  - "ttlMs maps to u64 on MEASURED grounds: the generated JSON Schema narrows the TypeScript `number` to {type: integer, minimum: 0}"
  - "SCHM-03's target set is SIX result types, not the five the requirement text names — DiscoverResult carries cacheScope/resultType/ttlMs in its own required array"
  - "schema/ stays OUT of Cargo.toml [package] exclude: 336 KB total is immaterial, and excluding it would break cargo test on the published crate"

patterns-established:
  - "Per-tree provenance loop: vendored_trees() enumerates schema/vendored/* at runtime; every failure message names the offending TREE as well as the file"
  - "Anti-vacuity floors at two levels: MINIMUM_VENDORED_TREES (a scan that finds no trees fails) and MINIMUM_VENDORED_FILES applied per tree"
  - "Whitespace-collapsed TypeScript scanning: a declaration whose `extends` clause wraps to the next line is invisible to a line-oriented scan, and undercounting six as five is exactly the defect the file exists to prevent"
  - "Every failure message ends in one shared REMEDY constant naming the change protocol, because the wrong remedy (edit the assertion) is always faster"

# DELIBERATELY EMPTY. This plan lands the EVIDENCE BASE for SCHM-01/02/03, not their
# implementation — which is 115-03…115-09's work. Booking them complete here would be a
# false claim. See "## Requirement Bookkeeping" below. 115-10/115-11 own the flip.
requirements-completed: []
requirements-evidenced: [SCHM-01, SCHM-02, SCHM-03]

# Metrics
duration: 38min
completed: 2026-08-01
---

# Phase 115 Plan 01: Vendor + Re-derive the 2026-07-28 Core Schema Summary

**The published MCP `2026-07-28` core schema is now in-tree at a 40-character pin with both digests verified against pre-known values, the provenance tripwire scans every vendored tree instead of one hardcoded path, and 8 runtime assertions re-derive every Phase 115 wire fact from those bytes — including the measured `ttlMs: {type: "integer", minimum: 0}` that settles `u64`.**

## Performance

- **Duration:** ~38 min
- **Started:** 2026-08-01T05:33Z
- **Completed:** 2026-08-01T06:11Z
- **Tasks:** 3 of 3
- **Files modified:** 5 (4 created, 1 rewritten)

## Accomplishments

- **Vendored the published core schema at a pin, verifiable two independent ways, with both digests known in advance.** `schema.ts` (98,426 B / 3,197 lines) and `schema.json` (181,474 B / 3,963 lines) fetched from `raw.githubusercontent.com` at commit `271ecc9accafdd9b83a3c869fa67c22953b2af80` — never at `main`. Every phase plan downstream now builds against a diff-able offline artifact instead of the CONTEXT's own self-described "strong prior, not a verified fact".
- **Closed the Pitfall-8 hole before it could open.** The provenance tripwire used to hardcode one directory; adding a second tree would have left it unverified while the suite stayed green. It now enumerates `schema/vendored/*` at runtime with a floor on the number of trees, and a dedicated test asserts both known trees are discovered.
- **Made the phase's wire facts self-rotting.** `tests/v2_core_schema_facts.rs` re-derives the `CacheableResult` contract from the vendored bytes at runtime. A re-vendoring that changes a fact now breaks a named test instead of silently invalidating `src/`.
- **Measured, rather than inferred, the one fact the cross-AI review flagged.** `ttlMs` is `{"type": "integer", "minimum": 0}` in the generated schema even though the TypeScript source says `number`.
- **Zero production bytes changed.** `git diff --stat e67c69e7~1..HEAD -- src/ Cargo.toml Cargo.lock` is empty across all three commits.

## Task Commits

Each task was committed atomically:

1. **Task 1: Vendor the pinned core 2026-07-28 schema with a PROVENANCE.md** — `e67c69e7` (docs)
2. **Task 2: Generalize the provenance tripwire to every tree under schema/vendored/** — `32ed7cab` (test)
3. **Task 3: Re-derive the CacheableResult contract — including ttlMs's type — from the pinned artifact** — `bff83725` (test)

## Files Created/Modified

- `schema/vendored/core-2026-07-28/schema.ts` — the 2026-07-28 TypeScript protocol source at the pin (98,426 bytes, 3,197 lines, 7 `CacheableResult` occurrences)
- `schema/vendored/core-2026-07-28/schema.json` — the generated JSON Schema at the same pin (181,474 bytes, 3,963 lines, 155 `$defs` entries)
- `schema/vendored/core-2026-07-28/PROVENANCE.md` — 213 lines; `## Source`, `## Vendored files`, `### Independent corroboration — git blob SHA-1`, `### Schema shape notes for readers`, `## Reproducing this fetch`, `## Why these are published, not final, values`, `## RE-VERIFICATION OBLIGATION (binding)`, `## Change protocol`
- `tests/vendored_schema_provenance.rs` — rewritten as a per-tree loop (446 lines, 6 tests; was 359 lines, 5 tests). All five original test names preserved verbatim
- `tests/v2_core_schema_facts.rs` — new, 711 lines, 8 tests

## Verification Results

### Digest verification — BOTH matched the pre-known values on the first fetch

| File | Expected SHA256 | Measured | Match |
|------|-----------------|----------|-------|
| `schema.ts` | `742750af0bb8c716e7030c4977c992b55d1adc4407e9e66997db5846baedc2cd` | identical | ✓ |
| `schema.json` | `ef70b61f99b6d2e5e3b46863822eab08dff6a45bedc7a08914e0e5b133f40203` | identical | ✓ |

| File | Expected blob SHA-1 | Local `git hash-object` | GitHub contents API @ pin | Match |
|------|---------------------|-------------------------|---------------------------|-------|
| `schema.ts` | `9b55feeb412bc3ae877f2eac10b5c01ba29a2eed` | identical | identical | ✓ |
| `schema.json` | `213c58f6d9a1c2ce6ad055afe90bbdb095a29ee8` | identical | identical | ✓ |

Byte counts also matched exactly (98426 / 181474), as did upstream's API-reported sizes. The commit's committer date (`2026-07-28T16:42:34Z`), subject, and the prior commit on the path (`b488c16623e5202a3961e551886044577ae0f096`, `Add 2026-07-28 MCP specification`, `2026-07-28T15:56:05Z`) were all confirmed live via `gh api`.

### The six-extender set — matched EXACTLY, from both files independently

Measured from `schema.json` (every `$defs` entry other than `CacheableResult` carrying a `cacheScope` property) **and** from `schema.ts` (every `export interface … extends …CacheableResult`), the two derivations agree:

```
DiscoverResult, ListPromptsResult, ListResourceTemplatesResult,
ListResourcesResult, ListToolsResult, ReadResourceResult
```

Each of the six lists all three of `cacheScope`, `resultType`, `ttlMs` in its own `required` array alongside its payload key — e.g. `ListToolsResult.required = ["cacheScope", "resultType", "tools", "ttlMs"]`, `DiscoverResult.required = ["cacheScope", "capabilities", "resultType", "supportedVersions", "ttlMs"]`.

**`DiscoverResult` is the measured sixth**, confirming Finding 5 against the phase requirement text's "five".

⚠ **A trap worth carrying forward:** `ListResourceTemplatesResult`'s `extends` clause is on the line AFTER its name in the vendored `.ts`. A line-oriented scan finds five, not six, and reports green. `ts_interfaces_extending_cacheable_result` collapses whitespace before scanning for exactly this reason.

### `ttlMs` — MEASURED

```json
{ "type": "integer", "minimum": 0, "description": "…" }
```

The TypeScript source declares `ttlMs: number` with an `@minimum 0` doc annotation. The **generated** JSON Schema — the artifact a peer validates against — narrows it to a non-negative integer. `u64` in `src/types/caching.rs` is therefore a measured mapping, not an inference from a comment. This is asserted by `v2_core_schema_facts_ttl_ms_is_a_nonnegative_integer` in two parts (`type` and `minimum`) so a widening of either fails by name.

Also measured: `cacheScope` is `{"type": "string", "enum": ["private", "public"]}` — a closed union, discharging D-09's retired risk. `CallToolResult.properties.structuredContent` carries a `description` and NO `type` and NO `properties` — an unconstrained JSON value; the v1 sentence `Currently restricted to` appears **0 times** in `schema.ts`. `Tool.properties.outputSchema` is `{"type": "object", "properties": {"$schema": {"type": "string"}}, "additionalProperties": {}}` with no `required` array — the spec itself declares the optional `$schema` that D-02 pins past.

### Negative controls — all three fired, messages transcribed

**NC-1 (Task 2): remove `schema/vendored/core-2026-07-28/PROVENANCE.md`.** Result: `6 tests run: 2 passed, 4 failed`. Observed message:

```
FAILURE MODE 2 — THE VENDORED ARTIFACT `core-2026-07-28` IS UNATTRIBUTED.
Could not read schema/vendored/core-2026-07-28/PROVENANCE.md: No such file or directory (os error 2)

The files in schema/vendored/core-2026-07-28/ are a copy of a third-party schema. Without
PROVENANCE.md nothing records which upstream repository and commit they came from, whether
they were edited after the fetch, or what obligation is held against them.

WHAT TO DO: restore schema/vendored/core-2026-07-28/PROVENANCE.md from git history
(`git log -- schema/vendored/core-2026-07-28/PROVENANCE.md`). Do NOT write a fresh record
from memory — re-fetch at a pinned commit and record the measured digests.
```

The tree is named, which is the whole point of the generalization. File restored; digest re-verified afterwards.

**NC-A (Task 3): a COPY of `schema.json` with `CacheableResult.required` truncated to `["ttlMs"]`.** Result: `v2_core_schema_facts_cacheable_result_requires_all_three_fields` FAILED. Observed message:

```
assertion `left == right` failed: /$defs/CacheableResult/required changed.

Measured (sorted): ["ttlMs"]
Expected (sorted): ["cacheScope", "resultType", "ttlMs"]
Artifact:          schema/vendored/core-2026-07-28

Note before assuming the artifact is wrong: `resultType` belongs to this SAME base and is
ALREADY implemented — Phase 114's `inject_v2_result_envelope` injects it, which is why nothing
in Phase 115 adds it. A two-element expectation of ["cacheScope", "ttlMs"] is the reader's
error, not the schema's.

WHAT TO DO: re-run the `## Change protocol` in schema/vendored/core-2026-07-28/PROVENANCE.md,
then re-derive the Rust side from the new artifact. Do NOT edit this assertion to match — an
assertion edited to fit records nothing and detects nothing thereafter.
```

**NC-B (Task 3): a COPY of `schema.json` with `ttlMs.type` widened to `"number"`.** Result: `v2_core_schema_facts_ttl_ms_is_a_nonnegative_integer` FAILED. Observed message (description elided):

```
assertion `left == right` failed: /$defs/CacheableResult/properties/ttlMs is no longer typed `integer`.

Measured: {"description":"…","minimum":0,"type":"number"}
Artifact: schema/vendored/core-2026-07-28

This is the assertion that justifies `u64` in src/types/caching.rs. If a re-vendoring changed
this to `number`, a conformant peer may legitimately send a fractional TTL and `u64` would
REJECT it at deserialization — a spec-conformant response failing to parse. The Rust
representation must change with the schema; this assertion must not.

WHAT TO DO: re-run the `## Change protocol` in …
  left: Some("number")
 right: Some("integer")
```

Both NC-A and NC-B edited a COPY at `.negctl-tmp/schema.json` with the test's `SCHEMA_JSON` constant temporarily repointed. The vendored bytes were never touched; `.negctl-tmp/` was deleted and the test file restored from a `/bin/cp` snapshot. `git stash` was NOT used.

### Suites and gates

| Check | Result |
|-------|--------|
| `cargo nextest run --features full -E 'binary(vendored_schema_provenance)'` | **6 tests run: 6 passed** (was 5) |
| `cargo nextest run --features full -E 'binary(v2_core_schema_facts)'` | **8 tests run: 8 passed** — exactly the 8 the plan specifies |
| `cargo fmt --all -- --check` | exit 0 |
| `make lint` | exit 0, "✓ No lint issues" |
| `make check-todos` | exit 0 |
| `git diff --stat -- src/ Cargo.toml` | **EMPTY** |
| `git diff --stat e67c69e7~1..HEAD -- src/ Cargo.toml Cargo.lock` | **EMPTY** across the whole plan |
| Deletions across the plan's three commits | **NONE** (`git diff --diff-filter=D` empty) |

Per the plan's `commit_policy`, the scoped gate was run rather than the full `make quality-gate`; the full gate runs once for the phase in 115-10.

## Decisions Made

1. **`ttlMs` → `u64` is now a measured decision** (see above). Downstream plans should cite `v2_core_schema_facts_ttl_ms_is_a_nonnegative_integer`, not the TypeScript `@minimum 0` annotation.
2. **SCHM-03 targets SIX result types.** `DiscoverResult` is included. Excluding it would ship a knowingly non-conformant v2 `server/discover` — the first call a v2 client makes.
3. **`schema/` stays out of `Cargo.toml`'s exclude list.** Restated with this tree's real arithmetic (279,900 new bytes; ~336 KB total) rather than copying ext-tasks' 56,324.
4. **`PROVENANCE.md` says "published, not final" rather than ext-tasks' "pre-final".** This tree comes from a versioned directory in the core spec repo, not a `draft/` in an Experimental one — so its values are published. But the pin is itself a post-publication fix applied 46 minutes after the directory was created, so the bytes can still drift. Both halves are stated; neither alone is honest.
5. **The record notes it satisfies the core-repository half of Phase 114's D-18 trigger** and explicitly does NOT assert the `ext-tasks` half, which stays owned by `114-SPEC-RECHECK.md`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Task 3's prescribed test name was unsatisfiable against Task 3's own acceptance grep**

- **Found during:** Task 3
- **Issue:** The plan names test 4 `v2_core_schema_facts_exactly_six_definitions_extend_cacheable_result`, and the same task's acceptance criteria require `grep -c 'definitions' tests/v2_core_schema_facts.rs` to return **0** — "the wrong pointer spelling is not present anywhere, including in comments". The two cannot both hold: the prescribed name contains the forbidden token.
- **Fix:** Renamed to `v2_core_schema_facts_exactly_six_defs_extend_cacheable_result`. The grep was treated as load-bearing because it enforces a real correctness property (the `$defs`-vs-older-spelling confusion cost the pre-review plan set a failing assertion), whereas the test name's English word carries no such property. All other seven names are verbatim as prescribed.
- **Files modified:** `tests/v2_core_schema_facts.rs`
- **Verification:** `grep -c 'definitions'` → 0; 8 tests run, 8 passed.
- **Committed in:** `bff83725`

**2. [Rule 3 - Blocking] The module doc's own explanation of a banned token tripped the ban**

- **Found during:** Task 3
- **Issue:** `grep -c 'include_str!'` must return 0, but the module doc explained *why the file does not use it* — naming it twice. Same class of defect as (1): a prose mention of a forbidden token trips a mechanical grep.
- **Fix:** Rephrased to "the standard string-embedding macro", with an explicit sentence recording that the macro's name is deliberately absent so the property stays greppable. The reasoning is preserved; only the token is gone.
- **Files modified:** `tests/v2_core_schema_facts.rs`
- **Verification:** `grep -c 'include_str'` → 0. The runtime-read property itself is unchanged (`read_to_string` throughout).
- **Committed in:** `bff83725`

**3. [Rule 2 - Missing Critical] The "copy verbatim" READ-ONLY bullet would have made the new record lie about itself**

- **Found during:** Task 1
- **Issue:** The plan says to copy ext-tasks' four READ-ONLY bullets verbatim. Its third bullet states that the vendored files' only test consumer is `tests/vendored_schema_provenance.rs`, "which reads them solely to recompute digests... That test asserts *attribution*, never schema *content*." That is true of `ext-tasks`. It is **false** for `core-2026-07-28`, whose whole point is that Task 3 adds a second consumer that DOES assert content.
- **Fix:** Bullets 1, 2 and 4 copied verbatim. Bullet 3 restated to name both consumers and to spell out the (b)/(c) split, with a parenthetical recording that it was restated rather than copied and why. A provenance record whose own self-description is inaccurate undermines the one thing it exists to provide.
- **Files modified:** `schema/vendored/core-2026-07-28/PROVENANCE.md`
- **Verification:** All required headings and literals present; provenance suite green.
- **Committed in:** `e67c69e7`

**4. [Rule 3 - Blocking] The module doc's own history reference tripped Task 2's acceptance grep**

- **Found during:** Task 2
- **Issue:** `grep -c '"schema/vendored/ext-tasks"'` must return 0, but the rewritten module doc quoted the removed constant *as history* — "It used to hard-code `const VENDORED_DIR: &str = "schema/vendored/ext-tasks"`" — scoring 1.
- **Fix:** Rephrased to "a single `VENDORED_DIR` constant naming the `ext-tasks` tree". The historical explanation survives; the quoted path does not.
- **Files modified:** `tests/vendored_schema_provenance.rs`
- **Verification:** grep → 0; 6 tests run, 6 passed.
- **Committed in:** `32ed7cab`

**5. [Rule 1 - Formatting] rustfmt reflowed three assertion call sites**

- **Found during:** Task 3
- **Issue:** `cargo fmt --all -- --check` exited 1 on the first draft (two `assert_eq!` argument lists and one method chain).
- **Fix:** Ran `cargo fmt --all`; re-ran both suites (14/14 green) and `make lint` (exit 0) after.
- **Files modified:** `tests/v2_core_schema_facts.rs`
- **Verification:** `cargo fmt --all -- --check` exit 0.
- **Committed in:** `bff83725`

---

**Total deviations:** 5 auto-fixed (3 blocking-grep contradictions, 1 missing-critical accuracy fix, 1 formatting)
**Impact on plan:** No scope creep. Three of the five are the same class of plan-text defect — an acceptance grep that the plan's own prescribed prose or test name violates — and in each case the grep was kept and the prose yielded, because the grep encodes the property and the prose merely describes it. Deviation 3 is the only substantive content change, and it makes the record truthful about its own consumers.

**A note for the phase's plan-text defect tally:** deviations 1, 2 and 4 are three measured contradictions **inside a single plan**, all of the same shape. Any future plan writing a "this token must not appear" acceptance criterion should state whether prose mentions count — otherwise the criterion forbids the file from explaining itself.

## Issues Encountered

- **`.planning/STATE.md` arrived already modified** by the orchestrator (phase-115-executing marker) before this executor's first commit. Left untouched by the task commits and folded into the plan metadata commit, which is where STATE.md belongs.
- Nothing else. Both fetches, both digest checks and all four blob cross-checks succeeded on the first attempt.

## Requirement Bookkeeping

**`requirements mark-complete` was deliberately NOT run, and `.planning/REQUIREMENTS.md` is untouched (0-byte diff).**

This plan's frontmatter carries `requirements: [SCHM-01, SCHM-02, SCHM-03]`, but so do seven other plans in this phase — including 115-10 and 115-11, the last to execute. What 115-01 lands is the **evidence base** those requirements will be booked on (D-15), not their implementation:

| Req | What 115-01 provides | Who implements it |
|-----|----------------------|-------------------|
| SCHM-01 | `Tool.outputSchema` declares an optional `$schema` — asserted, so pinning 2020-12 is a spec-aware choice | 115-03, 115-04, 115-08, 115-09 |
| SCHM-02 | `structuredContent` is an unconstrained JSON value and the v1 "Currently restricted to" sentence is gone — asserted | 115-03, 115-04, 115-09 |
| SCHM-03 | The SIX extenders, their per-type `required` arrays, `cacheScope`'s closed union and `ttlMs`'s integer/minimum — all asserted | 115-02, 115-06, 115-07, 115-08, 115-09 |

Marking a requirement `[x]` because the schema proving what it must do is now in-tree is the exact failure Phase 114 spent a sign-off preventing. The flip belongs to whoever lands the last implementing plan and can cite runnable behaviour, not to this one. `REQUIREMENTS.md` rows stay `[ ]` / `Pending`.

## Known Stubs

None. This plan ships no production code and no placeholder values; every constant in both test files is a measured value with a named source.

## Threat Flags

None. The plan touches no network surface, no auth path, no schema at a trust boundary, and installs no package. `T-115-07` / `T-115-08` / `T-115-09` / `T-115-33` are all discharged by measurement above; `T-115-SC` is vacuous here (no `cargo add`, no `slopcheck install` run in-repo).

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

**Ready.** Every downstream Phase 115 plan can now cite a pinned artifact instead of a network summary:

- **115-03 / 115-04 (SCHM-01, the 2020-12 pin):** `Tool.outputSchema` declaring an optional `$schema` is asserted, so D-02's "ignore the declared `$schema`" is a spec-aware choice with a test behind it.
- **115-05 (the Rust caching types):** `cacheScope` → a two-variant enum and `ttlMs` → `u64` are both measured, and both break loudly if a re-vendor moves them.
- **115-06…09 (SCHM-03 projection):** the target list is **six**, and each of the six requires all three keys on the v2 wire. A plan with exactly five field-addition tasks is wrong (Pitfall 7).
- **115-10:** owns the full `make quality-gate` run for the phase.

**Carry-forward traps:**

1. Use `binary(<file_stem>)` in nextest selectors, or prefix every test with the file stem. Both new suites do the latter, so `test(/v2_core_schema_facts/)` also selects correctly.
2. A line-oriented scan of `schema.ts` undercounts the extenders five-to-six. Collapse whitespace first.
3. Adding a third vendored tree now requires a `PROVENANCE.md` immediately — the suite fails by tree name until it exists. That is the intended behaviour.

## Self-Check: PASSED

All six claimed files exist on disk; all four claimed commit hashes (`e67c69e7`, `32ed7cab`, `bff83725`, `484ca732`) resolve in `git log`.

---
*Phase: 115-json-schema-2020-12-structured-output-caching-hints*
*Completed: 2026-08-01*
