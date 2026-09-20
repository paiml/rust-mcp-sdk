---
phase: 114-tasks-extension-migration
plan: 08
subsystem: api
tags: [mcp, tasks-extension, era-gating, json-rpc, method-not-found, protocol-versioning]

# Dependency graph
requires:
  - phase: 114-05
    provides: "`apply_tasks_capability_rule`'s v2 arm (the advertisement that made `V2_TASKS_NOT_NEGOTIATED`'s message untruthful) and `core::project_capabilities_for_v1`"
  - phase: 114-02
    provides: "`tests/common/v2.rs` (`spawn_tasks_server`, `AuthPosture`, `tasks_request_body`, `teardown`) and `tests/v1_tasks_golden.rs`, the 14-fixture v1 byte-identity guard"
  - phase: 114-01
    provides: "the vendored `ext-tasks` schema — the provenance for `tasks/list` and `tasks/result` being ABSENT from the v2 extension"
provides:
  - "Two named `const fn` era predicates, `tasks_list_serves_on_era` / `tasks_result_serves_on_era`, each with a truth-table rustdoc and its own negative control"
  - "`V2_TASKS_METHOD_RETIRED` — the truthful `-32601` message for a v2-retired `tasks/*` method — replacing the now-false `V2_TASKS_NOT_NEGOTIATED`"
  - "`TaskDispatch::has_task_backend()`, which keeps RETIRED and NO-BACKEND as distinguishable refusals"
  - "`pmcp::testing::V2_TASKS_METHOD_RETIRED`, so live-socket suites assert the shipped string instead of a hand-copied mirror"
  - "`tests/v2_tasks_era_gates.rs` — the per-method era matrix, 8 tests, both directions of each gate"
affects: [114-11, 114-09, 114-12, 114-13, 114-10, 118]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "One predicate per gate, delegating to one era definition — orthogonal negative controls without a second era answer"
    - "A gate is `!serves_on_era(era) && has_task_backend()`, so the no-backend refusal is never overwritten by a retirement claim"
    - "An era gate returns BEFORE the store/router, so refusal is by construction rather than by a later filter"

key-files:
  created:
    - tests/v2_tasks_era_gates.rs
  modified:
    - src/server/task_dispatch.rs
    - src/testing/mod.rs
    - src/server/mod.rs
    - tests/v2_prohibited_error_codes.rs

key-decisions:
  - "`V2_TASKS_NOT_NEGOTIATED` was DELETED, not reworded-and-kept: after 114-05 its message was false, and its only emission row now answers RETIRED. A second unreachable spelling of 'no' is how two plans come to disagree about one wire string."
  - "There is no 'client did not declare the extension' refusal in the tree, so no constant was minted for it — the plan that lands a server-side negotiation gate mints its own."
  - "Each era gate ALSO requires `has_task_backend()`, so a backend-less server keeps `Tasks not enabled` / `tasks/result not supported` on EVERY era. Three `-32601` conditions, three distinguishable messages."
  - "The `tasks/result` tail match now reads `tasks_result_serves_on_era` rather than `is_v1_task_era` — one predicate, two call sites — because a negative control measured that two independent predicates masked each other."
  - "`tasks/get` and `tasks/cancel` are deliberately NOT gated and take no era argument: both survive in the v2 schema; their v2 SHAPE is 114-11's."

patterns-established:
  - "Orthogonal era predicates: two functions answering the same question so each gate's negative control is attributable, both delegating to the single `is_v1_task_era` definition"
  - "Retirement gate = era predicate AND backend presence, so a retirement claim can never displace a no-backend refusal"
  - "Negative controls are run BEFORE the plan is declared done, because they measure gate topology a review cannot see"

requirements-completed: []

# Metrics
duration: 95min
completed: 2026-07-28
---

# Phase 114 Plan 08: v2 Retirement of `tasks/list` and `tasks/result` Summary

