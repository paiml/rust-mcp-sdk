---
phase: 117-agents-tester-v1-severability
plan: 12
subsystem: infra
tags: [rust, cargo-features, streamable-http, severability, paired-module, sse-resumability, last-event-id, mcp-2026-07-28]

# Dependency graph
requires:
  - phase: 117-09
    provides: "the seven era chokepoints living in the pair with fixed signatures on both feature sets, and a `V1State` that is a ZST on `full-v2` — the compile-time contract these bodies are written against"
  - phase: 117-06
    provides: "the `v1` paired module selected by two `cfg_attr` path attributes, and `tests/v1_severability_tripwire.rs`'s semantic null-twin checks (`FORBIDDEN_STATE_TYPES` / `FORBIDDEN_OPERATIONS`)"
  - phase: 117-02
    provides: "`tests/v1_byte_identity_after_cut.rs` — 9 goldens capturing v1 wire bytes BEFORE the cut, including the initialize response, the session header and the `Last-Event-ID` replay"
provides:
  - "the seven v1 session-lifecycle functions and the five SSE-replay / event-write functions living in `v1_session.rs`, with signature-identical constant twins in `v1_session_off.rs`"
  - "a `full-v2` build with NO reader of `Last-Event-ID` anywhere in the server — the twin names no header, so the T-113-29/30 ordering is structural rather than conditional"
  - "`session_id: Option<String>` still threading the ~10-function POST pipeline on both builds, always `None` on `full-v2`; zero pipeline surgery"
  - "the `:494-496` security audit table re-pointed at the moved sites"
  - "a doctest pinning `pmcp::server::streamable_http_server::InMemoryEventStore` as the public path (T-117-44)"
affects: [117-13, 117-14, SMPL-F1]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Structural absence over ordered absence: when a security property is 'do not even parse X', the strongest form is a build where nothing names X — not an early return placed before the parse"
    - "Seam collapse: when EVERY caller of a paired-module operation moves onto the real half, the operation stops being a seam. Privatise it in the real half and DELETE it from the twin, rather than inventing a call to keep the twin's copy alive"
    - "Constness follows the signature: a twin is `const fn` only where the real half's parameter types allow it (an owned `Option<String>` cannot be dropped in a const fn — E0493). Never buy constness by changing a parameter type"

key-files:
  created: []
  modified:
    - src/server/streamable_http_server.rs
    - src/server/streamable_http_server/v1_session.rs
    - src/server/streamable_http_server/v1_session_off.rs

key-decisions:
  - "The `EventStore` trait, `InMemoryEventStore`, `EventList` and `EventsMap` did NOT move, contrary to Task 2's instruction. Three independent blockers, any one of which is fatal: they are PUBLIC API at `pmcp::server::streamable_http_server::…` and the pair is `pub(crate)`; the PUBLIC field `StreamableHttpServerConfig::event_store` pins the concrete `InMemoryEventStore`, so `full-v2` cannot compile without it and gating that field is explicitly 117-13's subject; and `InMemoryEventStore` is in the tripwire's `FORBIDDEN_STATE_TYPES`, so the twin can never declare it and a `pub use v1::InMemoryEventStore` is unimplementable. Trait + store + config field are ONE edit, and it belongs to 117-13."
  - "`SessionCallback` did NOT move. The plan's stated reason — 'the session-lifecycle bodies name it' — is factually false (verified: its only three mentions are its own declaration and the two `StreamableHttpServerConfig` fields). Moving it would put a v1 config-field alias into the null twin for zero severance gain, inside 117-13's config-field scope."
  - "`active_session_generator` and `insert_session` are now PRIVATE to `v1_session.rs` and absent from the twin: both of their callers moved onto the real half in this plan, so neither is a seam any more. The alternatives for keeping the twin's copy alive were a `debug_assert!` whose real job is defeating the dead-code lint, or a branch with two identical arms — contrivances that a reviewer would rightly reject."
  - "The `resolve_sse_session` twin returns `405` for EVERY GET, including one carrying an `Mcp-Session-Id`. The real function's `Ok(sid)` passthrough exists only for v1 stateful mode; on a build with no sessions it would echo an attacker-supplied id back as a stream identity and open a stream that can never receive anything."
  - "`LAST_EVENT_ID` moved from the transport's file-scope `use` into the `#[cfg(test)] mod tests` block. Leaving it at file scope after its only production reader moved would be an unused import on the lib-only severance build, which `RUSTFLAGS=\"-D warnings\"` rejects."

