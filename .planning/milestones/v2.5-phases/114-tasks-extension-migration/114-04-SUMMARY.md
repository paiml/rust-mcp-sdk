---
phase: 114-tasks-extension-migration
plan: 04
subsystem: api
tags: [tasks, task-store, task-router, mrtr, input-delivery, owner-isolation, semver-additive]

# Dependency graph
requires:
  - phase: 113-v2-http-transport
    provides: "InputRequests / InputResponses / InputRequest::kind() / InputResponse::decode_for and the four inputResponses ingress bounds in src/types/mrtr.rs; the D-113-O kind-directed decode this plan supplies the server-side kinds source for"
  - phase: 114-01
    provides: "vendored ext-tasks draft schema + the 39-row wire-value inventory whose row 23 (inputRequests required on InputRequiredTask) this plan opens storage for"
provides:
  - "TaskStore::deliver_task_inputs — additive defaulted input-delivery seam returning TaskInputDelivery (accepted / ignored / complete)"
  - "TaskStore::task_input_snapshot + TaskInputSnapshot{,::kind_of,::outstanding,::is_complete} — the owner-scoped read path for the SERVER-recorded input kinds"
  - "TaskStore::record_input_requests — writes server-authored requests against the store-minted id and transitions to InputRequired in one write"
  - "TaskStore::set_error / get_error — JSON-RPC error persistence for a failed v2 task, carried as serde_json::Value"
  - "TaskStore::supports_inputs() probe, false by default, mirroring supports_results()"
  - "InMemoryTaskStore overrides for all five, so the core path works with no pmcp-tasks dependency"
  - "TaskRouter::handle_tasks_update — additive defaulted Value-in/Value-out seam below the D-11 reshape boundary"
