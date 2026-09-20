---
phase: 108-pmcp-agent-loop-crate
plan: 01
subsystem: api
tags: [transport, server, sampling, tool_use, tokio, cancel-safety, mcp-2025-11-25]

# Dependency graph
requires:
  - phase: 106-client-host-surface
    provides: client host surface (on_sampling/on_roots/on_elicitation, ClientHostRegistry, dispatch_host_sampling) that the new WithTools path extends and the D-106-A deferred deadlock this plan fixes
provides:
  - "Server::run Transport Actor: single-owner transport, never-block receive/drain, unbounded worker queue — in-tool peer.sample()/.list_roots() no longer deadlock"
  - "StdioTransport cancel-safety (persistent partial-line buffer) + Transport::receive # Cancellation contract"
  - "End-to-end WithTools sampling: HostSamplingHandlerWithTools + on_sampling_with_tools + LegacyHostSamplingAdapter"
  - "PeerHandle::sample_with_tools (additive default) + DispatchPeerHandle override with legacy decode fallback"
  - "CreateMessageResultWithTools::from_single / SamplingMessageContent::from_content canonical conversions"
  - "pmcp 2.17.0"
affects: [108-agent-loop-crate, plan-108-04-SamplingSource, AGNT-04]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Transport Actor: one task owns the transport (T, not Arc<RwLock<T>>) and select!s receive vs an unbounded send_tx; the real send runs AFTER the select! block so the receive future's borrow releases first (single &mut self transport, borrow-checker-safe, cancel-safe)"
    - "Cancel-safe line reads via read_until into a persistent Vec<u8> (not read_line's internal scratch)"
    - "Additive trait evolution via default method + concrete override (PeerHandle::sample_with_tools)"

key-files:
  created:
    - tests/in_tool_peer_roundtrip.rs
  modified:
    - src/server/mod.rs
    - src/server/server_request_dispatcher.rs
    - src/shared/transport.rs
    - src/shared/stdio.rs
    - src/shared/peer.rs
    - src/server/peer_impl.rs
    - src/client/host/sampling.rs
    - src/client/host/mod.rs
    - src/client/mod.rs
    - src/types/sampling.rs
    - Cargo.toml
    - CHANGELOG.md
    - cargo-pmcp/src/templates/workbook_server.rs

key-decisions:
  - "Actor sends AFTER the select! block (capture frame into a local, send once the receive future is dropped) — the compiling, cancel-safe idiom for a single non-splittable &mut self transport"
  - "StdioTransport uses read_until into a persistent Vec<u8> because tokio's read_line buffers into an internal scratch that a dropped future loses"
  - "dispatch_host_sampling keeps the legacy single-content wire path byte-identical (does NOT adapt legacy->WithTools in the dispatch output); the WithTools path is only taken when a WithTools handler is registered. The peer-side sample_with_tools legacy fallback covers the mixed case. This preserves the existing sample()/on_sampling roundtrip"
  - "WithTools result-review gate runs against a single-content text projection of the completion (tool blocks rendered as markers) so the reviewer is never silently bypassed; the preflight wallet gate runs identically for both paths"
  - "Added a new [2.17.0] CHANGELOG section above the still-unreleased 2.16.0 rather than folding, to clearly document Phase 108 work"

patterns-established:
  - "Transport Actor never-block pump for Server::run"
  - "Persistent-buffer cancel-safe transport reads"

requirements-completed: [AGNT-04]

# Metrics
duration: ~110min
completed: 2026-07-18
---

# Phase 108 Plan 01: pmcp 2.17.0 Transport Actor + End-to-End WithTools Sampling Summary

**Never-block Transport Actor in `Server::run` (in-tool `peer.sample()`/`.list_roots()` no longer deadlock) plus an additive end-to-end WithTools sampling path (`PeerHandle::sample_with_tools` + `on_sampling_with_tools`), shipped as pmcp 2.17.0.**

## Performance

- **Duration:** ~110 min
- **Completed:** 2026-07-18
- **Tasks:** 3
- **Files modified:** 12 (+1 created)

