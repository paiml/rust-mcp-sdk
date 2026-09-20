---
phase: 114-tasks-extension-migration
plan: 13
subsystem: server
tags: [tasks, v2, routing, semver, mrtr, ordering, source-tripwire]
requires:
  - "114-12 (CreateTrigger + TaskDispatch::create_gate; declares_tasks_extension as the shared v2 predicate)"
  - "114-09 (the ordered refusal chain: era -> backend -> declaration -32021 -> auth -32003 -> params)"
  - "114-08 (is_v1_task_era, the module's ONE era definition)"
  - "114-06 (TASK_NAME_BEARING_METHODS and the TASKS_UPDATE_METHOD spelling)"
  - "112 (InternalClientRequest / classify_internal_method / parse_request_or_internal, the server/discover precedent)"
provides:
  - "InternalClientRequest::TasksUpdate { params } — crate-private, RAW params, no request id"
  - "TaskDispatch::route_tasks_update — the five ordered gates, no delivery"
  - "Server::handle_tasks_update — the thin delegate, no gate of its own"
  - "HttpIngress::TasksUpdate + both POST dispatch arms + the two assemblers"
  - "attach_v2_mrtr_params is now method-gated on mrtr_eligible (T-114-63 / T-114-64 closed)"
  - "tests/v2_tasks_update_routing.rs — routing, ordering, and the three-part substitute for the lost compile-time MRTR guard"
affects:
  - "114-14 (owns the delivery body AND SPEC-RECHECK row 19's empty ack; the -32603 placeholder is its seam)"
  - "114-17 (the paired example can now drive a real tasks/update round trip)"
tech-stack:
  added: []
  patterns:
    - "internally-routed method: crate-private enum + method-string classifier, never a public enum variant"
    - "one predicate, applied at BOTH halves of a rule (extraction and ingest)"
    - "source tripwire + runtime predicate as two ORTHOGONAL guards, each with its own control"
key-files:
  created:
    - tests/v2_tasks_update_routing.rs
  modified:
    - src/types/protocol/mod.rs
    - src/shared/protocol_helpers.rs
    - src/server/task_dispatch.rs
    - src/server/mod.rs
    - src/server/streamable_http_server.rs
decisions:
  - "TASKS_UPDATE_METHOD is RE-EXPORTED from types::mrtr, not re-declared — the plan's premise that no constant existed was false"
  - "a request passing all five gates gets -32603, NOT an empty success ack: {} would make the Pitfall-4 disaster indistinguishable from correct behaviour"
  - "SPEC-RECHECK row 19 (UpdateTaskResult's empty ack) is 114-14's, claimed explicitly; this plan lands no new wire value"
  - "no change to src/server/core.rs: ServerCore/Server dispatch over ClientRequest and cannot receive an InternalClientRequest — measured, not assumed"
  - "route_tasks_update is deliberately NOT async: nothing here touches a store"
  - "attach_v2_mrtr_params gains a method gate reading the SAME mrtr_eligible predicate mrtr_ingest already reads"
metrics:
  duration: ~7h
  completed: 2026-07-29
  commits: 4
  tests_added: 22
  negative_controls: 11
---

# Phase 114 Plan 13: `tasks/update` Routing Summary

`tasks/update` now reaches `TaskDispatch` over streamable HTTP with all five gates in the
documented order and **zero public-API change** — and the tests written to prove the ordering
found a real, previously-invisible defect that inverted it.

## What landed

### The routing decision, and why it is not a `ClientRequest` variant

`ClientRequest` (`src/types/protocol/mod.rs:479-483`) carries
`#[derive(Debug, Clone, Serialize, Deserialize)]` and
`#[serde(tag = "method", content = "params", rename_all = "camelCase")]` with **no
`#[non_exhaustive]`** — verified on disk, not quoted. A variant there is `enum_variant_added`
on a public exhaustive enum: a semver-**MAJOR** break and a source break for every downstream
exhaustive `match`. Adding `#[non_exhaustive]` instead is *also* a source break, so it is not
an escape hatch.

So `tasks/update` rides the crate-private `InternalClientRequest` + `classify_internal_method`
+ `parse_request_or_internal` route that Phase 112 built for `server/discover` — the
**`server/discover` precedent**, not the HTTP-only `HttpIngress` route `subscriptions/listen`
uses. `subscriptions/listen` classifies HTTP-locally because it opens an HTTP *stream* and has
no meaning off that transport; `tasks/update` is an ordinary request/response, so its
classification belongs in `shared/`, where a later plan can widen its reach without a semver
break.

