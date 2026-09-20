---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 08
subsystem: streamable-http-transport
tags: [http-05, resumability, era-gate, id-replay, event-store, sse, v2, security]

# Dependency graph
requires:
  - phase: 113-04
    provides: "the `sessions_active(state, era)` era-gate shape this plan mirrors, `raw_request_id`, `create_error_response_with_id`, the v2 status mapper, and `tests/v2_stateless_http.rs`"
  - phase: 113-06
    provides: "the single-parse v2 header gate (`raw_body_json`) and the `attach_v2_mrtr_params` ingress this plan runs behind"
provides:
  - "resumability_active_for / resumability_active / resumability_store — the ONE predicate + gated borrow every read, replay and store decision routes through"
  - "EventStoreHandle (= Arc<dyn EventStore>) on ServerState — type erasure that keeps the public config field's concrete type, and makes a test spy possible"
  - "envelope_for_live_request(payload, live_id) — the ONE constructor for a direct JSON-RPC response envelope on this transport"
  - "the in-source 13-row direct-response audit block (assemble / clone / cache / route / store, each with a verdict)"
  - "SpyEventStore — direct evidence of zero v2 event-store traffic, with a non-vacuous v1 counterpart"
  - "the T-113-07 fix: `build_response` no longer routes a v2 reply into another caller's SSE stream"
affects: [113-09, 113-10, 113-12, 113-13, 117]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two INDEPENDENT era gates over the same era value, rather than one gate the other silently relies on"
    - "Type-erase behind a crate-private field so helpers can be written against a trait without changing a public field's concrete type"
    - "Structural guarantee via argument shape (payload + id as separate parameters) instead of an audit comment plus a debug_assert"
    - "A spy with a NON-VACUOUS counterpart: assert zero on v2 AND non-zero on v1, from the same instrumented store"
    - "Scope an invariant in prose next to the code, and assert its exception separately as correct behavior"

key-files:
  created: []
  modified:
    - src/server/streamable_http_server.rs
    - tests/v2_stateless_http.rs

key-decisions:
  - "The spy is injected on the crate-private `ServerState`, not on `StreamableHttpServerConfig`. The public `event_store` field is pinned to the concrete `InMemoryEventStore`; widening it to `Arc<dyn EventStore>` is a public-field type change, i.e. a MAJOR semver break the milestone rules out (D-113-D discipline). `ServerState` gains a type-erased `EventStoreHandle` derived once in `make_server_state`."
  - "`build_sse_response_from_single_message`, `build_json_response`, `build_response` and `build_success_response_with_middleware` are recorded in the audit block as FRAMING sites, not refactored to call the constructor. They serialize/route an envelope that `envelope_for_live_request` already built; routing them through it a second time would create a second writer for no additional guarantee."
  - "The plan-04 error paths (`create_error_response_with_id`, `v2_gate_reject_response`, `map_unparsed_body_for_v2`) CANNOT call the constructor: `RequestId` has no `Null` variant and a JSON-RPC error for an unparseable body legitimately carries `id: null`. Their id comes from `raw_request_id(<the LIVE body>)`, so the invariant holds by construction anyway — recorded as an audit verdict."
  - "`v1_resumability_unchanged` and `v1_replayed_event_retains_original_id` use a BOUNDED local SSE reader (`sse_first_data_frame`), not the shared harness `get()`, which reads to EOF and would hang forever on a live stream. Kept local to the test file because plan 13 owns the general streaming-client surface."
  - "The `EventStore` trait, `InMemoryEventStore`, the `LAST_EVENT_ID` constant and the whole v1 replay path are left FULLY INTACT (CONTEXT.md Claude's-Discretion, lighter option). Removing them is a Phase-117 / SMPL-01 severability concern; touching them now maximizes v1 blast radius for zero v2 benefit."

patterns-established:
  - "An era gate must not lean on a sibling era gate's side effect — the resumability gate is independent of the session gate on purpose"
  - "A direct-response envelope is built from (payload, live id), never re-used wholesale"

requirements-completed: []

# Metrics
duration: 25min
tasks: 2
files: 2
completed: 2026-07-25
---

# Phase 113 Plan 08: No v2 Resumability, Live-Request Ids (HTTP-05) Summary

**The v2 path now refuses resumability structurally — it never reads `Last-Event-ID`, never
replays, and never writes to the event store, proven by a spy that records zero on v2 and
non-zero on v1 — and every DIRECT response is rebuilt from its payload plus the live request's
id, which is what makes a cached payload structurally incapable of carrying a stale id. Along
the way the plan found and fixed a real cross-caller response-delivery bug: a v2 POST that merely
NAMED a v1 caller's open session id had its response handed to THAT caller's SSE stream.**

