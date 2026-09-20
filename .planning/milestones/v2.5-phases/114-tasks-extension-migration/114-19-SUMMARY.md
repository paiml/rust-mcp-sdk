---
phase: 114-tasks-extension-migration
plan: 19
subsystem: client
tags: [tasks, v2, client, era-split, decoding, semver]
requires:
  - "114-06 (client declares the tasks extension; era-aware assert_capability)"
  - "114-11 (the v2 projection types and the flat wire shapes)"
  - "114-08 (tasks/list + tasks/result retired on v2)"
  - "114-12 (v2 create fires on the client's declaration)"
provides:
  - "Client::tasks_get on v2 decodes the FLAT payload and still returns a Task"
  - "Client::tasks_get_detailed — the status-conditional result/error/inputRequests, one round trip"
  - "Client::tasks_cancel_ack — the empty-ack primitive; tasks_cancel re-reads on v2"
  - "Client::tasks_update — v2-only, sent untyped, empty ack"
  - "Client::wait_for_task reaches terminal on v2 without tasks/result"
  - "Client::wait_for_task_with_inputs — the input-supplying poll sibling"
  - "TaskV2::to_v1 — the ONE inverse of the two v2 renames"
  - "types::mrtr::{TASK_RESULT_TYPE, COMPLETE_RESULT_TYPE} — one declaration, server + client"
affects:
  - "114-17 (its paired agent example drives exactly these client methods)"
  - "114-13/114-14 (the client half of tasks/update now exists and expects an EMPTY ack)"
tech-stack:
  added: []
  patterns:
    - "era branch on is_v2(), never on response shape"
    - "one shared decoder for two public methods that had identical arms"
    - "raw-frame client tests instead of round-tripping through pmcp's own server"
key-files:
  created:
    - tests/v2_tasks_client_era.rs
  modified:
    - src/client/mod.rs
    - src/types/tasks.rs
    - src/types/mrtr.rs
    - src/server/core.rs
    - src/error/mod.rs
    - tests/v2_tasks_client.rs
decisions:
  - "tasks_cancel on v2 = tasks_cancel_ack + a follow-up tasks_get, never a synthesised Task"
  - "tasks/result and tasks/list keep Error::retired_on_v2, so is_retired_on_v2() is ONE typed check for both retired families"
  - "the input-round bound REUSES the configured mrtr_round_limit rather than minting a constant"
  - "the resultType discriminator strings moved to types::mrtr so the wasm-excluded server enum and the wasm-included client read one declaration"
metrics:
  duration: ~5h
  completed: 2026-07-29
---

# Phase 114 Plan 19: Era-Aware v2 Task Client Summary

A v2 pmcp client now decodes every task shape the v2 server emits — flat create,
flat `tasks/get` with its status-conditional detail, and the two empty
acknowledgements — branching on the `resultType` DISCRIMINATOR rather than on the
response's shape; `wait_for_task` reaches a terminal result on v2 without ever
touching the retired `tasks/result`, and `tasks_update` plus an input-supplying
poll sibling close the pause loop from the client side.

## What landed

### The four decode sites, era-split

| Site | v1 (unchanged) | v2 (new) |
|------|----------------|----------|
| `call_tool_with_task` / `..._and_meta` | try nested `CreateTaskResult`, else `CallToolResult` | read `resultType`; `"task"` → flat `TaskV2` → `to_v1()`, else `CallToolResult` |
| `tasks_get` | `GetTaskResult.task` | flat `TaskV2` → `to_v1()` (`ttlMs`→`ttl`, `pollIntervalMs`→`pollInterval`) |
| `tasks_cancel` | `CancelTaskResult.task` | `tasks_cancel_ack()` (accepts `{}`) then ONE `tasks_get` |
| `wait_for_task`'s terminal step | `tasks_result(task_id)` | the `result` INLINE in the payload the loop already fetched |

**The two create methods now share ONE decoder.** They carried a line-for-line
identical `ResponsePayload::Result` arm; making them era-aware would have created
a third copy. `grep -c 'from_value::<CreateTaskResult>' src/client/mod.rs` is
**1** (was 2).

