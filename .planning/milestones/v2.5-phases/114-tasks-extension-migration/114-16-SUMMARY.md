---
phase: 114-tasks-extension-migration
plan: 16
subsystem: api
tags: [tasks, mcp-2026-07-28, tripwire, source-scan, era-guards, allowlist, provenance, negative-controls]

requires:
  - phase: 114-08
    provides: "the two retirement predicates (tasks_list_serves_on_era / tasks_result_serves_on_era) this file names as guards"
  - phase: 114-09
    provides: "resolve_owner and the ordered refusal chain the endpoint dispatcher's entry names"
  - phase: 114-11
    provides: "the v2 -32602 store-not-found mapping the -32603 ban pins positionally"
  - phase: 114-12
    provides: "create_gate as ONE expression, so the create trigger has a single site to name"
  - phase: 114-14
    provides: "route_tasks_update's five gates and the internal-request route, so the update entry has guards to name"
  - phase: 114-01
    provides: "schema/vendored/ext-tasks/schema.ts — the artifact the status set and the provenance lock are read against"
provides:
  - "tests/v2_tasks_tripwires.rs — 25 source tripwires with justified, self-rotting allowlists over the tasks surface"
  - "three SEPARATE tests for the three rot conditions, so a deleted guard / an unlisted route / a stale entry fail disjoint sets"
  - "two independent route-discovery axes: by FUNCTION name and by WIRE name"
  - "a per-function, count-pinned enumeration of every -32603 in the tasks dispatch"
  - "a SOURCE-level TaskStatus set-equality lock read from the vendored schema at runtime"
  - "a two-directional provenance lock over the three wire values this phase introduced"
  - "the 2026-07-29 amendment: -32021 and -32602 pinned by NAME, -32002 pinned absent"
affects: [114-17, 114-18, 114-20, 118]

tech-stack:
  added: []
  patterns:
    - "A guard is checked in the FUNCTION it must run in, never merely in the file — one file can contain eight copies of the same predicate"
    - "Two independent discovery axes (function name and wire name) so evading one still fails the other"
    - "An allowlist entry carries the MEASURED hit COUNT, so a second emission inside an allowlisted function fails too"
    - "Derive a wire-string set from the DECLARATION, never from a test that has to name the variants — a sixth variant must be visible"
    - "A two-directional lock: an accepted weakness that silently becomes a strength fails and asks to be promoted"
    - "Split the rot conditions into separate TESTS when the masking check shows they share an assert line"

key-files:
  created:
    - tests/v2_tasks_tripwires.rs
  modified: []

key-decisions:
  - "Each GuardRef names the SITE the guard must live in, because route_tasks_list's retirement gate fires in retired_method one frame above the route and is_v1_task_era appears in eight places in the file"
  - "The three rot conditions became three TESTS after the masking check FIRED: NC-1/NC-2/NC-4 all failed the identical assert line, separable only by message text"
  - "TaskStatus wire strings are parsed out of the enum declaration (rename_all asserted, per-variant renames honoured) rather than serialized from five named variants, so a sixth variant is visible"
  - "The provenance lock is two-directional: a ProseOnly entry that GAINS a walkable artifact reference fails and asks to be promoted"
  - "The plan's `include_str!`-at-runtime instruction is self-contradictory; read_to_string was used so a re-vendoring moves the test without a rebuild"
  - "A NEW exclusion was required: test-only module FILES are discovered from their `#[cfg(test)] mod` declarations, because task_dispatch_tests.rs carries no marker of its own and a numeric -32002 scan hits it"

patterns-established:
  - "A control that fails a test on assertion N proves only assertion N — so pair a count control with a spelling control (NC-5 / NC-5b)"
  - "Prove two discovery axes independent by evading each one separately (NC-2 evades the wire scan, NC-3 evades the function scan)"

requirements-completed: []

duration: 170min
completed: 2026-07-31
---

# Phase 114 Plan 16: Tasks Source Tripwires Summary

**Every `tasks/*` route's era guard is now named against the function it must run in, no v2-reachable
path can map a store `NotFound` onto `-32603` without failing by line number, the five `TaskStatus`
strings cannot drift from the vendored schema, and three wire values cannot lose their attribution —
with eight negative controls, all failing sets disjoint, and zero production bytes changed.**

