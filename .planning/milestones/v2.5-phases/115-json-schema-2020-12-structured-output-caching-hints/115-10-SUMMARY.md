---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 10
subsystem: planning
tags: [phase-gate, requirements-booking, deferred-items, stale-doc-sweep, contract-bindings, semver, pmat, sign-off]

# Dependency graph
requires:
  - phase: 115-01
    provides: the pinned published core schema at `schema/vendored/core-2026-07-28/` that every SCHM booking cites as its evidence
  - phase: 115-11
    provides: the thirteen contract bindings and the ghost-binding resolver this plan makes load-bearing
  - phase: 115-03
    provides: the Draft 2020-12 pin, the `compile_for_era` split, and the measured correction to the era-divergence example
  - phase: 115-09
    provides: the fuzz target, the committed corpus and the runnable example this plan re-verifies by direct command
provides:
  - "SCHM-01/02/03 booked `[x]` with measured, re-derivable evidence and their deviations stated INSIDE the booking"
  - ".planning/phases/115-.../deferred-items.md — 36 entries, each owned or explicitly unowned, plus D-114-R/S, D-113-U and D-114-U"
  - "contracts/binding.yaml with ZERO `status: planned` and a fourteenth binding for `compile_for_era`"
  - "A measured correction: only TWO of the six cacheable results are handler-settable, and `resources/templates/list` has no `ResourceHandler` hook at all"
  - "An OWNER-ANSWERED sign-off (Guy Ernest, 2026-08-01, no corrections) and — only after it — the ROADMAP/STATE completion markers that close Phase 115"
affects: [116, 117, 118, 119, any phase running gates in this repository]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Phase-base deltas materialized with `git archive <base> | tar -x` into a scratch tree — no worktree, no stash, no checkout of the live tree"
    - "`cargo semver-checks check-release --baseline-rev <sha>` for the breaking-change verdict plus `cargo public-api --simplified` set-diff for the additions list"
    - "Ledger IDs constrained to a single leading character so a duplicate-ID grep stays meaningful"

key-files:
  created:
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-10-SUMMARY.md
  modified:
    - .planning/ROADMAP.md
    - .planning/STATE.md
    - contracts/binding.yaml
    - tests/phase115_contract_bindings.rs
    - src/types/tools.rs
    - src/types/prompts.rs
    - src/types/protocol/mod.rs
    - src/types/resources.rs
    - schema/vendored/ext-tasks/PROVENANCE.md
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-RESEARCH.md
    - .planning/REQUIREMENTS.md

key-decisions:
  - "Book `[x]`, not `[~]` — Phase 115's wire values come from the PUBLISHED core schema, so D-15's contingency did not fire and Phase 114's hold is not inherited"
  - "Do NOT edit CLAUDE.md's contract path: rewriting a project-wide standing instruction is not a phase executor's call; the deviation is recorded in the bookings and the ledger instead"
  - "Keep `ListResourceTemplatesResult`'s builders despite proving them dispatcher-unreachable — the type is `pub` and constructible by a custom transport; adding a templates seam to `ResourceHandler` is a breaking trait change"
  - "Accept the `structuredContent: null` typed-re-read collapse as a documented client-side limitation rather than changing `deserialize_with` on both eras"
  - "Do NOT re-run `make test-feature-flags` on a materialized base tree: `Cargo.lock` is gitignored, so a scratch checkout resolves a DIFFERENT dependency set and would be less faithful than the recorded base"

patterns-established:
  - "A booking states the judgement it makes rather than absorbing it — the naive-pin bypass, the absent v1 guard, and the in-process era bound are all written into the requirement record"
  - "An anti-vacuity assertion must pin an invariant, never a transient state: `planned > 0` inverted the moment the work it guarded was completed"

metrics:
  duration: ~3h10m
  completed: 2026-08-01
  tasks_completed: 3 of 3
  commits: 3
---

# Phase 115 Plan 10: Phase Gate, Requirement Bookings and Sign-off Summary

**Swept the tree BEFORE gating it, made the contract bindings assert what actually shipped, booked
SCHM-01/02/03 on measured evidence with their deviations inside the booking, held ROADMAP and STATE
untouched until the owner answered the sign-off — and then, and only then, applied the completion
markers that close Phase 115.**

**STATUS: 3 of 3 tasks complete. Task 3's `checkpoint:human-verify gate="blocking"` was returned
UNANSWERED rather than self-approved and was APPROVED by Guy Ernest (owner) on 2026-08-01 with no
corrections. Phase 115 is CLOSED — 11/11 plans, SCHM-01/02/03 `[x]`.**

---

## Task order, and why it is this order

The pre-review version of this plan ran the whole-phase gate FIRST and then edited rustdoc, README,
book text, provenance and planning state — files absent from its own header — with no gate rerun; and
it flipped requirement and roadmap completion markers BEFORE the human checkpoint, so a rejected
sign-off would have left the repository recording a complete phase. The cross-AI review found both.