## Performance

- **Duration:** ~25 min
- **Tasks:** 2 (4 commits: 2 RED/test, 2 GREEN)
- **Files modified:** 2

## Task Commits

| Task | Name | Commit | Key files |
| ---- | ---- | ------ | --------- |
| 1 | RED — a v2 response is delivered into a v1 caller's SSE stream | `614d95cf` | `src/server/streamable_http_server.rs` |
| 1 | Era-gate resumability off on v2 and prove it with a spy | `5a00cc89` | `src/server/streamable_http_server.rs` |
| 2 | Eight HTTP-05 regression tests for direct-response ids and v1 replay | `0d80f93d` | `tests/v2_stateless_http.rs` |
| 2 | Make the live-request id STRUCTURAL, not asserted | `5e61e714` | `src/server/streamable_http_server.rs` |

## What Was Built

### Task 1 — the resumability era gate, and a real bug it uncovered

**The gate** mirrors plan 04's `sessions_active` exactly — one pure rule, one state-reading
predicate, one gated borrow:

```rust
const fn resumability_active_for(cfg_has_event_store: bool, era: Option<Era>) -> bool {
    !matches!(era, Some(Era::V2)) && cfg_has_event_store
}
fn resumability_active(state: &ServerState, era: Option<Era>) -> bool { … }
fn resumability_store(state: &ServerState, era: Option<Era>) -> Option<&EventStoreHandle> { … }
```

`resumability_store` is now the ONLY way any code reaches the store, and it is what feeds
`store_response_event`, `replay_sse_events_from_header` and `sse_event_for_message`. The replay
helper's `let Some(store) = event_store else { return }` was moved to be the FIRST statement, so a
gated era returns before the function ever LOOKS at `Last-Event-ID` — the spec's "ignore it" taken
literally at the only site in the transport that reads that header.

**Why the gate is not redundant, even though v2 already stored nothing.** Before this plan a v2
request happened not to reach the event store — but only INCIDENTALLY, because
`store_response_event` is conditioned on a `response_session_id` that the SESSION gate already
zeroes on v2. An incidental guarantee is not a guarantee, and the bug below is precisely what
happens when one of two implicit couplings breaks and the other is assumed to cover it. The
resumability gate is deliberately INDEPENDENT of the session gate.

**The bug (`614d95cf`, genuine observed RED).** `build_response` routes a reply into
`state.sse_streams[sid]` keyed on the RAW INBOUND `Mcp-Session-Id` header — not on the
era-resolved `response_session_id`, which is always `None` on v2. So a v2 POST that merely NAMED a
v1 caller's open session id had its response **delivered into that caller's stream** (and written
to the event store on the way by `sse_event_for_message`), while the v2 caller received a bare
`202 Accepted`. That is T-113-07 (a direct response reaching a caller that never issued it) plus
T-113-29 / T-113-30 (v2 traffic reaching the event store) in one line of code. Observed RED:

```
assertion `left != right` failed: a v2 response must NEVER be handed to a session SSE stream
  left: 202
 right: 202
```

Fixed by threading `sessions_on` into `build_response` and filtering the stream lookup on it. On
v2 there is no session, so there is no stream to route to and the reply always returns to the
caller that asked for it.

**The type erasure.** `ServerState` gained a crate-private `event_store: Option<EventStoreHandle>`
(`= Arc<dyn EventStore>`), derived once in `make_server_state`. The public
`StreamableHttpServerConfig::event_store` field keeps its concrete `Arc<InMemoryEventStore>` type —
widening it is a public-field type change, i.e. a MAJOR semver break (D-113-D). The indirection is
what makes the spy possible at all.

**`SpyEventStore`.** Asserting "no replay happened" from a normal-looking 200 is weak: the response
is byte-identical whether replay ran and produced nothing or never ran. The spy counts stores and
replays through `AtomicUsize`, and every zero assertion has a non-vacuous v1 counterpart from the
same instrumented store:

| Exchange | stores | replays |
|----------|--------|---------|
| v2 POST carrying `Last-Event-ID: 12345` | **0** | **0** |
| v1 `initialize` POST | **> 0** | 0 |
| v1 GET carrying `Last-Event-ID` | 0 | **1** |
| v2 GET (405) | **0** | **0** |