## Performance

- **Duration:** ~170 min
- **Tasks:** 2 of 2 (plus one measurement-driven follow-up commit)
- **Files created:** 1 · **Files modified:** 0 (production diff is byte-EMPTY)
- **Commits:** 3

## Accomplishments

### `tests/v2_tasks_tripwires.rs` — 25 tests, 2083 lines, zero production bytes

| # | test | rot condition it fails on |
|---|------|---------------------------|
| 1 | `every_tasks_route_in_the_source_is_allowlisted` | a route function the allowlist has never heard of |
| 2 | `every_allowlisted_route_still_exists` | a stale entry for a route since removed |
| 3 | `every_allowlisted_routes_era_guard_is_present_in_its_named_site` | a guard deleted from the function it must run in |
| 4 | `the_route_scan_is_not_vacuous_and_every_entry_is_justified` | a scanner matching nothing; a copy-pasted reason |
| 5 | `every_v2_tasks_wire_method_maps_to_an_allowlisted_route` | a new `tasks/*` method under any function name |
| 6 | `the_minus_32603_population_in_the_tasks_dispatch_is_enumerated` | a new or moved `-32603`, by function and by COUNT |
| 7 | `the_v2_store_not_found_arm_still_maps_to_minus_32602` | the not-found ARM flipping while the count holds |
| 8 | `the_task_status_wire_strings_are_set_equal_to_the_vendored_schema` | a status string drifting from the vendored union |
| 9 | `the_task_status_mappings_carry_no_wildcard_arm` | a `_ =>` absorbing an unknown status |
| 10 | `every_wire_value_constant_this_phase_introduced_carries_an_attribution` | a wire value losing — or silently gaining — its provenance |
| 11 | `the_published_core_codes_this_phase_reused_are_pinned_by_name` | `-32021`/`-32602` renumbering, or the refusal re-pointing |
| 12 | `minus_32002_has_no_v2_reachable_emission_site` | a new `-32002` site, by NAME or by bare number |
| 13 | `the_test_only_exclusions_are_load_bearing` | either exclusion silently emptying or over-reaching |
| 14–25 | `scanner::*` | the scanner itself, twelve unit tests |

### The site is the point, and it was measured

Every `GuardRef` names a predicate **and the function that predicate must appear in**. That is not
decoration:

* `route_tasks_list` contains **no era guard at all**. Its retirement gate fires in `retired_method`,
  one frame above the route, which is exactly what makes enumeration impossible rather than merely
  refused — no owner binding, no store `list`, no router call.
* `is_v1_task_era` appears **eight times** in `src/server/task_dispatch.rs`. A check that asked
  "does this FILE still contain the token" would stay green after any single call site was deleted.

Negative control NC-1 proves it: replacing `route_tasks_cancel`'s `is_v1_task_era(era)` with an
inline `!matches!(era, Some(Era::V2))` — a second era definition, which is the rot the phase's own
`is_v1_task_era` rustdoc forbids — leaves the token in seven other places and still fails, naming
`route_tasks_cancel`.

### Two discovery axes, proven independent

| axis | finds a new route by | evaded by | caught by |
|------|----------------------|-----------|-----------|
| A | its FUNCTION name (`route_`/`handle_`/`serve_`/`dispatch_` + `tasks_`/`task_`) | naming it `frobnicate` | axis B |
| B | its WIRE name (every `"tasks/…"` literal in the dispatch and the routing table) | carrying no literal | axis A |

NC-2 adds `route_tasks_pause` with no wire literal and fails **only** axis A. NC-3 adds
`fn frobnicate` spelling `"tasks/pause"` and fails **only** axis B, printing both sets. Neither
control fails the other's test, which is what "independent" has to mean.

### The `-32603` ban is enumerated by function AND by count

`INTERNAL_ERROR_SITES` records **9 emissions across 6 functions**, measured against the tree:

| function | hits | lines | disposition |
|---|---|---|---|
| `store_error_response` | 2 | 686, 699 | `NotFoundRoutedElsewhere` (`is_v1_task_era`, in-body) |
| `handle_tasks_result` | 2 | 1634, 1651 | `V1Only` (`tasks_result_serves_on_era` in `retired_method`) |
| `route_tasks_list` | 2 | 1865, 1877 | `V1Only` (`tasks_list_serves_on_era` in `retired_method`) |
| `route_tasks_get` | 1 | 1818 | `RouterLeg` — **recorded gap D-114-P** |
| `route_tasks_cancel` | 1 | 1933 | `RouterLeg` — **recorded gap D-114-P** |
| `deliver_tasks_update` | 1 | 2409 | `RouterLeg` — **recorded gap D-114-P** |

The COUNT is part of each entry because a second `-32603` added inside an already-allowlisted
function is the exact shape a regression takes, and a presence check cannot see it.

A second, independent assertion pins the arm the count cannot: in `store_error_response`, the first
error-code token following the first `TaskStoreError::NotFound` must be `INVALID_PARAMS`. NC-6 flips
that one arm and fails **both** tests, each with a line number:

```
no INVALID_PARAMS follows the NotFound arm in store_error_response (src/server/task_dispatch.rs:691)
COUNT CHANGED: `store_error_response` was recorded with 2 INTERNAL_ERROR emission(s) and now has 3
               at line(s) [686, 693, 699].
```

### A REAL FINDING, recorded not fixed: D-114-P

Three router fall-through legs answer **`-32603` for a `TaskRouter` not-found**, where the extension
makes `-32602` a **MUST** for `tasks/get` and a SHOULD for `tasks/cancel` and `tasks/update`. So a
**router-backed** v2 deployment is non-conformant on `tasks/get`.

This is a router-only gap: every STORE-backed path — `InMemoryTaskStore` and `GenericTaskStore`
alike, i.e. every backend in this repository — reaches `store_error_response` and is correct, which
is why 114-15's live-socket cross-caller probes all read `-32602`.

It was not fixed here because `TaskRouter::handle_tasks_*` returns `crate::error::Error` with no
not-found discriminant, so closing it means widening a legacy-experimental public trait — a semver
and design decision that a coverage-only plan may not take. It is booked as **D-114-P**, named in
all three entries' justifications, and the count is pinned so a fourth router leg fails.

### The status lock derives the set, it does not name it

Serializing `TaskStatus::Working` and its four siblings would require the test to NAME all five, and
a sixth variant would then be invisible to it — which is precisely the drift the lock exists to
catch. So the variants are parsed out of the **enum declaration**: `rename_all = "snake_case"` is
asserted (a different transform would silently make every derived string wrong), the snake-case
transform is unit tested, and per-variant `#[serde(rename = "…")]` is honoured. The schema side
reads `schema/vendored/ext-tasks/schema.ts` at runtime with `read_to_string`, and the two are
compared for **exact set equality** — a subset would pass while a status went unrepresented.

Two controls, because a control that fails on assertion N proves only assertion N:

* **NC-5** adds a sixth variant `Paused` (with the five arms needed to compile) and fails at the
  COUNT assertion, reporting `{"cancelled", "completed", "failed", "input_required", "paused", "working"}`.
* **NC-5b** renames one variant with `#[serde(rename = "cancelled_by_user")]`, keeping the count at
  five, and fails at the **SET EQUALITY** assertion with both sides printed. It also proves the
  per-variant rename handling is live rather than decorative.

**The TASK-04 conclusion is written into the test's rustdoc so a future reader does not undo it:**
F15 measured the v1 five-state enum NAME-IDENTICAL to the v2 one, so "maps deterministically" is
satisfied by exactly two locks — set equality here, the running server in 114-11's behavioural twin
— and **not** by a conversion table. Anyone adding a v1→v2 status conversion function is adding a
second place for the mapping to be wrong; there is nothing for it to convert.

### The provenance lock is two-directional, and it found something

| constant | attribution site | strength |
|---|---|---|
| `TASKS_EXTENSION_KEY` | itself | **Pinned** — names the vendored schema, the pinned commit AND `114-SPEC-RECHECK.md` |
| `V2_TASKS_METHOD_RETIRED` | itself | **Pinned** — names the vendored schema |
| `TASKS_UPDATE_METHOD` | `TASK_NAME_BEARING_METHODS` | **ProseOnly** — cites the extension in prose, names no file |