**The variant carries RAW params and NO request id.** The classifier never receives an id —
`parse_request_or_internal` reads `request.id` itself and returns it as the first element of
its `(RequestId, IngressRequest)` tuple — so a field the classifier cannot populate would
either be a lie or force a signature change on the shared classifier. Params stay undecoded
because the classifier **must never reject a body**: see the ordering section below, where that
turned out to be load-bearing in a way the plan predicted and the tree did not honour.

### The gate chain

`TaskDispatch::route_tasks_update` is the sibling of `route_tasks_endpoint` for the one
`tasks/*` method with no `ClientRequest` variant (that function matches on `&ClientRequest`).
Every gate is an **existing helper**; this function defines none of its own:

| # | condition | answer | helper reused |
|---|-----------|--------|---------------|
| 1 | not v2 | `-32601` + `V1_TASKS_UPDATE_ABSENT` | `is_v1_task_era` |
| 2 | no task backend | `-32601` + the FROZEN `TASKS_NOT_ENABLED` | `TaskDispatch::has_task_backend` |
| 3 | extension not declared | `-32021` | `TaskDispatch::declares_tasks_extension` |
| 4 | unauthenticated on an auth-configured server | `-32003` | `TaskDispatch::resolve_owner` / `authentication_required` |
| 5 | no string `params.taskId` | `-32602` | `types::mrtr::logical_name_of` |

Case 1 sits **outside** the backend guard, unlike `route_tasks_endpoint`'s case 1: a method that
does not exist on the negotiated protocol version does not become conditional on how the server
is configured. Case 5 resolves the addressing key through `logical_name_of`, which reads
`TASK_NAME_BEARING_METHODS` — the same table the `Mcp-Name` routing header derives from — so
`taskId` is not re-spelled in the router.

`Server::handle_tasks_update` is a **thin delegate**, structurally identical to `handle_discover`
beside it. `git diff src/server/mod.rs` is `34 insertions(+), 0 deletions(-)`: pure call-site
wiring, no gate.

### What a fully-gated request gets, and why it is not `{}`

`-32603 "tasks/update is routed but its input delivery is not implemented on this build"`.

**`114-SPEC-RECHECK.md` row 19 — `UpdateTaskResult`'s empty acknowledgement — is claimed for
114-14, explicitly, and this plan lands no new wire value.** Emitting `{}` here would produce a
successful-looking empty ack while the task never left `input_required`, which is byte-for-byte
the failure mode Pitfall 4 describes for an MRTR-eligible `tasks/update` whose payload
`splice_mrtr_params` deleted. Shipping that shape as a placeholder would make the disaster and
the intended behaviour **indistinguishable on the wire** — and 114-19's client already decodes
an empty ack as SUCCESS (`v2_empty_update_ack_is_not_a_decode_error`). The code is `-32603`
rather than `-32601` because the method EXISTS on this server and this caller passed every gate;
"method not found" would send a conformant client to fix its negotiation, which is not the
problem. `114-SPEC-RECHECK.md` was therefore **not edited**, and the forward tripwire in
`tasks_get_never_carries_result_type_task` stays green.

## THE DEFECT THIS PLAN FOUND — the MRTR extractor was judging non-MRTR params

**Tests 3b and 4 FAILED on their first run**, and the failure was not in this plan's code:

```
{"jsonrpc":"2.0","error":{"code":-32602,"message":"inputResponses must be an object"},"id":22}
```

`attach_v2_mrtr_params` (`src/server/streamable_http_server.rs`) ran
`crate::types::mrtr::extract_mrtr_params` on the RAW top-level `params` of **every accepted v2
request, with no method awareness at all** — while `mrtr_ingest` has always returned `Inert` for
a non-eligible method under its own comment: *"T-113-23: the spec confines MRTR to three methods.
A `requestState` presented on any other method is IGNORED — not verified, not errored."* The two
halves of one rule disagreed, and only one half had ever been tested.

`tasks/update` is the method where that mattered, because **its entire payload IS
`inputResponses`**. Measured over a real socket, before the fix:

* an **UNAUTHENTICATED** caller sending `"inputResponses": "not-an-object"` received `-32602` at
  HTTP 400 from the transport header gate instead of `-32003` — a free parse of the caller's own
  choosing on an unauthenticated path (**T-114-64**), and an inversion of 114-09's documented
  gate order (**T-114-63**);
* an **UNDECLARING** caller received the same `-32602` instead of `-32021`, i.e. was sent to fix
  the wrong thing.

Both threats are registered `mitigate` in this plan's own threat model, so this is a Rule 1 / Rule 2
deviation, not scope creep. **The fix is one condition reading the SAME `mrtr_eligible` predicate
over the SAME `MRTR_METHODS` table `mrtr_ingest` reads**, using `body_method` — the value
`classify_v2_request` has just cross-checked against `Mcp-Method` — so no new read of the wire is
added. It is strictly **narrowing**: for the three eligible methods nothing changes, and no request
accepted today becomes rejected; what changes is that a non-eligible method's MRTR fields are now
ignored here exactly as `mrtr_ingest` already ignores them.

Regression tests landed at both levels: `attach_v2_mrtr_params_ignores_a_non_eligible_method` /
`attach_v2_mrtr_params_skips_an_unresolvable_method` (unit) and tests 3b/4 (end to end).

**This also generalises beyond tasks.** Before the fix, a `tools/list` or a `server/discover`
carrying a malformed top-level `inputResponses` was rejected `-32602` by the transport for a field
that method has no meaning for. That is now gone too.

## Transport reach — MEASURED, and narrower than an earlier draft claimed

The plan required this be settled by measurement rather than asserted. It was:

| question | measurement | answer |
|----------|-------------|--------|
| production consumers of `IngressRequest::Internal` | `grep -rn parse_request_or_internal src/` | **ONE**: `classify_http_ingress`, `streamable_http_server.rs:1482` |
| does `ServerCore` reach an internal-request route? | `grep -n InternalClientRequest src/server/core.rs` | ONE hit, line **4278**, inside `#[cfg(test)] server_discover_v2_projects_capabilities_with_extensions` — **not** a dispatch arm |
| did `core.rs` need a change? | `git diff --stat -- src/server/core.rs` | **EMPTY** |
| does `Server::handle_client_request` reach it? | it dispatches over `ClientRequest`, which cannot carry an `InternalClientRequest` | no |
| stdio? | `shared/transport.rs:138` calls the PUBLIC `parse_request`, which maps `Internal` to `method_not_found` | **`-32601`. No stdio support is claimed.** |
| wasm server core? | `grep -c "route_tasks_update\|task_dispatch" src/server/wasm_core.rs` | **0** — it does not reach this gate at all |

So `tasks/update` is served over **streamable HTTP only**, which is exactly the reach
`server/discover` has had since Phase 112. The design admits stdio later without a semver break;
nothing here claims it already has. v2-over-stdio remains out of scope for this milestone.

## Deviations from Plan

### Auto-fixed

**1. [Rule 1 + Rule 2 - Bug/Security] `attach_v2_mrtr_params` had no method gate**
- **Found during:** Task 3, by tests 3b and 4 failing on their first run
- **Issue / Fix / Files / Commit:** see the section above; `src/server/streamable_http_server.rs`, commit `45e014b6`

**2. [Rule 3 - Blocking] `src/server/streamable_http_server.rs` is a file beyond `files_modified`**
- **Why unavoidable:** the plan's `files_modified` lists `core.rs`, `mod.rs`, `task_dispatch.rs`,
  but the ONLY production consumer of the internal-request route is the streamable-HTTP
  classifier. Routing `tasks/update` without touching that file is impossible; `core.rs`, which
  the plan DID list, needed no change at all. Same deviation class as 114-11's.
- **Contents:** `HttpIngress::TasksUpdate`, the classifier arm and fast-reject, `is_initialize`,
  `resolve_v2_gate`, both POST dispatch arms, the two assemblers, `TasksUpdateCall`, the
  `DiscoverResponseShape` → `InternalResponseShape` rename, the MRTR method gate, and 4 new unit
  tests.