**`tasks/list` and `tasks/result` now answer `-32601` on protocol version 2026-07-28 behind two independently-disable-able named predicates with a truthful message, while v1 keeps every byte — and the negative control found a real gate-topology defect that no review would have.**

## Performance

- **Duration:** ~95 min
- **Tasks:** 2 of 2
- **Files modified:** 5 (1 created, 4 modified), `+935 / −68`

## Accomplishments

- **Both methods are retired on v2 by construction, not by filtering.** Each gate returns before the store, the router and owner resolution. For `tasks/list` that means no `store.list()` and no router call ever run, so a partial-serve regression cannot leak a task id (T-114-32); for `tasks/result` it means the last v2-reachable `-32002` emission path is gone (T-114-34).
- **The refusal message became true.** `V2_TASKS_NOT_NEGOTIATED` told a v2 caller the tasks extension "is not negotiated" — false since 114-05 made every backend-configured server advertise it. It is replaced by `V2_TASKS_METHOD_RETIRED`, whose rustdoc carries the vendored-schema provenance (only `tasks/get`, `tasks/update`, `tasks/cancel` exist) and the spec's own security framing for removing the enumeration primitive.
- **Three `-32601` conditions stay mutually distinguishable**, asserted by re-driving all four real paths and comparing observed strings pairwise — not by declaring them.
- **The negative control did its job.** It exposed that the `tasks/result` retirement was double-gated by two *independent* predicates, so disabling the head gate changed nothing observable. Fixed to one predicate with two call sites.

## Task Commits

1. **Task 1: Named era predicates + the two gates** — `a8906c59` (feat)
2. **Task 2: Live-socket era matrix, both directions** — `4ef923f7` (test)

## What Was Built

### `src/server/task_dispatch.rs`

| Item | Kind | Role |
|------|------|------|
| `tasks_list_serves_on_era` | `pub(crate) const fn` | v1 `true` / `None` `true` / v2 `false`, with a truth-table rustdoc |
| `tasks_result_serves_on_era` | `pub(crate) const fn` | the same for the other method — a SEPARATE function, for orthogonality |
| `V2_TASKS_METHOD_RETIRED` | `pub(crate) const &str` | the `-32601` body, emitted as `format!("{method} {…}")` |
| `TASKS_LIST_METHOD` / `TASKS_RESULT_METHOD` | `const &str` | one spelling per method, shared by gate and message |
| `retired_on_v2` | `fn` | the SINGLE refusal builder both gates use |
| `TaskDispatch::has_task_backend` | `const fn` | the second half of every gate condition |

The gate at both routes is one line:

```rust
if !tasks_list_serves_on_era(era) && self.has_task_backend() {
    return retired_on_v2(id, TASKS_LIST_METHOD);
}
```

`route_tasks_list` gained an `era` parameter (supplied by `route_tasks_endpoint`, which already received it). `route_tasks_get` and `route_tasks_cancel` deliberately did NOT: both methods survive in the v2 schema, and that asymmetry is written into `route_tasks_endpoint`'s rustdoc so a later reader does not "complete" the pattern.

### Docs this change falsified, corrected in the same commits

1. `is_v1_task_era`'s `# What this predicate deliberately does NOT do` block claimed it "gates ONLY the `-32002` emission" and that "`tasks/get`, `tasks/list` and `tasks/cancel` are unchanged on every era". Both sentences became false. Rewritten to state what is now true, including the distinction that `tasks/get`/`tasks/cancel` change SHAPE on v2 (114-11) but are not retired.
2. `route_tasks_endpoint`'s "`era` … is read by exactly one branch".
3. `src/server/mod.rs`'s call-site comment "Read by the `tasks/result` pending refusal only" (comment-only, see Deviations).
4. `tests/v2_prohibited_error_codes.rs`'s comment asserting "pmcp advertises no `io.modelcontextprotocol/tasks` entry" — false since 114-05.

### `tests/v2_tasks_era_gates.rs` — 496 lines, 8 tests