`TASKS_UPDATE_METHOD`'s own rustdoc is one line (``/// `tasks/update`. See [`TASKS_GET_METHOD`].``)
and two doc-link hops away sits a table citing "the ext-tasks specification's § *Streamable HTTP:
Routing Headers*" with no path. Measured, not assumed. Recorded as **D-114-Q** rather than fixed,
because the fix is a production rustdoc edit that this plan's own fence forbids.

The lock fails in **both** directions: a `Pinned` entry that loses its artifact reference fails
(NC-7 strips 46 doc lines from `TASKS_EXTENSION_KEY` and fires), and a `ProseOnly` entry that
**gains** one fails too, with the message "promote the entry to `Attribution::Pinned` and close the
deferral". An accepted weakness cannot silently become an unrecorded strength.

### The amendment's three subjects

`-32021` and `-32602` are pinned **by name**, and `-32021` is additionally pinned to the site that
emits it: `missing_tasks_declaration_refusal` must name `MISSING_REQUIRED_CLIENT_CAPABILITY` and
must name **none** of `INTERNAL_ERROR` / `INVALID_PARAMS` / `METHOD_NOT_FOUND` /
`AUTHENTICATION_REQUIRED` / `V1_TASK_PENDING` — case 3 of the ordered chain answers exactly one code,
and a second one there means the refusal branched.

`-32002` is scanned over **both its names and the bare numeric literal**, MEASURED at 8 hits across
4 files:

| file | hits | lines | disposition |
|---|---|---|---|
| `src/types/protocol/error_codes.rs` | 4 | 100, 100, 144, 144 | the two DEFINITIONS (name + number on each line) |
| `src/error/mod.rs` | 2 | 177, 178 | the `ErrorCode` delegating const |
| `src/server/core.rs` | 1 | 3431 | guarded by `v1_initialize_gate_applies` |
| `src/server/task_dispatch.rs` | 1 | 1677 | guarded by `is_v1_task_era` — **count pinned at 1** |

The numeric axis is the new coverage: `tests/v2_prohibited_error_codes.rs` tracks the NAME
`V1_TASK_PENDING` per file and proves the two emission sites v1-only by EXECUTION; this adds the
second name `UNSUPPORTED_CAPABILITY` and the bare number, so a future site writing `-32002` directly
is caught.

### A new exclusion the model file never needed

`cfg(test)`-REGION exclusion is not enough for a numeric scan. `src/server/task_dispatch_tests.rs`
carries no `#[cfg(test)]` marker of its own — the gate is on its `mod` DECLARATION in
`src/server/mod.rs` — and three such files assert on `-32002`. So test-only module FILES are
discovered from their declarations (honouring `#[path = "…"]`, which `wasm_core.rs` uses), not from
a filename convention. **5** files discovered; `the_test_only_exclusions_are_load_bearing` asserts
both directions, including that every `*_tests.rs` file in the tree is one of them, so a new
un-gated test-only file forces a decision.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — blocking] The plan's verify command selects ZERO tests — the SIXTH measured plan-text defect in this phase**