This version sweeps first (Task 1), gates the swept tree second (Task 2), and applies completion
markers only after approval (Task 3). **`git diff --stat 2955d28e..HEAD -- .planning/ROADMAP.md
.planning/STATE.md` was EMPTY** — verified after both commits, verified again by the orchestrator
immediately before the checkpoint was answered, and *re-verified as the first act of the continuation
agent that applied the markers*. The ordering held end to end: while the decision was open, the
repository recorded an OPEN phase. The markers in § *Task 3* below are the first bytes either file
received in this plan.

---

## Task 1 — Stale-doc sweep and the ledger (commit `ab49c132`)

### (a) Contract bindings now assert what actually shipped

| Action | Result |
|---|---|
| `status: planned` → `implemented` | **12 flipped**; `grep -c 'status: planned' contracts/binding.yaml` → **0** |
| Signatures diffed against shipped source | **14 checked, 11 byte-identical, 3 divergent** |
| Missing binding added | `compile_for_era` — the fourteenth |
| `binary(phase115_contract_bindings)` | **5 tests, 5 passed** |

**The three signature divergences, all the same harmless kind** — the recorded signature elided a
path the source writes in full:

| Function | 115-11 recorded | Shipped |
|---|---|---|
| `cached_validator` | `Result<Arc<…>, Arc<str>>` | `Result<std::sync::Arc<…>, std::sync::Arc<str>>` |
| `project_caching_hints` | `&mut Value`, `Option<Era>` | `&mut serde_json::Value`, `Option<crate::types::protocol::Era>` |
| `inject_v2_result_envelope` | `Option<&ProtocolContext>` | `Option<&crate::types::protocol::ProtocolContext>` |

Same types in every case. **No plan under-delivered.** Each binding was updated to the shipped text
with an inline `115-10 SIGNATURE CORRECTION:` note rather than silently rewritten. The
`project_caching_hints` spelling has a reason worth keeping: `src/types/caching.rs` is deliberately
cfg-free so the wasm32-only dispatcher can call it, and the inline path avoids a `use` that would
have to survive both cfg worlds.

### BLOCKING DEFECT FOUND AND FIXED — the anti-vacuity assertion inverted on success

`phase115_contract_bindings_planned_entries_are_scoped_to_phase_115` carried
`assert!(planned > 0, …)`. That predicate held only **while the Phase 115 implementation plans were
unlanded** and went **false at exactly the moment the section reached its intended end state**.
Flipping the twelve entries turned a green test red for the right reason and the wrong assertion.

Fixed under deviation **Rule 1**: the guard now asserts **at least 13 Phase 115 bindings parse** —
what "the section is present and the parser works" actually means — and its failure message states
that `planned == 0` is expected, so nobody restores a `planned` entry to satisfy it. The module doc
was corrected in the same edit. `tests/phase115_contract_bindings.rs` was **outside this plan's
declared `files_modified`**; see § Deviations.

### (b) Stale-doc sweep

**Statements found FALSE and corrected:**

1. **`ResourceHandler` reach — three copies of a false claim in PRODUCTION rustdoc.** 115-05 wrote
   *"unlike the three resource-side results, which a `ResourceHandler` returns"* into
   `src/types/tools.rs`, `src/types/prompts.rs` and `src/types/protocol/mod.rs`. **Measured
   2026-08-01: `ResourceHandler` (`src/server/mod.rs:382`) declares exactly two methods — `read`
   and `list`. There is no templates method.** Both native dispatchers answer
   `resources/templates/list` from a hardcoded empty result
   (`src/server/core.rs:1013`, `src/server/mod.rs:2512`).

   | Result | Handler-settable? |
   |---|---|
   | `ListResourcesResult` | **yes** — `ResourceHandler::list` |
   | `ReadResourceResult` | **yes** — `ResourceHandler::read` |
   | `ListResourceTemplatesResult` | **no** — builders exist, no dispatcher path reaches them |
   | `ListToolsResult` / `ListPromptsResult` / `ServerDiscoverResult` | no — dispatcher-built |

   All three copies corrected to name the two reachable types, and
   `ListResourceTemplatesResult::with_ttl_ms` gained an explicit *"⚠ Not reachable through either
   native dispatcher"* section. The builders are **kept** — the type is `pub` and constructible by a
   custom transport, a proxy or a test.

2. **`schema/vendored/ext-tasks/PROVENANCE.md`, two statements 115-01 falsified.**
   - *"The total is 56,324 bytes"* meant the whole of `schema/`. Rescoped to the two files in that
     directory, with a dated amendment noting `schema/` now holds ~336,000 bytes.
   - *"…and none in the core `modelcontextprotocol/modelcontextprotocol` repository either"* is
     **no longer true** — 115-01 vendored the core schema from a **versioned** `schema/2026-07-28/`
     directory. **This is a distinction with consequences:** it is half of the D-18 hold's trigger.
     The amendment records that the **core half is now satisfied**, the **`ext-tasks` half is not**
     (still `draft/` only, 0 tags, 0 releases), so the hold **stays engaged** and TASK-01…06 stay
     `[~]`.

