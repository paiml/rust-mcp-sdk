# Phase 106: Client Host Surface - Research

**Researched:** 2026-07-17
**Domain:** MCP protocol — client-side server→client request handling (host surface) in the pmcp Rust SDK
**Confidence:** HIGH (brownfield; all claims verified against the working tree at the cited line ranges)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- A `Client` can register a **client-side sampling handler** answering incoming spec-direction `sampling/createMessage` (incl. `tools`/`tool_choice`, `tool_use`/`tool_result`) — HOST-01.
- A `Client` can register an **elicitation handler** answering `elicitation/create` — HOST-02.
- A `Client` answers **`roots/list`** from a registered roots provider — HOST-03.
- Dispatch **replaces the `client/mod.rs:2234` "Unexpected message type" error** for registered handler types; unregistered request types return a proper JSON-RPC method-not-found/capability error, NOT a connection-level failure.
- Sampling path exposes an **async approval callback slot, defaulting to allow**, invoked before returning a completion (spec SHOULD) — HOST-04. Seam only, no UI.
- `ClientCapabilities` sent on initialize **reflect which host handlers are registered** (sampling/elicitation/roots) rather than being independently assertable — HOST-05. `ClientCapabilities.sampling` comes to mean "I can host".
- Legacy inverted path (`Client::create_message` → server-side `SamplingHandler`) is **kept, not deprecated**, documented as the **"LLM-server pattern"** in rustdoc + pmcp-book — HOST-06. Zero behavioral changes.
- New client-side handler trait **must not be confusable** with the existing server-side `SamplingHandler` (`src/server/traits.rs:62`). Distinct naming or distinct module paths + explicit docs (non-confusability locked; exact naming is discretion).
- Proven by a **duplex round-trip harness test** (server requests sampling → client handler answers); reuse `tests/common/duplex.rs`.
- ALWAYS requirements: property, unit, fuzz-where-meaningful, runnable example (sampling host). `make quality-gate` green. **Additive API only** (pmcp minor bump 2.15.0 → 2.16.0).

### Claude's Discretion
- Exact trait names, module layout, builder API shape for handler registration (`ClientBuilder` methods vs setters).
- How the receive loop routes incoming `Request::Server` variants to handlers (dispatch mechanics, task spawning, ordering with in-flight response waits).
- Error taxonomy for unhandled request types (which JSON-RPC error code).
- Whether roots is a full handler trait or a simpler provider closure.
- WASM client considerations — investigate whether the wasm client path needs the same surface now or is deferred (document either way).

### Deferred Ideas (OUT OF SCOPE)
- `SamplingSource` (CompletionSource over this surface) — Phase 108.
- Full "Sampling & Hosting" book chapter + examples beyond the one sampling-host example — Phase 111.
- Any deprecation decision on the LLM-server pattern — explicitly kept.
- AgentCore/deploy concerns — none this phase.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| HOST-01 | Client registers a client-side sampling handler answering incoming `sampling/createMessage` (tools/tool_choice incl.) | Dispatch hooks in at `src/client/mod.rs:2234`; wire types already complete (`src/types/sampling.rs`, `CreateMessageParams` carries `tools`/`tool_choice`). **Parse-ambiguity gotcha** (Pitfall 1) must be handled. |
| HOST-02 | Client registers an elicitation handler answering `elicitation/create` | `ServerRequest::ElicitationCreate` (`protocol/mod.rs:518`); params in `src/types/elicitation.rs`; result `ElicitResult`. Parses unambiguously as `Request::Server`. |
| HOST-03 | Client answers `roots/list` from a registered roots provider | `ServerRequest::ListRoots` (`protocol/mod.rs:515`); result type `ListRootsResult` (`src/server/roots.rs`). Parses unambiguously. Provider closure is sufficient (discretion). |
| HOST-04 | Human-in-the-loop approval hook (async, default allow) on sampling | Add an async callback slot in the client sampling dispatch, invoked after handler produces a result / before returning. No existing seam — new. |
| HOST-05 | `ClientCapabilities` reflect registered handlers | Derive `capabilities.sampling/elicitation/roots` from the registry at build/initialize (`src/client/mod.rs:300-315`). Structs exist (`src/types/capabilities.rs`). |
| HOST-06 | Legacy inverted path documented as "LLM-server pattern", no breaking changes | `Client::create_message` (`src/client/mod.rs:1846`) + server `SamplingHandler` (`src/server/traits.rs:62`) unchanged; rustdoc + book seed only. |
</phase_requirements>