The unit tests drive `handle_post_fast_path` / `handle_get_sse` directly — the real POST and GET
pipelines with no socket in the way, so nothing about the assertion is mocked except the store.

### Task 2 — the invariant, scoped and made structural

**Scoped in prose, next to the code** (Codex Plan-08 HIGH):

> Every DIRECT response to a live request carries THAT request's id, on BOTH eras. A REPLAYED
> HISTORICAL EVENT is not a direct response and legitimately retains its ORIGINAL id.

Without that scoping the claim contradicts v1 resumability, whose entire purpose is to re-emit
past events unchanged — a literal implementation would either break v1 replay or make the
assertion vacuous. The two halves are now asserted separately.

**Structural, not asserted** (Codex Plan-08 MEDIUM):

```rust
fn envelope_for_live_request(payload: ResponsePayload<Value, JSONRPCError>, live_id: RequestId)
    -> JSONRPCResponse
```

Payload and id are SEPARATE arguments, so a caller physically cannot pass a whole cached envelope
through and have its stale id survive. That argument shape is the guarantee; the `debug_assert!`
inside is belt and braces. All four direct-response assembly sites capture the live id BEFORE
dispatch consumes it and re-envelope the dispatch payload onto it: `handle_fast_path_request`,
`dispatch_message_with_middleware`, `assemble_discover_response_fast`,
`assemble_discover_response_with_middleware`.

**The audit, recorded in-source.** A 13-row table above the constructor names every site in this
transport that assembles, clones, caches, routes or stores a response, with its verdict. Two rows
are load-bearing rather than cosmetic:

- The plan-04 error paths **cannot** call the constructor: `RequestId` has no `Null` variant and a
  JSON-RPC error for a body that never parses legitimately carries `id: null`. Their id comes from
  `raw_request_id(<the LIVE body>)`, never from a cache, so the invariant holds by construction
  there too.
- `replay_sse_events_from_header` re-emits historical events VERBATIM with their ORIGINAL ids.
  That is correct, and it is what stops the direct-response invariant from being either vacuous or
  a v1 regression.

**No site was found reusing an envelope for a direct response.** One site WAS found handing a
direct response to the WRONG caller — the `sse_streams` route — and it is fixed (Task 1).

## Verification

| Check | Result |
| ----- | ------ |
| `cargo test --lib --features full -- streamable_http_server` | **53 passed** (37 pre-plan → 53) |
| `cargo test --test v2_stateless_http --features full` | **23 passed** (plan 04's 15 + these 8) |
| `cargo test --test v2_required_headers --features full` | 25 passed (Phase-112 baseline) |
| `cargo test --test v2_mrtr_ingress --features full` | 10 passed |
| `cargo test --test v2_client --features full` | 21 passed |
| `cargo test --test server_subscriptions --features full` | 6 passed (v1 subscribe path untouched) |
| `cargo test --test common_harness_smoke --features full` | 7 passed |
| `cargo build --lib --no-default-features` | OK (3 warnings, all pre-existing) |
| `cargo build --lib --target wasm32-unknown-unknown` | OK |
| `make lint` (UNPROXIED `/usr/bin/make`) | **exit 0, "No lint issues"** |
| **`cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp`** | **`223 checks: 223 pass, 30 skip` / `Summary no semver update required`** |
| **`make quality-gate`** (UNPROXIED `/usr/bin/make`) | **ALL TOYOTA WAY QUALITY CHECKS PASSED** |
| `git status --porcelain -- src/ tests/` after the gate | **empty** — the green gate is of the COMMITTED tree |

The gate was run unproxied via `/usr/bin/make` with cargo at `/Users/guy/.cargo/bin/cargo`, per the
plan-03 finding that the `rtk` shell proxy truncates the clippy stage and reports exit 0 for a run
that actually failed.

### Acceptance greps

| Criterion | Result |
|-----------|--------|
| `streamable_http_server.rs` contains `fn resumability_active` | **4** (`_for`, the state reader, and doc links) |
| `streamable_http_server.rs` contains `SpyEventStore` | **6** |
| every replay / store site guarded by `resumability_active` | yes — `resumability_store` is the only borrow, and the only caller of `state.event_store` |
| `LAST_EVENT_ID` retained for v1 (`>= 1`) | **6** |
| `streamable_http_server.rs` contains `fn envelope_for_live_request` taking payload + id separately | **3** (defn + doc links); **8** total call/mention sites |
| audit comment block enumerating every assembly/caching site | present — 13 rows above the constructor |
| all eight planned test fn names in `tests/v2_stateless_http.rs` | **8/8** |
| `response_id_concurrent_callers_do_not_cross` issues ≥ 8 concurrent requests | **12** |
| `cached_payload_is_reenveloped_with_live_id` provably reuses one payload object | yes — pointer addresses logged per call and asserted equal |
| `grep -c 'spawn_stateless_config' tests/v2_stateless_http.rs` | **0** (the file-header rule holds) |
| bare `-32xxx` literals in non-comment PRODUCTION code | **0** (the one match is a pre-existing plan-06 test assertion message) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] A v2 response was delivered into a v1 caller's SSE stream**