3. **`115-RESEARCH.md` § Finding 1 / Pattern 2** — `dependencies` is NOT the era-divergence case on
   `jsonschema` 0.49.2; **`contentEncoding`** is. The wrong example shipped into two plans before it
   was caught. Struck, replaced with the measured version in both directions (`contentEncoding`: v2
   more permissive; `$ref` siblings: v2 stricter), plus an instruction to re-measure any example
   copied from that section.

**Hits reviewed and DELIBERATELY LEFT — they are still true:**

| Grep | Reviewed | Verdict |
|---|---|---|
| `auto-detect\|autodetect` in `src/`, `pmcp-book/src/`, `README.md` | 20 hits | All correct. `output_validation.rs`'s module doc is properly era-scoped (v1 auto-detects, v2 pins) — 115-03's amendment landed. `protocol/version.rs:48`, `client/mod.rs:4792`, `builder.rs:94/1275`, `dns_rebinding.rs:22` are unrelated senses. `ch03-first-client.md` is about transport detection. |
| `restricted to.*object\|object-shaped\|must be an object` | 8 hits | `tools.rs:639` correctly quotes v1's spec text as HISTORY (*"The 2025-11-25 (v1) schema text was narrower"*). `mrtr.rs`, `mcp_apps.rs`, `streamable_http_server.rs` are about `inputResponses`/`requiredCapabilities`, not `structuredContent`. |
| `ext-tasks` in `schema/` | 15 hits | Only the two above were false; the core record already restates rather than copies the ext-tasks consumer bullet. |
| `structuredContent\|outputSchema` in `README.md`, `pmcp-book/src/` | 25 hits | `ch05-tools.md:308` (*"validates each bridged value … logs a warning"*) is still true; it does not mention the era-branched dialect, but that is **Phase 119's DOCS-05/06**, not a false statement. |

### (c) The ledger

`.planning/phases/115-…/deferred-items.md` — rewritten from a plan-scoped table into the Phase 114
one-heading-per-item form.

| Criterion | Result |
|---|---|
| `grep -c "^## D-115-"` | **36** (required ≥ 16) |
| Line count | **776** (required ≥ 120) |
| Every heading owned or explicitly unowned | **yes** — 0 headings missing `Owner:`/`**unowned**` |
| Duplicate-ID check | **empty** — no duplicates |
| Names `test-property`, `wasm-build`, `traits.rs`, `wasm_server_tests.rs`, `extract_request_meta_value`, `process_response_with_context` | **all present** |
| Contains `D-114-R`, `D-114-S`, `D-113-U` | **all present**, plus `D-114-U` |

