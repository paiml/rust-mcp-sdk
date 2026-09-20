# Phase 114 — Deferred Items

Out-of-scope discoveries logged during execution. These were **measured, attributed and
NOT fixed** — each is either pre-existing and unrelated to the plan that found it, or owned
by a later plan.

**Closed out by `114-18` on 2026-08-01.** Every item below now names an owner or says explicitly that
it is **unowned**. An unowned item is acceptable; an undocumented one is not.

---

## ID collisions, resolved (`114-18`, 2026-08-01)

**MEASURED, not suspected:** `grep -n "^## " deferred-items.md` showed **`D-114-M` used three times**
and **`D-114-N` twice**. Three plans appended entries without checking the file for the next free
letter, so `D-114-P`'s *"Related: D-114-M (114-14)"* pointed at an ambiguity rather than an entry —
which defeats the point of an ID.

Resolved by keeping the ID for whichever entry existing documents already cite, and renumbering the
rest into free letters (`R` was free: `114-16-SUMMARY.md` § *Deviations* records `D-114-R` only as a
**corrected commit-message typo**, never as an assigned ID).

| Old ID | Filed by | Subject | **New ID** | Cited elsewhere by the old ID? |
|---|---|---|---|---|
| `D-114-M` | 114-13 | `handle_tasks_update` default answers `-32603` | **`D-114-M`** (kept) | yes — `114-13-SUMMARY.md:240,366` |
| `D-114-M` | 2026-07-29 spec run | published core schema not vendored | **`D-114-R`** | **no** |
| `D-114-M` | 114-14 | a `TaskRouter` decodes `tasks/update` unaided | **`D-114-T`** | **yes** — `114-14-SUMMARY.md:49,418`, `D-114-P` *Related*, `STATE.md` |
| `D-114-N` | 2026-07-29 spec run | nothing watches `ext-tasks` | **`D-114-S`** | **no** |
| `D-114-N` | 114-14 | a store without inputs answers `TASKS_NOT_ENABLED` | **`D-114-N`** (kept) | yes — `114-14-SUMMARY.md:50,419`, `STATE.md` |

**Landed SUMMARY files are NOT being rewritten.** Rewriting a landed artifact to hide an
inconsistency is worse than a redirect: a reader arriving from `114-14-SUMMARY.md` should read
`D-114-M` as **`D-114-T`**, and the renamed entries each carry that note at their own heading.

**For the next plan that appends here:** the next free letter is **`D-114-X`**. Run
`grep -c "^## D-114-<letter>" deferred-items.md` before choosing one.

---

## Ledger completeness sweep (`114-18` Task 3, 2026-08-01)

The plan required this ledger to account for four classes of item beyond what execution discovered.
Each is resolved below, with the entry that owns it.

| Class the plan named | Where it lives now |
|---|---|
| Every finding or defect-not-fixed recorded in any `114-*-SUMMARY.md` | `D-114-A` … `D-114-W`, individually below |
| Server-side `Mcp-Name` enforcement for `tasks/*`, left OFF by DQ4 | **`D-114-C`** — already filed, owner **Phase 118** |
| The `notifications/tasks` push surface (spec **MAY**, declined this phase) | **`D-114-X`**, added below |
| The four still-deferred `114-CONTEXT.md` items | **`D-114-Y`**, added below |
| The inherited unowned Phase-113 items | **§ Inherited from Phase 113**, at the foot of this file |

**Two items are DELIBERATELY not in this ledger, and the reason is worth stating.** `D-114-B` and
`D-114-H` are recorded as **FIXED / CLOSED** in place rather than deleted, so a reader who follows a
citation to them finds the closure rather than a missing entry.

---

## D-114-A — `native root certificates` keychain flake in `shared::streamable_http::tests`

**Found by:** 114-04 (Task 2 verification)
**Status:** open, unowned, pre-existing, environment-caused
**Severity:** low (intermittent CI/local noise, not a product defect)

`shared::streamable_http::tests::v2_error_envelope::v1_still_errors_on_the_status_alone`
FAILED once during a broad nextest filter run, panicking at the **pre-existing** `.expect`
in `src/shared/streamable_http.rs:458`:

```
Failed to load native root certificates: Custom { kind: NotFound, error:
"no native root CA certificates found (errors: [Error { context: \"failed to load user
trust settings\", kind: Os(Error { code: -36, message: \"I/O error.\" }) }, ...])" }
```

**Measured, not assumed:**

- Re-run **in isolation: PASS** (`-E 'test(v1_still_errors_on_the_status_alone)'`, 1 passed).
- Re-run of the **same broad filter: 136/136 passed.**
- `df -h /` at the time of failure: **19 GiB available** — so this is **NOT** the known
  disk-exhaustion mode recorded in memory (`project_disk_exhaustion_fake_test_failures`),
  which presents with the same `ioErr -36` signature on a full volume. It fires here under
  concurrent-load contention on the macOS keychain instead.
- `make quality-gate` subsequently ran **exit 0** with this test green.

**Why not fixed here:** 114-04 changed only `src/server/task_store.rs` and
`src/server/tasks.rs`. The panicking `.expect` is in the transport layer, is pre-existing,
and hardening it (fall back to webpki roots, or surface a typed error) is a transport
decision with its own blast radius. Fixing it inside a task-store seam plan would bury
that change.

**Suggested owner:** whichever later plan next touches `src/shared/streamable_http.rs`,
or a standalone hardening plan. The narrow fix is to stop `.expect`-ing on
platform-verifier root loading in a test-reachable path.

**Addendum, 114-12 (2026-07-28) — two REPRODUCIBLE triggers, not just "intermittent".**
114-12 hit this hard enough to be worth pinning down, because it presents as 14 code
regressions in files the plan never touched:

1. **A SANDBOXED shell reproduces it 100% of the time.** Every run of
   `cargo nextest run --features full -E 'binary_id(pmcp) and test(/collected_body_cap/)'`
   under the default sandboxed Bash failed **8/8** with the `ioErr -36` signature; the
   identical command with the sandbox disabled passed **8/8**. The keychain read is simply
   denied there. `df -h /` showed 39 GiB free, so again NOT the disk-exhaustion mode.
2. **Unsandboxed, it is PARALLELISM-sensitive.** The default `-j <ncpu>` produced 14 then 4
   failures across two broad runs; `-j 4` produced **1845/1845 green** and `-j 2` re-ran the
   4 stragglers green. The affected tests are always the same set in
   `shared::streamable_http::tests`.