## Summary

This is a **brownfield protocol-compliance phase entirely inside the existing `pmcp` crate** — no new external dependencies, no new wire methods. Every wire type the phase needs already exists and is spec-correct: `ServerRequest::{CreateMessage, ListRoots, ElicitationCreate}` (`src/types/protocol/mod.rs:509-519`), `CreateMessageParams`/`CreateMessageResult` with `tools`/`tool_choice`/`tool_use`/`tool_result` (`src/types/sampling.rs`), `ElicitRequestParams`/`ElicitResult`, and `ListRootsResult`. The work is a client-side **handler registry + dispatch** that replaces the single error line at `src/client/mod.rs:2234`.

Two findings dominate the design and MUST be surfaced to the planner. **First: the pmcp `Client` has NO background receive loop.** The only site that ever receives a `TransportMessage::Request` is inside `send_request`'s response-wait loop (`src/client/mod.rs:2179-2242`). Consequently, server→client requests can only be answered *while a client-initiated request is in flight* (the nested sampling-during-`tools/call` flow). This is exactly the primary use case and it works cleanly, but a "pure idle host" (a client that sits waiting for sampling with no outbound request pending) is not supported without a background pump — that limitation should be documented, not solved, here. **Second: a parse ambiguity.** `shared::parse_request` (`src/shared/protocol_helpers.rs:16-32`) tries the *client* request grammar first, and `sampling/createMessage` is a valid `ClientRequest::CreateMessage` (the legacy path) as well as `ServerRequest::CreateMessage`. So an *inbound* sampling request at the client is delivered as `Request::Client(ClientRequest::CreateMessage)`, verified by the test at `protocol_helpers.rs:467-483`. Dispatch must treat *any* inbound request at a client as server→client and match the `Request::Client(CreateMessage)` shape too (or dispatch by method string). `elicitation/create` and `roots/list` have no client-request variant and parse cleanly as `Request::Server`.

The requesting side already exists and is well-factored: `Server::run` builds a `ServerRequestDispatcher` + drain task (`src/server/mod.rs:971-1010`); tool handlers reach the client via `extra.peer().sample()/.list_roots()` (`PeerHandle` → `DispatchPeerHandle` → dispatcher → outbound `TransportMessage::Request{ id: correlation_id, .. }`). The client must reply with a `TransportMessage::Response` carrying the **same id**; the server routes it back through `handle_transport_message`'s Response arm (`src/server/mod.rs:1093-1117`). This gives a ready-made duplex round-trip test: a `Server` whose tool calls `extra.peer().sample()`, driven by `tests/common/duplex.rs::call_via_server` (full `run` loop with dispatcher), against a `Client` that has a registered sampling handler.

**Primary recommendation:** Add a `ClientHostRegistry` (sampling handler + elicitation handler + roots provider + approval callback) held by `Client`, populated via `ClientBuilder` methods; dispatch it from a new match arm replacing `src/client/mod.rs:2234`, routing by method (handle the `Request::Client(CreateMessage)` alias explicitly); derive advertised capabilities from the registry at initialize; keep the legacy `create_message` untouched and rename it *conceptually* in docs as the "LLM-server pattern". Name the new trait distinctly (recommend module path `pmcp::client::host::*` with names like `SamplingHandler` re-exported only under that path, or unambiguous names such as `HostSamplingHandler`).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Answer inbound `sampling/createMessage` | Client (host) | — | Spec host direction; the client owns LLM access decision + approval |
| Answer inbound `elicitation/create` | Client (host) | — | User-input elicitation is a client/host responsibility |
| Answer inbound `roots/list` | Client (host) | — | Roots describe the client's workspace |
| Human-in-the-loop approval | Client (host) | — | Approval is a host-side policy seam (consoles plug in) |
| Issue `sampling/createMessage` (requesting side) | Server (existing) | — | Already implemented via `PeerHandle`/dispatcher — unchanged this phase |
| Capability advertisement | Client (initialize) | — | Derived from the registry, sent in `InitializeRequest` |
| Legacy `create_message` (LLM-server pattern) | Client→Server (inverted) | — | Kept as-is; docs-only disambiguation |