| # | test | what it proves |
|---|------|----------------|
| 1 | `v1_tasks_list_still_serves` | v1 enumeration returns the created task |
| 2 | `v2_tasks_list_is_gated` | `-32601` + RETIRED; raw text carries neither the task id nor a `tasks` key |
| 3 | `v1_tasks_result_still_serves_pending_minus_32002` | the FROZEN `-32002` and its exact message |
| 4 | `v2_tasks_result_is_gated` | `-32601` + RETIRED; `-32002` appears NOWHERE in the raw text |
| 5 | `v2_tasks_list_on_a_backendless_server_says_not_supported` | `Tasks not enabled`, ≠ RETIRED |
| 6 | `v2_tasks_result_on_a_backendless_server_says_not_supported` | `tasks/result not supported`, ≠ RETIRED |
| 7 | `v2_tasks_get_and_cancel_are_not_gated` | the scope fence: both survivors still serve a real result on v2 |
| 8 | `the_minus_32601_conditions_are_mutually_distinct` | all four refusals pairwise distinct, collected by re-driving |

Each v2 test re-establishes its OWN non-vacuity control on the SAME live server first — test 2 proves the task id IS enumerable via v1 before asserting its absence on v2; test 4 proves the fixture reaches the `-32002` branch on v1 before asserting `-32002` is absent on v2. Without that, both absence assertions could pass against an empty store or an unreachable branch.

## Negative Controls (run, measured, reverted)

Both controls edited only a predicate body, were reverted from a byte-for-byte backup verified with `shasum -a 256 -c` (**OK** both times), and **`git stash` was not used at any point**. Selection was `binary_id(pmcp::v2_tasks_era_gates) or binary_id(pmcp::v1_tasks_golden) or binary_id(pmcp::v2_prohibited_error_codes) or (binary_id(pmcp) and test(/era_gate_tests/))` = **46 tests**, run with `--no-fail-fast`.

### NC-1 — `tasks_list_serves_on_era` forced to `true`

**4 of 46 fail, every one a `tasks/list` probe:**

- `era_gate_tests::tasks_list_era_truth_table` (unit)
- `era_gate_tests::v2_tasks_list_is_retired` (unit)
- `v2_tasks_era_gates::v2_tasks_list_is_gated`
- `v2_tasks_era_gates::the_minus_32601_conditions_are_mutually_distinct` (cross-cutting: with the gate off a v2 `tasks/list` SERVES, so it produces no refusal message to compare)

**All six `tasks/result` tests stay GREEN**, as do all 14 `v1_tasks_golden` fixtures and all 18 `v2_prohibited_error_codes` tests. `v2_tasks_list_on_a_backendless_server_says_not_supported` also correctly stays green — it guards against an OVER-broad gate, not for the gate.

### NC-2 — `tasks_result_serves_on_era` forced to `true`

**5 of 46 fail, every one a `tasks/result` probe:**

- `era_gate_tests::tasks_result_era_truth_table` (unit)
- `era_gate_tests::v2_tasks_result_is_retired` (unit)
- `v2_tasks_era_gates::v2_tasks_result_is_gated`
- `v2_prohibited_error_codes::site_b_v2_http_request_must_not_elicit_a_prohibited_code`
- `v2_prohibited_error_codes::every_code_a_v2_request_elicits_here_is_inventoried`

The last two are the strongest attribution available: with the gate open, a real v2 HTTP `tasks/result` puts `-32002` back on the wire — the exact conformance violation Phase 113 built that suite to catch. Every `tasks/list` test and all 14 `v1_tasks_golden` fixtures stay GREEN.

### The defect NC-2 found (and the first run that did NOT find it)

NC-2 was run **twice**. On the first run it failed only **1** test — the truth-table unit test — while `v2_tasks_result_is_gated` and `v2_tasks_result_is_retired` both PASSED with the gate disabled.

**Cause:** `handle_tasks_result` had TWO independent era decisions. The head gate read `tasks_result_serves_on_era`; the tail match still read `is_v1_task_era` directly and answered the `(store present, era v2)` row with the identical RETIRED body. Disabling either one alone was masked by the other, so **neither was load-bearing for any test** — the head gate could have been deleted outright and the suite would have stayed green.

