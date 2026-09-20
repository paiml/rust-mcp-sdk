---
phase: 106-client-host-surface
plan: 01
subsystem: api
tags: [mcp, sampling, elicitation, roots, client-host, wasm, async-trait, proptest]

# Dependency graph
requires:
  - phase: 104-task-augmented-tool-results
    provides: tests/common/duplex.rs duplex transport harness convention
provides:
  - "pmcp::client::host module: HostSamplingHandler, HostElicitationHandler traits + RootsProvider closure alias"
  - "ClientHostRegistry + generic ClientBuilder registration methods (on_sampling/on_elicitation/on_roots/on_sampling_approval/on_sampling_result_review)"
  - "Client dispatch of inbound server->client requests (sampling/elicitation/roots): answer or -32601, sanitized -32603 on handler error, connection kept alive"
  - "Target-agnostic src/types/roots.rs (Root + ListRootsResult) — host module compiles on wasm32; server::roots keeps a back-compat re-export"
  - "Preflight + result-review sampling approval TYPES (invocation deferred to plan 02)"
  - "HOST-06 rustdoc: create_message named the LLM-server pattern, cross-linked to the real pmcp::SamplingHandler + client::host::HostSamplingHandler paths"
affects: [108-sampling-source, 111-sampling-hosting-docs, pmcp-agent, pmcp.run-durable-host]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure sync classify_host_request + HostRequestKind for fuzzable dispatch routing"
    - "Raw duplex pump to test/demo client host answering (server side hand-rolled)"
    - "Generic closure builder methods boxing into Arc<dyn Fn ... BoxFuture> aliases"

key-files:
  created:
    - src/types/roots.rs
    - src/client/host/mod.rs
    - src/client/host/sampling.rs
    - src/client/host/elicitation.rs
    - src/client/host/roots.rs
    - tests/client_host_roundtrip.rs
    - examples/s49_sampling_host.rs
  modified:
    - src/types/mod.rs
    - src/server/roots.rs
    - src/client/mod.rs
    - Cargo.toml
    - examples/README.md

key-decisions:
  - "Approval params are OWNED (CreateMessageParams), not &-borrowed, so they move into a 'static future — the reviewed &-signature is a compile-time lifetime violation"
  - "RootsProvider future returns Result<ListRootsResult> so provider failure maps to -32603 like the other handlers"
  - "All three inbound round-trips (incl. sampling+roots) are tested via a RAW duplex pump, not Server::run+PeerHandle — the high-level server loop deadlocks a peer.sample() during a tool call (pre-existing, flagged for Phase 108)"
  - "Injected inbound sampling as the Request::Client(CreateMessage) alias — the exact shape a real transport's parse_request yields (parse ambiguity)"

patterns-established:
  - "Pattern: classify_host_request pure fn maps both sampling parse variants + server elicitation/roots to a HostRequestKind"
  - "Pattern: dispatch_host_request delegates per-kind to small helpers to stay under cognitive-complexity 25"

requirements-completed: [HOST-01, HOST-02, HOST-03, HOST-06]

# Metrics
duration: 37min
completed: 2026-07-17
---

# Phase 106 Plan 01: Client Host Surface Summary

**A pmcp Client now answers inbound server->client sampling/elicitation/roots requests from a registered `client::host` handler registry (relocated wasm-clean roots types, generic builder registration, -32601/sanitized-32603 dispatch), replacing the "Unexpected message type" error and documenting the legacy inverted path as the LLM-server pattern.**

## Performance

- **Duration:** ~37 min
- **Started:** 2026-07-17T21:51Z
- **Completed:** 2026-07-17T22:28Z
- **Tasks:** 3
- **Files created:** 7 (+1 deferred-items log)  **Files modified:** 5

