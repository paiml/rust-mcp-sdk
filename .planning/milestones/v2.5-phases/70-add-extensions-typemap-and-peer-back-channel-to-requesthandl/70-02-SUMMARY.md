---
phase: 70
plan: 02
subsystem: server/server_request_dispatcher + server/mod (legacy Server run loop) + server/core (ServerCore builder)
tags: [parity-handler-01, dispatcher, peer-back-channel-foundation, correlation-layer, mpsc-oneshot]
dependency_graph:
  requires:
    - "Plan 70-01 RequestHandlerExtra.extensions typemap (landed 9d1a2257 / ab92e690)"
    - "tokio mpsc + oneshot (already in tree)"
    - "serde_json (already in tree)"
    - "http::Extensions (already in tree)"
  provides:
    - "pub ServerRequestDispatcher struct with dispatch() + handle_response() + correlation-id pending map"
    - "spawn_server_request_drain(transport, outbound_rx) — wraps (id, ServerRequest) as TransportMessage::Request(Request::Server(...))"
    - "Server.server_request_dispatcher: Option<Arc<ServerRequestDispatcher>> field + run-loop wiring (outbound mpsc<(String, ServerRequest)>(100) channel, drain task, Response-arm rewrite)"
    - "ServerCore.server_request_dispatcher field + with_server_request_dispatcher() builder method (graceful None-fallback)"
    - "#[doc(hidden)] pub mod __test_support in src/lib.rs re-exporting dispatcher types for integration-test access"
  affects:
    - "Phase 70 Plan 03 — will consume ServerCore.server_request_dispatcher at 9 dispatch sites to construct DispatchPeerHandle instances"
    - "src/server/mod.rs TransportMessage::Response arm (was: drop-and-log-warn; now: route through dispatcher.handle_response when present, fallback preserved when absent)"
    - "ElicitationManager UNCHANGED (explicit out-of-scope plan constraint)"
tech-stack:
  added: []
  patterns:
    - "Correlation-id-keyed pending-oneshot map (clone-adapted from ElicitationManager elicit_input pattern in src/server/elicitation.rs:21-119)"
    - "Outbound mpsc<(String, ServerRequest)> + drain task + transport.send(TransportMessage::Request) — generalises the pattern sketched but never wired in the original ElicitationManager"
    - "#[doc(hidden)] test-support re-export — avoids promoting pub(crate) dispatcher API to stable surface while still letting integration tests link"
    - "Graceful None-fallback on both Server and ServerCore — every existing constructor compiles unchanged; dispatcher is always Option<Arc<...>>"
key-files:
  created:
    - src/server/server_request_dispatcher.rs
    - tests/server_request_dispatcher_integration.rs
  modified:
    - src/server/mod.rs
    - src/server/core.rs
    - src/lib.rs
decisions:
  - "Made ServerRequestDispatcher struct + methods pub (not pub(crate)) because integration tests under tests/ link the crate externally — the pub-use in src/lib.rs __test_support would not compile with pub(crate) items. The module itself stays pub(crate) so the symbol only leaks via the explicitly #[doc(hidden)] __test_support re-export. Intentional: Plan 03's PeerHandle surface will be the stable public API, not ServerRequestDispatcher."
  - "Used `#[doc(hidden)] pub mod __test_support` instead of `#[cfg(test)] pub mod __test_support` — the latter only works for unit tests, not integration tests (which compile as separate crates that do not see cfg(test) of the library). Documented clearly in the rustdoc as not part of stable API."
  - "ResponsePayload::Error maps to serde_json::to_value(err) rather than a generic Null — preserves error-payload semantics for future DispatchPeerHandle callers who may want to distinguish success from error JSON shapes."
  - "Drain task continues on transient transport send failure (logs and continues) rather than breaking — matches spawn_notification_handler which also logs-and-continues on send errors. Prevents single transport hiccup from killing all subsequent outbound requests for the server lifetime."
  - "Kept spawn_server_request_drain in the dispatcher module (not mod.rs) — single-file ownership of the wiring contract makes Plan 03's peer-impl easier to review."
  - "Added #[allow(clippy::struct_field_names)] on both server_request_dispatcher fields — clippy::struct_field_names fires on field names that share a prefix with the struct (Server.server_request_dispatcher / ServerCore.server_request_dispatcher). Renaming would break the clearly-named wiring convention; localised allow is the correct disposition."