## Standard Stack

No new crates. All work uses the existing `pmcp` internal modules and its current dependency set.

### Core (existing, in-repo)
| Module / Type | Location | Purpose |
|---------------|----------|---------|
| `Client` / `ClientBuilder` | `src/client/mod.rs:107`, `:2285` | Where the registry field + builder methods land |
| `send_request` receive loop | `src/client/mod.rs:2179-2242` | The dispatch hook site (replaces `:2234`) |
| `ServerRequest` enum | `src/types/protocol/mod.rs:509-519` | The three inbound request shapes |
| `CreateMessageParams` / `CreateMessageResult` | `src/types/sampling.rs:200`, `:293` | Sampling I/O (already carries tools/tool_choice) |
| `ElicitRequestParams` / `ElicitResult` | `src/types/elicitation.rs` | Elicitation I/O |
| `ListRootsResult` | `src/server/roots.rs` (re-exported) | Roots result type |
| `ClientCapabilities` + `SamplingCapabilities`/`ElicitationCapabilities`/`RootsCapabilities` | `src/types/capabilities.rs:25` | Capability derivation for HOST-05 |
| Server `SamplingHandler` (do NOT reuse the name) | `src/server/traits.rs:62` | The naming-collision reference |
| `shared::parse_request` | `src/shared/protocol_helpers.rs:16` | Source of the parse ambiguity (Pitfall 1) |

### Supporting (async primitives already in use)
- `async_trait` for the object-safe handler traits (Send+Sync per design §8.5).
- `tokio::sync` (`RwLock`, `oneshot`, `mpsc`) — already used by `Client` and `ElicitationManager`.
- `parking_lot::RwLock`, `dashmap`, `indexmap` are available if a lock-free registry is preferred, but a simple `Arc<...>` field set at build time (immutable after `build()`) is the least-complexity choice and avoids interior mutability entirely.

**Installation:** none — `cargo build` only.

## Package Legitimacy Audit

**N/A — this phase installs no external packages.** All code is additive within the existing `pmcp` crate using its current dependency set (`tokio`, `async_trait`, `serde`, `serde_json`, `uuid`, already present in `Cargo.toml`). No `npm view` / `pip index` / `cargo search` verification required. slopcheck not applicable.

## Architecture Patterns

### System Architecture Diagram (the nested round-trip that makes this work)

```
  Client task                              Server task (Server::run loop)
  -----------                              ------------------------------
  client.call_tool("t", args)
     │ send TransportMessage::Request(tools/call, id=A)
     ├───────────────────────────────────────►  handle_request_message
     │                                              │ invokes ToolHandler
     │ (blocked in send_request                     │ handler calls
     │  receive loop @2179, awaiting id=A)          │   extra.peer().sample(params)
     │                                              │     → dispatcher.dispatch(
     │                                              │        ServerRequest::CreateMessage)
     │                                              │     → outbound drain wraps as
     │                                              │        Request(sampling/createMessage,
     │  ◄───────────────────────────────────────────       id=corr-1)
     │ receive loop @2234 sees inbound Request
     │   ── NEW DISPATCH ──
     │   route by method → client SamplingHandler
     │   → (optional) approval callback (default allow)
     │   → send TransportMessage::Response(id=corr-1, CreateMessageResult)
     ├───────────────────────────────────────►  handle_transport_message Response arm
     │                                              │ dispatcher.handle_response(corr-1)
     │                                              │ peer.sample() resolves
     │                                              │ ToolHandler completes
     │  ◄───────────────────────────────────────── send Response(id=A, CallToolResult)
     │ receive loop @2183 returns Response(id=A)
  call_tool returns
```

