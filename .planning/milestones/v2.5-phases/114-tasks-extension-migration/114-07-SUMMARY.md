---
phase: 114-tasks-extension-migration
plan: 07
subsystem: task-storage
tags: [tasks-extension, task-store, generic-store, cas, put_if_version, dynamodb, redis, input-delivery, serde-compat, owner-isolation]

# Dependency graph
requires:
  - phase: 114-04
    provides: "pmcp's five defaulted TaskStore seam methods, TaskInputDelivery / TaskInputSnapshot, and the defaulted TaskRouter::handle_tasks_update this plan overrides"
  - phase: 114-01
    provides: "the vendored ext-tasks schema that names taskId / inputResponses / inputRequests"
provides:
  - "GenericTaskStore<B>::deliver_inputs — ONE domain implementation of tasks/update input delivery, shared by the in-memory, DynamoDB and Redis backends"
  - "Four more Value-seam accessors on GenericTaskStore<B>: task_input_snapshot, record_input_requests, set_error, get_error"
  - "All three D-13 delegation sites live: GenericTaskStore, the blanket TaskStore impl, and the InMemoryTaskStore wrapper"
  - "TaskRouterImpl::handle_tasks_update — the router half, reading the owner from the ARGUMENT only"
  - "TaskRecord's three additive #[serde(default)] + skip_serializing_if fields, plus #[non_exhaustive]"
  - "A structural delegation tripwire that fails when a trait method is added without a forwarding line"
  - "D-114-D (eventual-consistency read obligation) and D-114-E (pre-existing red make test-feature-flags) recorded"
affects: [114-14, 114-11, 114-10, 114-09, 118]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Domain logic once in GenericTaskStore<B>; backends stay dumb KV stores"
    - "One put_if_version CAS per operation; no internal retry, no mutex around read-then-write"
    - "The shared transition validator IS the awaiting-input predicate — validate_transition(.., Working) is legal from InputRequired alone"
    - "Cross-crate policy sharing below a serde_json::Value seam via a pure key-set function (partition_input_delivery)"
    - "Additive persisted fields as #[serde(default)] + skip_serializing_if, pinned by a RAW BYTE fixture rather than a struct round trip"

key-files:
  created:
    - crates/pmcp-tasks/tests/input_delivery.rs
  modified:
    - crates/pmcp-tasks/src/store/mod.rs
    - crates/pmcp-tasks/src/store/generic.rs
    - crates/pmcp-tasks/src/store/memory.rs
    - crates/pmcp-tasks/src/router.rs
    - crates/pmcp-tasks/src/domain/record.rs
    - .planning/phases/114-tasks-extension-migration/deferred-items.md

key-decisions:
  - "The accept/ignore/complete partition is NO LONGER in generic.rs — it was extracted post-commit to pmcp::server::task_store::partition_input_delivery and is now shared with pmcp's in-crate InMemoryTaskStore"
  - "TaskRecord took the #[non_exhaustive] route (Task 1d), not the constructor-test fallback"
  - "GenericTaskStore ALLOWS multiple input-request rounds (merge, refuse a reused key); pmcp's in-crate store records exactly one — a deliberate, documented divergence"
  - "The plan's CasConflictBackend reuse was infeasible (it lives in a #[cfg(test)] mod, unreachable from an integration test); a behaviourally identical local ArmedConflictBackend replaces it"
  - "make test-feature-flags is RED and was PROVEN red at the plan's base commit — logged as D-114-E, not fixed"
  - ".planning/REQUIREMENTS.md left untouched; requirements mark-complete deliberately NOT run (D-18 verdict still PENDING)"

patterns-established:
  - "Build-time delegation tripwire: include_str! the trait, the blanket impl and the wrapper, then fail when a trait method has no forwarding line"
  - "Two orthogonal concurrency doubles: an always-conflict double proves conflict PROPAGATION, a barrier-backed double proves first-writer-WINS"
  - "A staleness double must keep stale-value / stale-absence / converged distinct; collapsing the middle case makes the test pass for the wrong reason"

requirements-completed: []  # TASK-02 and TASK-06 are IMPLEMENTED but stay [~] — see "Requirements" below

# Metrics
duration: 21min implementation + 47min close-out verification
completed: 2026-07-28
---

# Phase 114 Plan 07: `GenericTaskStore<B>` input delivery Summary

**`tasks/update` input delivery is implemented ONCE, over `StorageBackend::put_if_version`, so the memory, DynamoDB and Redis backends gain it from the same code — and the accept/ignore/complete policy has since been lifted OUT of that implementation into a `pmcp` function the in-crate store calls too, so the two sides of the `Value` seam can no longer drift.**

## Close-out note: what this SUMMARY is

The three task commits landed and the executor was interrupted before writing this file. This SUMMARY was produced by a **fresh agent** that verified the landed work against the plan rather than re-implementing it, ran the two negative controls the plan requires but that were never recorded, and re-ran the full verification suite.