**The v2 arm never shape-sniffs.** The existing v1 comment already warned against
key-name duck-typing; on v2 the warning becomes load-bearing, because the flat
create payload and a `CallToolResult` share no discriminating key —
`CallToolResult::content` carries `#[serde(default)]`, so essentially any object
decodes as one. The branch reads `resultType` and compares against the constant
the server emits. Every arm is NAMED, including the two that answer the same
thing (explicit `"complete"` and an ABSENT key are one answer — Phase 112's
absent-means-complete rule) and the unrecognised-value arm, which is that answer
too but logged, because an unknown discriminator is protocol skew a developer
needs to see.

### `tasks_get_detailed`, and why `tasks_get` does NOT use it

`tasks_get` keeps its `Task` return (public signature, no semver break) and
decodes only the flat BASE task. `tasks_get_detailed` is the additive sibling
that decodes the full status-discriminated `DetailedTaskV2`.

They are deliberately different strictness. `DetailedTaskV2::from_wire_value` is
STATUS-DIRECTED and REFUSES a `completed` task with no `result`. The server's own
`v2_detailed_task_value` degrades to a bare flat `Task` when a backend cannot
supply the detail (a `TaskRouter` with no terminal payload, for instance). If
`tasks_get` decoded strictly, "I could not read the detail" would become "I could
not read the task at all" and a caller would lose the status too.

Both go through ONE private fetch, `tasks_get_raw_v2`, which returns the raw
result object. The poll loop calls it once per tick and decodes the base task AND
(on terminal / on pause) the detail from the SAME bytes. Decoding twice from one
value is free; asking the server twice is a second round trip and a second chance
for the two answers to disagree about a task that moved in between.

### `tasks_cancel` — the choice and its reasoning (the plan asked for this on the record)

The plan offered two routes for a v2 cancel, whose result is an EMPTY ack with no
task body: re-read via `tasks_get`, or add a `tasks_cancel_ack()` sibling and
delegate. **Both were taken, in that order**, because they answer different
questions:

- `tasks_cancel_ack(task_id) -> Result<()>` is the zero-invention primitive: ONE
  round trip, accepts any successful result including `{}`, claims nothing about
  the task's status. It works on BOTH eras (on v1 it discards the returned
  `Task`).
- `tasks_cancel(task_id) -> Result<Task>` cannot change its return type without a
  MAJOR bump, so on v2 it calls the ack and then performs ONE follow-up
  `tasks_get`. It does **not** synthesise `status: cancelled`.

Synthesising was rejected outright: cancellation is cooperative and eventually
consistent — that is the SEMANTICS of the empty ack, which 114-11 recorded when it
deliberately added no wait and no poll server-side — so the task MAY still be
`working` when the call returns and MAY settle on a terminal status other than
`cancelled`. A fabricated `cancelled` would be the client lying about a fact only
the server holds. The test asserts the re-read reports `working`, i.e. that the
invented answer is NOT what comes back.

### The retired methods fail fast, locally

`tasks_result` and `tasks_list` are NOT deleted — that is a semver break and v1
still serves both. Each gains a v2 guard that fires before any bytes go out, via
one shared `reject_retired_tasks_method_on_v2`. Both mint
`Error::retired_on_v2`, the SAME marker the Phase-113 subscriptions retirement
uses, so a caller has exactly one typed check (`Error::is_retired_on_v2()`) for
"this method is gone on v2" across both families.

The replacements are honest rather than symmetric: `tasks/result` → `tasks/get`
(a real method that inlines the payload), `tasks/list` → `"client-side task
tracking"`. There is no v2 list — the enumeration primitive was removed as a
security improvement — and pointing a caller at `tasks/get`, which answers about
ONE id it already holds, would have been a lie in the one string a caller has for
choosing its next move.

### `tasks_update` and the input-supplying poller

`tasks_update(task_id, responses) -> Result<()>` goes out through
`send_untyped_request` with the `tasks/update` method string. There is no
`ClientRequest::TasksUpdate` variant and there must not be one — `ClientRequest`
is public and not `#[non_exhaustive]`. Its params are built as an explicit
`serde_json::Map` from the shared `TASK_ID_KEY` / `INPUT_RESPONSES_KEY`
spellings. It refuses v1 (`require_v2`) and an un-negotiated server
(`assert_capability`) before sending; all three checks are local, so every
refusal is zero bytes.