- **Found during:** Task 1, writing the spy fixture
- **Issue:** `build_response` selects the destination SSE stream from the RAW INBOUND
  `Mcp-Session-Id` header rather than from the era-resolved `response_session_id`. A v2 POST naming
  a v1 caller's open session id therefore had its response handed to that caller's stream, was
  written into the event store by `sse_event_for_message`, and the v2 caller got a bare
  `202 Accepted`.
- **Fix:** `sessions_on` is threaded into `build_response` and the stream lookup is filtered on it.
- **Files modified:** `src/server/streamable_http_server.rs`
- **Verification:** `v2_response_is_never_routed_into_a_session_sse_stream` (observed RED at
  `left: 202, right: 202`, GREEN after the fix)
- **Committed in:** RED `614d95cf`, GREEN `5a00cc89`

This is squarely inside the plan's own threat register (T-113-07, T-113-29, T-113-30) and inside
its must-have "No v2 response path … WRITES to the event store", so it is in scope rather than a
scope-boundary discovery.

### Intentional simplifications (recorded, not auto-fixes)

- **`build_sse_response_from_single_message` was NOT re-signatured to take `(payload, live_id)`.**
  The plan's action names it as a site to refactor. It — like `build_json_response`,
  `build_response` and `build_success_response_with_middleware` — FRAMES an envelope that
  `envelope_for_live_request` has already built one call earlier; routing it through the
  constructor a second time would mean two constructions of the same envelope per response and a
  second writer, for no additional guarantee. All four are instead enumerated in the audit block
  with an explicit `framing` verdict. The constructor is still called exactly once per direct
  response, at the assembly site, which is what the acceptance criterion is protecting.
- **The plan-04 error paths were NOT routed through the constructor.** This is a hard type
  constraint, not a preference: `RequestId` is `String(String) | Number(i64)` with no `Null`
  variant, and `create_error_response_with_id` must be able to emit `id: null` for a body that
  never parses. Recorded as an audit verdict with the reason, and their id already comes from
  `raw_request_id` over the LIVE body.
- **`v1_resumability_unchanged` / `v1_replayed_event_retains_original_id` use a local bounded SSE
  reader.** The shared harness's `get()` reads the body to EOF, which a live SSE stream never
  reaches, so driving these through it would hang forever. `sse_first_data_frame` reads chunks
  under a 5-second timeout and stops at the first `data:` frame. Kept local to the test file
  because plan 13 owns the general streaming-client surface.

### Plan assumptions that did not hold

**Task 2's direct-response half was already GREEN before the constructor landed.** All eight tests
in `0d80f93d` passed on first run. That is the honest measurement and it is worth recording
precisely: the id invariant held because `Server::handle_request_with_context` envelopes with the
id it was handed at dispatch — an INCIDENTAL property of the server core, not a transport-level
guarantee. The eight tests therefore land as regression guards, and
`envelope_for_live_request` is what converts the incidental property into a structural one. The
plan's own framing ("make the invariant structural, not asserted") anticipated exactly this; the
genuinely RED instance of this bug class was the cross-stream delivery found in Task 1, which is
the same defect one layer over.

---

**Total deviations:** 1 auto-fixed bug + 3 recorded simplifications + 1 measurement correction
**Impact on plan:** No stated behavior was dropped. Every `<behavior>` bullet and every
`<acceptance_criteria>` line is satisfied.

## TDD Gate Compliance

| Task | RED commit | RED evidence | GREEN commit |
|------|-----------|--------------|--------------|
| 1 | `614d95cf` `test(113-08)` | `assertion left != right failed … left: 202, right: 202` — a v2 response really was being handed to the v1 caller's stream | `5a00cc89` `feat(113-08)` |
| 2 | `0d80f93d` `test(113-08)` | **NOT observed RED** — all eight tests passed on first run (see "Plan assumptions that did not hold"). The predicate/constructor RED is by construction: `resumability_active_for`, `resumability_store`, `EventStoreHandle` and `envelope_for_live_request` did not exist, so a test naming them could only be a compile error, not an observable failure | `5e61e714` `feat(113-08)` |