## Accomplishments
- Refactored `Server::run` to a single-owner **Transport Actor**: the transport is moved into one task (no shared `Arc<RwLock<T>>`), inbound `Response`s route to the dispatcher immediately, inbound `Request`s go to one sequential worker via an **unbounded** queue, and all outbound frames funnel through one `send_tx`. Closes both reviewer-identified failure modes (bounded-channel re-deadlock + RwLock write-guard starvation).
- Made `StdioTransport` cancel-safe with a persistent partial-line buffer and added a `# Cancellation` contract to `Transport::receive` — required because the actor's `select!` drops the in-flight `receive()` when a queued send wins.
- Added the end-to-end WithTools sampling surface: `HostSamplingHandlerWithTools`, `LegacyHostSamplingAdapter`, `ClientBuilder::on_sampling_with_tools`, and `PeerHandle::sample_with_tools` (additive default) with a legacy single-content decode fallback on `DispatchPeerHandle`.
- Proved it all on the **stock** `Server::run` + `Client`: sampling, list_roots, request saturation, clean shutdown, and a WithTools ToolUse block surviving end-to-end (AGNT-04).
- Bumped pmcp to 2.17.0 and kept the cargo-pmcp scaffold-pin drift guard green.

## Task Commits

1. **Task 1: Transport Actor pump + stdio cancel-safety** - `6d30621f` (feat)
2. **Task 2: End-to-end WithTools client-host + peer surface** - `0b5b461f` (feat)
3. **Task 3: WithTools real-loop proof + 2.17.0 + scaffold-pin** - `9bd99f38` (feat)

_TDD note: Tasks 2 and 3 were marked `tdd="true"`. Because both add compile-coupled trait/API surface (a test cannot reference `sample_with_tools` / `on_sampling_with_tools` before they exist), strict RED-first commits were not separable; each landed as a single GREEN `feat` commit that includes its unit/integration tests. All new behavior is test-covered (adapter lift, WithTools decode, legacy fallback, end-to-end ToolUse)._

## Files Created/Modified
- `src/server/mod.rs` - `run()` rewritten to spawn the Transport Actor + single request worker; `run_transport_actor`, `route_inbound_message`, `route_response`, `route_notification`, `spawn_request_worker`; `run_main_loop` now joins the actor (shutdown).
- `src/server/server_request_dispatcher.rs` - `spawn_server_request_drain` forwards onto the actor's unbounded `send_tx` instead of locking the transport.
- `src/shared/transport.rs` - `# Cancellation` contract on `receive`.
- `src/shared/stdio.rs` - persistent `partial: Mutex<Vec<u8>>`; `read_cancel_safe_line` (read_until into the persistent buffer); drop-mid-read + EOF tests.
- `src/shared/peer.rs` - additive `PeerHandle::sample_with_tools` default method.
- `src/server/peer_impl.rs` - `DispatchPeerHandle::sample_with_tools` override (WithTools decode, legacy CreateMessageResult fallback) + tests.
- `src/client/host/sampling.rs` - `HostSamplingHandlerWithTools`, `LegacyHostSamplingAdapter`, lift helpers + tests.
- `src/client/host/mod.rs` - registry `sampling_with_tools` field + exports.
- `src/client/mod.rs` - `on_sampling_with_tools`; `dispatch_host_sampling` prefers the WithTools handler; `answer_sampling_legacy` / `answer_sampling_with_tools` / `project_with_tools_for_review`.
- `src/types/sampling.rs` - `CreateMessageResultWithTools::from_single`, `SamplingMessageContent::from_content`.
- `tests/in_tool_peer_roundtrip.rs` - real-loop proofs (sampling, list_roots, saturation, shutdown, WithTools).
- `Cargo.toml` - pmcp 2.16.0 → 2.17.0.
- `CHANGELOG.md` - [2.17.0] entry.
- `cargo-pmcp/src/templates/workbook_server.rs` - `PMCP_VERSION` → 2.17.0.