`wait_for_task_with_inputs` reuses `wait_for_task`'s machinery **wholesale**: both
are thin callers of one private `poll_task_to_terminal` that takes an
`Option<responder>`. The classifier match, `MIN_POLL_MS`, the remaining-budget
clamp and the `web_time::Instant` clock are therefore literally the same code, not
a copy — which is what the plan asked for and what makes T-114-102 (hot-spin on a
zero `pollIntervalMs`) mitigated by construction rather than by a second
implementation that happens to agree.

`wait_for_task` itself is behaviourally unchanged: with no responder the
`InputRequired` arm returns the identical error string it always did. Its rustdoc
now points at the sibling as the answer to that message.

**The round bound reuses `self.mrtr_round_limit`** (default
`DEFAULT_MRTR_ROUND_LIMIT` = 8), not a new constant, so no `const _: () =
assert!` was needed: it is the same knob, and it bounds the same thing — "how
many times will I answer this server's questions before concluding it is not
making progress". Two independently-tuned answers to one question is how they
drift apart. Documented on the method.

A v2 task that reaches `failed` surfaces its inlined JSON-RPC error through
`Error::from_jsonrpc_error`, so a failed task is indistinguishable, to a caller
matching on `code`, from the same failure delivered synchronously. A `cancelled`
task is an error too — it has no result. Both matter because
`CallToolResult::content` is `#[serde(default)]`: a permissive decode of a failed
task's payload yields a perfectly well-formed EMPTY SUCCESS.

## Deviations from Plan

### [Rule 3 - Blocking] Four files beyond `files_modified`

`files_modified` declared `src/client/mod.rs` and `tests/v2_tasks_client_era.rs`.
Five other files were touched, each unavoidable:

**1. `src/types/mrtr.rs` + `src/server/core.rs` — the discriminator constants.**
The plan says to take the `"task"` / `"complete"` strings from the existing shared
source and, if the server-side constant is not reachable from the client, to
introduce ONE shared `pub(crate)` constant read by both sides. It is not
reachable: `ResponseDisposition::as_wire_str` lives in `server::core`, which is
`#[cfg(not(target_arch = "wasm32"))]`, and the client compiles on wasm. So
`TASK_RESULT_TYPE` and `COMPLETE_RESULT_TYPE` now sit beside
`INPUT_REQUIRED_RESULT_TYPE` in `types::mrtr` (wasm-reachable), `as_wire_str`
returns them, and the client decoder compares against them. No `tasks/*` row was
added to `MRTR_METHODS`, and that table is byte-identical to plan-start.

**2. `src/types/tasks.rs` — `TaskV2::to_v1`.** `TaskV2::from_v1`'s own rustdoc
says it is "the ONLY place the two renames happen". The v2 client needs the
inverse to keep `tasks_get` returning a `Task`; doing that remap inside the
client would have been a second copy of the rename table. It lives immediately
beside its inverse, with two unit tests: a field-for-field round trip (plus a
serialized-equality check that catches a field the per-field list forgot) and a
rename-direction test asserting no v2 key spelling leaks onto a v1 `Task`.

**3. `src/error/mod.rs` — an untrue hard-coded parenthetical.**
`Error::retired_on_v2`'s message appended `(Client::subscriptions_listen)`, from
when the subscriptions pair were the only retired methods. For a `tasks/*` caller
that is simply FALSE, and a refusal message is the ONE signal a caller has for
choosing its next move (the exact reason 114-08 replaced
`V2_TASKS_NOT_NEGOTIATED`). Removed. The `replacement` argument is untouched, so
`retired_replacement()` still returns `"subscriptions/listen"` for the
subscriptions callers and nothing observable moved for them.

**4. `tests/v2_tasks_client.rs` — a 114-06 control whose VEHICLE this plan
removed.** See below; it is significant enough to stand alone.

### [Rule 1 - Bug] 114-06's empty-`Mcp-Name` control could no longer reach the wire

`tasks_list_emits_an_empty_mcp_name` fired `client.tasks_list(None)` on a v2
client purely as a vehicle for observing that a `tasks/*` method absent from the
routing table emits an empty `Mcp-Name`. This plan makes `tasks/list` fail fast
LOCALLY with zero bytes, so nothing arrived and its `wait_for(.., 1)` panicked.
**Caught by the quality gate, not by the new suite** — the new suite has no reason
to run 114-06's file.