patterns-established:
  - "A twin's doc comment carries the NEGATIVE instruction ('do not improve this by inspecting `headers` to log ignored cursors') next to the token the tripwire enforces, so the reader learns the rule and the test at the same place"
  - "When a plan instruction and a tripwire are irreconcilable, the tripwire wins and the conflict is recorded — the second time this has happened in Phase 117 (117-09 Deviation 3 was the first, on `EventStoreHandle`)"

requirements-completed: [SMPL-02]

# Metrics
duration: 95min
completed: 2026-08-08
---

# Phase 117 Plan 12: Move the v1 session lifecycle and the SSE-replay path into the pair Summary

**Twelve v1-only functions now live behind `v1-compat` with signature-identical constant twins, and the `full-v2` build has no reader of `Last-Event-ID` at all — the twin names no header, so "must not even parse an attacker-supplied replay cursor" stops being an ordering to preserve and becomes a property of the compiled crate.**

## Performance

- **Duration:** 95 min
- **Started:** 2026-08-08T18:00:00Z
- **Completed:** 2026-08-08T19:35:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- **The single most important twin in the phase is in.** `replay_sse_events_from_header` was the ONLY production reader of `LAST_EVENT_ID` in the server. It is now in the pair, the constant `LAST_EVENT_ID` is no longer imported at the transport's file scope, and the twin that replaces it takes a `&HeaderMap` and never touches it. Comment-stripped, `v1_session_off.rs` contains **zero** occurrences of `LAST_EVENT_ID` / `Last-Event-ID` — and zero of `HashMap`, `RwLock`, `MCP_SESSION_ID` and `headers.get`.
- **Twelve of twelve signature pairs match on parameter types, arity and return type**, verified programmatically (table below). `session_id: Option<String>` still threads ~10 pipeline functions unchanged; it is simply always `None` on `full-v2`. **No pipeline surgery occurred.**
- **Zero `#[cfg]` attributes at any call site.** `grep -c '#\[cfg(feature = "v1-compat")\]' src/server/streamable_http_server.rs` is **0**, unchanged from 117-09. The only feature attributes in the 6,000-line transport remain 117-06's two `cfg_attr` path lines, still single-line after `cargo fmt` (`grep -c 'cfg_attr(feature = "v1-compat"'` = **1**).
- **The v1 wire is byte-identical across the cut**: `v1_byte_identity_after_cut` **9/9** under `--features full`, including the initialize-response golden, the session-header golden and the `Last-Event-ID` replay golden — exactly the bytes these twelve functions produce.
- **`cargo test --lib --features full` is 1880 passed before and 1880 passed after.** An EXACT match, so no test was silently dropped. All five spy tests are green, including the three the plan names.
- **PMAT `--max-cognitive 25`: 0 violations in the three touched files before and after.** The move concentrated no branches.
- `make quality-gate` exits **0**; `make lint` and `make doc-check` exit **0** with zero warnings.

## Task Commits

1. **Task 1: Move the seven v1 session-lifecycle functions into the pair** — `124e132f` (refactor)
2. **Task 2: Move the SSE-replay path, with a twin that reads no header** — `fc21378b` (refactor)

## Files Created/Modified

- `src/server/streamable_http_server.rs` — 6293 → **6039 lines**. Twelve function bodies removed; 18 call sites and doc links re-pointed at `v1::`; `LAST_EVENT_ID` dropped from the file-scope import; the `:494-496` security audit table updated; the SEVERABILITY note rewritten to what is true at this commit; the `EventStore` trait and `InMemoryEventStore` given the DISTINCT-from-`shared::event_store` note plus a public-path doctest.
- `src/server/streamable_http_server/v1_session.rs` — 456 → **826 lines**. The real half: seven session-lifecycle functions and five SSE/event-store functions, bodies unchanged; `insert_session` and `active_session_generator` narrowed to module-private; 117-09's `session_is_initialized` / `mark_session_initialized` seams folded into the bodies that were their only callers.
- `src/server/streamable_http_server/v1_session_off.rs` — 252 → **436 lines**. The null twin: twelve constant-answer counterparts, four fewer declarations than before (the two folded seams plus the two privatised helpers).

**Measured severance:** the twelve functions occupy **272 lines** of code in the real half against **80 lines** of twins — 192 lines of v1-only production surface a `full-v2` build no longer compiles, plus the 23 lines of `insert_session` + `active_session_generator` that the twin no longer declares at all.

---

## The twelve signature pairs, side by side

Compared programmatically after stripping the twin's leading underscores: **parameter types identical 12/12, arity identical 12/12, return types identical 12/12.**

### Task 1 — session lifecycle

| # | Half | Signature |
|---|---|---|
| 1 | real | `pub(crate) fn process_init_session(state: &ServerState, era: Option<Era>, session_id: Option<String>, protocol_version: Option<String>) -> std::result::Result<(Option<String>, bool), Response>` |
|   | twin | `pub(crate) fn process_init_session(_state: &ServerState, _era: Option<Era>, _session_id: Option<String>, _protocol_version: Option<String>) -> std::result::Result<(Option<String>, bool), Response>` |
| 2 | real | `pub(crate) fn validate_non_init_session(state: &ServerState, era: Option<Era>, session_id: Option<String>) -> std::result::Result<Option<String>, Response>` |
|   | twin | `pub(crate) fn validate_non_init_session(_state: &ServerState, _era: Option<Era>, _session_id: Option<String>) -> std::result::Result<Option<String>, Response>` |
| 3 | real | `pub(crate) fn extract_negotiated_version(response: &TransportMessage) -> Option<String>` |
|   | twin | `pub(crate) const fn extract_negotiated_version(_response: &TransportMessage) -> Option<String>` |
| 4 | real | `pub(crate) fn update_session_after_init(state: &ServerState, session_id: Option<&String>, negotiated_version: Option<String>)` |
|   | twin | `pub(crate) fn update_session_after_init(_state: &ServerState, _session_id: Option<&String>, _negotiated_version: Option<String>)` |
| 5 | real | `pub(crate) fn validate_protocol_version_matches_session(state: &ServerState, era: Option<Era>, session_id: Option<&String>, protocol_version: Option<&String>) -> std::result::Result<(), Response>` |
|   | twin | `pub(crate) const fn validate_protocol_version_matches_session(_state: &ServerState, _era: Option<Era>, _session_id: Option<&String>, _protocol_version: Option<&String>) -> std::result::Result<(), Response>` |
| 6 | real | `pub(crate) fn is_initialize_request(message: &TransportMessage) -> bool` |
|   | twin | `pub(crate) const fn is_initialize_request(_message: &TransportMessage) -> bool` |
| 7 | real | `pub(crate) fn resolve_session_for_request(state: &ServerState, era: Option<Era>, is_init_request: bool, session_id: Option<String>, protocol_version: Option<String>) -> std::result::Result<Option<String>, Response>` |
|   | twin | `pub(crate) fn resolve_session_for_request(state: &ServerState, era: Option<Era>, is_init_request: bool, session_id: Option<String>, protocol_version: Option<String>) -> std::result::Result<Option<String>, Response>` |

Pair 7 is the one whose twin carries **no** underscores: it delegates to pairs 1 and 2 rather than returning a bare `Ok(None)`, so every parameter is genuinely consumed. That is 117-09's established shape ("null twins delegate to their pure rule instead of returning a bare constant"), and here it is load-bearing rather than stylistic — see Deviation 4.

### Task 2 — SSE replay and event writes

| # | Half | Signature |
|---|---|---|
| 8 | real | `pub(crate) async fn store_response_event(state: &ServerState, era: Option<Era>, response_session_id: Option<&String>, response_msg: &TransportMessage)` |
|   | twin | `pub(crate) async fn store_response_event(_state: &ServerState, _era: Option<Era>, _response_session_id: Option<&String>, _response_msg: &TransportMessage)` |
| 9 | real | `pub(crate) fn resolve_sse_session(state: &ServerState, incoming_session_id: Option<String>) -> std::result::Result<String, Response>` |
|   | twin | `pub(crate) fn resolve_sse_session(_state: &ServerState, _incoming_session_id: Option<String>) -> std::result::Result<String, Response>` |
| 10 | real | `pub(crate) async fn replay_sse_events_from_header(headers: &HeaderMap, tx: &mpsc::UnboundedSender<TransportMessage>, event_store: Option<&EventStoreHandle>)` |
|    | twin | `pub(crate) async fn replay_sse_events_from_header(_headers: &HeaderMap, _tx: &mpsc::UnboundedSender<TransportMessage>, _event_store: Option<&EventStoreHandle>)` |
| 11 | real | `pub(crate) fn sse_event_for_message(msg: &TransportMessage, session_id: &str, event_store: Option<&EventStoreHandle>) -> Event` |
|    | twin | `pub(crate) fn sse_event_for_message(_msg: &TransportMessage, _session_id: &str, _event_store: Option<&EventStoreHandle>) -> Event` |
| 12 | real | `pub(crate) fn attach_sse_response_headers(response: &mut Response, session_id: &str)` |
|    | twin | `pub(crate) const fn attach_sse_response_headers(_response: &mut Response, _session_id: &str)` |

Two textual differences, both carried forward from 117-09 and both deliberate: **leading underscores** on ignored twin parameters (without them `-D warnings` fails on `unused_variables`, so literal textual identity is unachievable), and **`const fn` on four twins** where the real half is a plain `fn` (the plan's "where the signature allows" — see Deviation 2 for the four where it does not).

---

## The `replay_sse_events_from_header` twin, in full

This is the plan's headline artifact. Pasted verbatim from `src/server/streamable_http_server/v1_session_off.rs`:

```rust
/// Nothing is ever replayed — and, critically, NO HEADER IS EVER READ.
///
/// This is the most load-bearing twin in the pair. The real function returns
/// before it looks at the replay cursor when resumability is off, and that
/// ORDERING is the mitigation for T-113-29 / T-113-30: an era that suppresses
/// resumability must not even PARSE an attacker-supplied replay cursor. Here the
/// ordering is not something to preserve on the next edit — there is no header
/// access to order. `headers` is taken so the signature matches its real
/// counterpart, and it is never touched.
///
/// Do not "improve" this by inspecting `headers` to log or count ignored
/// cursors: that would put an attacker-controlled parse back into the build that
/// exists to prove it absent, and `tests/v1_severability_tripwire.rs` fails on
/// the `LAST_EVENT_ID` token for exactly that reason.
///
/// Stays `async` because the signature is the real half's and the GET handler
/// `.await`s it.
pub(crate) async fn replay_sse_events_from_header(
    _headers: &HeaderMap,
    _tx: &mpsc::UnboundedSender<TransportMessage>,
    _event_store: Option<&EventStoreHandle>,
) {
}
```

The body is empty. There is no parameter dereference, no `headers.get`, no `LAST_EVENT_ID`, and no `Option` inspection — the function cannot reach a replay cursor because the code that would has not been compiled.

Enforcement is doubled: `tests/v1_severability_tripwire.rs::the_v1_null_twin_performs_no_state_or_header_operation` fails on the `LAST_EVENT_ID` token in comment-stripped source, and the negative instruction sits in the doc comment where the next editor will read it.

**Comment-stripped token counts in `v1_session_off.rs`:** `LAST_EVENT_ID` / `Last-Event-ID` **0**, `HashMap` **0**, `RwLock` **0**, `MCP_SESSION_ID` **0**, `headers.get` **0**. (The four raw-grep hits are all inside doc comments — the module doc and this function's doc — which is what the tripwire strips before asserting.)

---

## Before/after measurements

| Measurement | Before (`80661cc9`) | After (`fc21378b`) |
|---|---|---|
| `cargo test --lib --features full` | **1880 passed** | **1880 passed** (exact) |
| PMAT `--max-cognitive 25`, `streamable_http_server.rs` | `[]` (0 violations) | **`[]` (0 violations)** |
| PMAT `--max-cognitive 25`, `v1_session.rs` | `[]` | **`[]`** |
| PMAT `--max-cognitive 25`, `v1_session_off.rs` | `[]` | **`[]`** |
| `streamable_http_server.rs` lines | 6293 | **6039** |
| `v1_session.rs` lines | 456 | **826** |
| `v1_session_off.rs` lines | 252 | **436** |
| `#[cfg(feature = "v1-compat")]` in the transport | 0 | **0** |
| `cfg_attr(feature = "v1-compat"` single-line matches | 1 | **1** |
| production readers of `LAST_EVENT_ID` in the transport | 1 | **0** |

The PMAT query used is `pmat analyze complexity --format json --max-cognitive 25 | jq '[.violations[]? | select(.path | test("streamable_http_server"))]'`, which returned the empty list on both sides.

## Verification

| Gate | Result |
|---|---|
| `cargo build -p pmcp --features full` | exit **0** |
| `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | exit **0**, `grep -c 'warning:'` = **0** |
| `cargo test --lib --features "full"` | **1880 passed** |
| `spy_records_zero_event_store_traffic_for_a_v2_exchange` | **pass** |
| `spy_records_zero_replay_for_a_v2_get` | **pass** |
| `v2_response_is_never_routed_into_a_session_sse_stream` | **pass** |
| `spy_records_store_traffic_for_a_v1_exchange` (the v1 non-vacuity anchor) | **pass** |
| `spy_records_replay_for_a_v1_get_with_last_event_id` (the v1 non-vacuity anchor) | **pass** |
| `cargo test --test v1_byte_identity_after_cut --features "full"` | **9 passed** |
| `cargo test --test v1_severability_tripwire` | **9 passed** |
| `cargo test --doc --features full` | **446 passed** (includes the new `InMemoryEventStore` public-path doctest) |
| `make lint` | exit **0** |
| `make doc-check` | exit **0**, zero rustdoc warnings |
| `make quality-gate` | exit **0** |
| `git diff` on `extract_session_and_protocol_headers` / `compute_outbound_protocol_version` | **no change to either definition** — the only diff lines naming them are two doc-link mentions inside `handle_post_fast_path`'s rustdoc |

The severance build is reported SEPARATELY from `make quality-gate`, which runs `--all-features` and can never prove severance (Cargo features are additive).

The v1 anchors are listed alongside the three v2 spy tests deliberately: a "v2 writes zero" assertion is only meaningful next to a "v1 writes non-zero" one, and both survived the move.

---

## The `:494-496` security audit table — YES, it needed updating

Three of the table's thirteen rows named functions that moved in Task 2. All three were re-pointed; no row was added, removed or re-worded otherwise, and no VERDICT changed.

| Row | Before | After |
|---|---|---|
| caching | `` `store_response_event` `` | `` `v1::store_response_event` `` |
| caching | `` `sse_event_for_message` `` | `` `v1::sse_event_for_message` `` |
| historical | `` `replay_sse_events_from_header` `` | `` `v1::replay_sse_events_from_header` `` |

The prose line above the table — "HISTORICAL event -> re-emitted verbatim by `replay_sse_events_from_header`" — was updated to match. This closes T-117-43: the audit table is not orphaned.

The `v1::route_to_session_stream` row (the T-113-07 fix) already carried its `v1::` prefix from 117-09 and was left alone.

## How `InMemoryEventStore`'s public path was preserved

**It was preserved by not moving it.** The type is still declared in `src/server/streamable_http_server.rs`, so its public path is unchanged at:

```
pmcp::server::streamable_http_server::InMemoryEventStore
```

That is now PINNED rather than merely true, closing T-117-44. A doctest on the type names the full public path from outside the crate — the only form that proves a *public* path rather than a crate-internal spelling of it:

```rust
use pmcp::server::streamable_http_server::{InMemoryEventStore, StreamableHttpServerConfig};
use std::sync::Arc;

let store = Arc::new(InMemoryEventStore::default());
let config = StreamableHttpServerConfig {
    event_store: Some(Arc::clone(&store)),
    ..Default::default()
};
assert!(config.event_store.is_some());
```

A doctest, not a `#[test]`, on purpose: doctests run under `cargo test --doc` and therefore do NOT change the `cargo test --lib` pass count, which the plan requires to match 1880 EXACTLY. Adding an in-file test would have made that criterion fail for a good reason, which is the worst kind of failure.

The same edit added the DISTINCT-trait note the plan asked for, on the trait itself and on the store: this module's three-method `EventStore` is not `crate::shared::event_store::EventStore` (six methods), and this `InMemoryEventStore` is not `crate::shared::event_store::InMemoryEventStore`. Both same-named pairs now say so in their rustdoc.

---

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] The `EventStore` trait, `InMemoryEventStore` and the `EventList` / `EventsMap` aliases did NOT move into the pair**

- **Found during:** Task 2
- **Issue:** Task 2 instructs these four items into `v1_session.rs`. Three independent blockers make that impossible at this commit, any one of which is fatal:
  1. **Public API vs a `pub(crate)` module.** `EventStore` and `InMemoryEventStore` are `pub` items of `pub mod streamable_http_server`. The pair is `pub(crate) mod v1`, so moving them removes them from the public API unless re-exported.
  2. **A public config field pins the concrete type.** `StreamableHttpServerConfig::event_store` is `pub event_store: Option<Arc<InMemoryEventStore>>`, and `Default` constructs one. That field is compiled on BOTH feature sets, so the `full-v2` build cannot compile without `InMemoryEventStore` in scope. Gating the field is explicitly **plan 117-13's** subject ("`StreamableHttpServerConfig`'s four v1-only public fields"), and this plan's own instructions forbid touching 117-13's surface.
  3. **The tripwire forbids the token in the twin.** `InMemoryEventStore` is a literal entry in `FORBIDDEN_STATE_TYPES`, so the null twin can never declare it — which rules out the `pub use v1::InMemoryEventStore;` re-export that would have solved (1). The remaining route, a `#[cfg(feature = "v1-compat")]` re-export, would put the phase's first call-site feature gate into the transport and STILL leave blocker (2) open.
- **Fix:** All four stay in `streamable_http_server.rs`, exactly as `EventStoreHandle` did in 117-09 Deviation 3 — the same conflict, one plan later. The five FUNCTIONS moved as planned, which is the substantive part of Task 2. The SEVERABILITY note beside `EventStoreHandle` now records the blocker chain and hands trait + store + config field to 117-13 as ONE edit, so the next attempt does not rediscover it. The DISTINCT-trait doc line and the public-path pin the plan asked for were both delivered in place.
- **Files modified:** `src/server/streamable_http_server.rs`
- **Verification:** severance build exit 0, zero warnings; `v1_severability_tripwire` 9/9; the new doctest passes; `make doc-check` clean
- **Committed in:** `fc21378b`

**2. [Rule 3 — Blocking] Four twins are plain `fn`, not `const fn`**

- **Found during:** Task 1
- **Issue:** Writing all seven Task-1 twins as `const fn` produced six `E0493: destructor of Option<String> cannot be evaluated at compile-time` errors on the severance build. A `const fn` cannot take an owned `Option<String>` by value, because dropping it at the end of the function is not const-evaluable.
- **Fix:** `process_init_session`, `validate_non_init_session`, `update_session_after_init` and `resolve_session_for_request` are plain `fn`; the other three (and three of the five Task-2 twins) are `const fn`. Constness follows the SIGNATURE, which is fixed by the real half — it is never bought by changing a parameter type. The reason is written as a comment above the twin block so the next editor does not "fix" it.
- **Files modified:** `src/server/streamable_http_server/v1_session_off.rs`
- **Verification:** severance build exit 0
- **Committed in:** `124e132f`

**3. [Rule 3 — Blocking] 117-09's `session_is_initialized` and `mark_session_initialized` seams folded into the moved bodies and deleted from the pair**

- **Found during:** Task 1
- **Issue:** After the move, the only caller of each was inside `v1_session.rs`. On the `full-v2` build the twin's `process_init_session` / `update_session_after_init` answer with constants and never call them, so both twins became dead code and the severance build failed under `-D warnings` with `function ... is never used`.
- **Fix:** Both operations inlined into the bodies that were their only callers, and deleted from both halves. This is exactly what 117-09's summary predicted in its "Notes for 117-12 and 117-13": *"their bodies can inline the corresponding operation and the operation can be deleted — but only if EVERY caller moves."* The inlined `session_is_initialized` binds its answer to a local first so the read lock is released before the error response is built, preserving the original lock scope.
- **Files modified:** `v1_session.rs`, `v1_session_off.rs`
- **Verification:** severance build exit 0, zero warnings; 1880/1880; goldens 9/9
- **Committed in:** `124e132f`

**4. [Rule 3 — Blocking] `insert_session` and `active_session_generator` narrowed to module-private and dropped from the twin; `resolve_session_for_request`'s twin delegates**

- **Found during:** Task 2
- **Issue:** The same dead-code failure, one layer out. `insert_session`'s two callers (`process_init_session`, `resolve_sse_session`) and `active_session_generator`'s two callers (the same two) had all moved onto the real half by the end of Task 2, so both twins were unreachable. `active_session_generator` is one of 117-09's seven era chokepoints, so deleting its twin is not a neutral act.
- **Fix:** Both are now **private** to `v1_session.rs` — they still exist, still gate `config.session_id_generator` behind `sessions_active`, and are simply no longer seams the transport crosses. The twin declares neither, which is the direction the tripwire wants (the twin must declare nothing the real module does not). The transport's chokepoint list comment records the change so the seven-chokepoint claim from 117-09 is not left stale. Separately, `resolve_session_for_request`'s twin DELEGATES to the twins of `process_init_session` / `validate_non_init_session` rather than returning a bare `Ok(None)` — that keeps those two names live (the plan requires all seven in both halves) and keeps `is_init_request` visibly consumed. The rejected alternatives for `active_session_generator` were a `debug_assert!` whose real purpose is defeating the lint, and a `match` with two identical arms; both are contrivances a reviewer would see through, and neither would survive a later reader asking "why is this here?".
- **Files modified:** all three
- **Verification:** severance build exit 0, zero warnings; `v1_severability_tripwire` 9/9; `make lint` exit 0
- **Committed in:** `fc21378b`

**5. [Rule 1 — Correctness] `SessionCallback` did NOT move**

- **Found during:** Task 1
- **Issue:** Task 1 says to move it "since the session-lifecycle bodies name it". Measured: `grep -rn 'SessionCallback' src/` returns exactly three sites — its own declaration and the two `StreamableHttpServerConfig` fields (`on_session_initialized`, `on_session_closed`). **No moved body names the type**; `process_init_session` and `resolve_sse_session` call `state.config.on_session_initialized` without spelling it.
- **Fix:** Left in place. Moving it would force the twin to declare `Box<dyn Fn(&str) + Send + Sync>` — machinery in the file whose job is to contain none — for zero severance gain, since the two public config fields it types are compiled on both feature sets until 117-13 gates them. It belongs to the same one-edit cluster as the event store: config field and alias together, in 117-13.
- **Files modified:** none (deliberate non-change)
- **Verification:** the grep above; severance build exit 0
- **Committed in:** n/a — recorded here

**6. [Rule 1 — Correctness] The `resolve_sse_session` twin returns `405` for every GET, including one carrying a session id**

- **Found during:** Task 2
- **Issue:** The real function has two exits when sessions are inactive: `Ok(sid)` if the request supplied an `Mcp-Session-Id`, and `405 "SSE not supported in stateless mode"` if it did not. A twin that reproduced the first exit would echo an attacker-supplied id back as a stream identity and then open an SSE stream that can never receive anything (the twin's `route_to_session_stream` hands every message back).
- **Fix:** The twin returns the `405` unconditionally. This narrows one `full-v2` behaviour — a GET with a session header now gets `405` instead of a dead stream — and the narrowing is the point: the `Ok(sid)` passthrough exists only for v1 stateful mode, which this build does not have. The reasoning is in the twin's doc comment. No `full` behaviour changes, and the goldens are unaffected (they exercise `full`).
- **Files modified:** `src/server/streamable_http_server/v1_session_off.rs`
- **Verification:** goldens 9/9 on `full`; severance build exit 0
- **Committed in:** `fc21378b`

**7. [Rule 1 — Correctness] `LAST_EVENT_ID` moved from file-scope to test-scope import**

- **Found during:** Task 2
- **Issue:** With `replay_sse_events_from_header` moved, the transport's only remaining uses of `LAST_EVENT_ID` are three lines inside `#[cfg(test)] mod tests`. A file-scope `use` consumed solely by test code is an unused import on the lib-only severance build, which `-D warnings` rejects.
- **Fix:** Dropped from the file-scope `crate::shared::http_constants::{…}` list and imported inside the test module, with a comment explaining why. This is also a small severance win in its own right: the v2 transport no longer even *names* the constant. The constant itself in `src/shared/http_constants.rs` was NOT gated — its two readers (this transport's replay path and the client at `src/shared/streamable_http.rs`) must be gated together in one edit, which is 117-13's job, exactly as the plan instructs.
- **Files modified:** `src/server/streamable_http_server.rs`
- **Verification:** severance build exit 0 with zero warnings; 1880/1880 (the three test uses still compile)
- **Committed in:** `fc21378b`

**8. [Rule 1 — Correctness] Two rustdoc links to `EventStoreHandle` replaced with code spans**

- **Found during:** Task 2
- **Issue:** The new DISTINCT-trait note on the `pub trait EventStore` linked to `[`EventStoreHandle`]`, which is `pub(crate)`. `make doc-check` failed with `private_intra_doc_links` under `RUSTDOCFLAGS="-D warnings"`.
- **Fix:** Both links became plain code spans naming the alias and where to find it. A public item's rustdoc cannot link to a crate-private one.
- **Files modified:** `src/server/streamable_http_server.rs`
- **Verification:** `make doc-check` exit 0, zero warnings
- **Committed in:** `fc21378b`

**9. [Rule 1 — Correctness] Deleted an orphaned duplicate doc line**

- **Found during:** Task 2
- **Issue:** A stray `/// Handle GET requests for SSE streams` sat immediately above `resolve_sse_session` — a leftover from the 75-01 helper extraction, describing `handle_get_sse` rather than the function it was attached to. Moving the function would have left it dangling directly above `handle_get_sse`'s own doc, whose first line is the same sentence with a full stop.
- **Fix:** Deleted. `handle_get_sse` keeps its own, correct, doc comment.
- **Files modified:** `src/server/streamable_http_server.rs`
- **Verification:** `make doc-check` exit 0
- **Committed in:** `fc21378b`

---

**Total deviations:** 9 auto-fixed (4 blocking, 5 correctness)
**Impact on plan:** Deviation 1 is the only one that reduces delivered scope, and it is a scope the plan could not have had: the same conflict blocked `EventStoreHandle` in 117-09, and the correct resolution is to hand the whole cluster to 117-13 as one edit rather than to half-migrate it now. Deviations 3 and 4 are the plan's own mechanism working — 117-09 predicted them in writing. No external package was added, no public API changed, and no `#[cfg]` entered the transport.

## Issues Encountered

- **`cargo fmt` reflowed one moved call site after the mechanical `v1::` rewrite**, turning a hand-wrapped `.await` back onto one line and failing `make quality-gate`'s `fmt-check` before clippy ever ran. Fixed with `cargo fmt --all`; the `cfg_attr` single-line grep was re-checked afterwards and is still **1**. This is the third recorded instance of the carry-forward warning that `cargo fmt` can defeat single-line detectors.
- **Ordered string replacement matters.** Replacing the 4-space-indented `store_response_event(...)` call before the 12-space one silently consumed the deeper one's suffix and left it unmatched. Caught by an assertion in the rewrite script before anything was written to disk. A blind `sed -i` would have produced a half-rewritten file.
- **`cargo test --lib` remains weaker than `make lint`, and both are weaker than `make quality-gate`.** The formatting failure above was invisible to all of `cargo build`, `cargo test` and `make lint`.
- **`cargo test --lib --features full -- --exact <name>` with an unqualified test name silently matches zero tests** and still reports success. The lib test paths are `server::streamable_http_server::tests::…`, not `streamable_http_server::tests::…`; the first attempt reported "0 passed, 1880 filtered out" and exit 0. Derived the real paths with `-- --list` before asserting. Same class as the recorded `nextest test() vs binary()` hazard: a selector that matches nothing does not fail.

## Notes for 117-13

- **Your first edit should be the event-store cluster, as ONE change:** gate `StreamableHttpServerConfig::event_store` (and `on_session_initialized` / `on_session_closed` / whatever the fourth v1-only field is), then the `EventStore` trait, `InMemoryEventStore`, `EventList`, `EventsMap` and `SessionCallback` become gateable in the same commit. Doing the types first is impossible — the config field pins them. The blocker chain is written into the SEVERABILITY note beside `EventStoreHandle` in the transport.
- **`InMemoryEventStore` now has a doctest naming its public path.** If you gate the type, that doctest must be gated with it (`cargo test --doc` runs with `full`, so it will still run — but a `#[cfg]`-gated type needs its example gated too, or `make doc-check` on the twin feature list will fail).
- **`LAST_EVENT_ID` has exactly two readers left**: `v1::replay_sse_events_from_header` (now inside the pair, so a `#[cfg]` on the constant is invisible to it) and `src/shared/streamable_http.rs`. The transport's test module also imports it — gating the constant means gating that test-module import too.
- **`handle_get_sse` and `handle_delete_session` are untouched** and remain the MIXED verb-split subject, each still beginning with `v2_verb_rejection`. `handle_get_sse` now calls four `v1::` functions in a row after its rejection gate, which is a natural split point.
- **`compute_outbound_protocol_version` and `extract_session_and_protocol_headers` are byte-identical** to their pre-plan form; only two doc-link mentions of them inside another function's rustdoc changed.
- **`active_session_generator` is no longer in the pair's public surface.** If your split needs it back at the seam, un-privatise it in `v1_session.rs` and re-add the twin — but only if a caller outside `v1_session.rs` actually appears, or the twin goes dead again.

## User Setup Required

None — no external service configuration required. Zero packages added; this plan is a pure in-crate move.

## Next Phase Readiness

- SMPL-02's largest measured chunk is done: twelve v1-only functions plus the era decisions and all v1 state are now structurally absent from a `full-v2` build.
- 117-13 is unblocked and has a precise, pre-diagnosed first edit (the event-store cluster above).
- No blockers.

---
*Phase: 117-agents-tester-v1-severability*
*Completed: 2026-08-08*

## Self-Check: PASSED

All three modified source files and this summary exist on disk, and all three commits
(`124e132f`, `fc21378b`, `b5f326b8`) resolve in `git log --all`.

Note: the repository has no `.git/hooks/pre-commit` installed, so `make quality-gate` was run
MANUALLY (exit **0**) rather than by a hook. It was run against the final tree of Task 2, after
`cargo fmt --all`, and the severance build was re-verified afterwards.