**Full ID list.** `A` contract location · `B` 21 uncontracted equations · `C` `ErrorCode constants`
is prose · `D` pmat CB-1208 cache-driven count · `E` pmat CB-951 false positive · `F` signature
drift caught by review not gate · `G` SCHM booked on contract-only evidence (**closed**) · `H`
twelve bindings left `planned` (**closed**) · `I` the wrong era-divergence example (**closed**) ·
`J` `compile_for_era` had no binding (**closed**) · `K` pmat's `jq` path · `L` `structuredContent:
null` typed re-read · `M` hardcoded port 9005 · `N` wasm strip proven only natively · `O`
`traits.rs` / `wasm_server_tests.rs` orphans · `P` only two of six handler-settable ·
`Q` `extract_request_meta_value` era bound · `R` `process_response_with_context` ordering ·
`S` D-10 tripwire declined at types layer · `T` `make` stdout corrupted when redirected ·
`U` `make test-fuzz` fail-open twice over · `V` `make test-property` selects zero ·
`W` `make test-examples` never runs · `X` `make wasm-build` skips `validation` ·
`Y` `nextest test(/stem/)` selects zero · `Z` fuzz corpus gitignored + stale `fuzz/Cargo.lock` ·
`0` disk exhaustion · `1` grep-shaped criteria · `2` requirement-text deviations ·
`3` warn-only `outputSchema` mismatch · `4` gitignored `Cargo.lock` · `5` `u64` `ttlMs` unbounded ·
`6` trimmed builders and `CacheScope::Display` · `7` `pmcp-agent` `validator_for` unpinned ·
`8` dead toolkit `jsonschema` dep + no-op `unused-deps` · `9` the inverting anti-vacuity assertion.

Plus `D-114-R` (**CLOSED by 115-01**, with a redirect for readers arriving from Phase 114's ledger),
`D-114-S` (**still unowned**), `D-113-U` and `D-114-U`.

---

## Task 2 — The gate over the swept tree (commit `9c72ff88`)

**Phase base: `acd23b64`** — the last commit before `115-01` touched the tree. Materialized with
`git archive acd23b64 | tar -x` into a scratch directory: **no worktree, no stash, no checkout of the
live tree.**

### Gate results

| Command | Exit | Detail |
|---|---|---|
| `make quality-gate` | **0** | 8,846 lines, **309** `test result:` lines, **5045 passed / 0 failed / 81 ignored**, **0** non-`ok.` lines, **0** truncation markers, **0** keychain `-36` flakes |
| `make wasm-build` | **0** | — |
| `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` | **0** | **SCHM-01's real wasm-clean evidence** — `make wasm-build` never compiles `jsonschema` |
| `make lint` | **0** | run separately before the Task 1 commit |
| `pmat quality-gate --fail-on-violation --checks complexity` | **0** | *Quality Gate: PASSED, Total violations: 0* |
| `make test-feature-flags` | **2** | **inherited red — see below** |

### Deltas against the phase base

| Measure | Base `acd23b64` | HEAD | Delta |
|---|---|---|---|
| `pmat analyze complexity --max-cognitive 25`, violations under `src/` | **0** | **0** | **0** |
| …total violations (all paths) | 5 | 13 | +8, **all in `tests/`** — 7 in 115-08's tripwire file, 1 in 115-11's binding parser |
| `cargo semver-checks check-release --baseline-rev acd23b64` | — | **223 checks: 223 pass, 30 skip** | *"no semver update required"* — **zero breaking changes** |
| `cargo public-api --simplified` item count | **21,689** | **21,877** | **+188, ZERO REMOVED** |
| `make test-feature-flags` `^error` lines | **62** | **62** | **0** |

**Public API: additions only, and exactly the expected ones.** `comm -23 base now` is **empty** — no
item was removed or changed. The additions are `pub mod pmcp::types::caching`, `CacheScope`
(enum + `Public`/`Private` + its derives), `DEFAULT_TTL_MS`, `CallToolResult::structured_value`, the
**six** `with_ttl_ms`/`with_cache_scope` builders on the three resource-side result types, the twelve
`ttl_ms`/`cache_scope` fields (six types × two, each visible under several re-export paths), and
`Hash` for `Era`. **`Cacheable`, `project_caching_hints` and `fuzz_support` appear ZERO times** —
confirming the first two stay `pub(crate)` and the third stays behind `fuzzing`, which is in neither
`default` nor `full`.

**⚠ `make test-feature-flags` exits 2, and the criterion "exits 0" was unsatisfiable when written.**
The target was **already red at the phase base**: `114-18-SUMMARY.md` records exit 2 with 62 `^error`
lines at Phase 114's HEAD, which is Phase 115's base. Measured at Phase 115's HEAD: exit 2, **62**
`^error` lines, with a per-file distribution **identical to 114-18's recorded HEAD row**:

| file | 114-18 HEAD (= 115 base) | 115 HEAD |
|---|---|---|
| `src/types/mrtr.rs` | 39 | **39** |
| `src/server/subscriptions.rs` | 7 | **7** |
| `src/server/task_dispatch.rs` | 6 | **6** |
| `src/server/core.rs` | 4 | **4** |
| `src/shared/sse_parser.rs` | 2 | **2** |
| `src/types/protocol/mod.rs`, `src/shared/protocol_helpers.rs`, `src/server/mod.rs` | 1 each | **1 each** |

**Phase 115's delta is ZERO.** The failing row is the same one D-114-E and D-114-U record:
`cargo clippy -p pmcp-tasks --no-default-features -- -D warnings`, with **zero errors in
`crates/pmcp-tasks/`** — every one is a `dead_code` lint in root `pmcp` under a minimal feature set.
This was **not fixed and not worked around**: the gate was not weakened, and 13 `#[cfg]`/`allow`
decisions across five files owned by other plans are not a bookkeeping-plan edit. Carried as
`D-114-U`.

*The base was NOT re-measured by rebuilding the scratch tree, deliberately:* `Cargo.lock` is
gitignored (ledger entry `4`), so a scratch checkout resolves a **different** dependency set and
would be a less faithful base than the recorded one. The exact per-file distribution match is the
corroboration.

### Every Phase 115 suite re-run BY NAME

`make quality-gate`'s `validate-always` chain is fail-open on all three of its ALWAYS targets, and a
zero-selection can hide inside `test-all`. So the phase's evidence was re-run directly.

`cargo nextest run --features full -E 'binary(…) + …'` → **95 tests run, 95 passed (1 leaky), 0
skipped**:

| Binary | Tests | Owning plan's criterion | Verdict |
|---|---|---|---|
| `vendored_schema_provenance` | **6** | 115-01 | ✓ |
| `v2_core_schema_facts` | **8** | 115-01 | ✓ |
| `v1_lists_golden` | **7** | 115-02 (5 goldens + a leak guard proven to fire) | ✓ (grew by later plans) |
| `structured_tool_output` | **20** | 115-04 | ✓ |
| `v2_caching_hints` | **19** | 115-07 — six methods × two eras × both native dispatchers | ✓ |
| `v2_schema_tripwires` | **13** | 115-08 | ✓ |
| `property_tests` | **17** | 115-09 | ✓ |
| `phase115_contract_bindings` | **5** | 115-11 | ✓ |

**No count is below its owning plan's acceptance criterion.** One test is reported *leaky* —
`v2_schema_tripwires_the_projection_scan_is_not_vacuous` — it **passed**; nextest flags a lingering
handle, not a failure.

`cargo nextest run --lib --features full -E 'test(/output_validation::/) + test(/types::caching/) +
test(/inject_v2_result_envelope/)'` → **56 tests run, 56 passed** (`output_validation::` 15,
`types::caching` 15, `inject_v2_result_envelope` 26).

`cargo nextest run --features "full fuzzing" -E 'binary(property_tests)'` → **18 tests run, 18
passed** — one more than without the feature, i.e. the `fuzzing`-gated property test is reached.

### Fuzz — replayed and re-run, with `cargo +nightly`

**`make test-fuzz` was NOT used as evidence.** It is fail-open twice over: it wraps every target in
`|| echo "… completed"`, and it invokes the plain `cargo fuzz run`, which passes `-Zsanitizer=address`
that stable rustc refuses — so on this checkout's default toolchain every one of the 20 targets fails
to **build** and the target still prints success (ledger entry `U`).

| Command | Result |
|---|---|
| `cargo +nightly fuzz run fuzz_schema_draft_pin -- -runs=0 corpus/fuzz_schema_draft_pin` | **exit 0** — 12 committed seeds loaded, `#28 DONE cov: 8393 ft: 13716` |
| `cargo +nightly fuzz run fuzz_schema_draft_pin -- -max_total_time=60` | **exit 0** — **660,271 runs in 61 s** |
| `ls fuzz/artifacts/fuzz_schema_draft_pin/` | **EMPTY** — checked by `ls`, not by `make test-fuzz` |

`git status --short` after the fuzz session shows **no new tracked or untracked file** in `fuzz/` —
115-09's narrow `.gitignore` held.

### The example — RUN, not merely built

`make test-examples` builds without executing and reports a build failure as *"skipped"* (ledger
entry `W`), so the example was run directly:

`timeout 300s cargo run --example s52_v2_caching_hints --features full` → **exit 0**, ending
`all four demonstrations asserted — exiting 0`. Transcribed stdout:

```
1. resources/list — the HANDLER set the posture (v2)
    ttlMs = 300000   cacheScope = "public"
    raw   = {"resources":[…],"ttlMs":300000,"cacheScope":"public","resultType":"complete",…}

2. resources/read and tools/list — the SDK DEFAULT (v2)
    resources/read  ttlMs = 0  cacheScope = "private"
    tools/list      ttlMs = 0  cacheScope = "private"

3. the SAME server answering a 2025-11-25 client — NEITHER key
    resources/list (v1)  ttlMs = <absent>  cacheScope = <absent>
    tools/list     (v1)  ttlMs = <absent>  cacheScope = <absent>
    -> the handler SET a hint on resources/list, and the v1 projection STRIPPED it.

4. tools/call — NON-OBJECT structuredContent (v2)
    raw = {"content":[{"type":"text","text":"42"}],"isError":false,"structuredContent":42,…}
    CallToolResult::structured_value(json!(null)) => {…,"structuredContent":null}
```

### Gate files untouched

`git diff --stat acd23b64..HEAD -- Makefile .github/workflows/ci.yml deny.toml` → **empty**. No gate
was modified anywhere in Phase 115.

### The bookings

`grep -c '^- \[x\] \*\*SCHM-0' .planning/REQUIREMENTS.md` → **3**; `[~]` count → **0**. The SCHM
block is 143 lines and contains `271ecc9accafdd9b83a3c869fa67c22953b2af80` (×3), `0.49` (×4),
`DiscoverResult` (×2), `wasm,validation` (×1) and `core-2026-07-28` (×3). Each booking states, inside
itself: the measured evidence by binary name and count; the published-evidence citation; an explicit
sentence that this is `[x]` and **not** `[~]` because D-15's contingency did not fire; the
deviations; and the judgement the booking makes rather than absorbs.

---

## Task 3 — Sign-off, ANSWERED, and the completion markers (commit `496da96b`)

### The sign-off

- **Approved by:** **Guy Ernest (owner)** — answered the `checkpoint:human-verify gate="blocking"`
  with the resume signal `approved`
- **Date:** **2026-08-01**
- **Corrections requested:** **NONE**