The two requirements are not in conflict; only the vehicle was. `Mcp-Name` is
derived by the TRANSPORT from the frame bytes (T-113-08), so the control now
builds a `StreamableHttpTransport`, sets the negotiated version, and hands it a
raw `tasks/list` frame through `send_raw`. That is strictly MORE precise: it
measures the derivation itself rather than a client method that may or may not
still call it, and it keeps the property observable even though **no pmcp v2
client can emit a non-name-bearing `tasks/*` method any more** — `tasks/list` and
`tasks/result` are the only two, and both are now locally refused.

A TABLE-level restatement was written, then measured as an exact duplicate of the
existing `tasks_list_and_tasks_result_carry_no_routing_name`, and DELETED. A
second spelling of one fact is the drift this phase keeps recording.

### [Process] Tasks 1 and 2 landed in ONE commit

Both tasks edit the same four methods of one file and interleave at the line
level (Task 2's poll loop consumes Task 1's `tasks_get_raw_v2` /
`decode_v2_task_base`; Task 1's `tasks_result` guard is what Task 2's terminal
step exists to avoid). Splitting them would have required line-level patch
surgery on interleaved hunks in a single file. Both tasks' acceptance criteria
were measured independently; the commit message enumerates both.

### [Measured] Two acceptance criteria cannot pass as written

Consistent with the SIXTH and SEVENTH occurrences of this pattern in the phase,
the intents were measured directly instead:

1. **`grep -c 'serde_json::json!\|r#"{' tests/v2_tasks_client_era.rs` ≥ 12** — the
   suite imports `json!` and calls it unqualified, so the literal grep counts
   **0**. Measured intent: `grep -c 'json!(' tests/v2_tasks_client_era.rs` = **20**
   (matching LINES; the fixtures are shared helpers, so 20 lines feed all 21
   tests) and every decoding test feeds a raw JSON frame.

   A third instance of the same pattern, in this file's own prose: a naive
   `grep -c '#\[tokio::test\]'` returns **22** for **21** tests, because the
   module docstring's closing sentence says "the tests are `#[tokio::test]`".
2. **`grep -A25 '<new fn name>' src/client/mod.rs | grep -c '_ =>'` is 0** — passes
   as written (**0**), but note WHY: the decision match lives in the shared
   `poll_task_to_terminal`, not in the sibling, so the grep would have been
   vacuous either way. The real property was measured at the match itself: the
   `TaskPollDecision` match has three named arms and no wildcard, and the two
   `TaskDetailV2` matches added by this plan are exhaustive over its five
   variants with no wildcard (`TaskDetailV2` is deliberately not
   `#[non_exhaustive]`).

## Negative controls — ELEVEN, and the masking check FIRED

Each control was applied by exact-string replacement, the 21-test suite run with
`--no-fail-fast`, then reverted with `git checkout -- <file>` (never `git stash`)
and verified by `shasum -a 256 -c` on all six touched files: **OK on every file,
after every control.**

| # | Mutation | Tests it fails |
|---|----------|----------------|
| NC-1 | `tasks_get`'s v2 arm reverts to the nested decode | `v2_tasks_get_flat_payload_maps_ttl_ms_onto_ttl`, `v2_empty_cancel_ack_is_not_a_decode_error` |
| NC-2 | `wait_for_task`'s terminal fetch is unconditionally `tasks/result` | `v2_wait_for_task_never_calls_tasks_result`, `v2_failed_task_surfaces_its_inlined_error`, `v2_wait_for_task_with_inputs_answers_and_resumes` |
| NC-3 | the v2 create arm shape-sniffs instead of reading `resultType` | `v2_complete_result_type_decodes_to_an_ordinary_tool_result` |
| NC-4 | `tasks_result` loses its v2 fail-fast | `v2_tasks_result_fails_fast_with_zero_sends` |
| NC-5 | `tasks_list` loses its v2 fail-fast | `v2_tasks_list_fails_fast_with_zero_sends` |
| NC-6 | `tasks_cancel` keeps the v1 nested decode on v2 | `v2_empty_cancel_ack_is_not_a_decode_error` |
| NC-7 | `wait_for_task_with_inputs` loses `require_v2` | `v1_wait_for_task_with_inputs_is_refused_with_zero_sends` |
| NC-8 | a `failed` v2 task returns an EMPTY `Ok` success | `v2_failed_task_surfaces_its_inlined_error` |
| NC-9 | `tasks_get_raw_v2` loses `assert_capability` | `v2_undeclared_tasks_get_is_refused_before_the_wire` |
| NC-10 | `TaskV2::to_v1` drops both renames | `v2_flat_create_result_decodes_to_a_task_handle`, `v2_tasks_get_flat_payload_maps_ttl_ms_onto_ttl` |
| NC-11 | `tasks_update` loses `assert_capability` | `v2_undeclared_tasks_update_is_refused_before_the_wire` |

