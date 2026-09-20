---
phase: 114-tasks-extension-migration
plan: 12
subsystem: server-task-dispatch
tags: [tasks, v2, create-gate, era-aware, DQ1, TASK-01, TASK-04]
requires:
  - 114-09 (OwnerBinding, era-aware resolve_owner, declares_tasks_extension)
  - 114-10 (ReservedFieldOwner, per-owner reserved-field grant)
  - 114-11 (v2 create/get/cancel shapes, DispatchEnvelopeClaim plumbing)
  - 114-04 (TaskStore::record_input_requests / task_input_snapshot)
  - 114-06 (client-side per-request tasks-extension declaration)
provides:
  - "task_dispatch::CreateTrigger — the era-aware create trigger, one definition"
  - "task_dispatch::CreateGate + TaskDispatch::create_gate — the complete gate as ONE expression, reached from both dispatchers"
  - "TaskDispatch::extract_input_requests — the create -> pause loop, recorded against the STORE-minted id"
  - "tests/common/v2.rs::pausing_task_tool — the first CLIENT-REACHABLE input_required task"
  - "tests/v2_tasks_create.rs — 7 end-to-end trigger tests over a real socket"
affects:
  - 114-13 (tasks/update routing — depends on this wave)
  - 114-14 (tasks/update dispatch — its paused-task precondition is now reachable)
  - 114-17 (the paired create -> pause -> resume example)
  - 114-15 (cross-caller matrix — both dispatchers now reach one gate)
tech-stack:
  added: []
  patterns:
    - "named enum over adjacent booleans for era-dispatched inputs"
    - "one predicate, two callers: share the GATE, not the response building"
    - "three-valued gate verdict so a caller can log 'open but unshaped' without a second shape check"
key-files:
  created:
    - tests/v2_tasks_create.rs
  modified:
    - src/server/task_dispatch.rs
    - src/server/core.rs
    - src/server/mod.rs
    - tests/common/v2.rs
decisions:
  - "The v2 create trigger is the client's per-request tasks-extension declaration; the v1 `task` field is NOT consulted on v2 (DQ1, user-approved 2026-07-27)"
  - "Share the GATE PREDICATE, not the response building — ServerCore returns a ToolCallOutcome, Server returns a JSONRPCResponse, and forcing one shape would have been the churn, not the fix"
  - "CreateGate is three-valued (Create / NotTaskShaped / Closed) so ServerCore keeps its tool-authoring debug! without keeping a second copy of the task-shape check"
  - "The handler-declared pause is recorded inside build_task_created_response, the one place both the tool-fabricated and the store-minted id exist at once"
metrics:
  duration: ~3h
  completed: 2026-07-29
  tasks: 3
  commits: 3
  files_changed: 5
---

# Phase 114 Plan 12: v2 Task-Creation Trigger Summary

**One-liner:** A v2 `tools/call` from a client that declared `io.modelcontextprotocol/tasks`
now mints a real, pollable task handle — through ONE gate expression reached from both
dispatchers, with the v1 `task` field byte-frozen and inert on v2.

---

## What landed

### 1. The trigger is era-aware, and each era ignores the other's signal

`maybe_build_task_created`'s bare `task_requested: bool` is gone. In its place:

```rust
pub(crate) enum CreateTrigger {
    V1TaskField { task_field_present: bool },
    V2ClientDeclaration { client_declared_tasks: bool },
}
```

with **one** resolver, `CreateTrigger::resolve(era, task_field_present, protocol_context)`, that
the era — and only the era — uses to pick an arm. Two adjacent booleans at a call site is
precisely how the wrong one gets passed; a variant carrying its own fact makes "v1 consulted the
declaration" a shape that does not compile.

The v2 arm **reuses** `TaskDispatch::declares_tasks_extension` — the SAME predicate
`route_tasks_endpoint`'s case-3 `-32021` refusal already used — off the already-resolved
`ProtocolContext`. No second `params._meta` read exists anywhere on the create path.