## Decisions Made
See `key-decisions` frontmatter. The load-bearing ones:
- The actor performs the real `transport.send()` **after** the `select!` block (frame captured into a local inside the send arm), which is the compiling, cancel-safe idiom for one non-splittable `&mut self` transport.
- `StdioTransport` uses `read_until` into a **persistent** `Vec<u8>` because tokio's `read_line` buffers into the future's internal scratch (lost on drop) — verified by a failing-then-passing drop-mid-read test.
- WithTools dispatch preserves the legacy wire path exactly; the peer-side legacy decode fallback covers a WithTools-caller / legacy-host mix.

## Deviations from Plan

### Auto-fixed / design-resolved Issues

**1. [Rule 1 - Correctness] `dispatch_host_sampling` does NOT adapt legacy→WithTools in its output**
- **Found during:** Task 2
- **Issue:** The plan text said "prefer the WithTools handler, else adapt the legacy handler via the adapter." Emitting a `CreateMessageResultWithTools` for a legacy-only client would change the wire shape (array `content`, added `role`) and break the existing `client_host_roundtrip::sampling_answered_by_host_handler` test, which decodes a single-content `CreateMessageResult` — violating the plan's own "existing on_sampling path remains unchanged" invariant.
- **Fix:** Kept the legacy branch byte-identical; the WithTools branch is taken only when a WithTools handler is registered. The mixed case (server calls `sample_with_tools`, client answers legacy) is handled by the peer-side legacy decode fallback in `DispatchPeerHandle::sample_with_tools`. The `LegacyHostSamplingAdapter` is still provided, exported, and unit-tested (it is the mechanism the peer-side fallback mirrors).
- **Verification:** `client_host_roundtrip`, `client_host_approval`, `handler_peer_integration` (9 tests) still pass; 908 lib tests pass.
- **Committed in:** `0b5b461f`

**2. [Rule 2 - Correctness] Result-review gate for the WithTools path**
- **Found during:** Task 2
- **Issue:** The optional `on_sampling_result_review` callback takes a single-content `CreateMessageResult`, which a `CreateMessageResultWithTools` cannot losslessly become.
- **Fix:** The WithTools path projects the completion to a single text `CreateMessageResult` (tool blocks rendered as `[tool_use ...]` markers) and runs the reviewer against that, so the gate is never silently bypassed. The preflight wallet gate is unchanged (operates on params) for both paths.
- **Committed in:** `0b5b461f`

**3. [Rule 1 - Robustness] Inbound-notification cancellation error no longer tears down the loop**
- **Found during:** Task 1
- **Issue:** The old `handle_transport_message` propagated a cancellation-processing error, which broke the entire receive loop.
- **Fix:** `route_notification` logs the error and continues; a failed cancellation lookup should not kill the server.
- **Committed in:** `6d30621f`

---

**Total deviations:** 3 (1 correctness-preserving reinterpretation, 1 gate design, 1 robustness). **Impact:** No scope creep; all three keep the changes strictly additive and preserve existing behavior. The Task 1 reinterpretation is the reason the "existing lib tests unchanged" criterion holds.

## Issues Encountered
- First cancel-safety attempt used `read_line` into a persistent `String` and FAILED (partial bytes were lost because `read_line`'s future keeps its own scratch `Vec`). Switched to `read_until` into a persistent `Vec<u8>`; the drop-mid-read test then passed. Documented on the `partial` field.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The D-03 real-loop proof is green: the hosted agent flow (plan 108-04 `SamplingSource`, AGNT-04) can now rely on in-tool `sample_with_tools` completing on the stock `Server::run` and carrying `ToolUse` blocks.
- pmcp is 2.17.0; the scaffold-pin tracks it. Downstream release version-bump cascade (mcp-tester/cargo-pmcp pins) is a release-time concern, intentionally out of this plan's scope.

---
*Phase: 108-pmcp-agent-loop-crate*
*Completed: 2026-07-18*

## Self-Check: PASSED

- Created files verified present: `tests/in_tool_peer_roundtrip.rs`, `108-01-SUMMARY.md`
- All task commits verified in git log: 6d30621f, 0b5b461f, 9bd99f38, 7d86b07e
- Acceptance greps verified: `Arc::new(RwLock::new(transport))` count 0, `unbounded_channel` present, stdio `partial` present, Cargo.toml 2.17.0, scaffold-pin "2.17.0"