**Fix (Rule 1, folded into the Task 2 commit):** the tail match now reads `tasks_result_serves_on_era(era)`. One predicate, two call sites. The `(true, false)` arm is required for exhaustiveness and unreachable in production; it answers identically so the two spellings cannot diverge. This is the transferable finding: **defense-in-depth that duplicates a predicate destroys the negative control that would prove either copy load-bearing.** Prefer one predicate with N call sites.

`tasks/list` was never exposed to this because it has exactly one gate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Gate topology] The `tasks/result` retirement was double-gated and therefore unprovable**

- **Found during:** Task 2, by negative control NC-2
- **Issue:** two independent era predicates in one function masked each other; no test could fail when either was disabled
- **Fix:** the tail match reads `tasks_result_serves_on_era`, the same predicate as the head gate
- **Files modified:** `src/server/task_dispatch.rs`
- **Commit:** `4ef923f7`

**2. [Rule 3 — Blocking] `tests/v2_prohibited_error_codes.rs` pinned the OLD v2 message text**

- **Found during:** Task 1
- **Issue:** the plan's acceptance criterion says `git diff tests/v2_prohibited_error_codes.rs` must be empty. It cannot be: `site_b_v2_http_request_must_not_elicit_a_prohibited_code` asserts the v2 `tasks/result` refusal `.contains("not negotiated")` — the exact string this plan is chartered to remove as untruthful. Measured, not assumed: with only Task 1's source change in place, that suite failed **1 of 18**, on that assertion, with the new message on the wire.
- **Considered and rejected:** wording the RETIRED message so it still contains "not negotiated". That would reintroduce the framing the plan exists to kill.
- **Fix:** the assertion now reads `.contains(pmcp::testing::V2_TASKS_METHOD_RETIRED)` — the SHIPPED constant, so it cannot drift — and the stale comment above it (which asserted as fact that "pmcp advertises no `io.modelcontextprotocol/tasks` entry", false since 114-05) was corrected in the same edit. The assertion's intent, "the refusal must say WHY", is unchanged. Suite back to **18/18**.
- **Important:** the test the plan actually meant, `pending_tasks_result_preserves_minus_32002`, lives in `src/server/task_dispatch_tests.rs`, **not** in that file. It is **green and unedited**, as are `site_b_v1_pending_tasks_result_still_emits_minus_32002` and `site_b_typed_surface_cannot_carry_a_v2_era_signal`.
- **Files modified:** `tests/v2_prohibited_error_codes.rs`
- **Commit:** `a8906c59`

**3. [Rule 1 — Stale doc] `src/server/mod.rs` claimed the era was "read by the `tasks/result` pending refusal only"**

- **Found during:** Task 1
- **Issue:** the plan requires `git diff --stat -- src/server/core.rs src/server/mod.rs` to be empty, whose stated intent is "no era gate is added in those files". No gate was added to either. But `mod.rs`'s call-site comment became false the moment two more branches started reading `era`, and this plan's own Task 1 names stale-comment rot (113-29) as the failure class to avoid.
- **Fix:** a **comment-only** correction — **0 lines of code changed**, verified by reading the whole hunk (`@@ -1650,9 +1650,12 @@`, three `-` lines and six `+` lines, all inside a `//` block). `src/server/core.rs` is **byte-unchanged**: `git diff --stat -- src/server/core.rs` is empty.
- **Files modified:** `src/server/mod.rs`
- **Commit:** `a8906c59`

**4. [Rule 2 — Anti-drift] `pmcp::testing::V2_TASKS_METHOD_RETIRED` added (one file beyond the declared `files_modified`)**

