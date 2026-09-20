---
phase: 70
plan: 03
subsystem: shared/peer + server/peer_impl + server/cancellation (peer field) + server/core (4 dispatch sites) + server/mod (5 legacy dispatch sites) + tests/handler_peer_integration
tags: [parity-handler-01, peer-back-channel, sampling, list-roots, progress-notify, dispatcher-delegation, graceful-fallback]
dependency_graph:
  requires:
    - "Plan 70-01 #[non_exhaustive] + Extensions typemap on RequestHandlerExtra (landed 9d1a2257)"
    - "Plan 70-02 ServerRequestDispatcher + ServerCore.server_request_dispatcher field (landed 3b914e0b / fd4945f4 / bcc19762)"
    - "async_trait, tokio mpsc/oneshot, serde_json (already in tree)"
  provides:
    - "pub trait PeerHandle (Send + Sync + async_trait) in src/shared/peer.rs"
    - "pmcp::PeerHandle re-export at crate root (non-wasm)"
    - "pub struct DispatchPeerHandle in src/server/peer_impl.rs that DELEGATES to ServerRequestDispatcher (no ad-hoc channels)"
    - "RequestHandlerExtra.peer: Option<Arc<dyn PeerHandle>> field (non-wasm) + with_peer builder + peer() accessor"
    - "9 dispatch sites conditionally wire .with_peer(...) when ServerCore.server_request_dispatcher.is_some()"
    - "__test_support::DispatchPeerHandle re-export appended to src/lib.rs"
  affects:
    - "Plan 70-04 (examples s42 + s43 + fuzz target + migration prose + final quality gate)"
    - "Future follow-on: notification_tx plumbing through DispatchPeerHandle for live progress_notify"
    - "Future follow-on: ElicitationManager migration to shared dispatcher (explicit out-of-scope)"
    - "Future follow-on: session_id plumbing through ProtocolHandler::handle_request signature"
tech-stack:
  added: []
  patterns:
    - "PeerHandle trait delegates to ServerRequestDispatcher — single correlation authority"
    - "Graceful None-fallback on every dispatch site (if let Some(dispatcher) = self.server_request_dispatcher.as_ref())"
    - "Best-effort progress_notify (returns Ok(()) silently) — matches existing report_progress no-op contract"
    - "Object-safe async trait via async_trait for Arc<dyn PeerHandle> storage"
    - "Real constructors everywhere in tests (CreateMessageParams::new(Vec::new()), ProgressToken::String(...)) — no Default reliance"
key-files:
  created:
    - src/shared/peer.rs
    - src/server/peer_impl.rs
    - tests/handler_peer_integration.rs
  modified:
    - src/shared/mod.rs
    - src/lib.rs
    - src/server/cancellation.rs
    - src/server/mod.rs
    - src/server/core.rs
decisions:
  - "Made DispatchPeerHandle::new + struct pub (not pub(crate)) — same pattern Plan 02 used for ServerRequestDispatcher. The enclosing peer_impl module stays pub(crate) so discoverability is limited to the already-#[doc(hidden)] __test_support re-export. This is required for the integration tests under tests/ to link (integration tests compile as separate crates that cannot see pub(crate) items through the pub-use in lib.rs)."
  - "Added #[derive(Debug)] on DispatchPeerHandle — missing_debug_implementations lint fires at the crate root. The derive is sufficient because the single Arc<ServerRequestDispatcher> field has a custom Debug that already redacts pending correlation ids (Plan 02 Task 1 Step 3 decision)."
  - "progress_notify returns Ok(()) silently for this phase rather than plumbing notification_tx through DispatchPeerHandle. The dispatcher is for request/response RPCs; notifications are one-way and use a separate channel. Future follow-on can thread notification_tx into DispatchPeerHandle::new(..) if live progress reporting becomes phase-goal. The Ok(()) contract matches RequestHandlerExtra::report_progress's existing no-op-when-absent behavior."
  - "Used #[allow(unused_mut)] on 5 of the 9 dispatch sites instead of conditionally binding extra as mut. On wasm32, the entire peer-wiring block cfg's away, leaving `let mut extra = ...` with no mutation — the lint fires. Conditional binding with cfg_if was considered but rejected as more invasive than a one-line allow attribute at the binding site."
  - "Doctest on PeerHandle trait was rewritten to take an `Arc<dyn PeerHandle>` directly rather than going through `extra.peer()`. The `extra.peer()` accessor only exists after Task 2 lands, and Task 1 committed atomically before Task 2 — the Task 1 doctest must compile in isolation."
  - "Site ordering in src/server/mod.rs vs. enumerated line numbers in 70-RESEARCH.md shifted by ~50 lines due to Plan 02's edits above handle_call_tool (drain task spawn + Response arm rewrite). Semantic identity preserved — the 5 sites are the 5 handle_XXX methods in the legacy Server dispatch path (call_tool, get_prompt, list_resources, read_resource, create_message)."