**All eleven failing sets are pairwise DISTINCT.**

### The masking check fired, and the fix was a test split, not a new control

NC-9 and NC-11 initially failed the **IDENTICAL** single-test set — both hit
`v2_undeclared_client_is_refused_before_the_wire`, which asserted the refusal for
`tasks/get` and `tasks/update` in one body. Splitting them "by panic site" was not
available: both panicked at the SAME line (the scripted transport's "no script
entry" guard), distinguishable only by reading the panic MESSAGE
(`…for tasks/get` vs `…for tasks/update`).

Two properties detected by one test is one property's worth of evidence. The test
was SPLIT into `v2_undeclared_tasks_get_is_refused_before_the_wire` and
`v2_undeclared_tasks_update_is_refused_before_the_wire` — the two methods reach
`assert_capability` through different call chains — and both controls re-measured:
**1 failure each, disjoint.**

### Two of the plan's three predictions were WRONG (again)

- **NC-1 predicted "tests 4 and 5 fail".** Measured: test 4 and the CANCEL test.
  Test 5 (`v2_tasks_get_inlines_result_on_completed`) stays GREEN because it goes
  through `tasks_get_detailed`, a different method that NC-1 does not touch. The
  cancel test fails instead, because `tasks_cancel`'s v2 arm re-reads through
  `tasks_get` — a fixture dependency, not a second independent property.
- **NC-2 predicted "test 9 fails only".** Measured: **three** failures. The
  inlined-`failed`-error test and the input-supplying-poller test both reach
  terminal through the same step.
- **NC-3 predicted "test 2 fails only".** Measured: exactly that. The only correct
  prediction of the three.

### Properties with no control, and why

Four tests are failed by no control BY CONSTRUCTION — they are the v1 controls
(`v1_nested_create_result_still_decodes`,
`v1_tasks_get_still_decodes_the_nested_payload`,
`v1_tasks_result_and_tasks_list_still_serve`,
`v1_wait_for_task_still_calls_tasks_result`). A v2-scoped mutation that failed one
of them would be a mutation that leaked into v1; their evidence value is that they
stay green.

Four more have no control and this is recorded rather than papered over:
`v2_tasks_get_inlines_result_on_completed` / `..._error_on_failed` /
`..._input_requests_on_input_required` (the three `tasks_get_detailed` shapes) and
`v2_empty_update_ack_is_not_a_decode_error`. The status-directed decode they
exercise is `DetailedTaskV2::from_wire_value`, which is 114-11's code and carries
114-11's own controls; what this plan adds is only the client's reach to it. NC-2
covers that reach for the terminal path.
`wait_for_task_without_a_responder_still_errors_on_input_required` likewise has
no control — it pins that `wait_for_task` did NOT change, which is a
by-construction property of the `Option<responder>` refactor.

## Threat model

| Threat | Disposition | Where it landed |
|--------|-------------|-----------------|
| T-114-99 (a server steering a client's decode branch with a task-shaped ordinary result) | mitigated | the v2 branch reads `resultType` from the shared constant; `v2_complete_result_type_decodes_to_an_ordinary_tool_result` is the guard and **NC-3 proves it load-bearing** |
| T-114-100 (client decoder drifting from the server's emitted shapes) | mitigated | the suite's fixtures carry comments naming `tests/v2_tasks_shapes.rs` / `tests/v1_tasks_golden.rs` as their counterparts |
| T-114-101 (unbounded client-side input round loop) | mitigated | bounded by the configured `mrtr_round_limit`; documented relationship, no new constant |
| T-114-102 (hot-spin on a zero/absent `pollIntervalMs`) | mitigated BY CONSTRUCTION | the sibling IS `wait_for_task`'s loop, so the `MIN_POLL_MS` floor and budget clamp are the same code, not a re-derivation |
| T-114-103 (an un-negotiated tasks call leaking a request) | mitigated | `assert_capability` fires first on every tasks method; two tests assert a frame counter that did not move, **NC-9 and NC-11 each prove one** |
| T-114-104 (treating a task-delivered payload as higher-trust) | mitigated | `tasks_update` and `wait_for_task_with_inputs` both state the spec rule in rustdoc; the callback contract passes values through and executes nothing server-supplied |
| T-114-105 (accidental semver-major on `ToolCallResponse`) | mitigated | variants and payload types unchanged; verified below |
| T-114-SC (package installs) | accepted | ZERO packages installed; `Cargo.toml` / `Cargo.lock` byte-unchanged |

## Verification

Every figure below was taken with `/usr/bin/make` / `/usr/bin/git` / `~/.cargo/bin/cargo`
(never the RTK proxy) and only exit codes are treated as authoritative.

| Check | Command | Exit |
|-------|---------|------|
| format | `cargo fmt --all -- --check` | **0** |
| lint | `make lint` (`--features full --lib --tests`, pedantic + nursery, `-D warnings`) | **0** ×3 |
| build | `make build` | **0** |
| unit | `make test-unit` (`RUST_TEST_THREADS=1`) | **0** |
| doctests | `make test-doc` | **0** |
| property | `make test-property` | **0** |
| examples | `make test-examples` | **0** |
| integration | `make test-integration` | **101** — ONE environmental failure, see below |
| package gate | `make pmcp-package-gate` | **0** |
| audit | `make audit` | **0** — no vulnerabilities |
| todos / unwraps | `make check-todos check-unwraps` | **0** — no SATD, no production `unwrap()` |
| always-requirements | `make validate-always` (`RUST_TEST_THREADS=1`) | **0** |
| purity | `make purity-check` | **0** |
| comply | `make comply` | **0** — all four team-servers bindings resolve |
| semver | `cargo semver-checks --baseline-rev 22b36b5f` | **0** — 223 checks, 223 pass, **no semver update required** |
| public API | `cargo public-api diff 22b36b5f..HEAD` | **0** — **Removed (none), Changed (none)**; 5 purely ADDITIVE items |
| wasm | `make wasm-build` | **0** — 88 lib warnings, **0** naming any symbol this plan touched |
| complexity | `pmat analyze complexity --max-cognitive 25`, read at `.summary.violations` | inherited **4**, **0 in `src/`** |
| new suite | `cargo nextest run -E 'binary_id(pmcp::v2_tasks_client_era)'` | **0** — **21 tests, 21 passed** |

`cargo public-api` added exactly: `Client::tasks_cancel_ack`,
`Client::tasks_get_detailed`, `Client::tasks_update`,
`Client::wait_for_task_with_inputs`, `TaskV2::to_v1`. **`ToolCallResponse`'s
variants and payload types are unchanged** (T-114-105).

`Cargo.toml` and `Cargo.lock` are byte-unchanged (`git diff --stat 22b36b5f..HEAD --
Cargo.toml Cargo.lock` is EMPTY — T-114-SC). `.planning/REQUIREMENTS.md` is
untouched (0-byte diff) and `requirements mark-complete` was deliberately NOT run:
TASK-01..06 flip as a GROUP and `114-SPEC-RECHECK.md`'s `## Verdict` is still
`PENDING`. `crates/pmcp-tasks` has a zero-byte diff. `src/types/mrtr.rs` gained
only the two constants — **no `tasks/*` row in `MRTR_METHODS`**, and both method
tables are byte-identical to plan-start.

### `make quality-gate` could not reach exit 0 in this session — and it is D-114-A

The full gate exits **2**, and every attempt fails with EXACTLY ONE test panicking
at the pre-existing native-roots `.expect` in `src/shared/streamable_http.rs:458`:

```
Failed to load native root certificates: Custom { kind: NotFound, error:
  "no native root CA certificates found (errors: [ … kind: Os(Error { code: -36, … }) … ])" }
```

Four runs were measured, and the identity of the failing test **MOVED between
them** while the panic site did not:

| Run | Threads | Failing test(s) |
|-----|---------|-----------------|
| 1 (sandboxed shell) | default | **14**, all `shared::streamable_http::tests` |
| 2 (unsandboxed) | `NEXTEST_TEST_THREADS=4` | **8**, all `session_validation_tests` |
| 3 | `RUST_TEST_THREADS=1` | **1**, `session_validation_tests::test_double_initialization_rejected` |
| 4 (`make test-integration` alone) | `RUST_TEST_THREADS=1` | **1**, `sse_middleware_integration::test_middleware_modifies_request_headers` |

A regression does not relocate itself to a different file between runs. Both
files are ones this plan never touched, and both pass in ISOLATION:
`cargo test --test session_validation_tests` is **10/10 ok**;
`cargo test --test v2_tasks_client` is **10/10 ok**;
`cargo test --test sse_middleware_integration test_middleware_modifies_request_headers`
is **1/1 ok**. This is the D-114-A addendum recorded by 114-12, one notch worse
in this session: even `RUST_TEST_THREADS=1` leaves one keychain-denied read per
full run.

**Grep the stderr for `no native root CA certificates found` before bisecting
anything.** Aggregate over the longest clean pass (`gate5`, up to the single
failure): **2373 passed / 1 failed / 79 ignored across 124 result lines**, plus
the separately-run tail legs above.

### The one REAL failure the gate caught

`tests/v2_tasks_client.rs :: tasks_list_emits_an_empty_mcp_name` — 114-06's
control, whose vehicle this plan removed. Diagnosed, fixed at the transport
layer, and committed separately (`fix(114-19): …`). Recorded under Deviations.

## For 114-17

The paired example depends on this surface. Precisely:

```rust
// create — a declaring v2 client gets a handle from an ordinary tools/call
let response: ToolCallResponse = client.call_tool_with_task(name, args).await?;
match response {
    ToolCallResponse::Task(task) => { /* task.task_id, task.status, task.ttl, task.poll_interval */ }
    ToolCallResponse::Result(result) => { /* an ordinary CallToolResult */ }
}

// poll to terminal — v2 does ONE round trip per tick and never calls tasks/result
let result: CallToolResult = client.wait_for_task(&task_id, WaitForTaskOptions::default()).await?;

// poll THROUGH a pause — the responder receives the task's own inputRequests
let result = client
    .wait_for_task_with_inputs(&task_id, WaitForTaskOptions::default(), |requests: InputRequests| async move {
        Ok(build_responses(requests))   // -> pmcp::Result<InputResponses>
    })
    .await?;

// the pieces, if the example wants to show the loop by hand
let detailed: DetailedTaskV2 = client.tasks_get_detailed(&task_id).await?;   // v2 only
client.tasks_update(&task_id, responses).await?;                            // v2 only, -> ()
client.tasks_cancel_ack(&task_id).await?;                                   // -> (), one round trip
let task: Task = client.tasks_cancel(&task_id).await?;                      // v2: ack + re-read
```

Facts the example must not fight:

- **`tasks_result` and `tasks_list` are compile-time fine and runtime-refused on
  v2.** Calling either from a v2 example produces `Error::is_retired_on_v2()`
  with zero bytes sent. Do not "fix" that by building a v1 client for the tasks
  half.
- **`tasks_get` on v2 returns a `Task` with `ttl` / `poll_interval` populated
  from `ttlMs` / `pollIntervalMs`.** No example-side remap is needed or wanted.
- **A v2 `tasks/get` on a missing / other-owner / EXPIRED task is ONE `-32602`
  answer with one oracle-free message.** Do not write an example that
  distinguishes them.
- **`tasks_cancel` on v2 costs TWO round trips and may report `working`.**
  Cancellation is cooperative; if the example wants one round trip and no status
  claim, use `tasks_cancel_ack`.
- **The responder callback is `FnMut(InputRequests) -> impl Future<Output =
  pmcp::Result<InputResponses>>`.** Keys must match the server's request keys
  exactly.
- **The client must declare the extension** (`ClientBuilder::with_tasks_extension()`)
  or 114-12's v2 create trigger never fires and the tool answers with an ordinary
  result.

## Commits

| Hash | Message |
|------|---------|
| `5d1eb97c` | `feat(114-19): era-aware v2 task decoding, tasks_update, and an inline-terminal poller` |
| `0513f524` | `test(114-19): raw-frame client decoding suite with v1 controls` |
| `e25ce06c` | `test(114-19): split the un-negotiated refusal test so its controls stay orthogonal` |
| `56a2c4b8` | `fix(114-19): drive the empty-Mcp-Name control at the transport, not through tasks_list` |

## Self-Check: PASSED

All eight files verified present on disk; all four commit hashes verified in
`git log --oneline --all`.