- **Found during:** Task 1
- **Issue:** the constant is `pub(crate)`; two integration suites assert on the message across a real HTTP boundary. Without a seam both would hand-copy the sentence — the mirror-drift failure `src/testing/mod.rs` already records for the `Mcp-Name` encoder.
- **Fix:** a `#[cfg(not(target_arch = "wasm32"))] pub const` re-export, the same shape as the existing `META_SERVER_INFO` / `ANONYMOUS_PRINCIPAL` re-exports and the same seam 114-06 used for the two `pub(crate)` method tables. Purely additive; `cargo semver-checks` **223/223, "no semver update required"**.
- **Files modified:** `src/testing/mod.rs`
- **Commit:** `a8906c59`

### Plan-text corrections worth carrying

- The plan's Task 1 verify command is `cargo nextest run --features full -E 'test(/gate_tests/) or test(/task_dispatch/)'`. **nextest's `test()` predicate matches test NAMES, not binaries** — this is the FOURTH plan in this phase to be handed that selector (114-01, 114-02, 114-07, now 114-08). `test(/gate_tests/)` happens to work here only because the module path is part of a unit test's name; `test(/task_dispatch/)` likewise. Task 2's `test(/v2_tasks_era_gates/)` would have selected **0 tests and exited 4**, because no test in that file carries the token — the correct selector is `binary_id(pmcp::v2_tasks_era_gates)`, used throughout.
- The plan's acceptance grep for the corrected rustdoc, `grep -A30 'fn is_v1_task_era' … | grep -c 'ONLY the'`, passes **vacuously**: the doc block it targets sits ABOVE the `fn` line, so `-A30` never reaches it. It reports `0` (as required) but would report `0` for an unedited file too. The block was rewritten on the merits; the grep is not the evidence.

## Verification — exact commands and exit codes