### 2. ONE gate expression, two dispatch sites

`core.rs` carried a **divergent second copy** of the rule, under a comment that admitted it
("same shape gate as `task_dispatch::maybe_build_task_created`"):

```rust
let has_task_support = req.task.is_some()
    && self.task_store.is_some()
    && tool_task_support.is_some_and(|ts| matches!(ts, Required | Optional));
// … plus its own copy of the taskId+status shape check
```

That is deleted. Both dispatchers now call `TaskDispatch::create_gate`.

**Shape chosen, and why (the plan asked this be recorded):** the plan offered two shapes —
call `maybe_build_task_created` from both sites, or extract the PREDICATE and share that. The
predicate was extracted. `ServerCore::handle_call_tool` returns a `ToolCallOutcome` and builds
its `JSONRPCResponse` one frame up, at the `CallTool` dispatch arm, where the request id and the
`&mut DispatchEnvelopeClaim` out-param live; `Server::handle_call_tool` is bound to
`Result<Value>` and decomposes a full response back into it. Forcing one *response* shape would
have been the churn, not the fix. So: **the RESPONSE building is not shared, the PREDICATE is**,
and adding a future era's trigger means editing `create_gate` + `CreateTrigger` and nothing else.

`create_gate` returns three values rather than a `bool`:

| verdict | meaning |
|---------|---------|
| `Create` | every precondition holds — mint a task |
| `NotTaskShaped` | trigger + backend + `TaskSupport` hold, but the value is not task-shaped; fall through, and the caller MAY log it |
| `Closed` | a precondition is absent; fall through SILENTLY, no error leak (T-102-11) |

The middle variant is load-bearing: `ServerCore` logs a `debug!` for a tool that declares task
support and returns a non-task value. Collapsing to a `bool` would have forced `ServerCore` to
keep its own task-shape check — reintroducing the exact duplicate this plan deletes. Its own test
(`a_gate_open_but_unshaped_value_is_distinguishable_from_a_closed_gate`) pins the distinction.

### 3. The create → pause loop is closed

Cross-AI review's measurement verified on disk: `store.create()` mints the canonical id **after**
the tool handler has returned, discarding the tool's fabricated `taskId`. A handler therefore
could not associate its input requests with the id the client will poll — which made 114-14's
`tasks/update` path and 114-17's paired example unreachable in practice.

`build_task_created_response` now re-extracts a THIRD thing from the tool's value:
`extract_input_requests` returns the server-authored map IFF the value carries **both** a status
that deserializes to `TaskStatus::InputRequired` (through the TYPE — no re-spelled wire literal)
**and** a parseable `inputRequests` object. It is recorded against `store_id`, the same variable
the `CreateTaskResult` envelope reflects.

The two post-create writes are **mutually exclusive in the source** — `if let … else if let …`,
not two independent `if`s — because a task is either terminal or awaiting input, never both. The
`SIGNATURE NOTE` rustdoc now names all three re-extractions, so a refactor that stops
re-extracting has to add an explicit param rather than silently drop the pause.

### 4. `ResponseDisposition::Task`'s `allow(dead_code)` — ALREADY GONE, verified not re-done

The phase context said to verify before implementing. Measured the 114-06 way rather than by the
plan's proxy grep: the `ResponseDisposition` block was extracted from `git show 6bb89170:` and
from the tree and diffed — **IDENTICAL**. 114-11 removed the allow one plan early when it wired
`DispatchEnvelopeClaim::TASK_CREATED` into production (D-114-H, closed early). The block's only
remaining `cfg_attr` allow is on `InputRequired`, scoped `not(feature = "streamable-http")`, which
is Phase 113's and not this plan's. The plan's literal acceptance grep
(`grep -B3 -A1 'Task,' src/server/core.rs | grep -c 'allow(dead_code)'`) reads **0**.