**Practical guidance for later plans:** run test suites with the sandbox disabled and, for
broad selectors, `-j 4` (or `NEXTEST_TEST_THREADS=4` for `make quality-gate`, which is how
114-12's gate ran to exit 0). A failure whose stderr contains `no native root CA
certificates found` is THIS item, not the plan under test — check for that string before
bisecting anything.

---

## D-114-B — 1 ms-TTL setup races in `InMemoryTaskStore` expiry tests

**Found by:** 114-04 (Task 2)
**Status:** **FIXED for the two occurrences in `src/server/task_store.rs`**; the pattern may
exist elsewhere.

`cleanup_expired_drops_result` created a task with a **1 ms** TTL and then called
`set_result` on it. Every `InMemoryTaskStore` write runs through `Self::validate_access`,
which returns `TaskStoreError::Expired` — not a lost write — once the TTL has elapsed, so
under load the setup lost to the clock and the test failed at its `unwrap()` for a reason
unrelated to the property it asserts. **Observed firing** on run 1 of a 5-run repeat while
this plan was adding tests to the same binary.

Both occurrences in this file are now widened to 500 ms with the reason written at the site
(Rule 1, committed in `c3ff793e`). Recorded here because the same
`default_ttl_ms: Some(1)` + `sleep(10)` shape may appear in other expiry tests across the
tree; a sweep is out of scope for a seam plan.

---

## D-114-C — server-side `Mcp-Name` enforcement for `tasks/*` is deliberately OFF

**Found by:** 114-06 (Task 3) — a **scoped decision**, not a discovery
**Status:** open, owned by **Phase 118** (conformance hardening)
**Severity:** low today, rising once the ecosystem's clients are conformant

114-06 made pmcp's CLIENT emit the spec's `Mcp-Name: <params.taskId>` on `tasks/get`,
`tasks/update` and `tasks/cancel` (a spec **MUST**, inventory row 34). The SERVER half was
deliberately left unchanged: `is_name_bearing_method` in
`src/server/streamable_http_server.rs` still reads `logical_name_key`, which is derived from
`MRTR_METHODS` and therefore answers `None` for every `tasks/*` method, so
`cross_check_name` returns `Ok(())` for them before comparing anything.

**The tolerance this buys, stated plainly:** a pmcp v2 server accepts a conformant
`Mcp-Name: <taskId>` AND a legacy `Mcp-Name: ""`, and does **not** detect a header that
disagrees with `params.taskId`. That is what lets pre-existing clients keep working while
the ecosystem migrates.

**What turning it on would take:** point `is_name_bearing_method` at
`crate::types::mrtr::name_bearing_key` instead of `logical_name_key`. The routing-name half
already resolves through the combined lookup (`frame_routing_pair`), so the body value is
already available at the comparison site — one predicate is the whole change. It is a
BREAKING change for any client still sending the empty value, which is why it is a
separable decision rather than a line in a client plan.

The tradeoff is also recorded in the rustdoc on `TASK_NAME_BEARING_METHODS`, so a reader of
the table cannot miss it.

---

## D-114-D — an immediate `tasks/get` after create needs a strongly-consistent read

**Found by:** 114-07 (Task 3, `a_created_task_is_immediately_readable_from_its_returned_handle`)
**Status:** open, unowned — a BACKEND CONFIGURATION obligation, not a store defect
**Severity:** low today (no eventually-consistent backend is wired in CI), rising with the
first real DynamoDB deployment of the tasks extension

The tasks extension requires the handle a create returns to be resolvable straight away.
`GenericTaskStore::create` satisfies that structurally: it returns the record it just wrote
and issues **zero reads** of the new key — asserted by a get-counter on the backend double,
so "does not depend on read-after-write" is measured, not argued.

**What was measured, and it is the part worth carrying.** Driven through an
`EventuallyConsistentBackend` double whose `get` serves each key's PREVIOUS value once
before converging, a follow-up `store.get` on the freshly created key returns
`TaskError::NotFound`, and the next read succeeds. That is faithful to DynamoDB's default
eventually-consistent read, which may be served by a replica that has not yet received the
write. The record is durable throughout; only that one read is stale.

**A trap inside the trap.** The double got this wrong on the first attempt: it stored the
previous value as `Option<(bytes, version)>` and then `.flatten()`ed the lookup, so
`Some(None)` — "the key had no value before this write" — collapsed into "no staleness
recorded" and fell through to the converged value. The test PASSED for the wrong reason. A
staleness double must keep three cases distinct (`stale value` / `stale absence` /
`converged`); collapsing the middle one is exactly how it stops being faithful. The
corrected double is in `crates/pmcp-tasks/tests/input_delivery.rs` with the reason at the
site.

**What a deployment owes.** Either a strongly-consistent read on the `tasks/get` path
(DynamoDB's `ConsistentRead`, which is opt-in and costs double a read unit), or a client
retry on the first `NotFound` after a create. `DynamoDbBackend::get` in this crate does
**not** currently request a consistent read.

**Why not fixed here:** 114-07 is additive at the domain layer and changed no backend.
Flipping `DynamoDbBackend::get` to a consistent read is a cost and latency decision that
affects every existing v1 tasks deployment on that backend, not just the v2 input-delivery
path, so it does not belong inside a `tasks/update` plan.

**Suggested owner:** whichever plan next touches `crates/pmcp-tasks/src/store/dynamodb.rs`,
or the phase that first deploys the tasks extension against DynamoDB.

---

## D-114-E — `make test-feature-flags` is RED, and was already red before 114-07

**Found by:** 114-07 close-out (verification re-run)
**Status:** open, unowned, **pre-existing and PROVEN so**
**Severity:** medium — it is an acceptance criterion of 114-07 (and D-14 item 4) that no
plan in this phase can satisfy while it stays red, so it will keep being "failed" by
later plans that did not cause it

`make test-feature-flags` exits **2**. The failure is in row **1/4**, at its second
sub-command:

```
cargo clippy -p pmcp-tasks --no-default-features -- -D warnings   → exit 101
```

56 `dead_code` warnings in the **root `pmcp` lib** are promoted to errors by the target's
`-D warnings`. Building `pmcp` through `-p pmcp-tasks --no-default-features` selects a
reduced pmcp feature set in which those items have no caller.

**Attributed by file** (identical at HEAD and at the 114-07 base commit):

| File | Errors |
|------|--------|
| `src/types/mrtr.rs` | 42 |
| `src/server/subscriptions.rs` | 7 |
| `src/server/core.rs` | 4 |
| `src/shared/sse_parser.rs` | 2 |
| `src/server/mod.rs` | 1 |

**Zero** are in `crates/pmcp-tasks/` and **zero** are in `src/server/task_store.rs` — i.e.
none are on 114-07's surface. The named items (`write_canonical`, `salient_params`,
`salient_param_digest`, `project_capabilities_for_v2`, `EXPERIMENTAL_TASKS_KEY`, …) belong
to Phase 113 and to 114-05, both of which landed before 114-07.

**Measured, not argued.** A detached worktree was created at 114-07's base commit
`4327b246` (114-06's last commit) with its own `CARGO_TARGET_DIR`, and the same commands
were run there:

| Command | at base `4327b246` | at HEAD `9081be3b` |
|---------|--------------------|--------------------|
| `make test-feature-flags` | exit **2**, 56 errors | exit **2**, 56 errors |
| `cargo clippy -p pmcp-tasks --no-default-features -- -D warnings` | exit **101** | exit **101** |

Same exit codes, same error count, same per-file distribution. The worktree and its target
directory were removed afterwards (`git worktree list` back to its three pre-existing
entries).

**The dev-dep-free rows the plan actually cares about are GREEN.** 114-07's `verification`
block singles out "the `cargo check -p pmcp-tasks --features X` rows are the dev-dep-free
ones", and every one of those exits 0 at HEAD:

```
cargo check -p pmcp-tasks --no-default-features        exit 0
cargo check -p pmcp-tasks --features dynamodb          exit 0
cargo check -p pmcp-tasks --features redis             exit 0
cargo check -p pmcp-tasks --features dynamodb,redis    exit 0
cargo check -p pmcp-tasks                              exit 0
```

So the *compile* claim across the four feature rows holds; what is red is the root crate's
dead-code hygiene under a reduced feature set.

**Why not fixed here:** the fix lands in five root-`pmcp` files owned by other plans
(Phase 113's MRTR module and subscriptions; 114-05's capability projections). Adding
`#[cfg]` gates or `#[allow(dead_code)]` across them from inside a `pmcp-tasks` store plan
would bury a decision about the root crate's feature hygiene in a plan whose declared
`files_modified` is five files under `crates/pmcp-tasks/`. It is also NOT caught by
`make quality-gate` or CI, which lint the root crate with its own generous allow-list.

**Suggested owner:** a Phase 118 (conformance/hardening) item, or whichever plan next
touches `src/types/mrtr.rs`. The narrow fix is to gate or `allow` the reduced-feature dead
code at each site, then re-run `make test-feature-flags` to green.

---

## D-114-F — a known method with malformed params answers `-32601`, not `-32602`

**Found during:** `114-09` Task 3, while building the case-4-before-case-5 ordering probe.

**Measured (v2 Streamable HTTP, `tests/v2_tasks_owner_binding.rs`):** a `tasks/get`
carrying `{"taskId": 12345}` — a well-known method with a `taskId` of the wrong JSON
type — is answered:

```json
{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found: tasks/get"},"id":40}
```

`-32601 METHOD_NOT_FOUND`, not `-32602 INVALID_PARAMS`. The method plainly exists and is
served; only its params are malformed.

**Cause:** `ClientRequest` is deserialized at INGRESS, before any dispatch runs. The enum
is method-tagged, so a params-shape mismatch makes the `tasks/get` variant fail to match
and the request falls through to the not-a-known-method arm. Nothing in
`route_tasks_endpoint` is reached.

**Why this matters beyond tidiness:** it makes a whole class of ordering claims
unobservable from outside. `114-09`'s plan specified proving "the auth refusal fires
before the params parse" by sending malformed params with no credentials and asserting
`-32003` rather than `-32602`. That probe cannot work: the request never reaches the
refusal chain at all. `114-09` proved the ordering a different and stronger way instead
(identical well-formed params, differing only in the credential; the authenticated leg
reaches the store and hears "task not found", the unauthenticated leg hears `-32003` and
learns nothing) and pinned the measured caveat as leg C of
`an_unauthenticated_caller_is_refused_before_the_params_parse`.

**Why not fixed here:** the fix is in the transport/ingress request-deserialization path
(`ClientRequest`'s untagged fallthrough), not in `src/server/task_dispatch.rs`. It is
NOT specific to `tasks/*` — every method with typed params has the same behaviour — so
changing it from inside a tasks owner-binding plan would move a global wire behaviour
under a `files_modified` list of three server files, and would need its own golden-byte
review across every existing suite that may depend on the current code.

**Suggested owner:** a conformance/hardening plan, or whichever plan next touches
`ClientRequest`'s deserialization. The narrow fix is to match the method name FIRST and
only then attempt the params, so a known method with bad params yields `-32602`.

---

## D-114-G — `pmcp::testing` is now a wire-behaviour seam, not only a wire-SHAPE seam

**Found during:** 114-10 Task 1 (recorded 2026-07-28)
**Status:** open — needs an owner
**Severity:** low (documentation / module-charter drift, no wire impact)

`src/testing/mod.rs` opens with a charter: it exists so a `tasks/*` wire SHAPE is proven
against the real client deserialization types instead of an author-written fixture
(the Phase 101 tools-as-tasks incident).

114-10 added something structurally different to that module: `v2_result_envelope` /
`v1_result_envelope` do not check a shape against a type — they RUN the production
`inject_v2_result_envelope` and return the response bytes plus the `tracing` warnings it
emitted, so a test can distinguish "the key was never there" from "the registry removed
it". That is a behaviour-observation seam. It also carries a small hand-written
`tracing::Subscriber` (no `tracing-subscriber` dependency, so no feature gate beyond the
one `server::core` already has).

The seam is correct and its own rustdoc explains itself, but the MODULE-level doc comment
still describes only the shape-conformance charter, so a reader arriving at the top of the
file gets a description that no longer covers everything below it. Not fixed here because
rewriting the module charter of a file six Phase-113 plans build on is a wider edit than a
row-23 fix should carry, and the charter sentence is load-bearing for those plans.

**Suggested owner:** whichever plan next adds to `pmcp::testing`, or the phase's closing
documentation plan.

---

## D-114-H — `ReservedFieldOwner::TasksDispatch` has no PRODUCTION constructor yet

**Found during:** 114-10 Task 2 (recorded 2026-07-28)
**Status:** open — owned by **114-12**, tracked here so it cannot be forgotten
**Severity:** low (by design; the tripwire is already armed)

`ReservedFieldOwner::TasksDispatch` exists and is honoured by the registry, but its ONLY
constructor today is the `pmcp::testing` reserved-field seam. Nothing in `src/` dispatches
a v2 `tasks/get` that names it — 114-10 supplies the egress PERMISSION and dispatches
nothing, deliberately.

Measured, not assumed (all with `RUSTFLAGS="-D warnings" cargo clippy --lib`):

| feature selection | without the `allow` | with it |
|---|---|---|
| `--features full` | exit **0** (the testing seam constructs it) | exit 0 |
| `--no-default-features --features streamable-http` | exit **101**, exactly one error: `variant TasksDispatch is never constructed` | exit 0 |
| `--no-default-features` | exit 101, includes `variants Mrtr and TasksDispatch are never constructed` | (pre-existing D-114-E failures) |

The `allow` is therefore scoped `not(feature = "testing")` and NOT `not(test)`: `make lint`
runs `--lib --tests` with `full`, and the `--lib` half is a non-test build with `testing`
ON, so a `not(test)` scope would deactivate the lint for exactly that half. Under the
feature scope, BOTH halves of the gate lint the variant — deleting the seam before 114-12
wires production fails the gate rather than passing quietly.

**Closure condition:** 114-12 names `ReservedFieldOwner::TasksDispatch` at the v2
`tasks/get` egress; the `#[cfg_attr(not(feature = "testing"), allow(dead_code))]` can then
be deleted and the `--no-default-features --features streamable-http` row above becomes
exit 0 without it.

**CLOSED by 114-11 (2026-07-28) — one plan earlier than expected.**
`TaskDispatch::route_tasks_get` returns `DispatchEnvelopeClaim::TASKS_INPUT_REQUIRED` for an
`input_required` task, and that constant constructs `ReservedFieldOwner::TasksDispatch`.
`server::task_dispatch` is gated only on `not(target_arch = "wasm32")` and on **no feature**,
so the production constructor is present on every native build. Re-measured with
`RUSTFLAGS="-D warnings" cargo clippy --lib` before removing the `allow`: `--features full`
exit **0**; `--no-default-features --features streamable-http` reports **no** error naming
`TasksDispatch`; `--no-default-features` still reports its **55** pre-existing D-114-E
dead-code errors and **none** of them names `TasksDispatch` or the `Task` disposition variant.
Both `#[cfg_attr(…, allow(dead_code))]` guards — on `ReservedFieldOwner::TasksDispatch` and on
`ResponseDisposition::Task` — are therefore **deleted**, and the comments in their place record
the measurement so a future reader does not re-add them defensively.

**Note for 114-12:** the "supply the owner from the tasks route" item STATE.md assigned to you is
DONE. What remains yours is the v2 create TRIGGER. The claim plumbing you will find is:
`route_tasks_endpoint` and `maybe_build_task_created` / `build_task_created_response` return
`(JSONRPCResponse, DispatchEnvelopeClaim)`; `handle_request_internal` (core) and
`handle_client_request` / `process_client_request` / `handle_call_tool` (mod) carry a
`&mut DispatchEnvelopeClaim` out-param; both dispatch sites fold it with the MRTR egress's claim
through `DispatchEnvelopeClaim::or_egress`. Do NOT add a second path.

## D-114-I — `tests/common/v2.rs`'s tasks fixture now registers TWO tools

**Found during:** 114-11 Task 3 (recorded 2026-07-28)
**Status:** open — informational, no owner needed unless a later suite trips on it
**Severity:** low

`spawn_tasks_server` (and its new `spawn_tasks_server_with_store` primitive) registers
`long_task_tool` under `TASKS_TOOL_NAME` **and** `completing_error_task_tool` under
`COMPLETING_TOOL_NAME`. The second is required by `terminal_status_discipline`, which must
exercise the REAL create path for a synchronously-completing `isError: true` outcome rather
than poking the store into the answer it wants to assert.

Consequence a later plan should know about: any test against this shared fixture that asserts
on the **length or exact contents of `tools/list`** now sees two tools, not one. No suite does
today (verified: the eight tasks/MRTR binaries are green at 98/98), but a `tools/list`
cardinality assertion is a natural thing for 114-15's cross-caller matrix to write.

**Addendum, 114-12 (2026-07-28): the count is now THREE, not two.** `pausing_task_tool`
(`PAUSING_TOOL_NAME = "elicit_task"`) was added, because it is the first CLIENT-REACHABLE way
to produce an `input_required` task — before it, a suite had to reach past the wire and poke
`record_input_requests` on the store. 114-13 / 114-14 need a paused task the same way, which is
why it lives in the shared harness rather than in one suite. Re-verified after the addition:
`binary_id(pmcp) or <the eight tasks/MRTR binaries> or binary_id(pmcp::v2_tasks_create)` ran
**1858 passed / 4 failed**, and all four failures were D-114-A's keychain signature (green on
re-run at `-j 2`). Still no suite asserts `tools/list` cardinality.

## D-114-J — a v1 caller against the shared v2 harness must complete a real handshake

**Found during:** 114-11 Task 3 (recorded 2026-07-28)
**Status:** open — informational
**Severity:** low

`tests/v1_tasks_golden.rs` spawns with `spawn_stateless_config`, so its v1 requests need no
session. The shared harness's `spawn_tasks_server` uses `StreamableHttpServerConfig::default()`,
which is STATEFUL on purpose (RESEARCH Pitfall 1). A v1 request against it without a session id
answers **`-32600 "Session ID required for non-initialization requests"`** — which looks like a
tasks bug and is not one.

`tests/v2_tasks_shapes.rs::v1_session_headers` is the fix and the pattern: POST a real v1
`initialize`, read `Resp::mcp_session_id`, then carry
`pmcp::shared::http_constants::MCP_SESSION_ID` on every later v1 request. Any later plan writing a
v1 row against the shared harness should reuse that shape rather than rediscovering the `-32600`.

---

## D-114-K — a declaring v2 client now gets a task handle from ANY task-capable tool

**Found during:** 114-12 (recorded 2026-07-28)
**Status:** open — DELIBERATE, re-recorded from the plan's own scope decision, not a defect
**Severity:** medium (a UX / client-compatibility question, not a correctness one)
**Suggested owner:** a post-114 client-experience plan, or the v2.6 AI-Package milestone

114-12 made the v2 create trigger "the client declared `io.modelcontextprotocol/tasks` on this
request". The trigger is per-REQUEST and per-CLIENT, but it is **not** per-TOOL: once a client
declares the extension, EVERY tool on that server whose `TaskSupport` is `Required`/`Optional`
and which returns a task-shaped value will hand that client a `CreateTaskResult` instead of a
terminal result. That is what the extension's own text requires (the server is the sole decider,
and the declaration is its `MUST NOT` precondition), and it is what makes TASK-04 demonstrable at
all — DQ1, user-approved 2026-07-27.

What remains DEFERRED is the surrounding design the original CONTEXT.md deferral was plausibly
protecting:

- a client that declares the extension once, at the transport layer, has no way to say "not for
  THIS call" — there is no v2 equivalent of v1's per-request `task` field;
- a v2 client library therefore has to be prepared to handle a task handle back from any
  task-capable tool, and the ergonomics of that (auto-poll? surface the handle? opt out?) are
  unspecified here;
- there is no server-side per-tool or per-client policy knob for "advertise task support but
  only materialize a task when X".

None of this blocks the phase: the gate is conformant and the behaviour is the spec's. It is
recorded so a later reader does not mistake the absence of an opt-out for an oversight.

---

## D-114-L — the retired `tasks/*` methods can no longer be observed on the client's wire

**Found during:** 114-19 (Task 1, surfaced by the quality gate)
**Status:** open — DELIBERATE consequence, recorded so a later reader does not re-add a
control that cannot work
**Severity:** low (a testability narrowing, not a defect)
**Suggested owner:** whichever plan next wants to assert client-side header derivation

114-19 makes `Client::tasks_result` and `Client::tasks_list` fail fast LOCALLY on v2 with
zero bytes on the wire. `tasks/list` and `tasks/result` are also the ONLY two `tasks/*`
methods absent from `TASK_NAME_BEARING_METHODS`. Those two facts together mean:

> a pmcp v2 CLIENT can no longer emit a `tasks/*` method that is not name-bearing.

114-06's `tasks_list_emits_an_empty_mcp_name` used exactly that shape as its vehicle and
had to be re-pointed at `StreamableHttpTransport::send_raw`, which is the layer that owns
the derivation anyway. That repair is in-tree and green.

What is DEFERRED is the general problem it exposes: any future assertion about "how does
the client emit headers for method X" is only reachable through the client API while X
remains callable. As more methods are retired on v2, more such controls will have to drop
to the transport layer. There is no shared helper for "hand this transport a raw frame and
read back what the socket saw" — the repaired test hand-rolls it in ~10 lines. If a second
test needs it, lift it into `tests/common/`.

---

## D-114-A addendum 2 — one keychain-denied read survives `RUST_TEST_THREADS=1`

**Found during:** 114-19 (quality gate, four measured runs)

114-12's addendum recorded that the `no native root CA certificates found` failure is
100% reproducible in a sandboxed shell and parallelism-sensitive when unsandboxed, and
that `-j 4` cleared it. In the 114-19 session that was no longer enough: an UNSANDBOXED
`make quality-gate` with `RUST_TEST_THREADS=1` still lost exactly ONE test per full run,
and **the identity of that test MOVED between runs** — `session_validation_tests::
test_double_initialization_rejected` in one, `sse_middleware_integration::
test_middleware_modifies_request_headers` in the next — while the panic site stayed the
pre-existing `.expect` at `src/shared/streamable_http.rs:458`. Both files pass in
isolation (10/10 and 1/1). A regression does not relocate itself between runs.

Practical guidance for the next executor: `make quality-gate` may not reach exit 0 on this
machine at all. Run the legs individually, grep every failure for the CA string FIRST, and
re-run the single failing binary in isolation before treating it as a regression. The real
fix — replacing the `.expect` at `streamable_http.rs:458` with a fallible path, or pinning
`webpki-roots` for tests — is still unowned.

### Process trap re-confirmed while writing THIS entry

`/usr/bin/cat` does not exist on macOS (it is `/bin/cat`), so a
`/usr/bin/cat >> deferred-items.md <<'EOF'` heredoc silently wrote NOTHING while the
`echo appended` on the following line printed a reassuring "appended". This is the exact
failure 114-10 recorded for `/usr/bin/cp`. **Verify the mutation landed before trusting
the message that says it did.**

---

## D-114-M — `TaskRouter::handle_tasks_update`'s default returns `-32603` where `-32601` is arguably right

> **KEEPS its ID (`114-18`, 2026-08-01).** Two later entries were also filed as `D-114-M`; they are
> renumbered **D-114-R** and **D-114-T**. `114-13-SUMMARY.md` lines 240 and 366 cite *this* entry.

**Found by:** 114-13 (decide-now item carried in from the phase brief)
**Status:** **STILL OPEN.** It was owned by 114-14, and 114-14 added the router branch — but it did
**not** change the default, which `114-18` re-measured at
`src/server/tasks.rs`: `handle_tasks_update`'s default still returns
`Error::internal("tasks/update not supported by this router")`, i.e. `-32603`. Reassigned to
**Phase 118**, alongside **D-114-P** and **D-114-T**, which are the same question about what a
`TaskRouter` owes a v2 caller.
**Severity:** low today (unreachable), medium once a caller exists

`src/server/tasks.rs:91-95` — the defaulted `TaskRouter::handle_tasks_update` returns
`Error::internal("tasks/update not supported by this router")`, which reaches the wire as
`-32603`. At the protocol level, "this router does not implement this method" is `-32601`,
and that is what the sibling no-backend refusals in `task_dispatch.rs` already emit
(`TASKS_NOT_ENABLED` / `TASKS_RESULT_NOT_SUPPORTED`, both `-32601`).

**Why 114-13 recorded it instead of changing it — and the reason is a measurement, not a
preference:**

```
$ grep -rn handle_tasks_update src/
src/server/tasks.rs:91:    async fn handle_tasks_update(&self, _params: Value, _owner_id: &str) -> Result<Value> {
```

**Zero callers, anywhere in the tree.** 114-13 routes `tasks/update` but stops at the gates;
delivery — and therefore the store-vs-router split that would first invoke this default — is
114-14's. Changing an unreachable default's wire code would be **unverifiable by
construction**: no negative control could fail it, which is precisely the "a property no
control fails" pattern this phase has now hit five times. It would also be a wire change
landed by a plan that cannot demonstrate its effect.

**For 114-14:** the moment you add the router branch, this becomes decidable and testable in
one step. Prefer `-32601` with a message distinguishable from `TASKS_NOT_ENABLED` (a router
that exists but does not do updates is a different fact from a server with no task backend at
all — T-114-33's distinguishability rule), and pin it with a control.

---

## D-114-A addendum 3 (114-13) — `make test-unit` reads `RUST_TEST_THREADS`, not `NEXTEST_TEST_THREADS`

**Found by:** 114-13 (two full gate runs)

The phase brief's trap list says to use `NEXTEST_TEST_THREADS=4`. That is correct for
`cargo nextest` invocations and **does nothing for the leg that actually fails**:
`make test-unit` runs `cargo test --lib --features "full"`, which reads `RUST_TEST_THREADS`.

Measured, same tree, same commit:

| parallelism | result | `no native root CA` messages |
|-------------|--------|------------------------------|
| default (`-j ncpu`) | `1750 passed; 14 failed` | **14** |
| `RUST_TEST_THREADS=4` | `1760 passed; 4 failed` | **4** |
| `RUST_TEST_THREADS=1` | **`1764 passed; 0 failed`, exit 0** | **0** |

Failure count equals CA-message count exactly at every level, and the population shrinks
monotonically to zero as parallelism drops. A regression does not do that. The panic is in
`StreamableHttpTransport::new_internal` — the **client transport constructor**, which runs
before any request — so no server-side change can reach it.

**Run `RUST_TEST_THREADS=1 make test-unit`.** The underlying fix (making the
`.expect("Failed to load native root certificates")` fallible, or pinning `webpki-roots` for
tests) is still unowned.

---

## D-114-R — the PUBLISHED core `2026-07-28` schema is not vendored and has no provenance tripwire

> **RENUMBERED by `114-18` (2026-08-01). This entry was filed as `D-114-M`, which collided with two
> other entries.** No document anywhere cites it by that ID, so this rename breaks nothing. See
> § *ID collisions, resolved* at the top of this file.

**Found by:** the 2026-07-29 spec re-verification run (`114-SPEC-RECHECK.md` § `### Verdict
re-verification` → `#### 2026-07-29`)
**Status:** open, unowned, newly created by an upstream publication event
**Severity:** medium — it leaves authoritative values sourced by inference rather than by a pin

`114-01` vendored `modelcontextprotocol/ext-tasks` `schema/draft/` at
`2c1425d9a288b9b1f489430fe1e00bb392b47e48` with a SHA-256 + git-blob provenance tripwire, because
that was the only schema this phase read. On **2026-07-29** the core specification published
`modelcontextprotocol/modelcontextprotocol` `schema/2026-07-28/` (`schema.ts`, `schema.json`,
`schema.mdx`, `examples/`), declaring `LATEST_PROTOCOL_VERSION = "2026-07-28"`.

That published core schema now **governs** three groups of values this phase relies on:

| Group | Values | Inventory rows |
|---|---|---|
| The extension capability map | `extensions?: { [key: string]: JSONObject }` on `ClientCapabilities` **and** `ServerCapabilities` | 1-3 |
| Error codes | `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021`, `INVALID_PARAMS = -32602`, and the **absence** of `-32002` | 29, 30, 32, 33 |
| The result discriminator | `resultType: ResultType`, `ResultType = "complete" \| "input_required" \| string` | 16-20 |

**Nothing in the repo pins any of them.** They are currently asserted from a one-off HTTP read
recorded in prose. If upstream renumbers a code or narrows `ResultType` again, no test fails.

**Why not fixed at discovery:** vendoring a second schema tree is `114-01`-shaped work — a
directory, a `PROVENANCE.md` with dual digests, and a tripwire test — and the discovery was made
during a read-only verification run with no shell available (the harness Bash classifier was down),
so nothing could be fetched, hashed or committed. It is also genuinely optional for *this* phase:
the D-18 hold is still engaged and no requirement flips, so the pin buys future-run cheapness and
drift detection, not present correctness.

**Suggested owner:** a small standalone plan mirroring `114-01` (vendor + `PROVENANCE.md` + tripwire
over `schema/vendored/core-2026-07-28/`), or `114-18` if it is willing to widen. **Note the
asymmetry deliberately:** vendoring the CORE schema is safe because it is published and immutable;
vendoring anything from `ext-tasks` beyond the existing `draft` pin is **not**, because that
repository has not published and `## Recorded Exception` forbids promoting draft to authoritative.

---

## D-114-S — `ext-tasks` publishing is now the SOLE remaining D-18 trigger, and nothing watches it

> **RENUMBERED by `114-18` (2026-08-01). This entry was filed as `D-114-N`, which collided with
> 114-14's entry of the same name.** No document cites it by that ID, so this rename breaks nothing.
> `114-SPEC-RECHECK.md` § *Verdict re-verification* → `#### 2026-08-01` now cites it as **D-114-S**.
> See § *ID collisions, resolved* at the top of this file.

**RE-MEASURED 2026-08-01** (plan `114-18`, prescribed `gh api` form): still true, and now
**strictly cheaper to check and correspondingly easier to forget**. `ext-tasks` has **0 tags**, **0
releases**, `schema/` = `draft` only, `specification/` = `draft` only, and its `schema/draft` is
still at `29f83d5` (2026-05-22) — unchanged in the ten weeks since. Nothing in CI or the repo polls it.

**Found by:** the 2026-07-29 spec re-verification run
**Status:** open, unowned
**Severity:** medium — six held requirements hinge on an event no mechanism detects

The DQ6 trigger required a versioned (non-`draft`) schema directory in **both** repositories. As of
2026-07-29 the core half is **satisfied** and the extension half is **not**:
`modelcontextprotocol/ext-tasks` still carries `schema/draft/` and `specification/draft/` only, with
**no tags and no releases**, 17 commits on `main`, and a README describing an experimental extension
*"under development"* toward SEP-2663. Its `schema/draft` has not changed since **2026-05-22**
(`29f83d5`).

So the condition that releases TASK-01…TASK-06 — and, separately, re-enters the Phase-114
contract-first waiver — has collapsed from a two-repository check to a **one-repository** check.
That is strictly cheaper to monitor, and correspondingly cheaper to forget.

**What is missing:** any detector. `114-01`'s provenance tripwire watches the **vendored bytes** for
local tampering; it cannot see upstream publishing a new directory. Nothing in CI or the repo polls
`ext-tasks`. Today the trigger is noticed only if a human happens to look — which is precisely the
*"a waiver that quietly becomes permanent"* failure mode (**T-114-107**) that
`114-CONTRACT-DECISION.md` § 2 observed in Phase 113 in the wild.

**Why not fixed at discovery:** a watcher is infrastructure (a scheduled CI job, or a documented
manual checkpoint), not a wire value, and the run that found it had no shell.

**Suggested owner:** a scheduled check — e.g. a low-frequency CI workflow asserting
`gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'` still yields only
`draft`, failing loudly when it does not. Cheap, and it converts "someone must remember" into "the
build tells us". Until then, the recorded fallback is `114-SPEC-RECHECK.md` § `## Procedure`, re-run
by hand.

---

## D-114-T — a `TaskRouter` serving `tasks/update` performs its OWN decode, unaided (114-14)

> **RENUMBERED by `114-18` (2026-08-01). Filed as `D-114-M`, which collided with the 114-13 entry
> above.** **This one IS cited elsewhere by the old ID** — `114-14-SUMMARY.md` lines 49 and 418,
> `D-114-P`'s *Related* line, and `STATE.md`'s 114-14 paragraph all say `D-114-M`. **Those documents
> are not being rewritten; this ledger is authoritative.** A reader arriving from any of them should
> read `D-114-M (114-14)` as **`D-114-T`**. See § *ID collisions, resolved* at the top of this file.

**Discovered:** 2026-07-31, while landing `tasks/update` delivery (plan 114-14).

`TaskDispatch::deliver_tasks_update` is store-first with a router fall-through, the same
precedence every other `tasks/*` route uses. The STORE leg reads the server-recorded kinds through
`TaskStore::task_input_snapshot` and types every value with `InputResponse::decode_for`, which is
the whole point of the route (D-113-O). The ROUTER leg cannot: a `TaskRouter` is out-of-tree code
holding its own record, and the trait has no snapshot accessor, so it receives `params` VERBATIM
and owns its own decode.

**What IS still guaranteed on that path:** the four `inputResponses` bounds have already fired
(they run at case 6, before the store/router split), so a router never sees an unbounded payload;
the owner is the identity table's and is passed as `owner_id`, never read from `params`; and the
trait rustdoc on `TaskRouter::handle_tasks_update` states both facts.

**What is NOT:** nothing stops an out-of-tree router from running the untagged decoder on those
values and reproducing D-113-O inside its own crate. The kind-direction property is enforced for
`TaskStore` implementations and only *documented* for `TaskRouter` ones.

**Why not fixed here:** closing it means widening the `TaskRouter` trait with a snapshot accessor
(or handing it a pre-typed `InputResponses`, which requires kinds this dispatcher does not have for
a router-backed task). Both are additive trait changes with real design questions about who owns
the record, and `TaskRouter` is the *legacy experimental* backend — `TaskStore` is the supported
one. Doing it inside a delivery plan would have been an architectural change smuggled in as a fix.

**Suggested owner:** whichever plan next revisits `TaskRouter`. If the answer is "routers are
deprecated", the honest closure is to say so rather than to widen the trait.

---

## D-114-N — a store that does not accept inputs answers `-32601 "Tasks not enabled"` (114-14)

**Discovered:** 2026-07-31, writing the router fall-through arm of `deliver_tasks_update`.

Reachable in exactly one configuration: a server with a `TaskStore` whose `supports_inputs()` is
`false`, and NO `TaskRouter`. Case 2 of the gate chain already answered for a server with no task
backend at all, so this arm is about a backend that exists and cannot do this one thing.

It reuses `TASKS_NOT_ENABLED` — the FROZEN sibling message its three `tasks/*` siblings emit —
rather than minting a fifth `-32601` sentence, deliberately: `the_minus_32601_conditions_are_mutually_distinct`
asserts the population of those messages is pairwise distinct, and a fifth would have to be added
to that population and justified. The reuse is slightly imprecise ("Tasks not enabled" on a server
that plainly has tasks) and strictly better than a fifth near-synonym nobody can tell apart.

**Why not fixed here:** the precise message is `tasks/update input delivery is not supported by
this server's task backend`, which is a NEW member of the distinguishability set and therefore a
change to a frozen contract, not a delivery detail. `InMemoryTaskStore` and `pmcp-tasks`'
`GenericTaskStore` both return `true`, so no in-tree configuration reaches this arm.

**Suggested owner:** a plan that is already touching the `-32601` message population.

## D-114-O — a `pmcp` test cannot exercise `pmcp-tasks` behaviour, so a cross-crate security claim splits in two (114-15)

**Discovered:** 2026-07-31, writing `v1_local_and_v2_anonymous_buckets_are_disjoint`.

`pmcp-tasks` is a workspace MEMBER but not a dependency of the root `pmcp` crate in any profile —
not `[dependencies]`, not `[dev-dependencies]`, not behind a feature. A `tests/*.rs` integration
test of `pmcp` therefore cannot call `pmcp_tasks::…` at all.

The concrete consequence for TASK-05: the plan asked one test to assert BOTH that the v1 `"local"`
bucket and the v2 anonymous bucket are disjoint AND that `pmcp-tasks`' `is_anonymous_owner` treats
`""` and `"local"` identically. The first half is behavioural and was measured over a live socket
plus directly against the in-crate `InMemoryTaskStore`. The second half could only be asserted at
the SOURCE — a substring tripwire over `crates/pmcp-tasks/src/store/generic.rs` and
`crates/pmcp-tasks/src/store/backend.rs`. That is a real assertion (it rots loudly if the predicate
is split or `make_key` drops its owner prefix) but it is a weaker instrument than execution, and a
reader should know which half is which.

**Why not fixed here:** the fix is `pmcp-tasks = { path = "crates/pmcp-tasks" }` under the root
`[dev-dependencies]`, which is a manifest change. 114-15 is a coverage-only plan whose own threat
register (T-114-SC) requires `Cargo.toml`/`Cargo.lock` to be byte-unchanged, and a dev-dependency
edge from the core crate onto an experimental one deserves its own decision rather than arriving as
a side effect of a test.

**Interim mitigation, already in place:** `crates/pmcp-tasks/tests/input_delivery.rs` (114-07) owns
the BEHAVIOURAL twin of the predicate claim from inside the crate that can execute it
(`anonymous_owner_is_refused_by_default_on_this_backend`), and 114-15's source tripwire names it.
The two suites together cover what one cannot.

**Suggested owner:** a plan that already needs `pmcp-tasks` reachable from a `pmcp` test — most
likely a future backend-parity suite.

---

## D-114-P — a `TaskRouter`-backed v2 server answers `-32603` for a task its router cannot find (114-16)

**Discovered:** 2026-07-31, enumerating every `-32603` emission in `src/server/task_dispatch.rs`
for the `NotFound`-must-not-be-`INTERNAL_ERROR` tripwire.

Three router fall-through legs render a `TaskRouter` error as `-32603`:

| function | line | method |
|---|---|---|
| `route_tasks_get` | 1818 | `tasks/get` |
| `route_tasks_cancel` | 1933 | `tasks/cancel` |
| `deliver_tasks_update` | 2409 | `tasks/update` |

The tasks extension's error-handling section makes `-32602` a **MUST** for a `tasks/get` naming an
invalid or nonexistent `taskId`, and a SHOULD for `tasks/cancel` and `tasks/update` (inventory row
29). So a router-backed v2 deployment is **non-conformant** on `tasks/get` and non-ideal on the
other two.

**This is a router-only gap.** Every STORE-backed path — which is every backend in this repository,
`InMemoryTaskStore` and `GenericTaskStore` alike — reaches `store_error_response`, which maps
`NotFound`/`Expired` onto the one oracle-free `-32602` that 114-11 landed and 114-15 measured over a
live socket. `tests/v2_tasks_tripwires.rs :: the_v2_store_not_found_arm_still_maps_to_minus_32602`
pins that arm positionally.

**Why not fixed here:** `TaskRouter::handle_tasks_*` returns `crate::error::Error`, which carries no
not-found discriminant the dispatch can read, so the dispatch cannot map it without either
inspecting an error STRING (which would be a new, fragile wire dependency) or widening the trait
with a typed not-found. Widening a legacy-experimental public trait is a semver and design decision
of its own, and 114-16 is a coverage-only plan that touches no production byte.

**Recorded rather than hidden:** the three legs are `Disposition::RouterLeg` entries in
`INTERNAL_ERROR_SITES`, each naming this deferral in its justification, and the count is pinned — so
a fourth router leg, or a second `-32603` inside one of these three, fails the tripwire.

**Related:** D-114-M (114-14) records the sibling shape — a `TaskRouter` serving `tasks/update`
performs its own decode unaided, so the kind-direction property is enforced for `TaskStore` and only
documented for `TaskRouter`. Both close together, in a plan that decides what `TaskRouter` owes a v2
caller.

**Suggested owner:** Phase 118, alongside the conformance run that would grade `tasks/get` on a
router-backed server.

---

## D-114-Q — `TASKS_UPDATE_METHOD` has a PROSE attribution, not a walkable one (114-16)

**Discovered:** 2026-07-31, writing the provenance tripwire over the wire values this phase
introduced.

The D-18 gate's mechanism is that every provisional wire value carries an attribution a reader can
WALK — a path to the vendored artifact, or the recheck record. Two of the three values this phase
introduced do:

| constant | attribution site | strength |
|---|---|---|
| `TASKS_EXTENSION_KEY` | itself | names `schema/vendored/ext-tasks/schema.ts`, the pinned commit, AND `114-SPEC-RECHECK.md` |
| `V2_TASKS_METHOD_RETIRED` | itself | names `schema/vendored/ext-tasks/schema.ts` |
| `TASKS_UPDATE_METHOD` | `TASK_NAME_BEARING_METHODS` | **prose only** |

`TASKS_UPDATE_METHOD`'s own rustdoc is one line — ``/// `tasks/update`. See [`TASKS_GET_METHOD`].``
— and following that link twice arrives at `TASK_NAME_BEARING_METHODS`, whose rustdoc cites "the
ext-tasks specification's § *Streamable HTTP: Routing Headers*" and names **no file**. Measured, not
assumed: neither walkable token appears in the constant's own doc block, and the table's block
contains `ext-tasks` but neither `schema/vendored/ext-tasks` nor `114-SPEC-RECHECK`.

**Why not fixed here:** the fix is one added rustdoc paragraph in `src/types/mrtr.rs`, which is a
production edit. 114-16's own threat register (T-114-SC) and its `<verification>` both require
`git diff --stat -- src/ crates/` to be EMPTY, and a coverage plan that edits the thing it is
measuring stops being evidence.

**The lock is two-directional, so this cannot rot in either direction:**
`every_wire_value_constant_this_phase_introduced_carries_an_attribution` fails if the prose citation
is ALSO lost, and it fails if the rustdoc GAINS a walkable artifact reference — the second failure
message says "promote the entry to `Attribution::Pinned` and close the deferral". So closing this is
a two-line change that the test itself demands.

**Suggested owner:** any plan already editing `src/types/mrtr.rs` — most naturally the one that
re-vendors the schema at the D-18 gate, since that is when every attribution gets walked anyway.

---

## D-114-U — Phase 114 grew `make test-feature-flags`'s dead-code count from 48 to 61 (114-18)

**Discovered:** 2026-08-01, at the phase's whole-tree gate run (plan 114-18 Task 2).
**Status:** open, unowned. **Extends D-114-E; does not duplicate it.**
**Severity:** medium — D-114-E owns the *pre-existing* redness; this entry owns the *delta*, which is
this phase's and which D-114-E's "pre-existing and PROVEN so" wording would otherwise absorb.

D-114-E proved `make test-feature-flags` was already red at **114-07's** base. 114-18 measured it at
the **phase's** base commit `27364eb1` in a detached worktree with its own `CARGO_TARGET_DIR`, and at
HEAD:

| | exit | `^error` lines | `src/types/mrtr.rs` | `subscriptions.rs` | `core.rs` | `task_dispatch.rs` | `protocol_helpers.rs` | `protocol/mod.rs` | `server/mod.rs` | `sse_parser.rs` |
|---|---|---|---|---|---|---|---|---|---|---|
| base `27364eb1` | **2** | **49** | 36 | 7 | 2 | 0 | 0 | 0 | 1 | 2 |
| HEAD | **2** | **62** | 39 | 7 | 4 | 6 | 1 | 1 | 1 | 2 |

Same failing row as D-114-E — row 1/4, second sub-command,
`cargo clippy -p pmcp-tasks --no-default-features -- -D warnings`, exit 101. **Zero** errors are in
`crates/pmcp-tasks/`. **The compile claim the four rows exist to make is GREEN:** all five
`cargo check -p pmcp-tasks` rows (`--no-default-features`, `--features dynamodb`, `--features redis`,
`--features dynamodb,redis`, default) exit **0** at HEAD, exactly as D-114-E recorded.

**The +13 is attributable by symbol**, and every one is a Phase-114 item with no caller under the
reduced feature set `-p pmcp-tasks --no-default-features` selects:

| symbol(s) | owning plan |
|---|---|
| `route_tasks_update`, `parse_tasks_update_params`, `decode_inputs_against_record`, `deliver_tasks_update`, `deliver_update_through_store`, `store_error_or_fall_through` | 114-13 |
| `TasksUpdateParams`, `TASKS_UPDATE_MALFORMED_PARAMS`, `TASKS_UPDATE_MISSING_INPUT_RESPONSES`, `V1_TASKS_UPDATE_ABSENT`, `update_ack`, `InternalClientRequest::TasksUpdate::params` | 114-13 / 114-14 |
| `check_input_responses_map_bounds` | 114-14 |
| `TASK_NAME_BEARING_METHODS`, `name_bearing_key` | 114-06 |
| `EXPERIMENTAL_TASKS_KEY`, `project_capabilities_for_v2` | 114-05 |

**Why not fixed here:** identical to D-114-E's reasoning, and sharpened by it. The fix is `#[cfg]`
gates or `#[allow(dead_code)]` across five root-`pmcp` files owned by four other plans. Adding
`allow`s is not a neutral edit — it suppresses a lint that would also hide a *real* future dead item,
so each of the 13 needs a decision about whether the item should be feature-gated instead. 114-18 is
a bookkeeping-and-gate plan whose own threat register (T-114-96) requires its production diff to be
doc-only; a 13-site lint-suppression pass is neither doc-only nor reviewable inside it.

**The acceptance criterion this contradicts, stated plainly:** 114-18 Task 2 required
"`make test-feature-flags` exits 0 for all four rows". That criterion was **unsatisfiable at the
moment it was written** — the target was already red at the phase base. It is the second plan in this
phase to carry it (114-07 was the first). A future plan must either fix the 61 sites or drop the
criterion; restating it a third time will just produce a third recording of the same measurement.

**Suggested owner:** a Phase 118 hygiene item, or whichever plan next touches `src/types/mrtr.rs`.
Close D-114-E and this entry together — they are one problem measured at two commits.

---

## D-114-V — `make doc-check` is RED at the phase base and is NOT in `make quality-gate` (114-18)

**Discovered:** 2026-08-01, at plan 114-18 Task 1 (the stale-doc sweep ran it as its verify step).
**Status:** open, unowned, **pre-existing and PROVEN so**
**Severity:** medium — it is the reason a broken-rustdoc-link class can land unnoticed, and this
phase demonstrated exactly that.

`make doc-check` exits **2** with **26** `^error` lines. Measured at the phase base commit `27364eb1`
(detached worktree, own `CARGO_TARGET_DIR`) and at HEAD: the two error-header sets are **byte-identical**
under `diff`. So it is pre-existing and Phase 114 neither caused nor worsened it. The population is
`rustdoc::private_intra_doc_links` (public docs linking to private items — e.g. `mrtr` →
`splice_mrtr_params`, `ProtocolContext` → `VerifiedContinuation::state`) plus six unresolved links
and one ambiguous `Error` enum-vs-derive-macro link.

**`make quality-gate` does NOT invoke `doc-check`.** Its recipe is `fmt-check`, `lint`, `build`,
`test-all`, `pmcp-package-gate`, `audit`, `unused-deps`, `check-todos`, `check-unwraps`,
`validate-always`. So a rustdoc regression is invisible to the gate that CLAUDE.md makes mandatory
before every commit.

**The demonstration, measured rather than argued.** 114-19 landed two broken intra-doc links —
`[`Error::Parse`]` and `[`Error::Capability`]`, neither of which is a variant of `Error` — and the
green `make quality-gate` on that plan could not see them. 114-18 Task 1 found them only because it
ran `cargo doc` against a base-commit baseline: 30 warnings at HEAD versus **28** at base, the two
extras both in `src/client/mod.rs`. They are fixed in commit `6be9f5fe` (`Error::Protocol` +
`Error::UnsupportedCapability`), and the per-file warning distribution is now byte-identical to base.

**Why not fixed here:** clearing 26 pre-existing rustdoc errors across ~12 files is its own plan, and
most of them are a deliberate style question (whether a public doc may reference a private helper by
link) rather than a typo. Adding `doc-check` to `quality-gate` before those 26 are cleared would
block every commit in the repository.

**Suggested owner:** a docs-hygiene plan. The valuable half is cheap and separable: add a
`cargo doc`-warning-count tripwire (or `doc-check` restricted to `unresolved_link`, which is the
class that indicates a genuinely wrong doc) to `quality-gate`, leaving the
`private_intra_doc_links` population to be cleared on its own schedule.

---

## D-114-W — "223/223" means two different measurements, and the phase's plans conflated them (114-18)

**Discovered:** 2026-08-01, at plan 114-18 Task 2.
**Status:** open — a measurement-hygiene record, not a code defect. No owner needed; it needs to be
KNOWN.
**Severity:** low, but it will produce a false finding in every future plan that quotes the number.

`114-18-PLAN.md` Task 2 item 3 requires "**223/223, no update required**" and calls any deviation a
finding. Measured at both commits:

| invocation | at base `27364eb1` | at HEAD |
|---|---|---|
| `cargo semver-checks check-release --package pmcp` (baseline = **published crates.io 2.17.0**) | exit **100** — 223 checks: **222 pass, 1 fail**, 30 skip | exit **100** — 223 checks: **222 pass, 1 fail**, 30 skip |
| `cargo semver-checks check-release --package pmcp --baseline-rev 27364eb1` (baseline = **the phase base**) | — | exit **0** — 223 checks: **223 pass**, 30 skip, *no semver update required* |

The single failure is identical at both commits: `type_marked_deprecated` — `#[deprecated]` added on
`Struct OptimizedSseTransport` (`src/shared/sse_optimized.rs:95`). It predates Phase 114 entirely and
is a *correct* report: the published 2.17.0 did not carry that attribute.

**So both numbers are true and they answer different questions.** "223/223, no update required" is the
`--baseline-rev` form — the form 114-14's SUMMARY actually ran (`--baseline-rev aa651f74^`) and the
form that answers *"did THIS phase move the public API incompatibly?"* (answer: **no**). The bare
`check-release` form answers *"is the working tree releasable as a patch of the last publish?"*
(answer: **no, it needs a minor** — and it already did before this phase started).

**What to do about it:** a plan asserting a semver ratio must name the BASELINE alongside the ratio.
The bare form's 222/223 is not a regression and must not be reported as one; the phase-base form's
223/223 is the phase's own result. Both are recorded in `114-18-SUMMARY.md` § *Phase base manifest*.

**Related measurement corrections from the same run, recorded here so they are not rediscovered:**

- **`pmat analyze complexity --max-cognitive 25` reports ZERO violations in `./src/` at BOTH commits.**
  Base: **4** violations (all in `crates/*/tests/`). HEAD: **5** — the one addition is
  `tests/v2_tasks_update_routing.rs:1081 no_source_site_routes_tasks_update_through_the_mrtr_ingress`
  at cognitive **33**, which is 114-13's tripwire test and which 114-14's SUMMARY already attributed.
  `STATE.md`'s "the gate at 3 pre-existing violations, including D-113-U (`write_canonical` cog 26)"
  is **not reproducible** with the CLAUDE.md-pinned `pmat 3.15.0`: `write_canonical` does not appear
  in either violation list. **D-113-U's ownership obligation stands regardless** — it is recorded
  below as still unowned — but the cog-26 figure should not be re-quoted as a live gate reading.
- **`make wasm-build` exits 0** at HEAD. Warnings **86 (base) → 91 (HEAD)**, all `dead_code` on the
  wasm target, distributed `src/types/mrtr.rs` +3, `src/shared/protocol_helpers.rs` +1,
  `src/types/protocol/mod.rs` +1 — the same class as the pre-existing 37-strong `types::mrtr` dead
  block D-14 predicted, and the same symbols as D-114-U.
- **A zsh trap that produced a fake failure and was caught.** `for f in "--features redis"; do cargo
  check $f; done` passes `--features redis` as ONE argument, because zsh does not word-split unquoted
  parameter expansions. It exits 1 and looks like a broken feature row. All five rows exit **0** when
  invoked without the loop variable, or with `${=f}`. Verify a per-row failure by re-running the row
  literally before recording it.

---

## D-114-X — the `notifications/tasks` push surface is DECLINED, not missing (114-18)

**Recorded:** 2026-08-01 by plan `114-18`'s ledger sweep. **This is a scope decision, not a
discovery** — it is written down because an undocumented decline is indistinguishable from an
oversight.
**Status:** open, **unowned**
**Severity:** low today; re-check whenever `ext-tasks` publishes

Inventory row 36. The tasks extension lets servers **MAY** push `notifications/tasks`, with clients
subscribing through `subscriptions/listen` carrying `taskIds`. **pmcp implements none of it**, and
that is the correct state for this phase: a v2 client polls `tasks/get`, which is the mechanism the
extension is built around (SEP-2663 replaced blocking `tasks/result` with polling precisely so a
stateless server need hold nothing).

**Exposure assessed, not assumed** (`114-RESEARCH.md` **A7**, risk **MEDIUM**): a conformance suite
sometimes grades an optional feature *when it is advertised*. pmcp advertises no `taskIds` in any
acknowledgement, so a suite has nothing to grade. The residual risk is that the published extension
upgrades the `MAY` — which is why row 36 carries a re-check obligation.

**What implementing it would cost, so a future plan can size it:** the notification type, a
per-task-id fan-out on the existing `ListenRegistry`, and the `taskIds` field on the
`subscriptions/listen` acknowledgement. It also inherits Phase 113's recorded deployment limitation
— `ListenRegistry` is **instance-local**, so a push surface behind a non-sticky load balancer
under-delivers. That limitation is the reason to keep polling as the recommended enterprise
mechanism regardless.

**Suggested owner:** Phase 118 (conformance), but only if the published extension upgrades the
`MAY`. Otherwise this stays declined.

---

## D-114-Y — the four `114-CONTEXT.md` deferrals that DQ1–DQ4 did not absorb (114-18)

**Recorded:** 2026-08-01 by plan `114-18`'s ledger sweep. Restated here so a reader knows these were
**considered and declined**, not missed. Three of the four are design questions; the fourth is a
milestone-level requirement gap.
**Status:** open. Owners named per item below.

**(1) The broader server-directed-handle client-compatibility question — the part DQ1 did NOT
absorb.** *Unowned; suggested owner a post-114 client-experience plan or the v2.6 AI-Package
milestone.* DQ1 (user-approved 2026-07-27) settled only the **create trigger**: a declaring v2 client
gets a server-directed handle. What it did not settle is already filed in full as **`D-114-K`** — a
declaring client gets a handle from **every** task-capable tool, with no per-call opt-out, because
the v2 wire has no equivalent of v1's per-request `task` field. The ergonomics (auto-poll? surface
the handle? opt out?) are unspecified. **Cross-reference rather than duplicate: read `D-114-K`.**

**(2) A configurable proxy-header / claim-based identity source for v2 owner binding.** *Unowned.*
This is the **named future closure** for the TASK-05 scope gap that `114-SPEC-RECHECK.md`
§ *⚠ Known INTERNAL wording gap* records and that `.planning/REQUIREMENTS.md`'s TASK-05 booking now
carries: on a server with **no auth provider at all**, D-07 row 3 maps every anonymous caller onto
one `ANONYMOUS_PRINCIPAL` (`""`) bucket, so "fails closed" applies to auth-configured deployments
only. A configurable identity source is what would let that row fail closed too. **Deferred, not
scheduled** — and D-07 is LOCKED, so this is an ADDITION to the identity table, never a change to
row 3. Independently bounded today by `TaskSecurityConfig::default()`'s `allow_anonymous: false`.

**(3) Per-tool configurability of the `tasks/update` transition.** *Unowned; suggested owner
whichever plan next revisits the task state machine.* There is no knob for "this tool accepts input
mid-flight, that one does not" and no per-tool policy for what a delivered `inputResponses` does to
the task's status. **Closely related to a trap `114-17` measured and every future tasks plan
inherits:** `tasks/update` leaves a fully-answered task at `working`, and **nothing in the SDK
promotes it to `completed`** — `InMemoryTaskStore::deliver_task_inputs` stops at the
`delivery.complete && !delivery.accepted.is_empty()` arm, and the tool handler returned long before
the input arrived. **Every task-serving deployment needs an application-side worker**;
`examples/s50_v2_tasks_server.rs`'s `run_worker` (40 lines, owner-scoped `task_input_snapshot`,
tolerant of every store error) is the reference shape. A plan that assumes `tasks/update` completes
a task will produce a demo that hangs.

**(4) UNAS-01 — SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}`.** ***UNASSIGNED milestone-wide, and
explicitly NOT folded into Phase 114.*** It is carried in `.planning/REQUIREMENTS.md`
§ *Unassigned — Awaiting Phase Assignment* with a standing instruction not to absorb it into a phase
without an explicit scoping decision. Phase 113 declined it (A8, Open Question 4) and Phase 114
declines it again: it is a **transport header-mirroring** requirement closest to CLNT-01's outbound
header work and to Phase 112's `classify_v2_request` matrix, and it has nothing to do with tasks.
Recorded here only so that "Phase 114 touched `Mcp-Name`, did it quietly take UNAS-01 too?" has a
written answer. **It did not.**

---

## Inherited from Phase 113 — unowned, and Phase 114 must not silently adopt them

`114-18`'s sweep restates these so that "Phase 114 ran and said nothing about them" cannot be read as
"Phase 114 closed them". **Phase 114 changed none of them.** They belong to Phase 113's ledger,
`.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md`, which remains
the authoritative record for each.

| Item | Status entering Phase 114 | Status leaving it |
|---|---|---|
| **D-113-U** — `write_canonical` cognitive complexity, recorded at 26 | **unowned**, and `STATE.md` says it **needs an owner before this branch merges** | **STILL UNOWNED. Still needs an owner before merge.** See the measurement note below. |
| **D-113-Q** | unowned | unowned, untouched |
| **D-113-R** | unowned | unowned, untouched |
| **D-113-S** | unowned | unowned, untouched |
| **D-113-T** | unowned | unowned, untouched |
| **D-113-V** | unowned | unowned, untouched |
| **D-113-W** | unowned | unowned, untouched |

**A measurement about D-113-U that must not be mistaken for a closure.** `114-18` ran
`pmat analyze complexity --format json --max-cognitive 25` (pmat **3.15.0**, the version CLAUDE.md
pins for CI) at the phase base commit `27364eb1` and at HEAD. **Neither run lists `write_canonical`,
and neither run reports ANY violation in `./src/`** — base **4**, HEAD **5**, all in
`crates/*/tests/` and `tests/`, with the one addition being 114-13's own tripwire test at cognitive
33 (already attributed by `114-14-SUMMARY.md`).

**That is a fact about the instrument, not about the obligation.** The cog-26 figure is not
reproducible with the pinned pmat, so it should not be re-quoted as a live gate reading — but
**D-113-U's ownership requirement stands exactly as `STATE.md` states it**, and this ledger does not
discharge it. A plan that wants to close D-113-U must do so on Phase 113's terms, not by citing this
measurement.