No REFACTOR commit was needed on either cycle.

## Threat Model Coverage

| Threat ID | Disposition | How this plan discharged it |
|-----------|-------------|------------------------------|
| T-113-07 | mitigate | `envelope_for_live_request(payload, live_id)` makes a stale id structurally unconstructible for a direct response. Backed by `response_id_concurrent_callers_do_not_cross` (12 in-flight callers, distinct ids, plus a no-duplicates check) and `cached_payload_is_reenveloped_with_live_id`, whose fixture logs the payload's pointer address so the reuse is measured. **A live instance of this threat was found and fixed:** `build_response` was delivering a v2 reply into a v1 caller's SSE stream |
| T-113-29 | mitigate | The v2 path never reaches `replay_events_after` / `replay_sse_events_from_header`, and the replay helper returns BEFORE it looks at `Last-Event-ID` when the store is gated away. The spy records **0** replays for a v2 exchange, against **1** for the equivalent v1 GET |
| T-113-30 | mitigate | The v2 path also stores nothing — now by an INDEPENDENT gate rather than incidentally via the session gate. Spy: **0** stores on v2, **> 0** on the v1 `initialize` counterpart |
| T-113-19 | mitigate | `EventStore`, `InMemoryEventStore`, `LAST_EVENT_ID` and the v1 replay code are untouched. `v1_resumability_unchanged`, `v1_replayed_event_retains_original_id`, the spy's v1 rows and the 6 `tests/server_subscriptions.rs` tests are the guards |
| T-113-58 | mitigate | The invariant is scoped to DIRECT responses in the in-source doc block AND in the tests, with historical replay identity asserted separately as CORRECT behavior (`v1_replayed_event_retains_original_id` pins id `4711`, the original, and explicitly not `9999`, a later request's) |

## Known Stubs

None. No `TODO`/`FIXME`/`unimplemented!()` was introduced; the `make quality-gate` zero-SATD check
passes.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: response-routing | `src/server/streamable_http_server.rs` | `build_response` now gates its SSE-stream destination lookup on `sessions_on`. This CLOSES a cross-caller delivery path rather than opening one, but it is a behavior change at a trust boundary: a v2 POST that names an open v1 session id previously received `202 Accepted` and now receives its own `200` response. Any deployment that relied on the old (incorrect) fan-out would observe the difference |

## Follow-ups

1. **Plan 09 (MRTR egress hardening)** — `envelope_for_live_request` is now the only direct-response
   constructor on this transport. If egress ever needs to rebuild a response, build it from the
   payload plus the live id; do not re-use an envelope.
2. **Plan 10 / 13 (subscriptions/listen)** — HTTP-04 opens a LONG-LIVED v2 stream. It must not
   reintroduce event-store retention on that path: route any store access through
   `resumability_store`, which already answers `None` for `Era::V2`. The bounded SSE reader in
   `tests/v2_stateless_http.rs` (`sse_first_data_frame`) is deliberately local so plan 13 can design
   the general streaming-client surface without inheriting it.
3. **Phase 117 / SMPL-01 (severability)** — the `EventStore` trait, `InMemoryEventStore`, the
   `LAST_EVENT_ID` constant and the v1 replay code are all still present and are now reachable
   through exactly one gated borrow (`resumability_store`), which is the seam to cut at.
4. **Plan 12 (public-API + semver audit)** — no new public surface. `EventStoreHandle` and the
   `ServerState::event_store` field are both crate-private. Current measurement unchanged at
   `223 checks: 223 pass, 30 skip`.
5. **HTTP-05 is NOT marked complete** — per the 113-01 recorded exception, plan 12 owns the binding
   re-verification of the whole phase.

## Self-Check: PASSED

- `src/server/streamable_http_server.rs` — FOUND (`fn resumability_active_for`,
  `fn resumability_active`, `fn resumability_store`, `EventStoreHandle`, `SpyEventStore`,
  `fn envelope_for_live_request`, and the 13-row audit block all present; `LAST_EVENT_ID` retained)
- `tests/v2_stateless_http.rs` — FOUND (23 `#[tokio::test]`, all 8 planned names present,
  `spawn_stateless_config` count 0)
- Commit `614d95cf` — FOUND
- Commit `5a00cc89` — FOUND
- Commit `0d80f93d` — FOUND
- Commit `5e61e714` — FOUND

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-25*