## Accomplishments
- New `pmcp::client::host` module: object-safe `HostSamplingHandler` / `HostElicitationHandler` traits, a `Result`-returning `RootsProvider` closure alias, `ClientHostRegistry`, and the preflight/result-review approval TYPES (owned params) — all cfg-agnostic and compiling on `wasm32-unknown-unknown`.
- Relocated `Root` + `ListRootsResult` to target-agnostic `src/types/roots.rs`; `server::roots` keeps a back-compat re-export so `pmcp::server::roots::ListRootsResult` still resolves.
- `Client` gained a `host_registry` on every constructor + `Clone`; `ClientBuilder` gained five generic closure registration methods; the `:2234` error arm now routes inbound requests through `dispatch_host_request` (answer, or `-32601` for known-unhandled, `-32603` sanitized for handler errors) and continues the receive loop.
- Duplex round-trips prove sampling (HOST-01), roots (HOST-03), and elicitation (HOST-02) are answered by the registered handlers, plus a `prop_sampling_passthrough` proptest (tools/tool_choice/tool_use/tool_result pass through unchanged). Runnable `s49_sampling_host` example registered.
- HOST-06 rustdoc: `create_message` named the "LLM-server pattern" and cross-linked to the real `pmcp::SamplingHandler` + `pmcp::client::host::HostSamplingHandler` paths (the dead `server::traits::SamplingHandler` path is never referenced).

## Task Commits

1. **Task 1: Relocate roots types + wasm-clean client::host module** - `d8985918` (feat)
2. **Task 2: Wire Client dispatch + generic registration builders** - `6d0d0bdb` (feat)
3. **Task 3: Duplex round-trips + passthrough proptest + s49 example** - `86d4d295` (test)

**Follow-up:** `9025699d` (docs: drop redundant explicit link targets — rustdoc zero-warning)

## Files Created/Modified
- `src/types/roots.rs` - Target-agnostic `Root` + `ListRootsResult` (relocated).
- `src/types/mod.rs` - `pub mod roots;` (ungated).
- `src/server/roots.rs` - Back-compat re-export `pub use crate::types::roots::{ListRootsResult, Root}`.
- `src/client/host/{mod,sampling,elicitation,roots}.rs` - The client host surface + `classify_host_request`/`HostRequestKind`.
- `src/client/mod.rs` - `host_registry` field/Clone/Debug, builder methods, `dispatch_host_request` + helpers, receive-loop arm, `create_message` HOST-06 rustdoc, dispatch unit tests.
- `tests/client_host_roundtrip.rs` - Raw-pump round-trips (sampling/roots/elicitation) + passthrough proptest.
- `examples/s49_sampling_host.rs` - Runnable sampling-host example.
- `Cargo.toml`, `examples/README.md` - `[[example]]` registration + index entry.

## Decisions Made
- **Owned approval params:** `PreflightApproval`/`SamplingResultReview` take owned `CreateMessageParams` (the reviewed `&CreateMessageParams` cannot move into a `'static` `BoxFuture`).
- **Result-returning roots provider:** provider failures map to `-32603` uniformly.
- **Raw pump for all three round-trips:** the high-level `Server::run` cannot answer a `peer.sample()` issued during a tool call (see Deviations); the raw duplex pump exercises the exact client behavior under test.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Sampling & roots round-trips can't use `Server::run` + `PeerHandle` (deadlock) — used a raw duplex pump instead**
- **Found during:** Task 3 (duplex round-trips)
- **Issue:** The plan's must-have and acceptance criteria call for a `Server` whose tool calls `extra.peer().sample()` / `.list_roots()`, driven via `Server::run`. Empirically this hangs (>60s) and the tests FAILED. `Server::run`'s `spawn_message_handler` drives one serialized loop that `await`s each request handler inline (`handle_request_message`), so a tool blocking on `extra.peer().sample()` cannot be answered by the same client whose response that loop must read — and the default in-`run()` dispatcher has no timeout. This is a pre-existing server-side serialization limitation, not a fault in the client host surface.
- **Fix:** Drive the server side by hand over the duplex transport (answer `initialize`, push a raw inbound request while a client `tools/list` is in flight, capture the client's Response). This proves exactly the deliverable — the client answers inbound sampling/roots/elicitation from its registry — for all three requests. The `s49_sampling_host` example uses the same hand-rolled mock server. Sampling is injected as the `Request::Client(CreateMessage)` alias (what a real transport's `parse_request` yields), exercising the parse-ambiguity path end-to-end.
- **Files modified:** tests/client_host_roundtrip.rs, examples/s49_sampling_host.rs
- **Verification:** `cargo test --test client_host_roundtrip` → 4/4 pass in 0.02s; `cargo run --example s49_sampling_host` exits 0 and prints the round-trip completion.
- **Committed in:** 86d4d295 (Task 3 commit); limitation logged in `deferred-items.md` (D-106-A) and flagged for Phase 108.