**3. [Rule 1 - Bug] the plan's premise that no `TASKS_UPDATE_METHOD` constant existed was FALSE**
- The spelling already lived at `src/types/mrtr.rs:207` as a `TASK_NAME_BEARING_METHODS` row
  (114-06, DQ4). Declaring a second constant with the same name and value is precisely the "two
  spellings that can disagree" failure the single-sourcing rustdoc on `SERVER_DISCOVER_METHOD`
  exists to prevent. `types::protocol` **re-exports** the one definition instead, which satisfies
  the acceptance criterion's letter (`src/types/protocol/mod.rs` contains a `TASKS_UPDATE_METHOD`
  item) and its intent. Measured: exactly ONE non-test `"tasks/update"` string literal remains in
  `src/`.

### Deliberate corrections to plan text and to existing code

* **`classify_internal_method`'s second parameter renamed `_params` → `params`.** Arity and types
  are unchanged (the acceptance criterion's intent), but the new arm READS it, and an underscore
  prefix asserting "unused" on a used binding is both a `clippy::pedantic` violation and the
  stale-marker class 113-29 recorded. **No parameter was added, removed or retyped — in
  particular the classifier still does not receive the request id.**
* **`classify_http_ingress_routes_server_discover_only` → `..._server_discover`.** "only" was true
  while `server/discover` was the sole method reaching the `parse_request_or_internal` peek; it no
  longer is. A name asserting an exclusivity that no longer holds is the same stale-marker class.
* **`DiscoverResponseShape` → `InternalResponseShape`.** It is now shared by both internally-routed
  request/response assemblers, so a name after one of them is wrong. Mechanical, 7 sites.
* **A rustdoc claim corrected by the gate itself.** `route_tasks_update`'s "not async" note
  originally cited `clippy::unused_async`. The gate output shows that lint is on `make lint`'s
  **allow-list**, so it would NOT have caught it. The rustdoc now gives the real reason — an
  `async fn` that never awaits is a false promise of I/O to every caller deciding where to hold a
  lock — and says so.
* **`assemble_tasks_update_with_middleware` took 8 arguments (clippy max 7).** Caught by the gate,
  not anticipated. Fixed by introducing `TasksUpdateCall`, which groups exactly the router's four
  inputs, beside `InternalResponseShape`, which groups exactly the response tail's — so the two
  assemblers read as "route with these, then shape with those".

### Decided-now item carried forward, with the reason

The phase context flagged 114-04's defaulted `TaskRouter::handle_tasks_update`, which returns
`Error::internal` → `-32603` for "this router does not do updates" when the protocol-level answer
is arguably `-32601`. **Recorded, not changed, and here is why rather than a shrug:** that default
has **zero callers anywhere in the tree** (`grep -rn handle_tasks_update src/` → `src/server/tasks.rs`
only), and this plan does not add one — delivery, and therefore the router-vs-store split, is
114-14's. Changing an unreachable default's wire code would be **unverifiable by construction**:
no control could fail, which is exactly the "a property no control fails" pattern this phase keeps
finding. 114-14 adds the caller and can then measure the change. Logged as **D-114-M**.

## Negative Controls — 11 run, all reverted, and BOTH published predictions were wrong

| # | mutation | failing set (MEASURED) |
|---|----------|------------------------|
| NC-1 | `tasks/update` row added to `MRTR_METHODS` | lib: `mrtr_eligible_is_exactly_three_methods`, `tasks_methods_are_name_bearing_but_not_mrtr_eligible`, `enum_eligibility_agrees_with_the_method_table`; suite: **3, 3b, 4, 5** — **NOT 6** |
| NC-2 | `splice_mrtr_params` + `TASKS_UPDATE_METHOD` in ONE statement in `task_dispatch.rs` | suite: **6** only |
| NC-3 | params gate moved directly above the auth gate | suite: **4** only (`-32602` where `-32003` is required) |
| NC-4 | `-32021` declaration gate deleted from `route_tasks_update` | suite: **3b** only |
| NC-5 | the `attach_v2_mrtr_params` method gate REVERTED | lib: `attach_v2_mrtr_params_ignores_a_non_eligible_method`, `..._skips_an_unresolvable_method`; suite: **3b, 4** |
| NC-6 | era gate deleted from `route_tasks_update` | suite: **2** only |
| NC-7 | `classify_internal_method`'s `tasks/update` arm deleted | lib: 3 classifier tests; suite: **1, 2, 3, 3b, 4** |
| NC-8a | `TasksUpdate` text inserted INSIDE the `pub enum ClientRequest` block | suite: **7** only |
| NC-8b | the SAME text inserted just OUTSIDE the block | **none — PASSES** (proves test 7 is BLOCK-scoped, not file-scoped) |
| NC-9 | `-32603` replaced by `success_response(id, {})` | suite: **1** only; wire showed exactly `{"result":{}}` |
| NC-10 | an UNLISTED `src/` file names `TASKS_UPDATE_METHOD` | `every_tasks_update_site_is_allowlisted` only |

**The plan's prediction was WRONG.** It predicted NC-1 would fail tests 5 **and** 6. It fails 5 and
**not** 6 — a `MRTR_METHODS` row names no ingress FUNCTION, so the source tripwire cannot see it.
That is the point: tests 5 and 6 catch **different** mistakes, which NC-2 confirms from the other
side by failing 6 and not 5. Had the tokens included `MRTR_METHODS`, the two guards would have
answered identically and masked each other.

**My own prediction was also wrong**, in the other direction: NC-1 fails **three more** suite tests
(3, 3b, 4). The reason is worth carrying because it is a *fixture* coupling, not a third detector:
a `MRTR_METHODS` row also makes `logical_name_key("tasks/update")` non-`None`, which turns on the
server-side `Mcp-Name` cross-check that `types::mrtr`'s own rustdoc says is deliberately OFF this
phase — so those three fail with `-32020 "Mcp-Name header does not match the request's logical
name"`, because this suite sends `Mcp-Name: ""`. **One wrong table row breaks three separate
things.**

**MASKING CHECK — run, and it fired.** NC-5's *suite* failing set `{3b, 4}` is exactly
`NC-3 ∪ NC-4`. The three are separable only by their **lib** rows: NC-5 additionally fails the two
`attach_v2_mrtr_params_*` unit tests that neither NC-3 nor NC-4 touches, while NC-3 and NC-4 each
fail exactly one suite test. All 11 full failing sets are pairwise **DISTINCT**.

**Tests failed by no synthetic control, and why that is acceptable:**
`every_tasks_update_site_is_allowlisted` needed none — **it fired for real on its first run**,
reporting `UNLISTED site: src/client/mod.rs`, i.e. it discovered the 114-19 client's `tasks_update`
without anyone predicting it; NC-10 was added afterwards to pin the mechanism. The 7 `scanner::*`
tests and `every_tasks_update_site_entry_carries_a_substantive_justification` are the scanner's own
self-tests: their evidence value is that the tripwires above are not vacuous, and a control on them
would be a control on a control.

**A PROCESS TRAP CAUGHT IN THE ACT.** NC-10's first application used a `perl -0pi -e` substitution
that did not match; `grep -c '_NC10'` returned **0** and the test then "passed" — against the
UNMODIFIED tree. That is the false-measurement mode 114-10 recorded for `/usr/bin/cp`. Every control
in this table was gated on a `grep` confirming the mutation LANDED before its result was believed.

All 11 reverted; `shasum -a 256 -c` **OK on all 7 tracked files**; `git status --short -- src/ tests/`
empty. **`git stash` was not used.**

## Verification — verbatim, with exit codes

| check | command | exit |
|-------|---------|------|
| format | `cargo fmt --all -- --check` | **0** |
| lint | `make lint` | **0** |
| build | `make build` | **0** |
| unit tests | `RUST_TEST_THREADS=1 make test-unit` | **0** — `1764 passed; 0 failed`, **0** CA messages |
| plan suite | `cargo nextest run --features full -E 'binary_id(pmcp::v2_tasks_update_routing)'` | **0** — 18/18 |
| REQUIREMENTS.md | `git diff --stat d7b82f78..HEAD -- .planning/REQUIREMENTS.md` | **EMPTY (0-byte diff)**; `requirements mark-complete` deliberately NOT run |
| `Cargo.toml` / `Cargo.lock` | `git diff --stat` | **EMPTY** — zero packages installed (T-114-SC) |
| `crates/pmcp-tasks` | `git diff --stat` | **EMPTY** |
| `src/types/mrtr.rs` | `git diff --stat` | **EMPTY** — `MRTR_METHODS` (17 lines) and `TASK_NAME_BEARING_METHODS` both byte-IDENTICAL to plan start; **no `tasks/*` row was added** |
| `src/server/core.rs` | `git diff` | **0 lines** — so `enum_eligibility_agrees_with_the_method_table` is provably unedited |
| `pub enum ClientRequest` | block + attributes extracted from `git show HEAD:` and from the tree | **IDENTICAL**, 60 lines; no `#[non_exhaustive]` added |

### `make quality-gate` did NOT reach exit 0 — and it is D-114-A, not a regression

Two full runs, both stopping at the **`test-unit`** leg:

| run | parallelism | result | CA messages |
|-----|-------------|--------|-------------|
| 1 | default (`-j ncpu`) | `1750 passed; 14 failed` | **14** |
| 2 | `RUST_TEST_THREADS=4` | `1760 passed; 4 failed` | **4** |
| 3 | `RUST_TEST_THREADS=1`, leg alone | **`1764 passed; 0 failed`, exit 0** | **0** |

In runs 1 and 2 the failure count equals the CA-message count **exactly** (14=14, 4=4), and every
failing test is in `shared::streamable_http::tests` — the client transport constructor's
pre-existing `.expect("Failed to load native root certificates")` at
`src/shared/streamable_http.rs:447-457`, panicking on `Os(Error { code: -36 })` from the macOS
keychain. It runs **before any request is sent**, so no server code of this plan can reach it, and
the failing population **shrinks monotonically with parallelism and reaches zero at one thread** — a
regression does not do that. Disk was 19 GiB free, ruling out the known disk-exhaustion mode.

An earlier targeted run of the client-driven suites showed the same signature at larger scale:
23 failures, 23 CA messages, 1:1 — and `pmcp::v2_client emits_required_headers`, one of them,
**passes 1/1 in isolation**.

**Actionable refinement to D-114-A for the next executor:** `make test-unit` uses `cargo test --lib`,
which reads **`RUST_TEST_THREADS`** and ignores `NEXTEST_TEST_THREADS`. Setting only the nextest
variable — as the phase brief's trap list says to — does nothing for this leg. Run
`RUST_TEST_THREADS=1 make test-unit`.

## Known Stubs

`TaskDispatch::route_tasks_update` returns `-32603` for a fully-gated request instead of
delivering inputs. **Intentional and bounded:** the plan scopes delivery (decode, bounds, CAS via
`partition_input_delivery`) to 114-14, and keeping the gates and the delivery in separate plans is
what lets 114-14's controls fail for exactly one reason. It is a structured error, never a
successful-looking ack — see "What a fully-gated request gets".

## Threat Flags

None. The two surfaces this plan touches (`tasks/update` ingress and the MRTR extraction gate) are
both in the plan's `<threat_model>`; the work **closed** T-114-63 and T-114-64 rather than adding
surface.

## For 114-14

1. **Your seam is `TaskDispatch::route_tasks_update`'s last line.** Gates 1-5 are done and each has
   a control that fails only it (NC-3, NC-4, NC-6). Replace the `-32603` and nothing above it.
2. **You own SPEC-RECHECK row 19.** The client already expects an EMPTY ack
   (`v2_empty_update_ack_is_not_a_decode_error`), so a non-empty `UpdateTaskResult` breaks it.
   Test 1 (`tasks_update_reaches_dispatch_on_v2`) asserts the `-32603` placeholder and **will need
   updating** — that is by design; NC-9 proves it detects the ack shape exactly.
3. **`route_tasks_update` becomes `async`** when it touches the store. One call site
   (`Server::handle_tasks_update`) gains one `.await`; `tasks_update_json_response` already awaits.
4. **The owner is already bound** at case 4 and currently discarded. Use it; do not resolve a second.
5. **Use `pausing_task_tool` / `PAUSING_TOOL_NAME = "elicit_task"`** from `tests/common/v2.rs` — the
   only client-reachable `input_required` task. Test 1 already creates one and updates it.
6. **`partition_input_delivery` is the single shared delivery policy** (114-07). Do not re-express it.
7. **Never add a `tasks/*` row to `MRTR_METHODS`** — NC-1 measured that one row breaks
   `mrtr_eligible`, the `Mcp-Name` cross-check, and three lib tests at once.
8. **D-114-M** (`TaskRouter::handle_tasks_update`'s default `-32603` vs `-32601`) becomes decidable
   the moment you add its first caller.