| Command | Result |
|---------|--------|
| `make quality-gate` (detached, awaited, exit-marker file) | **exit 0** — 267 `test result:` lines, **4670 passed, 0 failed, 80 ignored**, **0** truncation markers, **0** non-`ok.` lines, 7859-line log |
| `make lint` (pedantic + nursery) | **exit 0** (run 3×: after Task 1's first draft it caught 2 errors, then exit 0 after the fix and again with the new test file) |
| `cargo fmt --all -- --check` | **exit 0** |
| `cargo semver-checks check-release --baseline-rev 27364eb1` | **exit 0** — 223 checks, **223 pass**, "no semver update required" |
| `pmat analyze complexity --format json --max-cognitive 25` (queried at `.summary.violations`) | **4 violations — the inherited set, unchanged**: `crates/mcp-tester/tests/property_tests.rs` (×2), `crates/pmcp-server-toolkit/tests/sql_server_http_example.rs`, `crates/pmcp-agent/tests/http_sources_mock.rs`. **0 in `src/`**, 0 in `task_dispatch.rs`, 0 in the new test file |
| `make wasm-build` | **exit 0**, 93 warnings, **0** naming `task_dispatch.rs` or `src/testing` |
| `cargo nextest run -E 'binary_id(pmcp::v2_tasks_era_gates)'` | **exit 0, 8/8** |
| `cargo nextest run -E 'binary_id(pmcp::v1_tasks_golden)'` | **exit 0, 14/14** — v1 bytes did not move |
| `cargo nextest run -E 'binary_id(pmcp::v2_prohibited_error_codes)'` | **exit 0, 18/18** |
| 46-test affected set (`--no-fail-fast`) | **46 passed** at HEAD; 4 / 5 fail under NC-1 / NC-2 respectively |

**Gate arithmetic reconciles exactly.** 114-07's baseline was 264 result lines / 4650 passed. This plan adds 6 lib tests and 8 integration tests. Measured from the gate log rather than assumed: the lib test binary runs in **5** gate legs, of which **2** actually execute the new module (the other three filter it out), and the new integration binary appears in **3** legs, of which exactly **1** executes its 8 tests (the other two report `0 passed; 8 filtered out`). So **6 × 2 + 8 = 20**, and **4650 + 20 = 4670** — exact. Result lines **264 → 267** is the one new test binary appearing in three legs.

### Untouched invariants, verified with `/usr/bin/git`

- `git diff --stat HEAD~2 HEAD -- crates/ Cargo.toml Cargo.lock .planning/REQUIREMENTS.md` → **empty**. Zero packages installed (T-114-SC).
- `src/server/core.rs` → **byte-unchanged**.
- The frozen `-32002` emission arm is byte-identical: no deletion line in the diff touches `"task result not available: task not completed"` (the one diff line carrying that literal is an ADDED line, in the new unit test that pins it).
- No `tasks/*` row added to `MRTR_METHODS`; `src/types/mrtr.rs` untouched.
- Row 23 (`own_reserved_result_fields` deleting `inputRequests`) is still **114-10's** and was not designed around.
- All **68** deletion lines across both commits inspected: doc rewrites, the removed `V2_TASKS_NOT_NEGOTIATED` block, the replaced match arm, the widened `route_tasks_list` signature. No code was lost.
- `git diff --diff-filter=D --name-only` on each commit → **no file deletions**.

## Requirements

TASK-03 is **IMPLEMENTED** but deliberately **NOT flipped**. `.planning/REQUIREMENTS.md` is **untouched (0-byte diff)** and `requirements mark-complete` was **not run**: `114-SPEC-RECHECK.md` flips TASK-01..06 as a GROUP and only on a `PUBLISHED-CONFIRMED` landing, and its `## Verdict` is still `PENDING`. Inventory rows 32, 37 and 38 — the three rows this plan owns — are now implemented in `src/server/task_dispatch.rs` exactly as the inventory assigns them.

No contract YAML was written (114-20's option-b waiver; `../provable-contracts/` does not exist on this machine).

## Known Stubs

None. Every symbol this plan added has a production caller: both predicates are called by their routes, `retired_on_v2` by both gates, `has_task_backend` by both gates, and `V2_TASKS_METHOD_RETIRED` by `retired_on_v2`.

## Threat Flags

None. No new network endpoint, auth path, file access pattern or schema change at a trust boundary was introduced; the plan REMOVES a surface (`tasks/list` enumeration on v2) rather than adding one.

## What the next plans need to know

- **114-11 (v2 `tasks/get` shape) owns the survivors.** `tasks/get` and `tasks/cancel` take **no** `era` argument by design and must not gain a retirement gate. `v2_tasks_get_and_cancel_are_not_gated` is the fence, and it asserts they serve a real result on v2 today — a change that makes them error would fail there. Their store-`NotFound` → `-32603` mapping is untouched and is still Pitfall 5's to remap.
- **The `-32002` site now has ONE guard, and it is `tasks_result_serves_on_era`.** `is_v1_task_era` is still the single era definition and is still named in `task_dispatch.rs` (so `V1_TASK_PENDING_SITES`' `guard: "is_v1_task_era"` entry in `tests/v2_prohibited_error_codes.rs` still passes), but it now guards the emission one delegation deep. A plan that tightens that tripwire may want to update the `guard` string to the predicate that is literally at the site.
- **Do not re-add a second era decision to a gated route.** See "The defect NC-2 found". One predicate, N call sites.
- **`V2_TASKS_NOT_NEGOTIATED` no longer exists.** A plan that lands a server-side "the client did not declare the tasks extension" refusal mints its own constant; do not resurrect the deleted one, whose text conflates negotiation with existence.
- The measured cross-plan facts 114-02 recorded still hold and were re-confirmed by the 14 green golden fixtures: a router-backed v1 `tasks/result` answers `-32603` from the fall-through, and a router-only high-level `Server` cannot mint a task from `tools/call`.

## Deferred

Nothing new was deferred by this plan. Open items are unchanged: **D-114-A**, **D-114-B**, **D-114-C** (server-side `Mcp-Name` enforcement, owner Phase 118), **D-114-D**, **D-114-E** (`make test-feature-flags` exit 2, pre-existing, owned elsewhere), **D-113-U**.

## Self-Check: PASSED