metrics:
  duration: ~30m (wall clock, inclusive of 3 per-task quality gates)
  completed: 2026-04-16
  tasks: 3
  commits: 3
  insertions: ~513
  deletions: ~10
---

# Phase 70 Plan 03: Peer back-channel wiring — Summary

**One-liner:** Added `pmcp::PeerHandle` trait (object-safe, non-wasm-gated) with `sample` / `list_roots` / `progress_notify` methods; shipped `DispatchPeerHandle` that delegates to Plan 02's `ServerRequestDispatcher` (single correlation authority, no per-site mpsc construction); extended `RequestHandlerExtra` with `pub peer: Option<Arc<dyn PeerHandle>>` field + `with_peer()` builder + `peer()` accessor; wired conditional `.with_peer(...)` at all 9 ServerCore + legacy Server dispatch sites with graceful None-fallback so every pre-existing test continues passing.

## Outcome

All three tasks in `70-03-PLAN.md` executed:

- **Task 1** (commit `85d1b87d`) — Created `src/shared/peer.rs` with the `PeerHandle` trait: object-safe, `Send + Sync`, `#[async_trait]`, three methods (`sample`, `list_roots`, `progress_notify`). Module is `#![cfg(not(target_arch = "wasm32"))]`-gated at the file level. Registered via `pub mod peer;` in `src/shared/mod.rs` under the same cfg gate. Re-exported as `pmcp::PeerHandle` at the crate root in `src/lib.rs`. Comprehensive rustdoc covers session isolation (per-Server dispatcher), authorization inheritance (tool-level authz runs before peer wiring), and the progress_notify best-effort contract. Doctest uses `Arc<dyn PeerHandle>` directly (Task 2's `extra.peer()` accessor does not yet exist at Task 1 commit boundary).

- **Task 2** (commit `75d9b4d4`) — Created `src/server/peer_impl.rs` with `DispatchPeerHandle`: a thin wrapper around `Arc<ServerRequestDispatcher>`. `sample` delegates via `dispatcher.dispatch(ServerRequest::CreateMessage(Box::new(params)))`, then `serde_json::from_value::<CreateMessageResult>(...)`. `list_roots` dispatches `ServerRequest::ListRoots` and deserializes into `ListRootsResult`. `progress_notify` returns `Ok(())` silently (phase-level no-op). Also extended `RequestHandlerExtra`: added `#[cfg(not(target_arch = "wasm32"))] pub peer: Option<Arc<dyn PeerHandle>>` field, `with_peer()` builder, `peer()` accessor, Debug impl placeholder (`"Arc<dyn PeerHandle>"` — no inner state leak). Default/`::new()` initialize `peer: None`. Appended `DispatchPeerHandle` to the existing `#[doc(hidden)] __test_support` module in `src/lib.rs`. Three unit tests: trait-shape smoke, progress_notify Ok(()) contract, sample timeout propagation.

- **Task 3** (commit `93439ad6`) — Wired conditional `.with_peer(...)` at all 9 dispatch sites: 4 in `src/server/core.rs` (handle_call_tool, handle_get_prompt, handle_list_resources, handle_read_resource) and 5 in `src/server/mod.rs` (legacy Server path: call_tool, get_prompt, list_resources, read_resource, create_message). Each site uses the pattern `if let Some(dispatcher) = self.server_request_dispatcher.as_ref() { extra = extra.with_peer(Arc::new(DispatchPeerHandle::new(dispatcher.clone())) as Arc<dyn PeerHandle>); }` inside a `#[cfg(not(target_arch = "wasm32"))]` block. Created `tests/handler_peer_integration.rs` with 2 round-trip tests proving the full `peer.sample()` → `dispatcher.dispatch()` → outbound channel → `handle_response()` → typed-result path.

Zero new `[dependencies]` entries. Zero new feature flags. `ElicitationManager` byte-for-byte unchanged (plan constraint).

## Verification Results

| Check | Status | Notes |
|-------|--------|-------|
| `cargo build --features "full"` | pass | 0 compile errors |
| `cargo check --target wasm32-unknown-unknown --features schema-generation` | pass | `peer` field + `DispatchPeerHandle` + `PeerHandle` trait all cfg-gated away on wasm32 |
| `cargo test --lib server::peer_impl::tests --features "full" -- --test-threads=1` | pass — 3/3 | test_peer_handle_trait_shape, test_peer_progress_notify_always_ok, test_peer_sample_propagates_dispatcher_timeout |
| `cargo test --lib --features "full" -- --test-threads=1` | pass — 1022/1022 | No regressions (1019 baseline Plan 02 + 3 new peer_impl tests) |
| `cargo test --test handler_peer_integration --features "full" -- --test-threads=1` | pass — 2/2 | test_peer_sample_round_trip_through_dispatcher, test_peer_list_roots_round_trip_through_dispatcher |
| `cargo test --test handler_extensions_properties --features "full" -- --test-threads=1` | pass — 5/5 | Plan 01 regression — extensions typemap proptests still green |
| `cargo test --test server_request_dispatcher_integration --features "full" -- --test-threads=1` | pass — 2/2 | Plan 02 regression — dispatcher integration tests still green |
| `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --features "full"` | pass | Trait + struct + builder/accessor rustdoc compiles clean |
| `make quality-gate` | pass | fmt + clippy pedantic + nursery + build + test + audit all green |

### Acceptance-criteria grep checks

- `grep -c 'pub trait PeerHandle' src/shared/peer.rs` → 1
- `grep -c 'async fn sample' src/shared/peer.rs` → 1
- `grep -c 'async fn list_roots' src/shared/peer.rs` → 1
- `grep -c 'async fn progress_notify' src/shared/peer.rs` → 1
- `grep -n 'pub mod peer' src/shared/mod.rs` → match (cfg-gated)
- `grep -n 'pub use shared::peer::PeerHandle' src/lib.rs` → match (cfg-gated)
- `grep -c 'impl PeerHandle for DispatchPeerHandle' src/server/peer_impl.rs` → 1
- `grep -c 'self.dispatcher.dispatch(' src/server/peer_impl.rs` → 2 (sample + list_roots)
- `grep -c 'pub peer: Option<Arc<dyn' src/server/cancellation.rs` → 1
- `grep -c 'pub fn with_peer' src/server/cancellation.rs` → 1
- `grep -c 'pub fn peer(&self)' src/server/cancellation.rs` → 1
- `grep -c 'CreateMessageParams::new(' src/server/peer_impl.rs` → 1 (REAL constructor)
- `grep -c 'CreateMessageParams::default\|CreateMessageResult::default' src/server/peer_impl.rs` → 0 (no reliance on nonexistent impls)
- `grep -c '\.with_peer(' src/server/core.rs` → 4
- `grep -c '\.with_peer(' src/server/mod.rs` → 5
- `grep -c 'DispatchPeerHandle::new(' src/server/core.rs src/server/mod.rs` → 9
- `grep -c 'server_request_dispatcher.as_ref' src/server/core.rs src/server/mod.rs` → 9 (conditional wiring at every site)
- `grep -c 'RequestHandlerExtra {' src/server/workflow/prompt_handler.rs` → 0 (Plan 01 refactor intact — no regression)

## Deviations from Plan

### Rule 3 — Blocking issue (auto-fixed)

**1. [Rule 3 - Blocking] Task 1 doctest referenced `extra.peer()` before Task 2 landed the accessor**

- **Found during:** Task 1 `make quality-gate` doctest phase
- **Issue:** The plan's Task 1 rustdoc example used `if let Some(peer) = extra.peer() { ... }`, but the `peer()` accessor only exists after Task 2 adds the field + builder/accessor methods on `RequestHandlerExtra`. Since each task commits atomically with its own quality-gate green, the Task 1 commit boundary cannot reference Task-2-only APIs.
- **Fix:** Rewrote the doctest to take an `Arc<dyn PeerHandle>` parameter directly and call `.list_roots()` on it. Semantically identical demonstration; compiles at Task 1 commit boundary.
- **Files modified:** `src/shared/peer.rs`
- **Commit:** `85d1b87d`

**2. [Rule 3 - Blocking] `DispatchPeerHandle::new` + struct must be `pub` (not `pub(crate)`) for integration tests to link**

- **Found during:** Task 2 quality-gate clippy phase (actually caught in Task 3 when writing the integration test)
- **Issue:** The plan specified `pub(crate) struct DispatchPeerHandle` with `pub(crate) fn new(...)`. But `tests/handler_peer_integration.rs` compiles as a separate crate that links against the library's public surface — `pub use` cannot re-export `pub(crate)` items (E0364: "only public within the crate, and cannot be re-exported outside"). Same architectural constraint Plan 02 hit with `ServerRequestDispatcher`.
- **Fix:** Made `DispatchPeerHandle` and its `::new` function `pub`. Kept the enclosing `peer_impl` module `pub(crate)` so discoverability from docs is limited. The `#[doc(hidden)] __test_support` re-export explicitly documents that this is not stable API.
- **Files modified:** `src/server/peer_impl.rs`
- **Commit:** `75d9b4d4`

**3. [Rule 3 - Blocking] `#[derive(Debug)]` required on `DispatchPeerHandle` for `missing_debug_implementations` lint**

- **Found during:** Task 2 `cargo build` after the pub-visibility fix
- **Issue:** Crate-root `missing_debug_implementations` lint fires on any public struct without a Debug impl. DispatchPeerHandle's single field (Arc<ServerRequestDispatcher>) has a custom Debug that already redacts sensitive state (Plan 02 decision), so deriving Debug is safe.
- **Fix:** Added `#[derive(Debug)]` attribute on the struct.
- **Files modified:** `src/server/peer_impl.rs`
- **Commit:** `75d9b4d4`

**4. [Rule 3 - Blocking] Module declaration order in `src/server/mod.rs` must match cargo fmt conventions**

- **Found during:** Task 2 `make quality-gate` fmt phase
- **Issue:** Placed `pub(crate) mod peer_impl;` after `pub(crate) mod server_request_dispatcher;` (logical Phase-70 grouping), but `cargo fmt --all -- --check` reorders modules alphabetically by leading doc comment and enforces consistent positioning.
- **Fix:** Ran `cargo fmt --all` to let the formatter place the module in its canonical alphabetical position (`peer_impl` before `preset` and before `progress`).
- **Files modified:** `src/server/mod.rs`
- **Commit:** `75d9b4d4`

**5. [Rule 3 - Blocking] `#[allow(unused_mut)]` required on 5 of 9 dispatch sites for wasm32 compile**

- **Found during:** Task 3 `cargo check --target wasm32-unknown-unknown`
- **Issue:** Several dispatch sites had immutable `let extra = RequestHandlerExtra::new(...)`. After adding the conditional `if let Some(dispatcher) = ... { extra = extra.with_peer(...) }` — which requires `mut` — the resulting `let mut extra = ...` fires `unused_mut` on wasm32 (where the entire peer-wiring block is cfg'd away).
- **Fix:** Added `#[allow(unused_mut)]` attribute immediately before each `let mut extra = ...` binding at the 5 sites where the original binding was immutable (2 in core.rs, 3 in mod.rs — the other 4 sites were already `mut extra` for unrelated reasons).
- **Files modified:** `src/server/core.rs`, `src/server/mod.rs`
- **Commit:** `93439ad6`

**6. [Rule 3 - Blocking] `clippy::doc_markdown` fired on `DispatchPeerHandle`, `list_roots`, `handle_response`, `ServerCore` references without backticks**

- **Found during:** Task 2 + Task 3 clippy phase
- **Issue:** `clippy::doc_markdown` (pedantic lint) requires all code-like identifiers (camelCase + snake_case symbols) to be surrounded by backticks in doc comments. Caught 2 sites in `src/server/peer_impl.rs` and 3 sites in `tests/handler_peer_integration.rs`.
- **Fix:** Wrapped bare identifiers in backticks: `DispatchPeerHandle` → `` `DispatchPeerHandle` ``, `list_roots` → `` `list_roots` ``, `handle_response` → `` `handle_response` ``, `ServerCore` → `` `ServerCore` ``, `peer.sample()` → `` `peer.sample()` ``.
- **Files modified:** `src/server/peer_impl.rs`, `tests/handler_peer_integration.rs`
- **Commits:** `75d9b4d4`, `93439ad6`

### No Rule 2 auto-added critical functionality

The plan's `<threat_model>` block enumerated 5 threats with full mitigation plans that were implemented as specified. No additional security or correctness gaps surfaced during execution.

## Known Limitations

- **`progress_notify` is a phase-level no-op.** Always returns `Ok(())` regardless of whether a client is connected. Matches the existing `RequestHandlerExtra::report_progress` contract (which is a no-op when `progress_reporter` is absent). Future follow-on can plumb `notification_tx` through `DispatchPeerHandle::new(..)` to enable live progress reporting — at that point the peer would hold `Option<Sender<Notification>>` alongside the `Arc<ServerRequestDispatcher>`.

- **`session_id` remains `None` at dispatch time.** `RequestHandlerExtra.session_id` is Option<String> but is never populated by the 9 dispatch sites — the current `ProtocolHandler::handle_request` signature does not thread it. This is documented as a known limitation rather than a silent drop. Session isolation is ENFORCED by architecture: each `Server` (and therefore each dispatcher) is per-process-per-transport, so cross-session confusion requires cross-process access which is out of threat model. A future phase can widen the handler signature if rmcp parity for session-scoped peer routing becomes a phase goal.

- **`ElicitationManager` keeps its own unwired `request_tx` channel.** Explicit plan constraint (Codex Finding 2 carried over from Plan 02). The new dispatcher and ElicitationManager coexist without interference. Migrating ElicitationManager to use the shared dispatcher is plausible future cleanup; explicitly out of scope for Plan 03.

- **Peer is fresh-per-request, not cached.** Each dispatch site constructs a new `DispatchPeerHandle` per handler invocation. Cost is near-zero — the struct wraps a single `Arc` clone — but the plan explicitly calls out not caching handles across requests (Pitfall #2). If profiling ever shows allocation pressure here, caching on the dispatcher itself (not the handle) would be the correct path.

## Threat Flags

None. All threat-register dispositions from the plan's `<threat_model>` block were honored:

- **T-70-02 (Tampering — session isolation)** mitigated: per-Server dispatcher ownership enforces routing isolation structurally. `test_peer_sample_round_trip_through_dispatcher` proves the peer handle routes to the dispatcher it was constructed with.
- **T-70-04 (DoS — malformed response)** mitigated: `serde_json::from_value::<CreateMessageResult>(value)` converts errors to `Error::protocol(INTERNAL_ERROR, "Invalid sample response: {e}")` rather than panicking. Plan 04 will add a dedicated fuzz target exercising this boundary.
- **T-70-05 (DoS — timeout)** mitigated: timeout is owned by the dispatcher (60s default); `test_peer_sample_propagates_dispatcher_timeout` unit test asserts a short-timeout peer returns Err within 500ms.
- **T-70-07 (Info Disclosure — peer in Debug)** mitigated: `RequestHandlerExtra` Debug impl emits `.field("peer", &"Arc<dyn PeerHandle>")` placeholder — does not expose inner dispatcher, pending map, or transport state. Plan 01's metadata redaction test remains green (no regression).
- **T-70-08 (Elevation of Privilege — peer inherits caller scope)** accepted and documented in `src/shared/peer.rs` rustdoc. Tool-level authz at `src/server/core.rs:431-440` runs BEFORE the dispatch site wires peer — an unauthorized caller never reaches the handler body.
- **T-70-17 (DoS — shared dispatcher)** accepted: bounded `mpsc::channel(100)` is a deliberate backpressure signal. Per-session dispatchers are explicitly future work.

## TDD Gate Compliance

Tasks 1 and 2 were marked `tdd="true"` in the plan but executed as **green-path-first** commits (the unit tests and structural changes landed in a single commit each) rather than the strict RED → GREEN → REFACTOR sequence. Rationale: each commit boundary must pass `make quality-gate` per CLAUDE.md zero-tolerance gate, and a RED commit (test exists, code absent) cannot compile the library — so the strict RED/GREEN split is structurally infeasible under pre-commit hooks here. The resulting test coverage is equivalent; what is missing is the "saw the test fail first" audit trail in git. This mirrors the Plan 01 and Plan 02 outcomes and matches how the existing pmcp test suite has historically been built out.

## Downstream Dependencies

- **Plan 04** adds examples (`s42_handler_extensions.rs`, `s43_handler_peer_sample.rs`), a fuzz target (`fuzz_peer_handle.rs`), migration prose with explicit `# Semver posture` + `# Known limitation: session-id plumbing` sections, and the final `make quality-gate` smoke. No Plan 03 surface changes required — the PeerHandle trait + DispatchPeerHandle + RequestHandlerExtra.peer field are all publicly reachable via `pmcp::PeerHandle` + `pmcp::__test_support::DispatchPeerHandle`.

## Self-Check: PASSED

- File created: `src/shared/peer.rs` (verified: contains `pub trait PeerHandle`, 3 async methods, `#![cfg(not(target_arch = "wasm32"))]` at module top)
- File created: `src/server/peer_impl.rs` (verified: contains `impl PeerHandle for DispatchPeerHandle`, 2 `dispatcher.dispatch(` delegations, 3 unit tests, `CreateMessageParams::new(Vec::new())` REAL constructor)
- File created: `tests/handler_peer_integration.rs` (verified: 2 `#[tokio::test]` functions, uses `pmcp::__test_support` re-exports, REAL `CreateMessageParams::new(Vec::new())`)
- File modified: `src/shared/mod.rs` (verified: `pub mod peer;` cfg-gated)
- File modified: `src/lib.rs` (verified: `pub use shared::peer::PeerHandle;` cfg-gated + `DispatchPeerHandle` in `__test_support`)
- File modified: `src/server/cancellation.rs` (verified: `pub peer: Option<Arc<dyn crate::shared::peer::PeerHandle>>`, `with_peer` builder, `peer()` accessor, Default + ::new() + Debug impls all updated)
- File modified: `src/server/core.rs` (verified: 4 `.with_peer(` + 4 `DispatchPeerHandle::new(` occurrences, all cfg-gated conditional-on-dispatcher)
- File modified: `src/server/mod.rs` (verified: 5 `.with_peer(` + 5 `DispatchPeerHandle::new(` occurrences, all cfg-gated conditional-on-dispatcher)
- Commit exists: `85d1b87d feat(70-03): add PeerHandle trait for server-to-client RPCs from handlers`
- Commit exists: `75d9b4d4 feat(70-03): add DispatchPeerHandle + peer field on RequestHandlerExtra`
- Commit exists: `93439ad6 feat(70-03): wire conditional .with_peer at 9 dispatch sites + round-trip tests`
- `make quality-gate` green on final state
- 1022 lib tests + 2 new integration tests + 2 Plan 02 integration tests + 5 Plan 01 proptests all pass
- WASM compile check green (`cargo check --target wasm32-unknown-unknown --features schema-generation`)
- `ElicitationManager` code is byte-for-byte unchanged (plan constraint)
