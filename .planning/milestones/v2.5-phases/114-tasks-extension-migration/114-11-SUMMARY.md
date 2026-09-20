---
phase: 114-tasks-extension-migration
plan: 11
subsystem: server/tasks
tags: [tasks-extension, v2, wire-shapes, TASK-04, ext-tasks]
requires:
  - 114-04 (TaskStore::task_input_snapshot / get_error — the only reachable path to the two values not on the wire Task)
  - 114-10 (ReservedFieldOwner::TasksDispatch — the egress PERMISSION for a top-level inputRequests)
provides:
  - "pmcp::types::tasks::{TaskV2, TaskDetailV2, DetailedTaskV2} — additive v2 projection types"
  - "server::core::DispatchEnvelopeClaim — the second envelope claimant, threaded from the write site"
  - "the four v2 task result shapes, projected above the serde_json::Value seam"
  - "TaskStoreError::NotFound|Expired -> -32602 on v2, oracle-free"
affects:
  - 114-12 (the create trigger — its claim plumbing is DONE; only the TRIGGER remains)
  - 114-13/114-14 (tasks/update — row 19's empty ack is still unowned)
  - 114-19 (client era decoding — DetailedTaskV2::from_wire_value is the status-directed decoder)
tech-stack:
  added: []
  patterns:
    - "era decides the SHAPE, not the routing: one store/router/no-backend path, two projections"
    - "the envelope claim is made WHERE THE WRITE HAPPENS and threaded, never re-derived (DQ2)"
    - "per-variant required fields expressed as enum variants, not Option + skip_serializing_if"
    - "schema `required` arrays read from the vendored artifact at compile time, never restated"
key-files:
  created:
    - tests/v2_tasks_shapes.rs
  modified:
    - src/types/tasks.rs
    - src/server/task_dispatch.rs
    - src/server/core.rs
    - src/server/mod.rs
    - tests/common/v2.rs
decisions:
  - "Separate v2 projection types over additive fields on Task — the union is five status-discriminated variants with per-variant required fields, which one flat struct cannot express"
  - "TaskStoreError::Expired folded onto the -32602 not-found answer (the plan text named only NotFound); the anti-oracle constraint enumerates expired, and TaskStoreError's own From impl already maps it to not_found"
  - "core.rs + mod.rs edited beyond the declared files_modified (Rule 3): the acceptance criteria are end-to-end and the claim cannot reach the envelope otherwise"
  - "D-114-H closed one plan early: both dead-code allows removed after measuring three feature selections"
metrics:
  duration: ~3h
  completed: 2026-07-28
  tasks: 3
  commits: 3
  tests-added: 36 (13 lib + 10 lib + 13 integration)
  negative-controls: 11
---

# Phase 114 Plan 11: v2 Task Result Shapes Summary

The v2 task surface now emits flat, schema-shaped payloads — `ttlMs`/`pollIntervalMs`, the
status-conditional `result`/`error`/`inputRequests` inlined at the top level, empty acknowledgements,
and `-32602` for task-not-found — all above the `serde_json::Value` seam, with the v1 wire
byte-frozen.

## What landed

**Three v2 result shapes, era-split.** `tasks/get` becomes the flat `DetailedTask` variant;
`tasks/cancel` becomes an empty acknowledgement; a `tools/call` create becomes the flat
`Result & Task` carrying `resultType: "task"`. v1 keeps the nested `{"task": {…}}` envelope with
`ttl` and `pollInterval`, byte-for-byte. Each route takes ONE store-first / router-fall-through /
no-backend path and branches only on the SHAPE at the end, so the era gate cannot accidentally
change which backend answers.

**`crates/pmcp-tasks` needed no change to serve v2** — that is D-11, verified rather than asserted:
`cargo nextest run -p pmcp-tasks` is **514/514** with a zero-byte diff to that crate. A
DynamoDB/Redis-backed tasks deployment gets the v2 shapes from day one.

**The projection types are additive and the per-variant requirement is STRUCTURAL.** `TaskV2` is the
flat payload; `TaskDetailV2` is an enum with one variant per `TaskStatus`, each carrying exactly the
key its `$defs` variant marks required; `DetailedTaskV2` pairs them with PRIVATE fields and a
constructor that makes the detail authoritative for `status`. A `completed` projection cannot be
built without `result` because `TaskDetailV2::Completed {}` does not compile — there is no
"completed projection with no result" test to write, which is the point.

**`DispatchEnvelopeClaim` is the second envelope claimant.** `mrtr_egress` already returned
`(ResponseDisposition, ReservedFieldOwner)` from the site that mints its reserved fields; the tasks
dispatch needed to say the same two things from ITS minting site, several frames below the one place
that calls `inject_v2_result_envelope`. The claim travels with the response and the two claimants are
folded through one named rule, `or_egress`, so precedence is stated rather than implied by argument
order.

## The measurement that mattered most

**Row 23 is now closed end to end, and the closure is proven on RAW BYTES over a real socket.**
114-10 supplied the egress permission and dispatched nothing. This plan dispatches: an
`input_required` task's `inputRequests` is read through 114-04's `task_input_snapshot`, inlined as a
TOP-LEVEL key, and accompanied by `DispatchEnvelopeClaim::TASKS_INPUT_REQUIRED`. Negative control
NC-1 — narrowing `ReservedFieldOwner::TasksDispatch`'s grant back to nothing — fails **exactly**
`v2_tasks_get_inlines_input_requests_on_input_required` in this suite and nothing else, plus two of
114-10's own tests. That is the whole chain, measured.

**The disposition and the ownership are genuinely independent, and this plan is the case that proves
it.** A v2 `tasks/get` on an `input_required` task carries `resultType: "complete"` — the JSON-RPC
REQUEST completed; it is the TASK that is waiting — WHILE legitimately publishing `inputRequests`.
Under 114-10's removed derivation those two facts were the same variable.

## Deviations from Plan

### Rule 3 — files beyond the declared `files_modified`

**1. `src/server/core.rs` and `src/server/mod.rs`**

- **Found during:** Task 2.
- **Issue:** the plan's acceptance criteria are end-to-end — "a v2 create response carries
  `resultType: "task"`", "test 5 asserts on raw response bytes containing `inputRequests`" — and
  neither is reachable without the claim arriving at `inject_v2_result_envelope`.
  `handle_request_internal` returns only the response, and `own_reserved_result_fields` OWNS
  `resultType` (it overwrites whatever a producer wrote), so a claim written into the result object
  would be silently discarded and a stripped `inputRequests` would leave test 5 red.
- **Fix:** `route_tasks_endpoint`, `maybe_build_task_created` and `build_task_created_response`
  return `(JSONRPCResponse, DispatchEnvelopeClaim)`; `handle_request_internal` (core) and
  `handle_client_request` / `process_client_request` / `handle_call_tool` (mod) carry a
  `&mut DispatchEnvelopeClaim` out-param that only the two claiming arms write. An out-param rather
  than a changed return type because ~two dozen other arms of `handle_request_internal` have nothing
  to say and had to stay untouched.
- **Note for 114-12:** STATE.md assigned "supply `ReservedFieldOwner::TasksDispatch` from the tasks
  route" to you. It is **done**. What remains yours is the v2 create TRIGGER. Do not add a second
  path — and in particular do not re-derive the owner from the disposition or the method string,
  both of which DQ2 rejected.

**2. `tests/common/v2.rs`**

- **Found during:** Task 3.
- **Issue:** `input_required` and `failed` have no client-facing trigger in this phase (both are
  server-side writes), so a shape suite could not construct two of the five shapes it must assert.
  The shared harness took the store by value.
- **Fix:** EXTENDED, not forked. `spawn_tasks_server` is now a wrapper over a new
  `spawn_tasks_server_with_store` primitive, so a tool or capability added there reaches every
  existing tasks suite unchanged. A synchronously-completing `isError` tool was added so
  `terminal_status_discipline` exercises the real create path rather than poking the store into the
  answer it wants to assert. All eight tasks/MRTR binaries stay green (98/98).

### Rule 2 — a correction to the plan's not-found scope

**3. `TaskStoreError::Expired` is folded onto the `-32602` not-found answer**

- **Found during:** Task 2.
- **Issue:** the plan's action text says "map `TaskStoreError::NotFound` → `-32602`; every other
  `TaskStoreError` stays `-32603`". But `TaskStoreError::Expired` is a DISTINCT variant whose
  `Display` renders the task id, and the plan's own threat model (T-114-50) and SPEC-RECHECK row 29
  both require the message to be identical for absent / wrong-owner / **expired**. Following the
  action text literally would have made the sharper `-32602` code the thing that revealed "that id
  existed until recently".
- **Fix:** `NotFound` and `Expired` both map to `-32602` with the single
  `V2_TASK_NOT_FOUND_MESSAGE`. This is consistent with the codebase's existing posture:
  `From<TaskStoreError> for Error` already maps `Expired` onto `not_found` with the comment "to
  avoid leaking existence of expired tasks". `InvalidTransition` and `Internal` stay `-32603`.
- **Recorded in `114-SPEC-RECHECK.md` row 29** as a correction to the plan text, not a deviation
  from the requirement.

### Rule 1 — clippy errors the weaker commands missed

**4. `literal_string_with_formatting_args` and `single_match_else`**

- **Found during:** Tasks 1 and 2.
- **Issue:** `cargo check --all-targets` and `cargo nextest` were BOTH green; `make lint` (pedantic +
  nursery under `-D warnings`) failed. Sixth consecutive plan in this phase where `make lint` caught
  what a weaker command would not.
- **Fix:** rewrote the `expect_err` message and converted a two-arm `match` to an early-return
  `if let`.

**5. `items_after_statements` on the v1 golden literal**

- **Found during:** Task 3.
- **Fix:** hoisted `V1_GET_WORKING` to module scope, where its "a diff here is a v1 WIRE BREAK"
  rustdoc is more visible anyway.

### An early closure, not a deviation

**6. D-114-H closed one plan earlier than assigned**

Both `ResponseDisposition::Task`'s `#[cfg_attr(not(test), allow(dead_code))]` and
`ReservedFieldOwner::TasksDispatch`'s `#[cfg_attr(not(feature = "testing"), allow(dead_code))]` are
now **deleted**: `DispatchEnvelopeClaim::TASK_CREATED` and `::TASKS_INPUT_REQUIRED` construct them,
`server::task_dispatch` is gated only on `not(target_arch = "wasm32")` and on no feature, so both
have production constructors on every native build. **Measured before removal**, with
`RUSTFLAGS="-D warnings" cargo clippy --lib`:

| feature selection | result |
|---|---|
| `--features full` | exit **0**, zero errors |
| `--no-default-features --features streamable-http` | **no** error naming `TasksDispatch` |
| `--no-default-features` | **55** pre-existing D-114-E dead-code errors, **none** naming `TasksDispatch` or the `Task` disposition variant |

The comments left in place record the measurement, so a future reader does not re-add the allows
defensively.

## Negative Controls

**Eleven, not the plan's two — and the extra nine are the point.** The plan named two controls; run
alone they would have left `task_status_wire_strings_match_the_extension_schema`,
`v1_shapes_are_still_nested`, `v1_task_not_found_is_still_internal_error`, both halves of the
`-32602` contract and both `resultType` boundary tests with **no evidence any of them is
load-bearing**. Each control below was applied, MEASURED, and reverted; all four touched files were
verified byte-identical afterwards with `shasum -a 256 -c`. **`git stash` was not used** (the stash
list is unchanged at its 13 pre-existing entries).

| # | mutation | fails in `v2_tasks_shapes` | also fails |
|---|---|---|---|
| NC-1 | `ReservedFieldOwner::TasksDispatch::may_emit` -> `false` (revert 114-10) | **test 5 only** | 2 of `v2_reserved_fields_tasks` |
| NC-2 | keep the nested `{task:{…}}` shape on v2 (`let v1 = true`) | **10 of 13** — 1,2,3,4,5,6,8,10,11 + the not-found row | none; all 14 v1 golden pass |
| NC-3 | rename the `cancelled` status string to `canceled` | **test 7 only** | 1 v1 golden |
| NC-4 | project the v2 shapes on v1 too (`let v1 = false`) | **test 9 only** | 8 v1 golden |
| NC-5 | make `store_error_response` era-BLIND (always `-32603`) | the not-found row | 3 `v2_shape_tests` |
| NC-6 | keep `-32602`, echo the task id in the message | the not-found row | 3 `v2_shape_tests` |
| NC-7 | keep the message, change ONLY the code | the not-found row | 1 `v2_shape_tests` |
| NC-8 | let a non-`input_required` `tasks/get` claim the TASK disposition | tests **2 and 10** | 1 `v2_shape_tests` |
| NC-9 | a synchronous `isError` outcome becomes `failed` | tests **8 and 10** | 1 v1 golden |
| NC-10 | `TASKS_INPUT_REQUIRED`'s disposition -> `Task` | tests **10 and 11** | 1 `v2_shape_tests` |
| NC-11 | give v1 the v2 error mapping | the v1 not-found row | 1 `v2_shape_tests` |

**Three findings from the control set that a reader should carry:**

1. **The plan's NC-2 prediction was wrong.** It predicted "fails tests 1-4 and 6 but NOT test 9".
   Measured: it fails **10 of 13**, and test 9 does indeed pass. The over-prediction is benign, but
   the shape of the error matters — several failures are *fixture* dependencies (`v2_minted_id`
   panics when the create is nested), not independent properties. That is why NC-8/NC-9/NC-10 exist:
   they isolate the boundary and status properties that NC-2 only reaches as collateral.
2. **NC-5 and NC-6 initially failed the SAME four tests**, which is exactly the duplicate-answer
   masking signal 114-08 and 114-10 recorded. NC-5 mutated both the code and the message at once.
   NC-7 was added to isolate the CODE (message unchanged) and NC-6 re-scoped to isolate the MESSAGE
   (code unchanged); they now fail disjoint-ish sets and the two constraints are independently
   proven.
3. **NC-8 does NOT fail test 11**, and that is informative rather than a gap: test 11's `tasks/get`
   is on an `input_required` task, which under NC-8 still takes the `TASKS_INPUT_REQUIRED` arm.
   NC-10 mutates that arm instead and fails test 11. The two controls together cover both arms of
   the same branch — one alone would have left half of it unproven.

## Verification — every check run, verbatim

| check | command | exit | result |
|---|---|---|---|
| quality gate | `make quality-gate` (detached, polled) | **0** | **276 result lines, 4759 passed, 0 failed, 80 ignored**, 0 truncation markers, 0 non-`ok.` lines |
| lint | `make lint` (×3 during development) | **0** | "No lint issues" |
| format | `cargo fmt --all -- --check` | **0** | clean |
| semver | `cargo semver-checks --baseline-rev 555f118c` | **0** | **223 checks: 223 pass, 30 skip — no semver update required** |
| public API | `cargo public-api --features full diff 555f118c..HEAD` | **0** | **Removed (none), Changed (none)** — additive only |
| wasm | `make wasm-build` | **0** | 93 warnings, **0** naming any touched symbol |
| complexity | `pmat analyze complexity --max-cognitive 25`, queried at `.summary.violations` | — | **4**, all pre-existing, all in `crates/**/tests/`, **0 in `src/`** |
| pmcp-tasks | `cargo nextest run -p pmcp-tasks` | **0** | **514/514** (the gate does NOT cover this crate) |
| tasks suites | `-E 'binary_id(pmcp::v2_tasks_shapes) or … v1_tasks_golden or v2_reserved_fields_tasks or v2_tasks_negotiation or v2_tasks_owner_binding or v2_tasks_client or v2_mrtr or common_harness_smoke'` | **0** | **98/98** |
| manifests | `git diff --stat -- Cargo.toml Cargo.lock` | — | **empty** — zero packages installed (T-114-SC) |
| requirements | `git diff --stat -- .planning/REQUIREMENTS.md` | — | **empty — 0-byte diff** |

**The gate delta reconciles to a single test.** Baseline 273 lines / 4700 passed → 276 / 4759:
**+3 result lines** (the new `v2_tasks_shapes` binary is `Running` in exactly **3** gate legs —
counted in the log — two of which report `running 0 tests` under a filter) and **+59 passed**
(13 from the new binary in the one leg that runs it, plus 23 new lib tests × 2 because the lib suite
is counted in both `test-unit` and `validate-always`: 13 + 46 = 59). Both inputs verified
independently: `v2_projection_tests` is **13** tests, `v2_shape_tests` is **10**.

**Acceptance greps, all satisfied:**

| grep | required | actual |
|---|---|---|
| `task_input_snapshot` in `task_dispatch.rs` | ≥ 1 | **2** |
| `get_error` in `task_dispatch.rs` | ≥ 1 | **2** |
| `include_str!\|read_to_string` in the suite | ≥ 1 | **1** |
| tests in the suite | ≥ 11 | **13** |
| lines in the suite | ≥ 200 | **933** |
| `ttlMs` in `task_dispatch.rs` (the `must_haves` artifact `contains`) | ≥ 1 | **5** |
| `MRTR_METHODS` block vs `HEAD` | identical | **IDENTICAL**; `git diff -- src/types/mrtr.rs` empty |
| `git diff` inside `Task` / `CreateTaskResult` / `GetTaskResult` / `CancelTaskResult` | zero | **all four IDENTICAL** by block extraction |

**One acceptance grep required a corrected measurement.** The plan asks that
`grep -c 'V1_TASK_PENDING' src/server/task_dispatch.rs` be unchanged from `HEAD~1`. The raw count
moved **6 → 7**. The intent — "no new emission of the frozen `-32002`" — was measured directly
instead, the 114-06/114-10 way: **non-comment** occurrences are unchanged at **4**, and the single
new occurrence is a rustdoc on `store_error_response` stating that this row is *not* the frozen
`-32002` question. The plan's own action text asks for exactly that documentation, so the literal
grep and the plan's instruction are in direct conflict; the emission-site count is the honest proxy.

**All 125 deletion lines enumerated** with `/usr/bin/git`: rewritten route bodies, the two removed
dead-code allows with their rustdoc, two `use` lines, and signature changes. Every one an intended
replacement. `git diff --diff-filter=D` reports no deleted files on any of the three commits.

## Interfaces for later plans

**114-12 (create trigger).** The claim plumbing is done — see the Rule 3 deviation above for the
exact signatures. `build_task_created_response` is already era-split: v2 returns
`v2_create_result_value` + `DispatchEnvelopeClaim::TASK_CREATED`, v1 returns the frozen nested
envelope + `NONE`. Your work is the GATE (`task_requested`), not the shape.

**114-13/114-14 (`tasks/update`).** SPEC-RECHECK row 19's empty acknowledgement is **not**
implemented — the method is not routed and answers `-32601` today. A forward tripwire is armed:
`tasks_get_never_carries_result_type_task` sweeps `tasks/update`'s raw bytes and holds vacuously for
an error response. When you land the method, emit `{}` on v2 and return
`DispatchEnvelopeClaim::NONE` from that arm (`route_tasks_endpoint` already names its claim
explicitly in every arm, so you must state yours).

**114-19 (client era decoding).** `DetailedTaskV2::from_wire_value` is the STATUS-DIRECTED decoder
for exactly what this plan emits — it reads `status` first and requires that status's key, so a
`completed` task with no `result` is REFUSED rather than best-effort-decoded into a shape that fits.
This is the `InputResponse::decode_for` discipline. `TaskV2` derives `Deserialize` normally.
`TaskDetailV2` and `DetailedTaskV2` deliberately do NOT derive `PartialEq`
(`crate::types::mrtr::InputRequest` does not implement it) — compare wire objects.

**114-15 (cross-caller matrix).** The `-32602` refusal is IDENTICAL for absent / wrong-owner /
expired and never renders the id back; `v2_task_not_found_is_invalid_params_and_not_an_oracle`
proves the first two over a socket with two real principals. Your matrix can assume that answer per
method rather than re-deriving it. See D-114-I (the shared fixture now registers TWO tools) and
D-114-J (a v1 caller against the shared harness must complete a real handshake) before writing
fixtures.

## Known Stubs

None. Every shape this plan claims is emitted by production code and asserted over a real socket.
The one deliberate incompleteness is stated rather than hidden: `tasks/update`'s empty ack (row 19)
is NOT implemented because the method is not routed yet, and it is recorded in `114-SPEC-RECHECK.md`
as still owned by 114-13/114-14.

The `v2_project_router_task` degradation path is not a stub either — it is a documented, tested
policy: a `TaskRouter` is out-of-tree code returning an untyped `Value`, and a value that cannot be
projected passes through UNCHANGED with a `tracing::warn!` rather than being half-projected.
`the_router_get_value_is_projected_on_v2` asserts both branches.

## Threat Flags

None. Every file touched sits inside surface the plan's `<threat_model>` already enumerates:
`task_dispatch.rs`'s store-error boundary (T-114-50, mitigated — one message, no id echo, two
independent negative controls), the v2/v1 serde boundary (T-114-52, mitigated — separate types, all
four v1 structs byte-identical, 14 golden fixtures green), and the terminal-status discipline
(T-114-53, mitigated — both directions asserted). No new network endpoint, auth path, file access
pattern or schema change at a trust boundary was introduced.

## Self-Check: PASSED

Files claimed as created/modified, verified on disk:

- `FOUND: tests/v2_tasks_shapes.rs`
- `FOUND: src/types/tasks.rs`
- `FOUND: src/server/task_dispatch.rs`
- `FOUND: src/server/core.rs`
- `FOUND: src/server/mod.rs`
- `FOUND: tests/common/v2.rs`

Commits claimed, verified in `git log`:

- `FOUND: 075269ae` — `feat(114-11): additive v2 task projection types`
- `FOUND: 19697f7b` — `feat(114-11): project the v2 task shapes above the Value seam`
- `FOUND: 5f2eb63f` — `test(114-11): per-shape v2 suite against the vendored schema`