**Nothing was re-implemented.** The plan's ordering rationale ("removed here because `make lint`
runs `--lib` as a non-test build") was correct in reasoning and already discharged by a different
route: `server::task_dispatch` is gated only on `not(wasm32)` and on no feature, so the production
constructor is present on every native build.

---

## Deviations from Plan

### Auto-fixed / adjusted

**1. [Rule 3 - Blocking] `tests/common/v2.rs` is one file beyond the declared `files_modified`**

- **Found during:** Task 3
- **Issue:** Test 1b needs a task-capable tool whose value declares `inputRequests` +
  `status: input_required`. No such fixture existed — before this plan the ONLY way to reach that
  status was to reach past the wire and poke `record_input_requests` on the store, which is
  exactly what a create→pause end-to-end test must not do.
- **Fix:** the shared harness was **EXTENDED, not forked** (114-11's precedent): `pausing_task_tool`
  / `PAUSING_TOOL_NAME` / `PAUSING_TOOL_REQUEST_KEY` added, registered in
  `spawn_tasks_server_with_store`. 114-13 and 114-14 need a paused task the same way, so it belongs
  in the shared fixture rather than in one suite. The store-handle rustdoc that claimed
  `input_required` has "no client-facing trigger in this phase" was corrected in the same commit
  rather than left to mislead.
- **Commit:** c51a378b

**2. [Rule 3 - Blocking] Tests 5 and 6 build their servers locally rather than from the shared tasks fixture**

- **Found during:** Task 3
- **Issue:** both rows need a server SHAPE the shared tasks fixture cannot have *by definition* —
  a tool with no `taskSupport`, and a server with no `TaskStore`.
- **Fix:** `spawn_gate_probe_server` composes `Server::builder()` + the harness's
  `spawn_default_config` / `OptionalBearer` / version constants. This is using the harness's
  primitives, not forking it: the v2 accept-list, auth posture and spawn helper are all the
  shared ones.
- **Also measured:** test 6's tool must declare `TaskSupport::Optional`, not `Required`.
  `apply_tasks_capability_rule` **refuses to build** a server whose tool declares `Required` with
  no backend, so the reachable no-backend row is the optional one. Stated in the test's rustdoc.

**3. [Correction] Two acceptance greps cannot pass as written, because the plan's own action text asks for the text they forbid**

The **fourth** time this class has appeared in this phase (114-06, 114-10, 114-11, now 114-12).
Both were measured directly instead:

| plan's grep | literal result | why | intent, measured |
|-------------|----------------|-----|------------------|
| `grep -c 'task_requested: bool' src/server/task_dispatch.rs` is 0 | **1** | the sole occurrence is the RUSTDOC explaining why the enum replaced the two-bool alternative | **non-comment occurrences: 0** |
| `git diff \| grep -cE '^\+.*_meta.*clientCapabilities'` is 0 | **1** | the plan asks for "a per-era trigger TABLE" in the rustdoc, and the table must name the channel the declaration arrives on | **non-comment added lines matching both tokens: 0** |

**4. [Correction, MEASURED] Test 2's `!raw.contains("taskId")` is UNSATISFIABLE — replaced with a strictly stronger assertion**

Measured rather than argued: the assertion was written, run, and the real wire captured.

```
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text",
 "text":"{\"taskId\":\"tool-fabricated\",\"status\":\"working\",…}"}],
 "isError":false,"resultType":"complete",
 "_meta":{"io.modelcontextprotocol/serverInfo":{…}}}}
```

The reason is **structural, not incidental**: the create gate only fires on a TASK-SHAPED value
(`taskId` + `status`), so "the same tool, same server" *forces* the non-creating leg to text-wrap
a payload that literally spells `taskId`. A bare substring check therefore cannot distinguish a
leaked protocol handle from a tool echoing its own payload — in either direction.

Replaced by `!raw.contains("\"taskId\":")` — the nine-byte PROTOCOL spelling. Inside the wrapped
text the quotes are backslash-escaped, so it does not occur; a real leaked handle is a JSON key,
so it always does. **Satisfiable and strictly more precise.** Folded into `assert_ordinary_result`,
so all six non-creating rows get it, alongside four more independent facts (`resultType` present /
`task` absent, no top-level `taskId`, no `task` wrapper, no `_meta.relatedTask`) plus a **store**
assertion that nothing was minted at all.

**5. [Commit granularity] Task 1's signature change cannot compile without Task 2's `mod.rs` update**

`maybe_build_task_created` is `pub(crate)` with two call sites. Committing Task 1 alone would have
committed a non-compiling tree. The work was committed as three compiling commits instead — the
gate (Task 1 + Task 2's twin-site parity), the pause loop (Task 2's second half), the suite
(Task 3) — with the pause-loop hunks temporarily reverted from a byte-for-byte scratch copy so
commit 1 is exactly the gate change. `shasum -a 256 -c` verified on restore. **`git stash` not
used.**

---

## Negative controls — SEVEN, not the plan's two

Every control was applied, **verified to have landed** (`diff -q` against a byte-for-byte copy,
per the 114-10 process trap where an `&&`-chained probe aborted before the mutation and the
subsequent "exit 0" was measured against an unmodified tree), run, and reverted.

| # | mutation | tests failed (of 7) | plan's prediction |
|---|----------|---------------------|-------------------|
| NC-1 | v2 arm ALSO requires the v1 `task` field | **1, 1b** | "1 and 3" — **WRONG** |
| NC-2 | v2 gate opens unconditionally (declaration ignored) | **1, 1b, 2, 3** | "test 2 only" — **WRONG** |
| NC-3 | v1 arm always fires (ignores the `task` field) | **4** only | not specified |
| NC-4 | drop the `task_support` clause from `create_gate` | **5** only | not specified |
| NC-5 | drop the `task_store.is_some()` clause from `create_gate` | **6** only | not specified |
| NC-6 | `extract_input_requests` returns `None` unconditionally | **1b** only | not specified |
| NC-7 | mint the task under a shadow owner (well-shaped handle, unreachable) | **1, 1b** | not specified |

**Both of the plan's predictions were wrong, and the corrections are the useful part:**

* **NC-1 does not fail test 3.** Test 3's *declaring* leg deliberately carries the v1 `task` field
  (it is the non-vacuity twin), so under NC-1 it still creates and the test stays green. What NC-1
  actually fails is test **1b**, which the plan did not name at all.
* **NC-2 fails four tests, not one.** Every v2 row whose *non-creating* leg would suddenly create
  fails: tests 1, 1b, 2 and 3. The three that stay green (4, 5, 6) are exactly the rows closed by a
  *different* clause — v1 era, `TaskSupport`, backend — which is what makes the control
  attributable rather than indiscriminate.

**The masking check, run explicitly.** NC-1 and NC-7 fail the SAME test set, which is the signal
the phase lesson says to split. They were split by panic SITE:

* NC-1 panics at `tests/v2_tasks_create.rs:124` — `minted_id`, i.e. the CREATE produced no handle.
* NC-7 panics at `tests/v2_tasks_create.rs:115` — `result_of(&polled)`, i.e. the create SUCCEEDED
  and the **poll** failed.

So the "the handle is REAL and reachable by the caller that received it" assertion is load-bearing
and is failed by NC-7 alone. Without NC-7, test 1's `tasks/get` would have been a property no
control ever failed — which is the exact 114-10/114-11 finding, reproduced.

Every one of the seven properties (v2 opens, v2 refuses, v1 field inert, v1 frozen,
`TaskSupport`, backend, pause recorded) is failed by at least one control, and NC-3/4/5/6 each
fail **exactly one** test.

**Reverted, verified:**

```
src/server/task_dispatch.rs: OK
tests/v2_tasks_create.rs: OK
tests/common/v2.rs: OK
```

---

## Verification — verbatim commands and exit codes

| check | command | exit | result |
|-------|---------|------|--------|
| quality gate | `NEXTEST_TEST_THREADS=4 make quality-gate` | **0** | **279 result lines, 4778 passed, 0 failed, 80 ignored**, 0 truncation markers, 0 non-`ok.` result lines, 8038 log lines |
| lint | `make lint` | **0** | run ×3 (after the gate change, after the pause loop, after the test suite) |
| format | `cargo fmt --all -- --check` | **0** | |
| build | `cargo build --features full` | **0** | |
| semver | `cargo semver-checks --baseline-rev 6bb89170` | **0** | **223 checks: 223 pass, 30 skip — no semver update required** |
| public API | `cargo public-api --features full diff 6bb89170..HEAD` | **0** | **Removed (none), Changed (none), Added (none)** — every new item is `pub(crate)` |
| wasm | `make wasm-build` | **0** | 92 lib warnings, **0** naming any touched symbol |
| complexity | `pmat analyze complexity --format json --max-cognitive 25` | **0** | **4 violations at `.summary.violations`, all pre-existing, 0 in `src/`** |
| new suite | `cargo nextest run --features full -j 4 --test v2_tasks_create` | **0** | **7 tests run: 7 passed** |
| tasks + lib | `cargo nextest run --no-fail-fast --features full -j 4 -E 'binary_id(pmcp) or <8 tasks/MRTR binaries> or binary_id(pmcp::v2_tasks_create)'` | 100 | **1858 passed, 4 failed** — all 4 are D-114-A's keychain signature; re-run at `-j 2` in isolation: **4 passed** |

**Gate arithmetic reconciles to a single test** against 114-11's recorded 276 lines / 4759 passed:

* **+3 result lines** — the new `v2_tasks_create` binary is `Running` in three gate legs
  (`grep -c "Running tests/v2_tasks_create.rs"` = 3), matching 114-11's own +3 for `v2_tasks_shapes`.
* **+19 passed** = 6 new lib `gate_tests` rows **counted twice** (the lib suite runs in two gate
  legs) = 12, plus the 7 new integration tests counted once = 7. **4759 + 19 = 4778.** ✓

**PMAT was queried at `.summary.violations`** — the top-level path still reports `0 violations`
**vacuously**, as every plan since 114-06 has recorded.

**wasm baseline:** the plan asks for "green versus a `HEAD~1` baseline". `git diff --stat HEAD~1
HEAD -- src/` is **EMPTY** (the Task-3 commit is test-only), so the `HEAD~1` wasm artifact is
identical **by construction** rather than by a second 8m41s build. Independently:
`server::task_dispatch` is `#[cfg(not(target_arch = "wasm32"))]` at `src/server/mod.rs:104-105`,
and `grep -c 'create_gate\|CreateTrigger' src/server/wasm_core.rs` is **0** — **the wasm server
core does not reach this gate at all.**

**Untouched, verified byte-for-byte vs plan-start `6bb89170`:**

* `Cargo.toml` / `Cargo.lock` — empty diff, **zero packages installed** (T-114-SC)
* `.planning/REQUIREMENTS.md` — **empty diff**, and `requirements mark-complete` deliberately
  **NOT** run: TASK-01…TASK-06 flip as a GROUP and `114-SPEC-RECHECK.md`'s `## Verdict` is still
  `PENDING`
* `crates/pmcp-tasks/` — **empty diff** (`make quality-gate` does not cover that crate anyway)
* `src/types/mrtr.rs` — not in the changed-file list; the `MRTR_METHODS` block diffs **IDENTICAL**
  to plan-start. **No `tasks/*` row was added.**

**84 deletion lines** in `src/` vs plan start, enumerated with `/usr/bin/git` — every one an
intended replacement: 3× the old `maybe_build_task_created` gate-test call arity, the two clauses
of the old `gate_open` expression, `core.rs`'s `has_task_support` + its duplicate `is_task_shaped`
block, and the reflowed `terminal_result` binding.

---

## Environment finding — recorded under D-114-A, not a code regression

A broad `cargo nextest` run reported **14 failures** in `shared::streamable_http::tests`, a file
this plan never touched. Two reproducible triggers were isolated:

1. **A sandboxed shell reproduces it 100%.** 8/8 failed sandboxed; the identical command
   unsandboxed passed 8/8. The macOS keychain read is denied there
   (`Os(Error { code: -36 })` → `no native root CA certificates found`).
2. **Unsandboxed it is parallelism-sensitive.** Default `-j <ncpu>` gave 14 then 4 failures;
   `-j 4` gave **1845/1845 green**; `-j 2` re-ran the stragglers green.

`df -h /` showed **39 GiB free**, so this is NOT the disk-exhaustion mode recorded in memory. The
gate was run with `NEXTEST_TEST_THREADS=4` and exited **0**. Guidance appended to **D-114-A**: a
failure whose stderr contains `no native root CA certificates found` is that item, not the plan
under test — check for the string before bisecting.

---

## Deferred items logged

* **D-114-A** — addendum: the two reproducible triggers above, with the `-j 4` /
  `NEXTEST_TEST_THREADS=4` guidance.
* **D-114-I** — addendum: the shared tasks fixture now registers **THREE** tools, not two.
  Re-verified that no suite asserts `tools/list` cardinality.
* **D-114-K** — NEW. The v2 trigger is per-REQUEST and per-CLIENT, never per-TOOL: a declaring
  client receives a handle from *every* task-capable tool on the server and has no per-call
  opt-out (there is no v2 equivalent of v1's `task` field). That is the spec's own shape — the
  server is the sole decider — and the plan's objective explicitly re-records the surrounding
  client-compatibility / UX design as STILL DEFERRED. Logged so a later reader does not mistake
  the absence of an opt-out for an oversight.

`114-SPEC-RECHECK.md` **row 39** was updated with what LANDED and what row 39 does **not** settle
(pointing at D-114-K). **Row 19 (`UpdateTaskResult`'s empty ack) was left untouched** — it is
114-13/114-14's, and the forward tripwire armed in `tasks_get_never_carries_result_type_task`
stays green.

---

## For 114-13 / 114-14

* The v2 create trigger is **live**. `tests/common/v2.rs::pausing_task_tool`
  (`PAUSING_TOOL_NAME = "elicit_task"`) gives you a **client-reachable** `input_required` task from
  a real `tools/call` — no store poking required. `tests/v2_tasks_create.rs::a_handler_declared_
  input_request_is_recorded_against_the_minted_id` is the worked example.
* `TaskInputSnapshot::kind_of` reads the map recorded by *this* plan's re-extraction, so a
  kind-directed decode of the client's `inputResponses` has a real record to read against.
* **Do not add a `tasks/*` row to `MRTR_METHODS`** — the routing name is already supplied by
  `TASK_NAME_BEARING_METHODS` (114-06), and `MRTR_METHODS` eligibility would delete
  `tasks/update`'s entire payload.
* Adding a future trigger means editing **`TaskDispatch::create_gate` and `CreateTrigger`, and
  nothing else.** If you find yourself writing a second copy of the condition, that is the
  regression T-114-58 names.

---

## Self-Check: PASSED

Files claimed created/modified — all present:

```
FOUND: tests/v2_tasks_create.rs
FOUND: src/server/task_dispatch.rs
FOUND: src/server/core.rs
FOUND: src/server/mod.rs
FOUND: tests/common/v2.rs
```

Commits claimed — all present in `git log`:

```
FOUND: 8e6b5c0c  feat(114-12): era-aware create gate, one predicate, two dispatch sites
FOUND: c24186d4  feat(114-12): record a handler-declared pause against the store-minted id
FOUND: c51a378b  test(114-12): v2 create trigger end to end over a real tools/call
```