metrics:
  duration: ~35m (wall clock, inclusive of quality-gate + test runs)
  completed: 2026-04-16
  tasks: 3
  commits: 3
  insertions: 542
  deletions: 2
---

# Phase 70 Plan 02: ServerRequestDispatcher foundation — Summary

**One-liner:** Added `pub ServerRequestDispatcher` with correlation-id-keyed pending-oneshot map + outbound mpsc drain task; rewired `Server::run` to spawn the drain and route `TransportMessage::Response` through `dispatcher.handle_response` instead of dropping; added `ServerCore.server_request_dispatcher` field + `with_server_request_dispatcher()` builder — all with graceful None-fallback so every existing constructor compiles unchanged. This is the architectural fix for Codex HIGH Findings 2 + 3 that the 4-plan replan identified as blocking Plan 03's peer wiring.

## Outcome

All three tasks in `70-02-PLAN.md` executed:

- **Task 1** — `src/server/server_request_dispatcher.rs` created (329 lines). Holds outbound `mpsc::Sender<(String, ServerRequest)>` + pending `HashMap<String, oneshot::Sender<Value>>` + 60s default timeout. `dispatch()` generates a monotonic correlation id via `AtomicU64`, inserts a pending oneshot, enqueues the pair, awaits `timeout(60s, rx)`, cleans pending on every non-success branch. `handle_response(correlation_id, value)` fulfills the matching oneshot; unknown ids return `INVALID_REQUEST` without crashing. `spawn_server_request_drain(transport, rx)` forwards each pair as `TransportMessage::Request(Request::Server(...))`. `Debug` impl prints only `timeout_duration` + `outbound_tx_closed` (T-70-15 mitigation). Five unit tests cover enqueue, fulfillment, timeout-cleans-pending, unknown-id-err, and debug-does-not-leak-correlation-ids.
- **Task 2** — `src/server/mod.rs` gains the `server_request_dispatcher: Option<Arc<...>>` field on `Server`. `ServerBuilder::build()` initialises to `None` (existing constructors compile unchanged). `Server::run` now creates the `mpsc<(String, ServerRequest)>(100)` channel, constructs the dispatcher `Arc`, stores it on `self`, and spawns `spawn_server_request_drain(transport.clone(), outbound_rx)` alongside the existing notification handler. The `TransportMessage::Response` arm in `handle_transport_message` no longer drops: it extracts `response.id.to_string()` as the correlation id, serialises `ResponsePayload::Result(value)` (or errors via `serde_json::to_value(err)`), and routes through `dispatcher.handle_response(&correlation_id, payload)`. Fallback `log_warning` remains when dispatcher is absent — but in the live wiring it never will be, because `Server::run` always populates it. `src/server/core.rs` gains the optional dispatcher field + `with_server_request_dispatcher(self, Arc<...>) -> Self` builder. No dispatch-site wiring yet (Plan 03).
- **Task 3** — `tests/server_request_dispatcher_integration.rs` created with two tests. `test_single_request_response_roundtrip` proves end-to-end correlation: dispatch enqueues, manual drain reads the pair, `handle_response` fulfills, awaiting `dispatch` returns `Ok(payload)`, `pending_count` returns to 0. `test_concurrent_multiplex_out_of_order` fires three concurrent dispatches, drains all three unique correlation ids, then fulfills in REVERSE order (c → b → a). Each awaiting future receives the correct payload keyed by id, proving the pending map is correlation-id-keyed (not FIFO) — the exact invariant that makes multi-in-flight peer RPCs safe.