- **Found during:** Task 1, first verification run (pre-empted by 114-15's recorded finding).
- **Issue:** `cargo nextest run --features full -E 'test(/v2_tasks_tripwires/)'` matches nextest's
  `test()` predicate against test NAMES, not binary names. Measured: **`0 tests run: 0 passed, 2590
  skipped`, `error: no tests to run`**. The acceptance criterion "exits 0 with at least 6 tests" is
  therefore unsatisfiable as written.
- **Fix:** every run in this plan used `-E 'binary(v2_tasks_tripwires)'`, which selects **25**. The
  plan's `<verification>` three-binary command, corrected the same way, runs **56/56**.

**2. [Rule 2 — missing critical functionality] The masking check FIRED, and one test became three**

- **Found during:** the negative-control pass, after both task commits.
- **Issue:** NC-1 (deleted guard), NC-2 (unlisted route) and NC-4 (stale entry) all failed the
  IDENTICAL test at the IDENTICAL `assert!` line, separable only by reading the collected message
  text. That is the weaker separation 114-19 had to force its own split out of.
- **Fix:** `every_tasks_route_is_allowlisted_and_era_guarded` was split into three tests, one per rot
  condition, and the guard test SKIPS an entry whose function is missing so the stale-entry control
  cannot also fail it. Re-measured: three **disjoint single-test** failing sets.
- **Commit:** `11e4b7e3`

**3. [Rule 3 — blocking] The plan's `include_str!`-at-runtime instruction is self-contradictory**

- **Issue:** Task 2 says "Read the vendored file at runtime with `include_str!`". `include_str!`
  bakes the bytes in at COMPILE time; the two halves cannot both hold. The acceptance criterion
  accepts either macro or `read_to_string`.
- **Fix:** `read_to_string` through a runtime-resolved path, which is what makes a re-vendoring at
  the D-18 gate move the test without anyone remembering to rebuild. Written into the constant's
  rustdoc so the choice is not re-litigated.

**4. [Rule 3 — blocking] A `cfg(test)`-REGION scan is not enough for a numeric scan**

- **Issue:** `-32002` appears as a bare number in three `*_tests.rs` files that carry no
  `#[cfg(test)]` marker inside them. The model file's exclusion never had to notice, because those
  files do not name `V1_TASK_PENDING`.
- **Fix:** `test_only_module_files()` — a second exclusion discovered from `#[cfg(test)] mod`
  DECLARATIONS across the runtime-walked tree.

### Scope decisions, recorded rather than silently taken

- **Two new deferrals filed instead of two production edits.** D-114-P (router legs answer `-32603`
  for a not-found) and D-114-Q (`TASKS_UPDATE_METHOD`'s attribution is prose-only). Both were
  DISCOVERED by writing the tripwires — which is the instrument working — and both fixes are
  production edits that `git diff --stat -- src/ crates/` being EMPTY forbids. Each is encoded in the
  allowlist so it cannot rot in either direction.
- **The plan's Task 2 asked for a `_ =>` check "anywhere in the mapping".** Scoped to the two TOTAL
  mappings over `TaskStatus` — its `Display` impl (which produces the strings the lock derives) and
  `Task::poll_decision` (which every client poll loop branches on) — rather than every `match` in
  the file, so the check names what it measures.
- **A commit-message/code deferral-ID mismatch, corrected here.** The Task 2 commit body writes
  `D-114-Q` for the router gap and `D-114-R` for the provenance gap. The CODE and
  `deferred-items.md` are authoritative: the router gap is **D-114-P** and the provenance gap is
  **D-114-Q**. Recorded rather than amended, because rewriting a landed commit to hide a typo is
  worse than a line in the summary.

## Negative Controls — EIGHT, all reverted

Each applied, measured with `--no-fail-fast`, then reverted from a `/bin/cp` scratchpad snapshot
verified with `shasum -a 256 -c` on all four touched files. **`git checkout --` was not used**
(114-14's recorded self-inflicted loss) and **`git stash` was not used at any point.**

| # | Control | Site | Failing set | Count |
|---|---------|------|-------------|-------|
| NC-1 | era guard replaced by an inline re-derivation | `route_tasks_cancel` | `every_allowlisted_routes_era_guard_is_present_in_its_named_site` | 1 |
| NC-2 | a new `route_tasks_pause` with NO wire literal | `task_dispatch.rs` | `every_tasks_route_in_the_source_is_allowlisted` | 1 |
| NC-3 | `fn frobnicate` spelling `"tasks/pause"` | `task_dispatch.rs` | `every_v2_tasks_wire_method_maps_to_an_allowlisted_route` | 1 |
| NC-4 | a stale entry for `route_tasks_frobnicate` | the allowlist | `every_allowlisted_route_still_exists` | 1 |
| NC-5 | a SIXTH `TaskStatus` variant (`Paused`) | `types/tasks.rs` + `task_dispatch.rs` | `the_task_status_wire_strings_…` (COUNT assertion) | 1 |
| NC-5b | one variant renamed, count stays FIVE | `types/tasks.rs` | `the_task_status_wire_strings_…` (SET EQUALITY assertion) | 1 |
| NC-6 | the v2 `NotFound` arm mapped to `-32603` | `store_error_response` | `the_minus_32603_population_…` **and** `the_v2_store_not_found_arm_…` | 2 |
| NC-7 | 46 doc lines stripped from the extension key | `types/capabilities.rs` | `every_wire_value_constant_…` | 1 |

**Masking check: RUN, FIRED ONCE, and fixed rather than explained away.** NC-1/NC-2/NC-4 originally
shared one failing test; the split (commit `11e4b7e3`) makes all eight failing sets pairwise
**distinct**. NC-6 fails two tests by design — the count and the arm are two different claims about
the same edit, and each carries its own line number.

**NC-5 and NC-5b exist as a pair for 114-15's recorded reason:** NC-5 trips the COUNT assertion
before the set-equality one can fire, so it proves only the count. NC-5b keeps the count at five and
isolates the equality.

**Two tests are failed by no control, BY CONSTRUCTION.**
`the_route_scan_is_not_vacuous_and_every_entry_is_justified` and
`the_test_only_exclusions_are_load_bearing` are anti-vacuity guards: a control that made either fail
would be a broken scanner, not a removed production guard. Their evidence value is that they stay
green while the other controls fire — which is what proves the eight failures above were measured
over a non-empty, correctly-scoped population.

## Verification

| gate | result |
|------|--------|
| `make quality-gate` | **exit 0** — **4899 passed / 0 failed / 81 ignored across 294 result lines**; 0 non-`ok.` lines; **0** real truncation markers; **0** occurrences of the D-114-A keychain flake |
| `make lint` | **exit 0** (first attempt, and again after every control revert) |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo nextest run -E 'binary(v2_tasks_tripwires)'` | **25/25**, four consecutive runs, **0 LEAK** |
| plan `<verification>`, corrected to `binary()` | **56/56** across the three tripwire binaries |
| the plan's literal `test(/v2_tasks_tripwires/)` form | **0 tests run**, `error: no tests to run` (defect #6) |
| `git diff --stat HEAD~3..HEAD -- src/ crates/ Cargo.toml Cargo.lock fuzz/ examples/ schema/` | **EMPTY** |
| `git diff --stat HEAD~3..HEAD` | `tests/v2_tasks_tripwires.rs \| 2083 ++++` — one file, one direction |
| `git diff HEAD~3..HEAD -- tests/v2_prohibited_error_codes.rs` | **EMPTY** (and it still passes, in the 56) |
| `pmat analyze complexity --max-cognitive 25` | **5** at `.summary.violations`, **0 in `src/`**, **0 in the new file** — identical to 114-14's and 114-15's set |
| `grep -c read_dir tests/v2_tasks_tripwires.rs` | **2** |

**Gate reconciliation, exact.** The new `v2_tasks_tripwires` binary is `Running` in **three** gate
legs — two filter to `running 0 tests`, one runs all 25. 291 + 3 = **294** result lines and
4874 + 25 = **4899** passed, matching 114-15's recorded baseline to the test. No lib tests were
added, so there is no doubled-row term.

**The five `truncated` matches in the gate log are TEST NAMES, not output truncation.** They are
`a_non_envelope_body_becomes_a_truncated_transport_error`,
`an_untrusted_frame_is_truncated_in_error_messages` (×2 legs) and
`an_over_bound_resource_subscriptions_list_is_truncated_and_reported`. The log is 8299 lines and
complete, captured through `/usr/bin/make` per 114-15's recorded `rtk`-truncation trap.

**Why `cargo semver-checks`, `cargo public-api` and `make wasm-build` were NOT re-run.** The
production diff across all three commits is **byte-empty**, and the only added file is
`tests/v2_tasks_tripwires.rs`, which is `#![cfg(not(target_arch = "wasm32"))]` and is on no published
surface. Those three gates answer questions about production bytes that did not move; 114-14's
results (223/223 no update required; Removed/Changed/Added none; wasm exit 0) carry forward by
construction rather than by re-measurement — the same reasoning 114-15 recorded.

**Disk:** 86% used / 129 GiB free at plan start, 88% / 114 GiB at the gate. Zero occurrences of the
D-114-A keychain panic.

## Threat Model Coverage

| Threat | Disposition | Evidence |
|--------|-------------|----------|
| T-114-83 silent removal of an era guard | mitigated | per-route allowlist naming each guard AND its site; three rot conditions in three DISJOINT tests, each with its own control (NC-1, NC-2, NC-4) |
| T-114-84 `-32603` for a not-found leaking a different failure class | mitigated **with a recorded gap** | 9 emissions enumerated by function and COUNT, plus a positional arm assertion; NC-6 fails both with line numbers. **D-114-P**: three `TaskRouter` legs remain `-32603`, pinned and justified |
| T-114-85 status-string drift from the published schema | mitigated | exact SET equality read from the vendored artifact at runtime; NC-5 (sixth variant) and NC-5b (renamed variant) fire on the count and the equality respectively |
| T-114-86 a wire value entering the tree without attribution | mitigated **with a recorded gap** | two-directional lock over all three values; NC-7 fires. **D-114-Q**: `TASKS_UPDATE_METHOD` is prose-only, recorded and pinned in both directions |
| T-114-87 vacuously-green scanner | mitigated | zero discovered routes FAILS; `>= 4` routes asserted; both exclusions proven load-bearing on real files; justifications >= 40 chars and pairwise distinct across all three allowlists; twelve scanner unit tests |
| T-114-SC npm/pip/cargo installs | accepted | **zero** packages installed; `Cargo.toml`/`Cargo.lock` byte-unchanged; production diff byte-EMPTY |

**Threat surface scan:** no new network endpoint, auth path, file access or schema change — this plan
adds one test file and changes no production byte. No threat flags.

## Known Stubs

None. Every check in the file scans the real tree and every allowlist count was measured against it.

## For the next plans

- **`.planning/REQUIREMENTS.md` is UNTOUCHED (0-byte diff)** and `requirements mark-complete` was
  deliberately NOT run. TASK-01…06 flip as a GROUP under the phase's contract-first waiver; the
  `## Verdict` stays `PENDING`. **114-18 owns the flip.** TASK-03's structural evidence is
  `tests/v2_tasks_tripwires.rs` (the retirement guards, named and self-rotting) and TASK-04's is the
  status set-equality lock plus 114-11's behavioural twin — cite the file, not this summary.
- **`114-SPEC-RECHECK.md` was deliberately NOT edited.** This plan landed no wire value; it CONSUMES
  114-01's vendored schema, 114-08's retirement predicates and 114-11's `-32602` mapping. Rows
  16/17/18/19/20 are untouched.
- **114-18 must not treat D-114-P as closed.** A router-backed v2 server answers `-32603` for a
  not-found `tasks/get`, where the extension says MUST `-32602`. Phase 118's conformance run against
  a `TaskRouter` deployment would grade it. Store-backed servers are correct.
- **A trap for anyone adding a `tasks/*` route:** the tripwire will fail you twice — once for the
  function name, once for the wire literal — and it wants the guard named against the FUNCTION IT
  RUNS IN, not the file. If your route's gate lives one frame up (as `tasks/list`'s does), name that
  frame.
- **A trap for anyone touching `TaskStatus`:** the lock parses the enum DECLARATION. Adding a variant
  fails it even if no route can return the status, and even if every match arm compiles. That is
  deliberate — see the rustdoc on `the_task_status_wire_strings_are_set_equal_to_the_vendored_schema`
  for why a v1→v2 status conversion table must NOT be added.
- **A nextest selector trap, now measured for the SIXTH time in this phase:**
  `-E 'test(/<file-stem>/)'` selects **ZERO** tests. Use `-E 'binary(<file-stem>)'`.

## Self-Check: PASSED

- Artifacts on disk: `tests/v2_tasks_tripwires.rs` (**2083 lines**, `min_lines: 250`) — FOUND.
- Commits reachable: `a8afc1a4`, `4dc81c61`, `11e4b7e3` — all FOUND in `git log`.
- `must_haves` key-link greps: `read_dir` in `tests/v2_tasks_tripwires.rs` → **2**; the file
  references `src/server/task_dispatch.rs` through the runtime-discovered `DISPATCH` constant and
  through `src_files()`/`shipped_files()`; **25** `#[test]` functions against a 6-test floor.
- Production diff `HEAD~3..HEAD -- src/ crates/` → **EMPTY**, as the plan's coverage-only fence
  requires.
- Working tree at hand-off: clean apart from `.pmat/*` cache churn and the pre-existing untracked
  files that were present at dispatch.