**The checkpoint was returned UNANSWERED rather than self-approved**, and a fresh continuation agent
applied the markers only after the answer. Phase 114's own record shows that this is the correct
handling; the same posture was taken here and it held. `workflow.auto_advance` is `false` on this
project, so no auto-approval path was even available — but note that the plan additionally marks this
gate `blocking`, and GSD's auto-mode rules would still have STOPPED for a package-legitimacy gate
only. **A future phase running under auto-advance must not let a `gate="blocking"` sign-off be
auto-selected: the value of this checkpoint is precisely that an agent cannot answer it.**

The owner accepted, explicitly, all three items the executing agent had refused to self-approve:

| # | Item accepted | Where it is recorded |
|---|---|---|
| 1 | The three requirement-text deviations: `jsonschema` at **0.49** (not the literal `0.48`), with an exact `=0.49.2` pin **declined** on library-semver grounds; **six** result types carry hints, not five, because `DiscoverResult extends CacheableResult`; contracts live **in-repo** at `contracts/`, not at CLAUDE.md's `../provable-contracts/` path | inside the SCHM bookings (`.planning/REQUIREMENTS.md`), ledger `A` and `2`, and the ROADMAP deviation notes |
| 2 | The `deferred-items.md` ledger **as it stands**, including every **unowned** item — the unpinned `pmcp-agent` `validator_for` (`7`), no builder-level override, the wasm strip being compile- and tripwire-checked but never behaviourally EXECUTED (`N`), warn-only `outputSchema` mismatch (`3`), middleware still able to mutate the projected keys (`R`), and the three fail-open ALWAYS `make` targets (`U`/`V`/`W`) | the 36-entry ledger |
| 3 | Booking the three SCHM requirements **`[x]` rather than `[~]`** — D-15's contingency did not fire and Phase 114's publication hold is not inherited | `.planning/REQUIREMENTS.md` lines 146 / 206 / 241 |

The owner-facing checks in `<how-to-verify>` were independently re-run before the answer:
`make quality-gate` **exit 0** (0 truncation markers, 0 failure lines);
`cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"`
**exit 0**; `cargo run --example s52_v2_caching_hints --features full` **exit 0** with all four
demonstrations asserted — v2 responses carrying `ttlMs`/`cacheScope`, the v1 response carrying
neither, and **the v1 path observed ACTIVELY STRIPPING a hint the handler had set** rather than
merely failing to add one, which is the distinction that makes demonstration 3 evidence.

### The completion markers, applied AFTER the answer

**`git diff --stat 2955d28e..HEAD -- .planning/ROADMAP.md .planning/STATE.md` was re-verified EMPTY
as the first act of this task** — the ordering invariant T-115-43 exists to protect held from
plan start to owner answer.

`.planning/ROADMAP.md`:

| Marker | Change |
|---|---|
| `115-10-PLAN.md` | `[ ]` → `[x]`, with the sign-off outcome appended. The other ten were already ticked — **eleven** total (ten original + `115-11` from the `--reviews` replan) |
| Phase 115 entry (phase list) | `[ ]` → `[x]`, and its one-line description **corrected to what shipped**: it still said *"jsonschema 0.48"* and *"the five list/read results"*, both falsified by this phase |
| Milestone progress table | `10/11 · In Progress` → **`11/11 · Complete · 2026-08-01`** |
| Planning-deviation note | **Verified, not assumed.** Both existing paragraphs hold as written, and the contract-location deviation was already present in the replan paragraph. A third **Execution deviation** paragraph was added for the four divergences the phase acquired *during* execution: fourteen bindings not thirteen; the declined `=0.49.2` pin plus the gitignored `Cargo.lock`; the `ResourceHandler` reach correction; and `make test-feature-flags`'s unsatisfiable criterion with its measured ZERO delta |
| New note under the progress table | States that Phase 115's `Complete` counts plans **and** requirements — unlike the Phase 113 note directly above it — and, in the same breath, that it closes **neither** `D-114-S` nor `D-113-U` |

`.planning/STATE.md`:

| Marker | Change |
|---|---|
| frontmatter | `stopped_at` → `Completed 115-10-PLAN.md — Phase 115 CLOSED by owner sign-off`; `completed_plans` 349 → **350**; `completed_phases` 60 → **61**; `percent` 83 → **85** (the file's own convention is phases/total, measured across four historical revisions, not the SDK's plan-based percent) |
| `## Current Position` | Phase 115 **COMPLETE in both senses**; the 114-18 prose that still occupied `Next action:` was demoted to `Prior-phase context (Phase 114, 114-18)` rather than discarded |
| `## Session Continuity` | `Stopped at:` → this plan; `Next:` → **Phase 116**, carrying forward `D-114-S`, `D-113-U` and `UNAS-01` |
| Decisions / Blockers | six decisions and one blocker appended (the blocker re-asserts that `D-114-S` is still unowned) |
| Performance Metrics | `Phase 115 P10 · 3h10m · 3 tasks · 12 files` |