Zero new Cargo.toml `[dependencies]` entries. Zero new feature flags. WASM build intact.

## Verification Results

| Check | Status | Notes |
|-------|--------|-------|
| `cargo build --features "full"` | pass | 0 compile errors |
| `cargo check --target wasm32-unknown-unknown --features schema-generation` | pass | Dispatcher is `#![cfg(not(target_arch = "wasm32"))]`; wasm Server unaffected |
| `cargo test --lib server::server_request_dispatcher::tests --features "full" -- --test-threads=1` | pass — 5/5 | enqueue, fulfill, timeout-cleans-pending, unknown-id-err, debug-no-leak |
| `cargo test --lib --features "full" -- --test-threads=1` | pass — 1019/1019 | No regressions to any existing lib test |
| `cargo test --test server_request_dispatcher_integration --features "full" -- --test-threads=1` | pass — 2/2 | single_request_response_roundtrip + concurrent_multiplex_out_of_order |
| `cargo test --test handler_extensions_properties --features "full" -- --test-threads=1` | pass — 5/5 | Plan 01 regression check — extensions typemap proptests still green |
| `make quality-gate` | pass | fmt + clippy pedantic + nursery + build + test + audit all green |

### Acceptance-criteria grep checks

- `grep -c 'pub struct ServerRequestDispatcher' src/server/server_request_dispatcher.rs` → 1
- `grep -c 'async fn dispatch' src/server/server_request_dispatcher.rs` → 1
- `grep -c 'async fn handle_response' src/server/server_request_dispatcher.rs` → 1
- `grep -c 'pub(crate) mod server_request_dispatcher' src/server/mod.rs` → 1
- `grep -c 'server_request_dispatcher' src/server/mod.rs` → 7 (struct field + builder default + run() construct + run() store + run() drain-spawn + Response-arm match + module decl)
- `grep -c 'spawn_server_request_drain' src/server/mod.rs` → 1 (invocation — definition lives in the dispatcher module per decision #5)
- `grep -c 'dispatcher.handle_response' src/server/mod.rs` → 2 (Response-arm routing + doctest)
- `grep -c 'server_request_dispatcher' src/server/core.rs` → 7 (struct field with doc + 2 cfg-gates + new() initializer + builder method + Arc<...> type + doc link)
- `grep -c 'pub fn with_server_request_dispatcher' src/server/core.rs` → 1

## Deviations from Plan

### Rule 3 — Blocking issue (auto-fixed)

**1. [Rule 3 - Blocking] Dispatcher items must be `pub` (not `pub(crate)`) to satisfy integration-test linking**
- **Found during:** Task 1 verify step (`cargo build --features "full"`)
- **Issue:** The plan's Task 1 Step 3 specifies `#[cfg(all(test, not(target_arch = "wasm32")))] pub mod __test_support { pub use crate::server::server_request_dispatcher::{...} }`. Two problems: (a) `#[cfg(test)]` on a library module does not make the symbol visible to integration tests (which compile as separate crates that link against the library's non-test build); (b) `pub use` cannot re-export `pub(crate)` items — rustc emits `E0364: only public within the crate, and cannot be re-exported outside`. The plan's `__test_support` approach is structurally unachievable with `pub(crate)` items.
- **Fix:** Two changes: (1) changed `pub(crate)` → `pub` on all dispatcher types (`ServerRequestDispatcher`, `DEFAULT_DISPATCH_TIMEOUT`, `dispatch`, `handle_response`, `new_with_channel`, `with_timeout`, `pending_count`, `spawn_server_request_drain`); (2) changed `#[cfg(all(test, ...))]` → `#[doc(hidden)] #[cfg(not(target_arch = "wasm32"))]` on the `__test_support` module. The rustdoc on `__test_support` explicitly documents that this is not part of the stable API surface. Module containing dispatcher remains `pub(crate)` so the only externally-visible path is through the hidden `__test_support` — internal consumers (Plan 03's peer impl in the same crate) use the direct `crate::server::server_request_dispatcher::...` path. Net effect: Codex Finding 3 fix preserves API hygiene. The plan's guidance explicitly allowed this path: "if the dispatcher needs to be `pub` for integration test access, mark it `pub`".
- **Files modified:** `src/server/server_request_dispatcher.rs`, `src/lib.rs`
- **Commit:** 3b914e0b

**2. [Rule 3 - Blocking] `clippy::struct_field_names` lint rejects `server_request_dispatcher` field on `Server` / `ServerCore`**
- **Found during:** Task 2 verify step (`make quality-gate` after format-fix pass)
- **Issue:** Clippy pedantic/nursery fires `struct_field_names` because the field name shares the `Server` prefix with the struct (`Server.server_request_dispatcher`, `ServerCore.server_request_dispatcher`). The lint suggests renaming. But renaming to something like `request_dispatcher` would lose the explicit direction marker — it's specifically a SERVER-to-client request dispatcher, not a general request router. All 9 dispatch sites in Plan 03 will reference this by its full name for grep-ability.
- **Fix:** Added `#[allow(clippy::struct_field_names)]` attribute on both field declarations — localised, one-line, with clear rationale captured in the commit message and this summary. This pattern is already used elsewhere in the crate (e.g., `auth_context` inside a `cancellation` module).
- **Files modified:** `src/server/mod.rs`, `src/server/core.rs`
- **Commit:** fd4945f4

### Rule 2 — Missing critical functionality (auto-added)

**3. [Rule 2 - Critical] Added Debug-redaction test for correlation-id leak**
- **Found during:** Task 1 writing phase (threat register review)
- **Issue:** T-70-15 (Information Disclosure — Debug leaks pending correlation ids) is listed in the plan's threat register as "mitigate" with the mitigation being a custom `Debug` impl that prints only `timeout_duration` + `outbound_tx_closed`. But the plan's 4 unit tests did not include an explicit assertion that the Debug output does NOT contain `"dispatch-"` (the correlation-id prefix). Without an executable guard, future refactors could accidentally re-introduce field enumeration in Debug and no test would catch it.
- **Fix:** Added 5th unit test `test_dispatcher_debug_does_not_leak_correlation_ids` that (a) inserts a pending dispatch via a spawned task, (b) sleeps 10ms to let the pending map populate, (c) asserts `format!("{:?}", dispatcher).contains("dispatch-") == false`. This bakes the T-70-15 mitigation into the test suite as a guard rail.
- **Files modified:** `src/server/server_request_dispatcher.rs` (tests module)
- **Commit:** 3b914e0b

## Threat Flags

No new threat surface introduced. All threat-register dispositions from the plan's `<threat_model>` block are honored:

- **T-70-12 (DoS — pending map unbounded growth)** mitigated: every `dispatch()` non-success branch calls `self.pending.write().await.remove(&correlation_id)`. Unit test `test_dispatcher_timeout_cleans_pending` asserts `pending_count()` returns to 0 after timeout. Integration test `test_single_request_response_roundtrip` asserts `pending_count()` returns to 0 after successful roundtrip. `test_concurrent_multiplex_out_of_order` asserts the same after 3 concurrent fulfillments.
- **T-70-13 (Tampering — forged correlation id)** mitigated: ids generated via `AtomicU64::fetch_add(1, Relaxed)`, prefixed `dispatch-`. An attacker cannot predict live counter state from inbound responses alone. Unit test `test_dispatcher_handle_response_unknown_id_returns_err` asserts unknown ids are rejected with `INVALID_REQUEST` rather than silently matched.
- **T-70-14 (Spoofing — drain task id mismatch)** accepted: the `(correlation_id, request)` tuple carries the id from dispatch() to drain, and the drain uses `RequestId::from(correlation_id.clone())` verbatim. Structural invariant; no separate gate needed.
- **T-70-15 (Info Disclosure — Debug leaks pending ids)** mitigated: custom `Debug` impl prints only `timeout_duration` + `outbound_tx_closed`. Unit test `test_dispatcher_debug_does_not_leak_correlation_ids` asserts the `"dispatch-"` prefix is never in Debug output.
- **T-70-16 (DoS — outbound channel saturation)** accepted: bounded `mpsc::channel(100)` is a deliberate backpressure signal. Saturation requires 100+ outstanding server-to-client RPCs; absent client responses, timeouts clear within 60s by default.

## Known Limitations

- **ElicitationManager unchanged.** Per plan constraint, `ElicitationManager` keeps its own pre-existing `request_tx` channel (which has never been wired to a transport drain — `set_request_channel` is never called in the tree). Migrating ElicitationManager to use the new dispatcher is a plausible future cleanup; explicitly out of Plan 02 scope. The new dispatcher and ElicitationManager coexist without interference.
- **session_id not populated at dispatch sites.** Codex Finding 1 identified that `RequestHandlerExtra.session_id` is always `None` at the 9 dispatch sites because the builders never call `.with_session_id(...)`. Plan 02 does NOT fix this — Plan 03 will need to address (or explicitly defer) session-id plumbing before DispatchPeerHandle can do per-session routing. This plan's dispatcher is deliberately session-agnostic (one dispatcher per Server).
- **Dispatcher absent outside `Server::run` lifecycle.** `server_request_dispatcher` is `None` on a freshly-built `Server` (and on a `ServerCore` without `with_server_request_dispatcher` called). This is intentional graceful fallback for the 12+ test fixtures in the crate that construct `Server::builder().build().unwrap()` without running the transport loop. Plan 03 must either (a) accept that `peer` is `None` outside the run loop (reasonable — there's no client to talk to before a transport connects), or (b) provide a test-harness dispatcher that can be attached synthetically.

## Downstream Dependencies

- **Plan 03** consumes `ServerCore.server_request_dispatcher` at the 9 dispatch sites. The `DispatchPeerHandle` impl calls `self.dispatcher.dispatch(ServerRequest::CreateMessage(...))` for sample, `self.dispatcher.dispatch(ServerRequest::ListRoots)` for list_roots, and delegates `progress_notify` to the existing `progress_reporter` field. No further dispatcher-core changes needed.
- **Plan 04** adds examples/fuzz/docs. No dispatcher-core changes expected.

## Self-Check: PASSED

- File created: `src/server/server_request_dispatcher.rs` (verified: 329 lines; contains `pub struct ServerRequestDispatcher`, `async fn dispatch`, `async fn handle_response`, `pub fn spawn_server_request_drain`)
- File created: `tests/server_request_dispatcher_integration.rs` (verified: contains `test_single_request_response_roundtrip` + `test_concurrent_multiplex_out_of_order`)
- File modified: `src/server/mod.rs` (verified: `server_request_dispatcher` field + builder default + Response-arm rewrite + spawn_server_request_drain invocation all grep-visible)
- File modified: `src/server/core.rs` (verified: optional dispatcher field + `with_server_request_dispatcher` builder grep-visible)
- File modified: `src/lib.rs` (verified: `#[doc(hidden)] pub mod __test_support` with `pub use` re-exports)
- Commit exists: `3b914e0b feat(70-02): add ServerRequestDispatcher module with correlation-id pending map`
- Commit exists: `fd4945f4 feat(70-02): wire ServerRequestDispatcher into Server::run + ServerCore builder`
- Commit exists: `bcc19762 test(70-02): add dispatcher integration tests — roundtrip + concurrent multiplex`
- `make quality-gate` green on final state
- 1019 lib tests + 2 new integration tests + 5 Plan 01 proptests all pass
- WASM compile check green (`cargo check --target wasm32-unknown-unknown --features schema-generation`)
- `ElicitationManager` code is byte-for-byte unchanged (plan constraint)