**The code moved after 114-07's commits.** A separate `/simplify` cleanup pass (4 commits) edited 114-07's files. Everything below describes the tree **as it stands at `9081be3b`**, not as 114-07 left it. The differences are called out explicitly wherever they matter.

## Performance

- **Duration:** 21 min implementation (2026-07-28 07:16:27 → 07:37:20 PDT), plus a 47-min close-out verification pass
- **Tasks:** 3 of 3
- **Files modified:** 6 source/planning files + 1 new test file (114-07's own commits); 3 further files touched by the cleanup pass

## Task Commits

1. **Task 1: defaulted trait method + the `GenericTaskStore` CAS implementation** — `36bc031a` (feat)
2. **Task 2: the delegating wrapper (D-13 site 2) and the router override** — `2b7291e7` (feat)
3. **Task 3: input-delivery tests, pre-114 byte fixture, and the delegation tripwire** — `ba432812` (test)

**Post-commit cleanup pass (NOT part of 114-07, but it reshaped 114-07's surface):**

- `478185d6` (test) — hash vendored schema digests in-process with `sha2`
- `21084a2d` (refactor) — single-clone `project_capabilities_for_v1`
- `80f87c85` (feat) — make the cross-crate JSON mirror **enforced**, not aspirational
- `9081be3b` (refactor) — **extract the shared input-delivery partition and finish the `generic.rs` helper lift**

## THE THING A FUTURE READER MUST NOT GET WRONG

**`generic.rs` no longer holds its own copy of the accept/ignore/complete partition.**

114-07 wrote that decision procedure inline inside `GenericTaskStore::deliver_inputs`. Commit `9081be3b` **extracted it** to a new public pure function:

```
pmcp::server::task_store::partition_input_delivery
    (src/server/task_store.rs:313)
```

It has **exactly two callers** in the tree — verified by grep, not assumed:

| Caller | File:line |
|--------|-----------|
| `InMemoryTaskStore::deliver_task_inputs` (pmcp, typed) | `src/server/task_store.rs:1279` |
| `GenericTaskStore::deliver_inputs` (pmcp-tasks, `Value`-shaped) | `crates/pmcp-tasks/src/store/generic.rs:683` |

The extraction was possible with **no manifest change**: `pmcp::server` is ungated, `task_store` is gated only on non-wasm, and `pmcp-tasks` already depends on `pmcp` with `default-features = false`. `Cargo.toml`/`Cargo.lock` are byte-unchanged across the whole range.

**What the shared function decides, and what it does NOT:**

- It reads **key sets only, never values**. That is the property that lets a typed `InputResponses` store and a `serde_json::Map` store share it.
- It decides `accepted` (outstanding AND not already answered), `ignored` (everything else), and `complete` (`!outstanding.is_empty() && every outstanding key is accepted-or-already-answered`).
- It **mutates nothing and persists nothing**. Each store persists the accepted values itself.

**The deliberate per-store differences were PRESERVED, not unified:**

| Concern | `pmcp` in-crate store | `pmcp-tasks` generic store |
|---------|----------------------|---------------------------|
| Awaiting-input check | `can_transition_to(&Working)` + explicit `InvalidTransition` | `validate_transition(task_id, &Working)?` |
| Timestamp | inline `chrono::Utc::now().to_rfc3339()` | `Self::touch` (millisecond-precision `to_rfc3339_opts`) |
| Persistence | `DashMap` guard held across the mutation | `put_if_version` CAS |
| Resume guard | `complete && !accepted.is_empty()` | identical |

The second thing `9081be3b` did: **it finished the helper lift.** 114-07 introduced `check_owner` / `check_not_expired` / `touch` but wired them into only the 3 newest methods, leaving the identical blocks hand-inlined in `get`, `update_status`, `set_variables`, `set_result` and `complete_with_result`. Those are now migrated. At HEAD:

- `Self::check_owner(` — **8** call sites
- `Self::check_not_expired(` — **7** call sites
- `Self::touch(` — **7** call sites
- remaining inline `record.owner_id != owner_id` blocks — **0**

So the security-critical "return `NotFound`, never reveal another owner's task" rule now has ONE implementation in this file instead of six copies whose `warn!` text had already begun to differ. The helpers moved above the v2 section banner, since they are general record guards rather than input-delivery ones.

Third: `TaskInputDelivery` and `TaskInputSnapshot` gained `Serialize, Deserialize` with `#[serde(rename_all = "camelCase")]` (`80f87c85`), and a test in `crates/pmcp-tasks/tests/input_delivery.rs` now asserts the derived key sets equal the key sets the generic store's hand-written `json!` literals actually emit. That test passes against the **unchanged** literals, which is the proof the derives are byte-identical to what was already on the wire. Before this, `TaskInputSnapshot`'s snake_case field names would not even have produced the literals' `inputRequests` / `inputResponses` keys — the "mirrors pmcp field for field" claim in the trait rustdoc was something a reader had to check by eye.

## Per-Criterion Verdicts

### Task 1 — defaulted trait method + the `GenericTaskStore` CAS implementation

| # | Criterion | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | `store/mod.rs` has `async fn deliver_inputs` with a defaulted body returning `TaskError::StoreError` | **MET** | `crates/pmcp-tasks/src/store/mod.rs:472-481`, message `"store does not support task input delivery"` |
| 2 | The `generic.rs` impl calls `check_anonymous_access`, `make_key`, `put_if_version` and the existing transition validator | **MET** | lines `642`, `645`, `715`, `668` respectively, all inside `deliver_inputs` |
| 3 | Exactly ONE `put_if_version` in the new fn body | **MET** | `git show HEAD:…/generic.rs \| sed -n '636,724p' \| grep -c put_if_version` → **1** |
| 4 | No `Mutex`/`RwLock` introduced in `generic.rs` | **MET** | `git diff 4327b246..HEAD -- …/generic.rs \| grep -cE '^\+.*(Mutex\|RwLock)'` → **0** |
| 5 | The new record fields carry `#[serde(default)]` | **MET** | `domain/record.rs:93,101,110` — each is `#[serde(default, skip_serializing_if = "Option::is_none")]` |
| 6 | The wrong-owner branch returns `NotFound`, and the rendered message names neither `owner` nor the other id | **MET** | `Self::check_owner` returns `TaskError::NotFound`; the owner identity travels only in a `tracing::warn!`, never in the rendered error. Asserted by `deliver_inputs_for_another_owner_is_not_found` (`!rendered.contains("owner")` AND `!rendered.contains("owner-a")`) |
| 7 | `cargo nextest run -p pmcp-tasks` exits 0 with all pre-existing tests passing; `git diff --stat -- crates/pmcp-tasks/tests/` shows only the NEW file | **MET (count corrected)** | exit **0**, **514 run / 514 passed**. Tests diff: `input_delivery.rs \| 1472 +` — **one file, zero deletions**. See "The plan's test count was stale" below |

**Task 1(c) — the accessors above the seam:** all present and each is one CAS or one owner-scoped read — `task_input_snapshot` (`generic.rs:736`), `record_input_requests` (`784`), `set_error` (`854`), `get_error` (`890`). None re-derives the owner from params.

**Task 1(d) — the source-compatibility fence. Route taken: `#[non_exhaustive]`.** `TaskRecord` carries `#[non_exhaustive]` at `domain/record.rs:54`, with the reason in its own rustdoc (lines 44-53): the type is PUBLIC in this crate, root-crate `cargo semver-checks` does not cover this unpublished crate, and `#[serde(default)]` preserves persisted-DATA compatibility but not downstream Rust struct literals. The constructor-test fallback the plan offered was **not** needed — in-crate construction goes through `TaskRecord::new`.

**The plan's test count was stale.** The plan says "all 197 pre-existing tests passing". The measured pre-existing count is **485** (514 total minus this plan's 29). Per-binary at HEAD: lib 288, `state_machine` 46, `protocol_types` 36, `store_tests` 34, `context_tests` 32, **`input_delivery` 29**, `security_tests` 19, `property_tests` 13, `lifecycle_integration` 11, `workflow_integration` 6. `cargo test -p pmcp-tasks` (which additionally runs doctests) reports **590 passed**. Zero `#[test]`/`#[tokio::test]` attributes were added or removed anywhere under `crates/pmcp-tasks/src` across the range, so all 29 new tests live in the new integration file and the 485 are provably unmodified.

### Task 2 — the delegating wrapper (D-13 site 2) and the router override

| # | Criterion | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | `memory.rs` has a `deliver_inputs` line delegating to `self.inner` | **MET** | `store/memory.rs:436-445`; all five methods delegate (`436`, `447`, `451`, `462`, `471`) |
| 2 | `router.rs` has an `async fn handle_tasks_update` impl | **MET** | `router.rs:329-341` |
| 3 | `resolve_owner_id` count inside the new fn body is 0 | **MET** | `git show HEAD:…/router.rs \| sed -n '329,341p' \| grep -c resolve_owner_id` → **0**. The identifier appears only in the rustdoc above the fn (explaining why it is not used) and in the unrelated `resolve_owner` method |
| 4 | No `params.get("owner` / `ownerId` access | **MET** | `TaskUpdateParams` (`router.rs:111-119`) declares **only** `task_id` and `input_responses`; there is no other read of `params`. Proven live by `the_router_ignores_an_owner_supplied_in_params`, which sends BOTH `"ownerId"` and `"owner"` in the body and asserts the call is refused |
| 5 | `cargo check -p pmcp-tasks` and `--no-default-features` both exit 0 | **MET** | both exit **0** |

### Task 3 — tests, pre-114 byte fixture, and the delegation tripwire

| # | Criterion | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | `cargo nextest run -p pmcp-tasks -E 'test(/input_delivery/)'` exits 0 with ≥12 tests | **PLAN-TEXT DEFECT — the underlying property is MET** | As written: exit **4**, `Starting 0 tests across 10 binaries (514 tests skipped)`, `error: no tests to run`. See below |
| 2 | `cargo nextest run -p pmcp-tasks` exits 0 (all pre-existing tests still pass) | **MET** | exit **0**, 514/514 |
| 3 | `git diff --stat -- crates/pmcp-tasks/tests/` lists ONLY `input_delivery.rs` | **MET** | `1 file changed, 1472 insertions(+)`, zero deletions |
| 4 | Test 7 uses a raw literal, not a struct round trip: `grep -c 'serde_json::to_string'` inside it is 0 | **MET** | `a_pre_114_record_still_deserializes` spans lines 938-982 and contains no `serde_json::to_string`. `PRE_114_RECORD` is a `const &str` raw JSON literal at line 936. (Line 989's `to_string` belongs to the *separate* test `an_untouched_record_does_not_grow_on_the_wire`.) |
| 5 | Test 9 reads `memory.rs` and `mod.rs` at runtime (`include_str!` or `read_to_string` present) | **MET** | `include_str!("../src/store/mod.rs")` and `include_str!("../src/store/memory.rs")` at lines 1054-1055 |
| 6 | `make test-feature-flags` exits 0 | **UNMET — proven PRE-EXISTING, deferred as D-114-E** | See "The one unmet criterion" below |
| 7 | Negative control recorded: deleting the `deliver_inputs` delegation makes test 9 FAIL naming that method | **MET (run during this close-out)** | See "Negative Controls" |
| 8 | Negative control recorded: replacing `put_if_version` with an unconditional `put` makes test 5 FAIL | **MET (run during this close-out)** | See "Negative Controls" |

**Criterion 1, the nextest selector defect — same class 114-01 recorded.** nextest's `test()` predicate matches test **NAMES**, not binary names, and no test in this file has `input_delivery` in its name. The plan's command therefore selects nothing and exits 4. The correct selector is:

```
cargo nextest run -p pmcp-tasks -E 'binary_id(pmcp-tasks::input_delivery)'
   → exit 0, 29 tests run: 29 passed, 0 skipped
```

29 ≥ 12, so the criterion's substance holds. **This is the third plan in this phase to hit this trap; the `-E 'test(/…/)'` form should not be written into a plan again unless the test names really do carry the token.**

All 12 test groups the plan enumerated exist and pass (29 tests total):

| Plan item | Test(s) |
|-----------|---------|
| 1 complete set → Working | `deliver_inputs_completing_the_set_transitions_to_working` |
| 2 partial stays `input_required` | `deliver_inputs_partial_set_stays_input_required` |
| 3 ignore semantics | `deliver_inputs_ignores_keys_that_are_not_outstanding`, `deliver_inputs_ignores_an_already_answered_key` |
| 4 terminal refusal, one per status | `…_on_a_completed_task_is_refused`, `…_on_a_failed_task_is_refused`, `…_on_a_cancelled_task_is_refused` (plus a bonus `…_on_a_working_task_is_refused`) |
| 5 CAS conflict | `concurrent_deliver_inputs_first_writer_wins` |
| 6 cross-owner | `deliver_inputs_for_another_owner_is_not_found`, `deliver_inputs_for_a_record_whose_owner_disagrees_is_not_found` |
| 6b backend contract | `a_created_task_is_immediately_readable_from_its_returned_handle`, `two_writers_first_writer_wins`, `a_full_input_response_set_fits_the_dynamodb_item_budget`, `partial_updates_do_not_amplify_writes_superlinearly` |
| 7 pre-114 bytes | `a_pre_114_record_still_deserializes` (+ `an_untouched_record_does_not_grow_on_the_wire`) |
| 8 anonymous owners | `anonymous_owner_is_refused_by_default_on_this_backend` |
| 9 delegation tripwire | `every_generic_store_method_is_delegated_by_the_memory_wrapper` (+ runtime twins `the_memory_wrapper_delivers_inputs_through_a_trait_object`, `the_router_delivers_inputs_across_the_value_seam`, `the_router_ignores_an_owner_supplied_in_params`) |
| — (cleanup pass) | `the_emitted_json_mirrors_pmcps_typed_shapes_key_for_key` |
| — (accessors) | `record_input_requests_*` ×3, `set_error_then_get_error_round_trips_verbatim`, `task_input_snapshot_reports_requests_responses_and_status`, `deliver_inputs_refuses_a_non_object_payload` |

### Plan-level `<verification>` block

| Check | Verdict | Exit |
|-------|---------|------|
| `cargo nextest run -p pmcp-tasks` exits 0; tests diff shows only the new file | **MET** | 0 |
| `make test-feature-flags` exits 0 | **UNMET (pre-existing, D-114-E)** | 2 |
| `cargo check -p pmcp-tasks --no-default-features` | **MET** | 0 |
| `cargo check -p pmcp-tasks --features dynamodb` | **MET** | 0 |
| `cargo check -p pmcp-tasks --features redis` | **MET** | 0 |
| `cargo check -p pmcp-tasks --features dynamodb,redis` | **MET** | 0 |
| `make lint` exits 0 | **MET** | 0 |
| `git diff --stat -- Cargo.lock crates/pmcp-tasks/Cargo.toml` is empty | **MET** | empty |

### Plan-level `<success_criteria>`

- **One domain implementation, one delegation, one router override — all three D-13 sites** — MET, and each is proven *live* (not just by grep): the trait-object test, the router test, and the structural tripwire.
- **CAS atomicity and partial-vs-complete semantics proven, each with a recorded negative control** — MET; both controls run and recorded below.
- **Pre-114 durable records proven readable from raw bytes** — MET, and the test goes further than the plan asked: the pre-114 record is proven still **OPERABLE** (a delivery against it is accepted as a call, ignores every key, and leaves the task paused), not merely deserializable.
- **The pmcp-tasks suite green with zero modifications to it** — MET (485 pre-existing tests, zero test attributes added or removed under `crates/pmcp-tasks/src`, zero deletions in `crates/pmcp-tasks/tests/`).

## The one unmet criterion: `make test-feature-flags`

`make test-feature-flags` exits **2**. It fails in row **1/4**, at its second sub-command:

```
cargo clippy -p pmcp-tasks --no-default-features -- -D warnings   → exit 101
```

56 `dead_code` warnings in the **root `pmcp` lib** are promoted to errors by that `-D warnings`. Building `pmcp` through `-p pmcp-tasks --no-default-features` selects a reduced feature set in which those items have no caller.

**This was proven pre-existing, not argued.** A detached worktree was created at 114-07's base commit `4327b246` with its own `CARGO_TARGET_DIR`, and the same commands were run there:

| Command | base `4327b246` | HEAD `9081be3b` |
|---------|-----------------|-----------------|
| `make test-feature-flags` | exit **2**, 56 errors | exit **2**, 56 errors |
| `cargo clippy -p pmcp-tasks --no-default-features -- -D warnings` | exit **101** | exit **101** |

Same exit codes, same error count, same per-file distribution — `src/types/mrtr.rs` 42, `src/server/subscriptions.rs` 7, `src/server/core.rs` 4, `src/shared/sse_parser.rs` 2, `src/server/mod.rs` 1. **Zero** in `crates/pmcp-tasks/`, **zero** in `src/server/task_store.rs`. The named items (`write_canonical`, `salient_params`, `project_capabilities_for_v2`, `EXPERIMENTAL_TASKS_KEY`, …) belong to Phase 113 and 114-05, both of which landed before 114-07. The probe worktree and its target directory were removed afterwards.

The rows the plan's own `<verification>` singles out as load-bearing — "the `cargo check -p pmcp-tasks --features X` rows are the dev-dep-free ones" — are all **exit 0**. What is red is the root crate's dead-code hygiene under a reduced feature set, which lands in five files owned by other plans.

Recorded as **D-114-E** in `deferred-items.md` with the full measurement, per the scope-boundary rule. It is **not** caught by `make quality-gate` or CI, which lint the root crate with its own allow-list — which is why it has gone unnoticed since Phase 113.

## Negative Controls (run during this close-out, both reverted)

Both files were backed up before editing and `shasum -a 256 -c` confirmed byte-identity after reverting. **`git stash` was not used at any point.**

**NC-1 — delete the `deliver_inputs` delegation from `memory.rs`.** Compiles cleanly (that is the whole point of the tripwire). Result: **26 of 29 passed, 3 FAILED.**

```
FAIL every_generic_store_method_is_delegated_by_the_memory_wrapper
  → "InMemoryTaskStore does not delegate `deliver_inputs` -- it would silently
     inherit the trait default while GenericTaskStore's implementation works"
FAIL the_memory_wrapper_delivers_inputs_through_a_trait_object
FAIL the_router_delivers_inputs_across_the_value_seam
  → Internal("store error: store does not support task input delivery")
```

The tripwire fails **naming the missing method**, exactly as the criterion requires. The other two failures are the runtime twins and are the failure mode the plan predicted verbatim: the router reaches a memory-backed store and is told the store does not support input delivery, while the identical DynamoDB/Redis path would have worked. 26 unrelated tests stayed green, so the control is attributable rather than indiscriminate.

**NC-2 — replace `put_if_version` with an unconditional `put` in `deliver_inputs`.** Result (with `--no-fail-fast`): **27 of 29 passed, 2 FAILED**, and the two are exactly the two concurrency tests:

```
FAIL concurrent_deliver_inputs_first_writer_wins
  → "a version conflict must propagate, got: Ok({accepted:["city"], ignored:[], complete:true})"
FAIL two_writers_first_writer_wins
  → "exactly one writer must land: [Ok({accepted:["city"],…}), Ok({accepted:["country"],…})]  left: 2  right: 1"
```

**This is the pair that makes the CAS claim load-bearing rather than decorative.** The first double is always-conflicting: it proves the conflict PROPAGATES, but it can never prove a first writer *wins*, because it never lets one win. The second is barrier-backed — both writers read the same version, then a barrier releases both writes — so it proves that of two genuine racers exactly one lands, the other is TOLD, and the landed record is intact and unmixed. Under the unconditional `put`, both writers "succeed" and one silently overwrites the other: the exact lost-update bug on DynamoDB/Redis that a process-local mutex could not have prevented either.

## The store contract 114-14 must dispatch against

114-14 implements the `tasks/update` DISPATCH half above the seam. Stated explicitly so it does not have to be reverse-engineered:

**What `partition_input_delivery` decides (shared, both stores):**

| Input condition | Outcome |
|-----------------|---------|
| key is outstanding AND unanswered | → `accepted` |
| key was never issued | → `ignored` (NOT an error) |
| key was already answered | → `ignored` — a delivered response can never be replayed over |
| key was superseded | → `ignored` |
| every outstanding key is accepted-or-already-answered, and `outstanding` is non-empty | `complete = true` |
| `outstanding` is empty | `complete = false` — vacuous completeness cannot resume a task |

**What each store decides for itself:**

- **Persistence.** Only `accepted` values are written. Ignored values are reported back and DROPPED.
- **Resume.** `InputRequired → Working` happens **only if `complete && !accepted.is_empty()`**. A delivery that changed nothing cannot resume a paused task. A PARTIAL delivery persists and the task **STAYS `input_required`**.
- **Atomicity.** The atomic unit is *(persist accepted [+ transition iff complete])*. In `GenericTaskStore` both land in ONE `put_if_version`; a task is never observable as resumed with its answers missing, nor answered while still paused.

**Error contract — what the store returns and what the router maps it to:**

| Condition | `TaskError` | JSON-RPC via `task_error_to_pmcp` |
|-----------|-------------|-----------------------------------|
| **Unknown kind / unknown key** | *none* — `deliver_inputs` **never inspects kinds or values**, only key sets. The key is `ignored` and reported | success (`-` ) |
| **Terminal task** (`completed` / `failed` / `cancelled`) — and also a still-`working` task | `InvalidTransition` | `-32602` invalid params |
| Task absent, or owned by someone else | `NotFound` | `-32602` invalid params |
| TTL elapsed | `Expired` | `-32602` invalid params |
| Another writer landed first | `ConcurrentModification` | `-32603` internal |
| **Bounds violation** | *none* — bounds are **NOT enforced here**, by design | n/a |
| `responses` is not a JSON object (or `null`) | `null` → empty; anything else → `StoreError` naming only the TYPE, never echoing the payload | `-32603` internal |
| Anonymous owner (`""` or `"local"`) while `allow_anonymous` is false | `StoreError("anonymous access …")` | `-32603` internal |

**Kind decoding is 114-14's job, above the seam.** The store's `task_input_snapshot` returns `{inputRequests, inputResponses, status}`; `TaskInputSnapshot::kind_of(key)` resolves a key to its `InputRequestKind` **from the server's own record**, and returns `None` for a key the server never issued. That is the only trustworthy kinds source — a client must never get to choose how its own answer is typed.

**Bounds: FOUR, not five.** Entry COUNT, ONE entry's serialized size, TOTAL serialized size, and one entry's nesting DEPTH — all enforced at request ingress before any decode, so an oversized payload never reaches a store. The fifth adjacent bound, `MAX_REQUEST_STATE_LEN`, does **not** apply: a `tasks/update` carries no MRTR continuation token. This division is written into the rustdoc at BOTH ends so neither side can assume the other checked.

**A divergence 114-14 should know about:** `GenericTaskStore::record_input_requests` permits **multiple rounds** — new keys are MERGED, and a key that is already recorded is REFUSED rather than overwritten (overwriting would orphan or erase a response already delivered against it). `pmcp`'s in-crate `InMemoryTaskStore` records exactly **one** round and refuses a second outright. Both are documented at their sites; the generic store is the production one, where multi-round elicitation is ordinary.

## Deviations from Plan

### 1. [Rule 3 — blocking] `CasConflictBackend` was unreachable; a local double replaced it

- **Found during:** Task 3
- **Issue:** The plan instructed reuse of `generic.rs`'s existing `CasConflictBackend` (line 1228). It lives inside `#[cfg(test)] mod tests`, which an integration test — a separate crate — cannot reach.
- **Fix:** A behaviourally identical local `ArmedConflictBackend`, with an arming switch so the fixture can be BUILT before the conflicts start (the original always conflicts, which would have made `paused_task` setup impossible). The reason is written into the double's own rustdoc at the site.
- **Commit:** `ba432812`

### 2. [Rule 2 — missing critical proof] The always-conflict double cannot prove "first writer wins"

- **Found during:** Task 3
- **Issue:** The plan's test 5 name claims *first-writer-wins*; an always-conflict double can only prove conflict PROPAGATION, because it never lets a writer win. (The plan's own item 6b noticed this and asked for the barrier test; both are present.)
- **Fix:** `two_writers_first_writer_wins` uses a `BarrierBackend` that holds every armed `put_if_version` at a `tokio::sync::Barrier`, guaranteeing both writers READ the same version before either WRITES. It asserts one winner, one `ConcurrentModification`, version advanced by exactly 1, and the landed record intact and unmixed.
- **Commit:** `ba432812`

### 3. [Rule 3 — beyond declared `files_modified`] `crates/pmcp-tasks/src/domain/record.rs`

- **Issue:** The plan declared 5 files; `TaskRecord` lives in a 6th. Adding the three `#[serde(default)]` fields and `#[non_exhaustive]` is unavoidable there.
- **Commit:** `36bc031a` (+51/-1)

### 4. [plan-text defect] The nextest selector in Task 3's `<verify>` and acceptance criterion selects zero tests

- Recorded above. Not an implementation gap; the corrected selector passes 29/29.

### 5. [plan-text defect] "197 pre-existing tests" is stale

- Measured: **485**. Recorded above.

### 6. [scope boundary] `make test-feature-flags` red

- Proven pre-existing at the base commit; logged as **D-114-E** rather than fixed. Recorded above.

### Post-commit reshaping by the `/simplify` cleanup pass

Not deviations by 114-07, but they changed 114-07's surface and are therefore part of what this SUMMARY describes: the partition extraction (`9081be3b`), the completion of the helper lift over 13 pre-existing inline copies (`9081be3b`), and the enforced cross-crate JSON mirror (`80f87c85`). All three are described in detail above.

## Measurements Worth Carrying

- **Worst-case DynamoDB item, measured not estimated.** A record carrying a maximum-legal delivery (64 entries, 260,481 B of `inputResponses` — 99.4% of the 262,144 B ingress bound, so it is genuinely a worst case and the test asserts both halves of that) plus task metadata, variables, the full outstanding request set, a terminal result and an error object serializes to **269,948 B** against DynamoDB's **409,600 B** item limit. **139,652 B of headroom (34%).** The test `println!`s the numbers so the measurement is reproducible with `--nocapture` rather than only quotable from a document.
- **Write amplification is bounded by the write COUNT, not by a byte estimate.** `partial_updates_do_not_amplify_writes_superlinearly` drives 8 partial deliveries through a `CountingBackend` and asserts exactly ONE write per delivery — no read-modify-write loop, no retry storm — which is the sharp form of the guarantee on a pay-per-write backend.
- **A staleness double has a trap inside it.** The eventual-consistency double first stored the previous value as `Option<(bytes, version)>` and `.flatten()`ed the lookup, collapsing `Some(None)` ("the key had no value before this write") into "no staleness recorded". The test PASSED for the wrong reason. A staleness double must keep **stale value / stale absence / converged** distinct. Recorded in full at D-114-D.
- **PMAT is at the inherited 4 violations, 0 in `src/`, 0 in `crates/pmcp-tasks/`.** All four are in `crates/**/tests/` (`mcp-tester/tests/property_tests.rs` ×2, `pmcp-server-toolkit/tests/sql_server_http_example.rs`, `pmcp-agent/tests/http_sources_mock.rs`) — byte-for-byte the set 114-06 recorded. A 1472-line test file and a 491-line store change introduced **no** new violation. Query the JSON at `summary.violations`; the top-level path returns "0 violations" **vacuously**.

## Verification (re-run in full during this close-out — verbatim exit codes)

| Command | Exit | Result |
|---------|------|--------|
| `make quality-gate` | **0** | 264 `test result:` lines, **4650 passed, 0 failed**, 80 ignored, **0 truncation markers**, 0 non-`ok.` result lines |
| `cargo test -p pmcp-tasks` | **0** | **590 passed** (11 suites) |
| `cargo clippy -p pmcp-tasks --all-targets -- -D warnings` | **0** | no issues |
| `cargo nextest run -p pmcp-tasks` | **0** | 514 run, 514 passed |
| `cargo nextest run -p pmcp-tasks -E 'binary_id(pmcp-tasks::input_delivery)'` | **0** | 29 run, 29 passed |
| `cargo nextest run -p pmcp-tasks -E 'test(/input_delivery/)'` | **4** | plan-text defect: 0 tests selected — see above |
| `cargo clippy -p pmcp-tasks --no-default-features -- -D warnings` | **101** | **pre-existing**, identical at base — D-114-E |
| `make test-feature-flags` | **2** | **pre-existing**, identical at base — D-114-E |
| `make lint` | **0** | no lint issues |
| `cargo fmt --all -- --check` | **0** | — |
| `cargo check -p pmcp-tasks` | **0** | — |
| `cargo check -p pmcp-tasks --no-default-features` | **0** | — |
| `cargo check -p pmcp-tasks --features dynamodb` | **0** | — |
| `cargo check -p pmcp-tasks --features redis` | **0** | — |
| `cargo check -p pmcp-tasks --features dynamodb,redis` | **0** | — |
| `cargo semver-checks check-release --baseline-rev 4327b246` | **0** | **223/223 pass, 30 skip — "no semver update required"** |
| `pmat analyze complexity --max-cognitive 25` | — | 4 violations, all in `crates/**/tests/`, **0 in `src/`** — the inherited set |

**The gate arithmetic reconciles exactly.** 114-06 recorded **264 result lines / 4649 passed**. This run: **264 / 4650**. The delta is **+1**, and it is attributable to a single line in the log — `test src/server/task_store.rs - server::task_store::partition_input_delivery (line 294) ... ok`, the doctest on the function the cleanup pass extracted. The result-line count is unchanged because a doctest adds a row inside the existing doctest binary's tally rather than a new binary.

**`make quality-gate` does NOT cover `crates/pmcp-tasks`.** Its `test-all` / `validate-always` legs run `cargo test --lib/--doc/--test` against the **root** package only. The pmcp-tasks suite is covered separately by `cargo test -p pmcp-tasks` (590) and `cargo nextest run -p pmcp-tasks` (514); the 76-test difference is doctests, which nextest does not run. **A green gate is not evidence that this plan's tests pass** — run the crate suite explicitly.

**Process notes.** `make quality-gate` again exceeded a 10-minute foreground window and completed only when launched fully detached with an exit-marker file. The RTK shell proxy rewrites `cargo test` output into a one-line summary and can truncate redirected logs, so **only exit codes are authoritative**; every `git` measurement here used `/usr/bin/git`. `df -h /` showed **121 GiB free** throughout, so none of this is the known disk-exhaustion failure mode.

## Constraints Honoured

- **No `tasks/*` row added to `MRTR_METHODS`** — this plan does not touch that table at all.
- **Row 23 left alone** — `own_reserved_result_fields`' silent deletion of `inputRequests` remains **114-10's**.
- **`.planning/REQUIREMENTS.md` untouched (0-byte diff across the whole range).** TASK-02 and TASK-06 are IMPLEMENTED at the store and router layer but stay `[~]`, and `requirements mark-complete` was deliberately **NOT** run: `114-SPEC-RECHECK.md` flips TASK-01..06 as a GROUP and only on a `PUBLISHED-CONFIRMED` D-18 landing, and `## Verdict` is still **`PENDING`**.
- **No contract YAML authored and `contracts/` untouched** (114-20's owner waiver).
- **`Cargo.toml` / `Cargo.lock` byte-unchanged; zero packages installed** (T-114-SC).
- **No `git stash` at any point**; no `git clean`; no `--no-verify`.
- Only this plan's own files were staged. The pre-existing unrelated modifications (`.pmat/*`, `pmcp-course/*`, untracked `.agents/`, `.codex/`, `.serena/`, `.superpowers/`, `AGENTS.md`, `docs/design/*`) were left untouched.

## Threat Model Coverage

| Threat ID | Disposition | Status |
|-----------|-------------|--------|
| T-114-25 lost update on concurrent `tasks/update` | mitigate | **DONE** — one `put_if_version`, no retry, no mutex; proven by two orthogonal doubles plus NC-2 |
| T-114-26 cross-owner task feed | mitigate | **DONE** — `make_key` isolation + `check_owner` → `NotFound`; message asserted to name neither ownership nor the other id; the defence-in-depth branch reached by planting bytes |
| T-114-27 feeding a terminal task | mitigate | **DONE** — refused via the shared 46-transition state machine; one test per terminal status plus `working` |
| T-114-28 replay of an answered key | mitigate | **DONE** — already-answered keys are IGNORED and reported, never re-accepted |
| T-114-29 unbounded `responses` | transfer | **TRANSFERRED to 114-14** — recorded in the trait rustdoc at both ends, with the four-not-five count stated |
| T-114-30 pre-114 record read failure | mitigate | **DONE** — `#[serde(default)]` absent-means-empty, pinned by a RAW BYTE fixture and proven still operable |
| T-114-31 owner re-derived from client params | mitigate | **DONE** — `handle_tasks_update` reads `owner_id` from the parameter only; `resolve_owner_id` count 0 in the body; proven by a test that plants `ownerId` AND `owner` in the payload |
| T-114-SC package installs | accept | **HELD** — `Cargo.toml`/`Cargo.lock` byte-unchanged, zero packages installed |

No new security surface was introduced beyond the threat register.

## Known Stubs

None. `supports_inputs()` still has no caller in root `src/` — that is by design; this plan dispatches nothing. 114-14 owns the dispatch half.

## Next

- **114-14** — the `tasks/update` DISPATCH half, against the contract stated above. It owns the four ingress bounds (T-114-29) and the kind-directed decode through `TaskInputSnapshot::kind_of`.
- **114-10** — inventory row 23, before anything depends on a v2 `tasks/get`.
- **114-11** — inlining `inputRequests` / the persisted error on a v2 `tasks/get`; `task_input_snapshot` and `get_error` are the accessors it needs.
- **Open deferrals:** D-114-A, D-114-B, D-114-C, **D-114-D** (eventual-consistency read obligation), **D-114-E** (red `make test-feature-flags`), D-113-U.

## Self-Check: PASSED

All 9 claimed files exist on disk; all 7 claimed commits (3 task commits + 4 cleanup-pass commits) resolve in `git log`.