Key property: the client dispatches server→client requests **synchronously inside its own response-wait loop**. No background task, no reentrancy hazard — the loop simply gains a branch that answers-and-continues instead of erroring.

### Recommended Module Structure
```
src/client/
├── mod.rs            # Client fields + ClientBuilder methods + dispatch arm @2234
└── host/             # NEW — client host surface, distinct path from server::traits
    ├── mod.rs        # ClientHostRegistry, re-exports
    ├── sampling.rs   # host sampling handler trait + approval callback type
    ├── elicitation.rs# host elicitation handler trait
    └── roots.rs      # roots provider (trait or closure alias)
```
Placing the traits under `pmcp::client::host::*` is the strongest non-confusability guarantee (HOST locked): even if a trait is named `SamplingHandler`, its fully-qualified path `client::host::SamplingHandler` is unmistakably distinct from `server::traits::SamplingHandler`. If the planner prefers name-level distinction too, `HostSamplingHandler` / `HostElicitationHandler` are clear. Pick one convention and document the contrast in rustdoc on both traits (cross-link them).

### Pattern 1: Dispatch by method, not by parsed variant
**What:** In the new `:2234` arm, do not assume `Request::Server`. Match the request's method (or handle both `Request::Client(ClientRequest::CreateMessage)` and `Request::Server(ServerRequest::CreateMessage)`), because inbound `sampling/createMessage` parses as the *client* variant (Pitfall 1).
**When to use:** always, for the sampling case.
**Example:**
```rust
// Source: derived from src/client/mod.rs:2234 + protocol_helpers.rs:467-483 (verified)
// Inside the send_request receive loop, replacing the error arm:
crate::types::TransportMessage::Request { id, request } => {
    // ANY inbound request at a client is server→client by definition.
    let response = self.dispatch_host_request(id.clone(), request).await;
    self.transport.write().await.send(TransportMessage::Response(response)).await?;
    // Continue waiting for the original response (do NOT remove id=A yet).
}
```
`dispatch_host_request` normalizes: `sampling/createMessage` (whether it arrived as `ClientRequest::CreateMessage` or `ServerRequest::CreateMessage`) → sampling handler; `elicitation/create` → elicitation handler; `roots/list` → roots provider; anything else → JSON-RPC method-not-found error response (not a transport failure).

### Pattern 2: Registry built once, immutable after `build()`
**What:** Hold `Option<Arc<dyn HostSamplingHandler>>`, `Option<Arc<dyn HostElicitationHandler>>`, `Option<RootsProvider>`, and `ApprovalCallback` as plain `Client` fields set from `ClientBuilder`. No runtime mutation, no `RwLock` needed on the registry itself.
**Why:** matches the existing builder pattern (`ClientBuilder` at `:2285`), keeps dispatch lock-free, and sidesteps interior mutability. Capabilities are then a pure function of which fields are `Some`.