**2. [Rule 3 - Blocking] `host_registry` also initialized in `with_client_options` (4th constructor)**
- **Found during:** Task 2
- **Issue:** The plan named `with_info`/`with_options`; grepping `Self {` found a third `Client` literal, `with_client_options`, which also needed the field to compile.
- **Fix:** Added `host_registry: ClientHostRegistry::default()` there too (plus `with_info`, `with_options`, and the `Clone` impl).
- **Files modified:** src/client/mod.rs
- **Verification:** `cargo build --lib` succeeds; `host_registry` appears 6× in client/mod.rs.
- **Committed in:** 6d0d0bdb (Task 2 commit)

**3. [Rule 1 - Bug] rustdoc redundant-link warnings on host builders**
- **Found during:** Post-task verification (`cargo doc`)
- **Issue:** `[`RootsProvider`](host::RootsProvider)` / `[`ApprovalDecision`](host::ApprovalDecision)` produced 3 "redundant explicit link target" warnings (the types are re-exported into client scope), which would trip the CI rustdoc zero-warning gate.
- **Fix:** Switched to the bare intra-doc link form.
- **Files modified:** src/client/mod.rs
- **Verification:** `cargo doc --no-deps --lib --features full` → only 2 pre-existing warnings remain (`crate::client::oauth`, `assert_roundtrips_through_client`), none from this plan.
- **Committed in:** 9025699d (docs follow-up)

---

**Total deviations:** 3 auto-fixed (2 blocking, 1 bug). **Impact:** No scope creep. The Server::run limitation is pre-existing and out of scope for this client-focused, additive plan; the client host surface is fully delivered and proven.

## Issues Encountered
- Initial `Server::run`-driven sampling/roots tests deadlocked (see Deviation 1). Root-caused to the server's single serialized request loop; resolved by switching to a raw duplex pump that directly validates the client behavior.
- Minor: `Transport` requires `Debug` (example duplex needed `#[derive(Debug)]`); an end-of-block `MutexGuard` temporary borrow needed hoisting to a local. Both fixed inline.

## Threat Flags
None — no new trust-boundary surface beyond the plan's `<threat_model>`. Inbound params are typed-serde-deserialized; handler errors are logged locally and returned sanitized (-32603); known-unhandled methods return -32601 without dropping the connection (T-106-01/02/10 mitigations in place, fuzz of routing deferred to plan 02 per the register).

## Next Phase Readiness
- Client host surface is complete and additive; `wasm32` lib check is clean; back-compat re-export intact.
- **Blocker for the full agent-hosting flow (Phase 108):** `Server::run` must be able to process an inbound response while a tool handler awaits a `peer.*` request (spawn per-request or the peer round-trip). Tracked in `deferred-items.md` D-106-A.
- Approval INVOCATION (preflight + result-review) and routing fuzz are deferred to plan 02, which also executes the pmcp 2.15.0 -> 2.16.0 minor bump.

## Self-Check: PASSED

- All 7 created source/test/example files exist on disk.
- All 4 commits (d8985918, 6d0d0bdb, 86d4d295, 9025699d) present in git history.
- Back-compat re-export in `src/server/roots.rs` intact; `dispatch_host_request` + `LLM-server pattern` rustdoc present; `host_registry` referenced 17× in `src/client/mod.rs`.

---
*Phase: 106-client-host-surface*
*Completed: 2026-07-17*