affects: [114-05, 114-06, 114-07, 114-10, 114-11, 114-12, 114-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "additive-defaulted-trait-method (copied verbatim in shape from set_result/get_result)"
    - "capability-probe (supports_inputs mirroring supports_results)"
    - "internal-record storage so input state is purged with the task by cleanup_expired"
    - "state-machine-derived precondition: can_transition_to(Working) IS the awaiting-input predicate"

key-files:
  created: []
  modified:
    - src/server/task_store.rs
    - src/server/tasks.rs

key-decisions:
  - "TaskInputDelivery is a value, not Ok(()) — accepted/ignored/complete must stay distinguishable or 114-14 has to re-read the record to recover partial-vs-complete"
  - "The awaiting-input gate is TaskStatus::can_transition_to(&Working), which is true for InputRequired ALONE (Working->Working is a rejected self-transition, terminals reject outright) — measured, not a hand-written match"
  - "Transition to Working requires complete AND at least one accepted key, so a delivery that changed nothing cannot resume a paused task"
  - "TaskInputSnapshot::input_requests carries the FULL recorded set (what 114-11 inlines); outstanding() is the derived unanswered subset — resolving the plan's ambiguous 'outstanding InputRequests' in favour of exposing both"
  - "record_input_requests refuses whenever any non-empty request set is already recorded (one round per task in the in-crate store), making the erase-answers failure unreachable by construction rather than merely tested against"
  - "Some(empty) input_requests is a distinct state from None ('a round that asked for nothing'), echoing 113-27's Option-vs-empty-map discipline"
  - "TaskInputDelivery and TaskInputSnapshot are deliberately NOT #[non_exhaustive]: out-of-tree TaskStore implementations must be able to construct what they return"
  - "Errors are stored as serde_json::Value, not a typed error, because the JSON-RPC error object crosses the D-11 Value seam unchanged"

patterns-established:
  - "Bounds division of responsibility written at BOTH ends: the four inputResponses bounds are ingress-enforced, named by role, with the count stated ('Four, not five') so a future reader cannot add a fifth check here by miscounting MAX_REQUEST_STATE_LEN in"
  - "Owner-mismatch refusals are asserted to render neither the substring `owner` nor the other owner's id — three tests carry this, not a comment"

requirements-completed: []

# Metrics
duration: 79min
completed: 2026-07-28
---

# Phase 114 Plan 04: TaskStore + TaskRouter input-delivery seams Summary

**Five additive defaulted `TaskStore` methods plus a `supports_inputs()` probe and a defaulted `TaskRouter::handle_tasks_update`, with the `InMemoryTaskStore` implementing input delivery atomically under one dashmap entry guard — semver 223/223 green and a pre-114 out-of-tree store compiling untouched.**

## Performance

- **Duration:** 79 min
- **Started:** 2026-07-28T08:07:55Z
- **Completed:** 2026-07-28T09:27:15Z
- **Tasks:** 3
- **Files modified:** 2 (`src/server/task_store.rs`, `src/server/tasks.rs`)

## Accomplishments

- **`tasks/update` can now reach a backend without a second registration knob and without breaking any existing implementor.** Five new `TaskStore` methods (`deliver_task_inputs`, `task_input_snapshot`, `record_input_requests`, `set_error`, `get_error`) plus the `supports_inputs()` probe, every one defaulted, and one defaulted `TaskRouter::handle_tasks_update`.
- **The seam cross-AI review flagged as missing is present and its necessity was re-verified on disk, not taken on trust.** `TaskStore::get` returns only the wire `Task` (`:249` pre-change) and `TaskRecord` is a private struct, so without `task_input_snapshot` there was no way for `task_dispatch.rs` to read the server-recorded kinds that 114-14's `decode_for` must decode against, nor the `inputRequests` 114-11 must inline on a v2 `tasks/get`. Both now have an owner-scoped accessor.
- **The in-crate store works standalone**, so the phase's paired example needs no `pmcp-tasks` dependency: `InMemoryTaskStore` overrides all five methods and reports `supports_inputs() == true`.
- **18 new tests, 0 removed.** A function-name set-difference against the phase start commit reports **REMOVED: (none)** — the 28 deletion lines in the diff are hunk realignment plus the three intended edits, not lost code.

## Task Commits

1. **Task 1: `TaskStore` input-delivery seam + `supports_inputs()` probe** — `fb23ab14` (feat)
2. **Task 2: `InMemoryTaskStore` input delivery (D-13 site 3)** — `c3ff793e` (feat)
3. **Task 3: `TaskRouter::handle_tasks_update` defaulted seam** — `6855bbb0` (feat)

## Files Created/Modified

- `src/server/task_store.rs` (+1219/-28) — the five defaulted trait methods, `supports_inputs()`, `TaskInputDelivery`, `TaskInputSnapshot` (with `outstanding()` / `kind_of()` / `is_complete()`), three new private `TaskRecord` fields, the `InMemoryTaskStore` overrides, and 18 tests
- `src/server/tasks.rs` (+35/-0) — `TaskRouter::handle_tasks_update`, defaulted to an explicit error
- `.planning/phases/114-tasks-extension-migration/deferred-items.md` — created; two out-of-scope findings recorded (see below)

## Decisions Made

### `can_transition_to(&Working)` IS the awaiting-input predicate — measured, not asserted

The plan said to gate delivery through `TaskStatus::can_transition_to` "rather than a hand-written match". Reading the state machine (`src/types/tasks.rs:56`) shows the check is not merely *a* way to express the rule, it is **exactly** the rule:

| current status | `can_transition_to(&Working)` |
|---|---|
| `Working` | **false** — self-transitions are rejected per spec |
| `InputRequired` | **true** |
| `Completed` / `Failed` / `Cancelled` | **false** |

So one call to the shared, 46-transition-test-pinned state machine is precisely "only an `input_required` task may be fed", with no second predicate to drift. `record_input_requests` gets its terminal-task refusal from the same call in the other direction (`can_transition_to(&InputRequired)`).

### Transition requires `complete` **AND** a non-empty `accepted` set

`complete` is defined as *at least one request recorded AND every recorded key answered*. A task with **no** recorded requests is therefore never "complete" — vacuous completeness would have let a client resume a paused task by posting keys the server never issued. The extra `!accepted.is_empty()` guard means a delivery that changed nothing changes nothing, including the status.

### `TaskInputSnapshot::input_requests` is the FULL set; `outstanding()` is the derived subset

The plan described the snapshot as carrying "the outstanding `InputRequests`", but its two named consumers want different things: 114-11 inlines the full `inputRequests` the server asked for (inventory row 23), while a kind-directed decode wants to know which keys are still open. Carrying the full set plus a derived `outstanding()` serves both unambiguously instead of picking one and leaving the other to re-derive it.

### One round of input requests per task in the in-crate store

`record_input_requests` refuses whenever a non-empty request set is already recorded. That makes "a second call erases answers already delivered" **unreachable by construction** rather than merely tested against, which is the right trade for a dev/test store. The rustdoc and a source comment both say a multi-round production backend (114-07's `GenericTaskStore<B>`) may relax this to supersede-with-merge.

### `Some(empty)` ≠ `None` on `input_requests`

Absent means "the server never recorded anything"; `Some(empty)` means "a round that asked for nothing" — a state 113-27 established is genuinely reachable. Conflating them is the mistake 113-27's `Option<InputRequestKinds>` discipline exists to avoid, so the distinction is preserved and documented at the field.

### Neither new public struct is `#[non_exhaustive]`

Both are **returned by** trait methods that out-of-tree stores override, so they must be constructible downstream. `#[non_exhaustive]` would have made every out-of-tree `deliver_task_inputs` impossible to write. The reason is recorded in `TaskInputDelivery`'s own rustdoc so it is not "corrected" later.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] A temporary `#[allow(dead_code)]` to keep Task 1's commit lint-green**

- **Found during:** Task 1 (`TaskRecord` field additions)
- **Issue:** The plan splits the work so Task 1 adds `input_requests` / `input_responses` / `error` to `TaskRecord` and Task 2 adds the reads. In between, the three fields are write-only, and `make lint` runs with `RUSTFLAGS = -D warnings`, so `dead_code` on an unread private field is a **hard error**, not a warning. Task 1's commit would not have linted.
- **Fix:** Scoped `#[allow(dead_code)]` on exactly those three fields, with a source comment stating it exists only because the reads land in the very next commit of the same plan, and **removed in Task 2** the moment the reads existed.
- **Files modified:** `src/server/task_store.rs`
- **Verification:** `make lint` **exit 0** at `fb23ab14`, and `git grep 'allow(dead_code)' src/server/task_store.rs` returns nothing at HEAD.
- **Committed in:** `fb23ab14` (added), `c3ff793e` (removed)

**2. [Rule 1 - Bug] Two clippy `iter_on_single_items` errors `cargo build` did not surface**

- **Found during:** Task 2
- **Issue:** `["units"].into_iter().collect()` in two new assertions. `cargo build` and `cargo nextest` were both green; `make lint` failed with **2 errors** (`-D clippy::iter-on-single-items` implied by `-D warnings`).
- **Fix:** `BTreeSet::from(["units"])` / `BTreeSet::from(["city"])`.
- **Verification:** `make lint` exit 0, 0 warning/error lines.
- **Committed in:** `c3ff793e`
- **Note:** this is the **fifth consecutive plan** in this wave where `make lint` caught what a weaker command would not. It remains mandatory.

**3. [Rule 1 - Bug] A 1 ms-TTL setup race, in my own new test and in a pre-existing sibling**

- **Found during:** Task 2
- **Issue:** `cleanup_expired_drops_recorded_input_state` (new) and `cleanup_expired_drops_result` (**pre-existing**) both created a task with a **1 ms** TTL and then wrote to it. Every `InMemoryTaskStore` write goes through `Self::validate_access`, which returns `TaskStoreError::Expired` once the TTL has elapsed — so under load the setup loses to the clock and the test panics at its `unwrap()` for a reason that has nothing to do with the property it asserts.
- **Measurement, not inference:** the new test failed once in a batch run and **passed in isolation**; after hardening it, run 1 of a 5-run repeat then failed the **pre-existing** `cleanup_expired_drops_result` — directly confirming the diagnosis and that the shape, not my code, is the cause.
- **Fix:** both widened to a 500 ms TTL with the reason written at the site; asserted properties unchanged. Deliberately a `let`, **not a `const`**, so the plan's own "no new numeric constant" acceptance grep stays honest.
- **Verification:** **6 consecutive** clean runs of `-E 'test(/task_store/) or test(deliver_inputs)'` — 61/61 each.
- **Committed in:** `c3ff793e`

---

**Total deviations:** 3 auto-fixed (1 blocking, 2 bugs)
**Impact on plan:** No scope creep. Deviation 1 is a two-commit-lived mechanical necessity of the plan's own task split; 2 and 3 are correctness fixes inside the plan's declared files. Deviation 3 touched one pre-existing test in the same file, justified because a known flake in the plan's own file jeopardizes the gate this plan must leave green.

## Verification — exactly what was run

Every command below was run; nothing is claimed that was not measured.

| Check | Command | Result |
|---|---|---|
| Plan verification 1 | `cargo nextest run --features full -E 'test(/task_store/) or test(deliver_inputs)'` | **61/61 passed**, repeated **6×** clean |
| Named delivery tests | `cargo nextest run --features full -E 'test(deliver_inputs)'` | **exactly 6 tests, 6 passed** |
| Named accessor tests | `cargo nextest run --features full -E 'test(snapshot) or test(record_input_requests) or test(_error)'` | **136/136 passed** (the filter's `_error` arm sweeps in unrelated transport tests by design) |
| Plan verification 2 | `cargo test --features full --doc task_store` | **14 passed, 1 ignored** |
| Plan verification 3 | `cargo semver-checks check-release --baseline-rev 27364eb1` | **223 checks: 223 pass, 30 skip — "no semver update required"** |
| Plan verification 4 | `cargo check -p pmcp-tasks` | **exit 0**; `git diff --stat -- crates/pmcp-tasks/` **empty** |
| Plan verification 5a | `make lint` | **exit 0**, 0 warning/error lines |
| Plan verification 5b | `make wasm-build` | **exit 0**, **86 warnings** = the inherited baseline 114-03 measured, **0** naming either changed file |
| Mandated gate | `make quality-gate` | **exit 0** — **258** `test result:` lines, **all `ok.`**, **4576 passed, 0 failed** |
| `T-114-SC` | `git diff --stat 27364eb1..HEAD -- Cargo.toml Cargo.lock` | **empty — byte-unchanged, zero packages installed** |
| v1 wire lock | `cargo nextest run --features full -E 'test(/tasks/)'` | **60/60 passed**, including the whole `v1_tasks_golden` suite |

### The gate arithmetic corroborates the change

114-03's baseline was **258** result lines at **4534**. This plan adds **18** lib tests and **6** doctests. The gate counts lib tests **twice** (`test-unit` and `validate-always`) and doctests once:

```
4534 + (18 × 2) + 6 = 4576
```

Measured: **4576**. Exact, with **no new test binary** (258 lines before and after). The two inputs were verified independently: `task_store.rs` went 43 → 61 tests (**+18**), and doctest fences in that file went 8 → 20 (**+6** doctests).

### Semver was isolated, not argued

Per the quality-gate note this plan inherited, `cargo semver-checks` against the **crates.io** baseline carries a pre-existing `type_marked_deprecated` failure on `OptimizedSseTransport` from 113.1-03 (`9b33a00f`). This plan used `--baseline-rev 27364eb1` (the phase start commit) instead and reports **223/223, no update required** — which is the isolating measurement, and it also confirms that adding a **defaulted** method to a public non-sealed trait is not a breaking change.

### Additivity was proven at compile time, not by inspection

The test module's `DefaultOnlyStore` implements only the pre-114 **required** `TaskStore` methods and overrides **none** of the seven additions. Had any addition been made a required method, that block would fail with `E0046`. Its comment now says exactly that, so the guard is not deleted as dead weight. `default_impl_store_reports_inputs_unsupported` then asserts every default is **honest**: `Internal` for the three writes (explicit, never a silent success) and `NotFound` for the two reads.

## Issues Encountered

**A truncated log is not a green log — RTK corrupted two separate measurements.** Both process gotchas this plan inherited recurred, plus a new instance:

1. `make lint`'s output came back with a `[full output: ...]` marker and no verdict, so every lint run afterwards was executed as `/usr/bin/make lint > log 2>&1; echo $?` and judged on the **exit code**, never the tail.
2. **New:** `git diff … | grep -c '^-[^-]'` returned **0** through the RTK proxy while the true count was **28**. Piping `git diff` into `grep` is unreliable here; the deletion audit was redone with `/usr/bin/git`. Anyone repeating that audit must use the absolute binary path or they will "prove" an insertions-only diff that is not.
3. `make quality-gate` again exceeded a 10-minute foreground window (SIGTERM/143). It completed only when launched fully detached with an exit-code marker file and polled — the same shape 114-03 recorded, extended with the marker so the verdict survives the poll.

`git stash` was **not used at any point.**

Two findings were logged to `deferred-items.md` rather than fixed:

- **D-114-A** — `shared::streamable_http::tests::v2_error_envelope::v1_still_errors_on_the_status_alone` failed once with the macOS keychain `ioErr -36` signature at the **pre-existing** `.expect` in `src/shared/streamable_http.rs:458`. It passes in isolation and the broad filter re-ran 136/136. Critically, `df -h /` showed **19 GiB available**, so this is **NOT** the known disk-exhaustion mode that produces the identical signature — worth recording so a future bisect does not chase the wrong cause. Out of scope: this plan touched no transport code.
- **D-114-B** — the 1 ms-TTL test shape; fixed at both occurrences in this file, recorded in case it exists elsewhere.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

**Ready.** The three consumers named in the phase context can build directly on these seams:

- **114-12** calls `record_input_requests` with the store-minted id, closing the loop that `build_task_created_response` opens by minting the id inside dispatch *after* the handler returns.
- **114-14** reads `task_input_snapshot(...).kind_of(key)` for its kind-directed decode. The kinds come from the server's own record; a key the server never issued returns `None`, which is asserted.
- **114-11** reads `input_requests` for the v2 `tasks/get` inline, and `get_error` for the `failed` case that previously had no storage model at all.
- **114-05 / 114-07** consult `supports_inputs()` before serving the store path, mirroring the existing `supports_results()` consultation at `src/server/task_dispatch.rs:605`.

**Deliberately NOT done, and each for a stated reason:**

- **Nothing negotiates or dispatches yet.** `supports_inputs()` has no caller in `src/` — dispatch wiring is 114-05/114-07's. This plan opens seams only.
- **`.planning/REQUIREMENTS.md` is untouched (0-byte diff)** and `requirements mark-complete` was **deliberately not run**. TASK-02 and TASK-06 are implemented at the store/router layer but stay unflipped: `114-SPEC-RECHECK.md` flips TASK-01..06 as a **group** and only on a `PUBLISHED-CONFIRMED` D-18 landing, and `## Verdict` is still **`PENDING`**.
- **No contract YAML.** 114-20 settled contract-first as an **option-b waiver**; `contracts/` was not touched.
- **Row 23 was not worked around.** `inputRequests` is required on `InputRequiredTask` while Phase 113's `own_reserved_result_fields` silently deletes that key. This plan supplies the *storage* for it and leaves the deletion bug to **114-10**, which must land before anything depends on a v2 `tasks/get`.
- **No `tasks/update` row was added to `MRTR_METHODS`.** Inventory row 34's trap (a row there makes the method MRTR-eligible, so `splice_mrtr_params` deletes its whole payload) belongs to 114-06/114-14 and was not opened here.
- **D-113-U is still open and unowned** (`write_canonical` cognitive 26 vs. the PR-blocking PMAT cap of 25). This plan changed neither that file nor that function.

## Self-Check: PASSED

- `src/server/task_store.rs` — FOUND
- `src/server/tasks.rs` — FOUND
- `.planning/phases/114-tasks-extension-migration/114-04-SUMMARY.md` — FOUND
- `.planning/phases/114-tasks-extension-migration/deferred-items.md` — FOUND
- Commit `fb23ab14` — FOUND
- Commit `c3ff793e` — FOUND
- Commit `6855bbb0` — FOUND

---
*Phase: 114-tasks-extension-migration*
*Completed: 2026-07-28*