**A stale-record correction made while applying the counters, worth its own line.** `114-18` wrote a
note into `## Session Continuity` reading *"the SDK RECOMPUTES `completed_phases` and reports 60
while this file correctly STORES 59 … Do not edit STATE.md to match the derived view."* **Measured
here: the stored value moved 59 → 60 in `1d1493b8` — the very next STATE-touching commit — via an SDK
helper's recompute.** The forbidden edit was made by the tool, silently, and eight Phase-115 plans
then incremented `completed_plans` off that base. It was **not** reverted (that would falsify eight
landed records); the note was **rewritten to say what the number now means**: `61` = 60 (which
already counts Phase 114, still `[~]` and HELD) + Phase 115. The counter is a plan-shipped tally, not
a requirements tally, and **Phase 114's `[~]` marker remains the authoritative statement of its
status.**

**Two independent corroborations of the markers, both run after the hand edits and both writing
zero bytes:**

| Check | Result |
|---|---|
| `gsd-sdk roadmap update-plan-progress 115` | `plan_count: 11, summary_count: 11, status: "Complete", complete: true` — **file byte-identical**, so the SDK derived the same row from disk that was written by hand |
| `gsd-sdk requirements mark-complete SCHM-01 SCHM-02 SCHM-03` | `updated: false, already_complete: [SCHM-01, SCHM-02, SCHM-03]` — **file byte-identical**; Task 2's bookings already satisfied it |
| `gsd-sdk state update-progress` | reports 350/350; **file byte-identical** (its percent is plan-based, the file's is phase-based) |

### What this approval does NOT do

Recorded here because *"the owner approved"* is exactly when an unrelated hold gets released by
accident — `114-18`'s own summary makes the same warning about its own sign-off:

- **Phase 114's D-18 hold stays ENGAGED.** TASK-01..06 stay `[~]`, `114-SPEC-RECHECK.md`
  `## Verdict` stays `PENDING`, and Phase 114 stays `[~]` in the ROADMAP. `115-01` closed only the
  **core half** of D-18's two-repository trigger (and `D-114-R` with it) by vendoring the published
  core schema; `modelcontextprotocol/ext-tasks` still carries `draft/` only.
- **`D-114-S` is still unowned** — nothing watches `ext-tasks` for publication.
- **`D-113-U` still needs an owner** before this branch merges.
- **`D-114-U`** — the `make test-feature-flags` redness — is inherited, not closed; Phase 115's delta
  is zero.
- **`UNAS-01`** (SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}`) is still an unassigned v2.5
  requirement with no phase.

---

## Deviations from plan

### 1. `[Rule 1 — Bug] tests/phase115_contract_bindings.rs edited, outside `files_modified``

- **Found during:** Task 1(a).
- **Issue:** `assert!(planned > 0, …)` inverted the moment the twelve bindings were flipped —
  the assertion was true only while the work it guarded was incomplete. Flipping made a green
  test red.
- **Fix:** the guard now asserts ≥13 Phase 115 bindings parse, with a failure message stating that
  `planned == 0` is the expected end state. Module doc corrected in the same edit.
- **Files:** `tests/phase115_contract_bindings.rs`. **Commit:** `ab49c132`.
- **Why this was not a plan violation to escalate:** the plan's own acceptance criteria require
  *both* `grep -c 'status: planned'` → 0 *and* `binary(phase115_contract_bindings)` exit 0. Those two
  were unsatisfiable simultaneously without this edit.

### 2. `[Rule 2 — Missing critical correctness] Three copies of a FALSE production rustdoc claim`

Not in the plan's enumerated sweep list, but squarely inside Task 1(b)'s mandate. `ResourceHandler`
declares only `read` and `list`; the rustdoc claimed three resource-side results reach it.
Corrected in `src/types/tools.rs`, `src/types/prompts.rs`, `src/types/protocol/mod.rs` and
`src/types/resources.rs`. **Commit:** `ab49c132`.

### 3. `115-RESEARCH.md` edited, outside `files_modified`

Ledger entries `I`/`K` explicitly name 115-10 as the owner of this correction, and Task 1(b)'s
mandate is *"find and fix every in-repo statement this phase falsified"*. The `dependencies` example
is such a statement, and it had already propagated into two plans.

### 4. Acceptance criterion `make test-feature-flags` exits 0 — **NOT MET, and unsatisfiable as
written**

Red at the phase base with the identical error count and per-file distribution. Phase 115's delta is
**zero**. Reported rather than absorbed, and the gate was **not** weakened. This is the second
consecutive phase to carry an unsatisfiable version of this criterion — `114-18-SUMMARY.md` § 6
records the first.

### 5. The duplicate-ID criterion constrains the ledger's own prose

`grep -n 'D-115-' … | cut -c1 | sort | uniq -d` must return nothing, which is only meaningful if
every line containing the literal is a heading. The crosswalk therefore spells the old plan-scoped
IDs **without** their leading `D-`, and two explanatory sentences were reworded. Recorded as ledger
entry `1`, the same class as 115-05's and 115-09's grep-shaped-criterion collisions.

### 6. Instrument finding — `pmat analyze complexity … 2>&1` corrupts its own JSON

pmat writes progress lines to **stderr**. Merging streams (`2>&1`) makes the output unparseable by
`jq`. Separate the streams. This compounds ledger entry `K` (the documented `jq` path
`.violations[]` does not exist; the working path is `.summary.violations[]`, and the field is
`file`, not `path`).


### 7. `[Rule 2 — Missing critical correctness] A stale STATE.md note rewritten, not silently obeyed`

- **Found during:** Task 3, applying the counters.
- **Issue:** `114-18` recorded *"the SDK reports 60 while this file correctly STORES 59 … Do not edit
  STATE.md to match the derived view."* The stored value was **60** on arrival. Measured across the
  intervening commits: it moved 59 → 60 in `1d1493b8`, the very next STATE-touching commit, via an
  SDK helper's recompute — **the tool made the edit the note forbade, silently.**
- **Fix:** the counter was **not** reverted (eight Phase-115 plans have since incremented
  `completed_plans` off that base, and rewriting it would falsify eight landed records). The note was
  rewritten to state what the number now means and to re-assert, in the same paragraph, that Phase
  114's `[~]` marker — not the counter — is the authoritative statement of its status.
- **Why this is Rule 2 and not a plan violation:** a record that instructs a reader to trust a value
  the file no longer holds is worse than no record. Task 3's mandate covers exactly these lines.
- **Files:** `.planning/STATE.md`. **Commit:** `496da96b`.

### 8. Instrument finding — `gsd-sdk state add-decision "text"` fails SILENTLY with exit 0

The executor workflow documents positional arguments (`state add-decision "…"`,
`state record-metric "$PHASE" "$PLAN" …`). **This build requires flags** (`--summary`, `--text`,
`--phase/--plan/--duration/--tasks/--files`) and, given the positional form, prints
`{"error": "summary required"}` **and exits 0**. Six decisions and one blocker were lost on the first
attempt and only noticed because the follow-up `grep` for them came back empty — a caller using
`set -e` with `>/dev/null` would have recorded nothing and reported success. **Verify a state write
by grepping the file, never by the exit code.** `roadmap update-plan-progress` is positional
(`--phase 115` errors with *"Phase --phase not found"*), so the two conventions coexist.

---

## Self-Check

**Files claimed created/modified — all present:**

```
FOUND: .planning/phases/115-…/deferred-items.md          (776 lines, 36 ## D-115- headings)
FOUND: .planning/phases/115-…/115-10-SUMMARY.md
FOUND: contracts/binding.yaml                            (0 `status: planned`, 14 Phase 115 bindings)
FOUND: tests/phase115_contract_bindings.rs               (5 tests, all passing)
FOUND: src/types/tools.rs, prompts.rs, protocol/mod.rs, resources.rs
FOUND: schema/vendored/ext-tasks/PROVENANCE.md           (both amendments present)
FOUND: .planning/phases/115-…/115-RESEARCH.md            (contentEncoding correction present)
FOUND: .planning/REQUIREMENTS.md                         (3 × `[x] **SCHM-0`, 0 × `[~]`)
```

**Commits claimed — all present:**

```
FOUND: ab49c132  docs(115-10): sweep stale docs and write the phase deferred-items ledger
FOUND: 9c72ff88  docs(115-10): book SCHM-01/02/03 on measured, re-derivable evidence
FOUND: 496da96b  docs(115-10): apply Phase 115 completion markers after owner sign-off
```

**Task 3's markers — re-measured on disk after the commit, not asserted:**

```
FOUND: .planning/ROADMAP.md   grep -c '^- \[x\] 115-'  -> 11   (and '^- \[ \] 115-' -> 0)
FOUND: .planning/ROADMAP.md   "| 115. JSON Schema 2020-12 + Caching Hints | 11/11 | Complete   | 2026-08-01 |"
FOUND: .planning/ROADMAP.md   line 2222 "- [x] **Phase 115: …**"
FOUND: .planning/STATE.md     completed_plans: 350 · completed_phases: 61 · percent: 85
FOUND: .planning/STATE.md     "Stopped at: Completed 115-10-PLAN.md — Phase 115 CLOSED by owner sign-off"
FOUND: .planning/STATE.md     "| Phase 115 P10 | 3h10m | 3 tasks | 12 files |"
CLEAN: git diff --diff-filter=D HEAD~1 HEAD -> empty (no file deleted by the marker commit)
```

## Self-Check: PASSED