### Pattern 3: Approval callback as a boxed async closure, default allow
```rust
// HOST-04 — the seam consoles/approval-mcp plug into; no UI here.
pub type ApprovalCallback = Arc<
    dyn Fn(&CreateMessageParams) -> BoxFuture<'static, ApprovalDecision> + Send + Sync,
>;
// Default: |_| async { ApprovalDecision::Allow }
```
Invoke after the handler produces a `CreateMessageResult` (or before calling the handler — planner's choice; spec SHOULD says approve before returning a completion, so *before returning* is the safe reading). On `Deny`, return a JSON-RPC error result rather than a completion.

### Anti-Patterns to Avoid
- **Spawning a background client receive loop this phase.** It would change `Client` lifecycle semantics (currently receive-while-awaiting) and risk reentrancy with `send_request`. The nested flow covers HOST-01..04. Document the idle-host limitation instead (Open Question 1).
- **Reusing/renaming the server `SamplingHandler` trait.** Locked non-confusability. Introduce a distinct trait/path; do not move or alias the server one.
- **Asserting server capability on the host path.** The legacy `create_message` calls `assert_capability("sampling", …)` against the *server* (`src/client/mod.rs:1848`). The host path is the inverse — do not gate the inbound handler on server capabilities.
- **Mutating `ClientCapabilities` independently of the registry** — HOST-05 requires derivation, not an independent setter.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Server→client request correlation | A client-side pending-request map | Nothing — the *server* owns correlation (`ServerRequestDispatcher`); the client just echoes the request `id` on its `Response` | The wire id round-trips; client only needs to reply with the same id |
| Sampling/elicitation/roots wire types | New structs | `src/types/sampling.rs`, `src/types/elicitation.rs`, `ListRootsResult` | Complete + spec-correct (design §2.1 verified) |
| JSON-RPC error response construction | Ad-hoc JSON | `Error` → response payload helpers already used in `send_request` | Consistent error taxonomy |
| Duplex transport for tests | New mock transport | `tests/common/duplex.rs::{DuplexTransport, call_via_server}` | Established Phase 104/105 convention |

**Key insight:** the requesting half (server issuing sampling + correlating the response) already exists and is tested (`tests/handler_peer_integration.rs`). This phase only adds the *answering* half on the client; almost everything else is reuse.

## Runtime State Inventory

N/A — greenfield-additive within the SDK. No rename/refactor/migration; no stored data, live-service config, OS-registered state, secrets, or build artifacts are affected. (Verified: change is new fields + new module + one edited match arm + docs; no identifiers renamed.)

## Common Pitfalls

### Pitfall 1: Inbound `sampling/createMessage` parses as `Request::Client`, not `Request::Server`
**What goes wrong:** A dispatch arm that matches only `Request::Server(ServerRequest::CreateMessage)` silently misses inbound host sampling and falls through to the error path.
**Why it happens:** `shared::parse_request` (`src/shared/protocol_helpers.rs:16-32`) tries `parse_client_request` FIRST; `sampling/createMessage` is a legitimate `ClientRequest::CreateMessage` (the legacy LLM-server path). Proven by `protocol_helpers.rs` test at `:467-483` which asserts the parse yields `ClientRequest::CreateMessage`.
**How to avoid:** Dispatch by method string, or explicitly match both `Request::Client(ClientRequest::CreateMessage)` and `Request::Server(ServerRequest::CreateMessage)` and route both to the host sampling handler. `elicitation/create` and `roots/list` have no client variant and are unaffected. Add a unit test asserting an inbound `sampling/createMessage` frame reaches the host handler.

### Pitfall 2: Assuming a client can host while idle
**What goes wrong:** Designing a "pure sampling host" that waits for requests with no outbound request pending — it will never receive anything.
**Why it happens:** No background receive loop; only `send_request`'s loop reads inbound requests (`src/client/mod.rs`, single `TransportMessage::Request` receive site confirmed at `:2234`).
**How to avoid:** Scope the phase to the nested flow (sampling during an in-flight client request — the real agent/tool use case). Document the idle-host limitation; defer any background-pump design.

### Pitfall 3: Reintroducing the naming collision
**What goes wrong:** Two `SamplingHandler` traits with different semantics confuse users and rustdoc.
**How to avoid:** Distinct module path (`client::host`) and/or distinct name (`HostSamplingHandler`); cross-link both traits' rustdoc; add the HOST-06 "LLM-server pattern" disambiguation to `Client::create_message` rustdoc.

### Pitfall 4: Capability drift
**What goes wrong:** Advertising `sampling` when no handler is registered (or vice-versa), violating HOST-05.
**How to avoid:** Compute `capabilities.sampling/elicitation/roots` from the registry immediately before sending `InitializeRequest` (`src/client/mod.rs:308-315`). If the caller *also* passes a `ClientCapabilities`, merge (registry wins for the three host fields). Add a test asserting registered-handler ⇒ capability present, and not-registered ⇒ absent.

### Pitfall 5: WASM compilation gates
**What goes wrong:** New handler traits or the approval closure use `tokio`-only types and break the `wasm32` build.
**Why it happens:** `src/client/mod.rs` compiles for `wasm32` (cfg gates at `:26-40`); `send_request`'s loop is not wasm-gated.
**How to avoid:** Keep the registry and dispatch cfg-agnostic; use `async_trait` and `futures::future::BoxFuture` rather than tokio-specific spawns. Verify with a `cargo check --target wasm32-unknown-unknown` (or the repo's existing wasm check) in the plan's verification. See Open Question 2.

## Code Examples

### Existing full round-trip template (dispatcher half) — use as the test model
```rust
// Source: tests/handler_peer_integration.rs (verified, passing)
// Server-side peer.sample() → dispatcher → outbound channel → handle_response.
// Phase 106's duplex test adds the CLIENT answering across a real transport.
let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
let dispatcher = Arc::new(ServerRequestDispatcher::new_with_channel(tx));
let peer: Arc<dyn PeerHandle> = Arc::new(DispatchPeerHandle::new(dispatcher.clone()));
// ... peer.sample(params) emits (correlation_id, ServerRequest::CreateMessage);
// dispatcher.handle_response(&correlation_id, json!({...})) resolves it.
```

### Recommended duplex round-trip shape for Phase 106
```rust
// Source: composed from tests/common/duplex.rs::call_via_server (verified) +
//         examples/s30_tool_with_sampling.rs (verified peer.sample pattern)
// 1. Build a Server with a tool whose handler calls extra.peer().sample(params).
// 2. Build a Client via ClientBuilder::new(client_t).on_sampling(MyHostHandler).build().
// 3. client.call_tool("t", args) — blocks in send_request loop, answers the
//    server's sampling/createMessage from the registered handler, then returns.
// This exercises the entire HOST-01 path across a real transport.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Client errors "Unexpected message type" on any inbound request | Client host registry answers sampling/elicitation/roots | This phase (pmcp 2.16.0) | Spec compliance for the host direction |
| `ClientCapabilities.sampling` = independently set flag | Derived from registered host handler | This phase (HOST-05) | Capability now truthful |
| Sampling only via inverted `create_message` (LLM-server) | Spec host sampling is first-class; inverted path documented as distinct pattern | This phase (HOST-06) | Removes the direction confusion |

**Deprecated/outdated:** none — the legacy `create_message` is explicitly retained (design §6.4).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Answering server→client requests only within the nested (in-flight-request) window satisfies HOST-01..04 for the phase's target use case | Pitfall 2 / Open Q1 | If a reviewer expects idle-host support, scope expands; mitigation: document limitation explicitly and get sign-off |
| A2 | Placing traits under `client::host` + distinct names fully satisfies the locked non-confusability requirement | Architecture Patterns | Low — naming is discretion; only the non-confusability is locked |
| A3 | Deriving the three host capability fields from the registry (registry wins on merge) is the intended HOST-05 semantics | Pitfall 4 | Low — matches CONTEXT "reflect which host handlers are registered" |

**Note:** No `[ASSUMED]` package or version claims — this phase adds no dependencies.

## Open Questions (RESOLVED)

*All three resolved 2026-07-17 during `/gsd:plan-phase 106` (locked in CONTEXT + adopted in the 106-01/106-02 plans).*

1. **Idle-host support scope.** — RESOLVED: nested-only. No background receive pump is built this phase; the limitation is documented in rustdoc (106-01) and the pmcp-book Sampling & Hosting page (106-03), and flagged to Phase 108 planning.
   - What we know: no background receive loop; nested flow covers the agent/tool use case.
   - What's unclear: whether Phase 108's `SamplingSource` (design says it uses "the agent's server-side peer") needs the client to host while idle. Design §3/§8.5 imply sampling happens *while the hosted agent is servicing a request*, i.e. nested — so the constraint is likely fine.
   - Recommendation: implement nested-only; document the limitation in rustdoc; flag to Phase 108 planning. Do not build a pump now.

2. **WASM surface: apply now or defer?** — RESOLVED: cfg-agnostic, compile-clean surface now; example + duplex tests native-only. 106-01 keeps `client::host` free of tokio-only types (uses `async_trait` + `futures::BoxFuture`) and adds a `cargo check --target wasm32-unknown-unknown` step; the sampling-host example and roundtrip tests are gated `#[cfg(not(target_arch = "wasm32"))]`.
   - What we know: `src/client/mod.rs` compiles for wasm; the dispatch site is shared.
   - What's unclear: whether the sampling-host example and duplex test (both `#[cfg(not(target_arch = "wasm32"))]`) leave any wasm-specific gap.
   - Recommendation: make the registry + traits cfg-agnostic and compile-clean on wasm (Send+Sync per design §8.5); scope the example/test to native; add a wasm `cargo check` to verification. Document that the host surface *compiles* on wasm and the nested-flow constraint is identical there.

3. **Approval hook ordering.** — RESOLVED: approve after the handler produces the `CreateMessageResult`, before returning it (lets the approver see the actual completion). Implemented in 106-02 Task 1; `Deny` returns a -32603 error response (connection stays alive), default is allow.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust stable toolchain | build/test | ✓ | current stable (CI uses `dtolnay/rust-toolchain@stable`) | — |
| `wasm32-unknown-unknown` target | Open Q2 verification | verify with `rustup target list --installed` | — | scope example/test to native; note wasm as compile-only |

No external services, databases, or network dependencies. Purely in-crate code + tests.

## Validation Architecture

nyquist_validation is enabled (`.planning/config.json` → `workflow.nyquist_validation: true`).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` + `proptest` (property) + `cargo-fuzz` (fuzz) |
| Config file | none — `cargo test` (CI runs `--test-threads=1` per CLAUDE.md) |
| Quick run command | `cargo test --lib client::host` (unit + module tests) |
| Full suite command | `make quality-gate` (fmt --all, clippy pedantic+nursery, build, test, audit) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HOST-01 | Server `peer.sample()` answered by client host handler across transport | integration (duplex) | `cargo test --test client_host_roundtrip sampling` | ❌ Wave 0 |
| HOST-01 | Inbound `sampling/createMessage` (parsed as `Request::Client`) reaches host handler | unit | `cargo test --lib client::host::dispatch_sampling_alias` | ❌ Wave 0 |
| HOST-01 | `tools`/`tool_choice` + `tool_use`/`tool_result` pass through intact | property (proptest) | `cargo test --test client_host_roundtrip prop_sampling_passthrough` | ❌ Wave 0 |
| HOST-02 | `elicitation/create` answered by client handler | integration | `cargo test --test client_host_roundtrip elicitation` | ❌ Wave 0 |
| HOST-03 | `roots/list` answered from provider | integration | `cargo test --test client_host_roundtrip roots` | ❌ Wave 0 |
| HOST-04 | Approval callback default-allow returns completion; deny returns error | unit | `cargo test --lib client::host::approval` | ❌ Wave 0 |
| HOST-05 | Registered handler ⇒ capability advertised; unregistered ⇒ absent | unit | `cargo test --lib client::host::capability_derivation` | ❌ Wave 0 |
| HOST-05 | Unhandled request type ⇒ JSON-RPC method-not-found (not transport failure) | unit | `cargo test --lib client::host::unhandled_method_error` | ❌ Wave 0 |
| HOST-06 | Legacy `create_message` unchanged (regression) | integration | existing `create_message` tests still green | ✅ (guard) |
| all | Fuzz the inbound host-request dispatch (malformed params don't panic/hang) | fuzz | `cargo fuzz run client_host_dispatch` | ❌ Wave 0 |
| all | Runnable sampling-host example | example | `cargo run --example s44_sampling_host` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib client::host` + `cargo clippy` (fast).
- **Per wave merge:** `cargo test --test client_host_roundtrip` + example run.
- **Phase gate:** `make quality-gate` green before `/gsd:verify-work`.

### Wave 0 Gaps
- [ ] `tests/client_host_roundtrip.rs` — duplex integration covering HOST-01/02/03 (add `#[path = "common/duplex.rs"] mod duplex;`); may need a small `call_via_server`-style helper that registers host handlers on the client.
- [ ] `src/client/host/*` unit tests — dispatch alias, approval, capability derivation, unhandled-method error.
- [ ] `fuzz/fuzz_targets/client_host_dispatch.rs` — fuzz inbound host-request params.
- [ ] `examples/s44_sampling_host.rs` — sampling host with a mock/OpenAI-compat handler (next free `sNN`: existing are s12/s13/s30/s43 → **s44 free**; verify at plan time).
- [ ] proptest for sampling passthrough (tools/tool_choice/tool_use/tool_result invariants).

## Security Domain

`security_enforcement` not disabled in config → included.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Transport-level; unchanged by this phase |
| V3 Session Management | no | — |
| V4 Access Control | yes (light) | The approval callback (HOST-04) IS the host-side access-control seam for sampling; default-allow is documented, deny returns a JSON-RPC error |
| V5 Input Validation | yes | Inbound `CreateMessageParams`/`ElicitRequestParams` are deserialized via serde (typed); dispatch must return method-not-found for unknown methods rather than panic; fuzz the params path |
| V6 Cryptography | no | No crypto introduced |

### Known Threat Patterns for a client host surface
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malicious/malformed inbound request params crash or hang the client | Denial of Service | Typed serde deserialization + return JSON-RPC error, never panic; fuzz target on the dispatch path |
| Unbounded/unexpected inbound method treated as connection failure | Denial of Service | Method-not-found error response keeps the connection alive (CONTEXT locked) |
| Server coerces client into unapproved LLM calls | Elevation of Privilege | Approval callback seam (HOST-04); host owns allow/deny before returning a completion |
| Capability lie (advertise sampling without a handler) | Spoofing/Repudiation | Derive capabilities from the registry (HOST-05) so advertisement matches actual behavior |

## Sources

### Primary (HIGH confidence — read from the working tree 2026-07-17)
- `src/client/mod.rs:107,300-349,1794-1862,2150-2349` — Client struct, initialize, legacy `create_message`, `send_request` receive loop, `ClientBuilder`.
- `src/shared/protocol_helpers.rs:16-32,137-161,467-483` — parse ordering + the ambiguity test.
- `src/shared/transport.rs:115-151` — `parse_message` → `parse_request` path.
- `src/types/protocol/mod.rs:490-529` — `ServerRequest` / `Request` enums.
- `src/types/sampling.rs` (full) — `CreateMessageParams`/`Result`, `ToolChoice`, `SamplingMessageContent` (tool_use/tool_result).
- `src/types/capabilities.rs:1-120` — `ClientCapabilities` + sampling/elicitation/roots sub-structs.
- `src/server/traits.rs:60-69` — existing server `SamplingHandler` (collision reference).
- `src/server/peer_impl.rs` (full) — `DispatchPeerHandle` (requesting side).
- `src/server/elicitation.rs:1-140` — `ElicitationManager` request path.
- `src/server/mod.rs:971-1120` — `Server::run` dispatcher/drain + response routing.
- `tests/common/duplex.rs` (full) — duplex harness (`call_via_server`/`call_via_core`).
- `tests/handler_peer_integration.rs:1-80` — dispatcher round-trip template.
- `examples/s43_handler_peer_sample.rs`, example listing (s12/s13/s30/s43) — example conventions + next free number.
- `pmcp-book/src/SUMMARY.md`, `pmcp-book/src/ch17-03-sampling-tools.md` — book location for HOST-06 seed.
- `docs/design/agents-teams-sdk-extraction-plan.md` §2.1/§4-A/§8.5 — approved design.
- `Cargo.toml:3` — version 2.15.0 (→ 2.16.0 minor bump).

### Secondary / Tertiary
- None required; all claims verified against source.

## Metadata

**Confidence breakdown:**
- Standard stack (reuse map): HIGH — every type/module read directly.
- Architecture (nested dispatch + parse ambiguity): HIGH — confirmed via single receive site at `:2234` and the parse test at `protocol_helpers.rs:467-483`.
- Pitfalls: HIGH — each maps to a cited line range.
- WASM scope (Open Q2): MEDIUM — compiles for wasm but not exhaustively verified at research time; verification step included in the plan.

**Research date:** 2026-07-17
**Valid until:** ~2026-08-16 (stable brownfield; only invalidated by refactors of the client receive loop or `parse_request` ordering).
