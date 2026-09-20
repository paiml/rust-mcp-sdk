# Phase 106: Client Host Surface - Pattern Map

**Mapped:** 2026-07-17
**Files analyzed:** 8 (5 new src/example/test/fuzz files + 3 modification zones in `src/client/mod.rs`)
**Analogs found:** 8 / 8 (all in-crate; brownfield)

This phase is 100% additive inside the `pmcp` crate. Every wire type already exists;
the work is a client-side handler registry + a new dispatch arm replacing the single
error line at `src/client/mod.rs:2234`. All analogs below are verified against the
working tree at the cited line ranges.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/client/host/sampling.rs` (new) | trait/handler | request-response | `src/server/traits.rs:60-69` (`SamplingHandler`) | exact (mirror direction) |
| `src/client/host/elicitation.rs` (new) | trait/handler | request-response | `src/server/traits.rs:62` + `src/server/elicitation.rs:66-118` | role-match |
| `src/client/host/roots.rs` (new) | provider (closure/trait) | request-response | `src/server/peer_impl.rs:68-76` (`list_roots`) + `src/server/roots.rs:26` | role-match |
| `src/client/host/mod.rs` (new) | registry/config | — | `ClientBuilder` fields `src/client/mod.rs:2285-2418`; `ElicitationManager` struct `src/server/elicitation.rs:20-47` | role-match |
| `src/client/mod.rs` — Client struct + builder + dispatch (modified) | client core | request-response | itself: struct `:107-120`, builder `:2300-2418`, receive loop `:2179-2242`, initialize `:300-351` | self (edit-in-place) |
| `tests/client_host_roundtrip.rs` (new) | test (integration/duplex) | request-response | `tests/common/duplex.rs::call_via_server` + `tests/handler_peer_integration.rs` | exact |
| `fuzz/fuzz_targets/client_host_dispatch.rs` (new) | test (fuzz) | transform | `fuzz/fuzz_targets/fuzz_peer_handle.rs` | exact |
| `examples/s49_sampling_host.rs` (new) | example | request-response | `examples/s43_handler_peer_sample.rs` (+ `s30_tool_with_sampling.rs`, `s12_sampling_llm.rs`) | exact |

**Example number note:** RESEARCH.md said "s44 free" but s44 is now taken (`s44_server_skills.rs`).
Highest existing is `s48_durable_poll_decision.rs` → **next free is `s49`**. Verified 2026-07-17.

## Pattern Assignments

### `src/client/host/sampling.rs` (new — host sampling handler trait + approval callback)

**Analog:** `src/server/traits.rs:60-69` — the EXISTING server-side `SamplingHandler`.
This is the collision reference (HOST-06 locked non-confusability). Copy the *shape*,
NOT the name/path. Place under `client::host::*` and/or name `HostSamplingHandler`.

**Trait shape to mirror** (`src/server/traits.rs:60-69`):
```rust
/// Handler for message sampling (LLM operations).
#[async_trait]
pub trait SamplingHandler: Send + Sync {
    async fn create_message(
        &self,
        params: CreateMessageParams,
        extra: RequestHandlerExtra,
    ) -> Result<CreateMessageResult>;
}
```
Recommended new trait (distinct path + name, cross-link rustdoc to the server one):
```rust
// src/client/host/sampling.rs
use async_trait::async_trait;
use crate::error::Result;
use crate::types::sampling::{CreateMessageParams, CreateMessageResult};

/// Host-side sampling handler: answers an INBOUND spec-direction
/// `sampling/createMessage` (the MCP host direction). Distinct from
/// [`crate::server::traits::SamplingHandler`], which is the inverted
/// "LLM-server pattern" (client asks server). See HOST-06.
#[async_trait]
pub trait HostSamplingHandler: Send + Sync {
    async fn handle_create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResult>;
}
```
Note the server trait takes `RequestHandlerExtra`; the host trait has no such context
(there is no `RequestHandlerExtra` on the client receive path) — drop that param.

**Approval callback (HOST-04)** — boxed async closure, default allow (from RESEARCH Pattern 3):
```rust
pub type ApprovalCallback = Arc<
    dyn Fn(&CreateMessageParams) -> BoxFuture<'static, ApprovalDecision> + Send + Sync,
>;
// Default: |_| Box::pin(async { ApprovalDecision::Allow })
```
Object-safe + `Send + Sync` is mandatory (design §8.5 / pmcp.run durable-host reuse).
Use `futures::future::BoxFuture`, NOT tokio spawns, to keep wasm-clean (Pitfall 5).

---

### `src/client/host/elicitation.rs` (new — host elicitation handler trait)

**Analog:** `src/server/elicitation.rs:66-118` (the requesting side — mirror it) +
the trait shape from `src/server/traits.rs`.

Server side SENDS `ServerRequest::ElicitationCreate(Box<ElicitRequestParams>)` and
awaits an `ElicitResult` (`src/server/elicitation.rs:84`). The client host must ANSWER
that: take `ElicitRequestParams` → return `ElicitResult`.

```rust
// src/client/host/elicitation.rs
use async_trait::async_trait;
use crate::error::Result;
use crate::types::elicitation::{ElicitRequestParams, ElicitResult};

#[async_trait]
pub trait HostElicitationHandler: Send + Sync {
    async fn handle_elicitation(
        &self,
        params: ElicitRequestParams,
    ) -> Result<ElicitResult>;
}
```
`elicitation/create` parses cleanly as `Request::Server(ServerRequest::ElicitationCreate)`
(no client-request alias — unlike sampling). No Pitfall-1 ambiguity here.

---

### `src/client/host/roots.rs` (new — roots provider)

**Analog:** `src/server/peer_impl.rs:68-76` (`list_roots` returning `ListRootsResult`) and
result type `src/server/roots.rs:26` (`ListRootsResult { roots: Vec<Root> }`).
`ServerRequest::ListRoots` is a **unit variant** (`src/types/protocol/mod.rs:513-515`) — no params.

Discretion (CONTEXT): a closure provider is sufficient rather than a full trait. Suggested:
```rust
// src/client/host/roots.rs
use crate::server::roots::ListRootsResult;
use futures::future::BoxFuture;
use std::sync::Arc;

pub type RootsProvider =
    Arc<dyn Fn() -> BoxFuture<'static, ListRootsResult> + Send + Sync>;
```
Mirror the mock in `examples/s43_handler_peer_sample.rs:48-50`:
`Ok(ListRootsResult { roots: Vec::new() })`.

---

### `src/client/host/mod.rs` (new — `ClientHostRegistry` + re-exports)

**Analog for the "struct-of-optional-handlers" shape:** `ElicitationManager`
(`src/server/elicitation.rs:20-47`) — a plain struct holding handler/channel state plus a
manual `Debug` impl (handlers aren't `Debug`).

RESEARCH Pattern 2: registry is built ONCE, immutable after `build()`. Hold plain
`Option<Arc<...>>` fields — NO `RwLock` on the registry itself (dispatch stays lock-free):
```rust
// src/client/host/mod.rs
#[derive(Clone, Default)]
pub struct ClientHostRegistry {
    pub(crate) sampling: Option<Arc<dyn HostSamplingHandler>>,
    pub(crate) elicitation: Option<Arc<dyn HostElicitationHandler>>,
    pub(crate) roots: Option<RootsProvider>,
    pub(crate) approval: Option<ApprovalCallback>,
}
```
Follow the manual-`Debug` convention from `ElicitationManager` (`:30-37`) — print
`has_sampling`/`has_elicitation` booleans, not the trait objects.

Re-export the three traits + callback types here so the public path is unambiguously
`pmcp::client::host::HostSamplingHandler` (HOST-06 non-confusability guarantee).

---

### `src/client/mod.rs` — 3 modification zones (edit-in-place)

**Zone A — Client struct field** (`src/client/mod.rs:107-120`). Add one field mirroring
the existing `options: ClientOptions` field:
```rust
pub struct Client<T: Transport> {
    // ... existing fields ...
    options: ClientOptions,
    host_registry: crate::client::host::ClientHostRegistry, // NEW
}
```
Update the manual `Debug` impl (`:122-133`) if you surface it (optional).

**Zone B — `ClientBuilder` methods + `build()`** (`:2300-2418`). Copy the exact
consuming-builder method pattern (each returns `Self`):
```rust
// analog: enforce_strict_capabilities @2310-2314
pub fn on_sampling(mut self, h: impl HostSamplingHandler + 'static) -> Self {
    self.host_registry.sampling = Some(Arc::new(h));
    self
}
pub fn on_elicitation(mut self, h: impl HostElicitationHandler + 'static) -> Self { /* ... */ self }
pub fn on_roots(mut self, provider: RootsProvider) -> Self { /* ... */ self }
pub fn on_sampling_approval(mut self, cb: ApprovalCallback) -> Self { /* ... */ self }
```
Add a `host_registry: ClientHostRegistry` field to `ClientBuilder` (`:2285-2289`, next to
`middleware_chain`) and thread it through `build()` (`:2407-2417`) exactly like
`middleware_chain` is threaded: `client.host_registry = self.host_registry;`.

**Zone C — dispatch arm replacing the error at `:2234-2240`** (THE core change). Current:
```rust
crate::types::TransportMessage::Request { .. } => {
    // Unexpected message type
    self.active_requests.write().await.remove(&request_id);
    return Err(Error::protocol_msg(
        "Unexpected message type while waiting for response",
    ));
},
```
Replace with dispatch-and-continue (do NOT remove `request_id` — keep waiting for the
original response). RESEARCH Pattern 1 (dispatch by method — Pitfall 1: inbound
`sampling/createMessage` arrives as `Request::Client(ClientRequest::CreateMessage)`, NOT
`Request::Server`, proven at `protocol_helpers.rs:467-483`):
```rust
crate::types::TransportMessage::Request { id, request } => {
    // ANY inbound request at a client is server→client by definition.
    let response = self.dispatch_host_request(id, request).await; // JSONRPCResponse
    self.transport.write().await.send(
        crate::types::TransportMessage::Response(response)
    ).await?;
    // continue loop — still awaiting the original request_id
},
```

**Zone C helper — `dispatch_host_request`** builds a `JSONRPCResponse`. Copy the
result/error response construction EXACTLY from `Server::create_response`
(`src/server/mod.rs:1331-1350`):
```rust
// Ok arm
JSONRPCResponse {
    jsonrpc: "2.0".to_string(),
    id,
    payload: crate::types::jsonrpc::ResponsePayload::Result(value),
}
// Unhandled-method / deny arm → method-not-found, NOT a transport failure (CONTEXT locked)
JSONRPCResponse {
    jsonrpc: "2.0".to_string(),
    id,
    payload: crate::types::jsonrpc::ResponsePayload::Error(
        crate::types::jsonrpc::JSONRPCError { code: -32601, message, data: None }
    ),
}
```
(Server used -32603 for internal errors; use **-32601 method-not-found** for the
unhandled-request case per CONTEXT/RESEARCH Pitfall-handling; keep -32603 for handler
internal failures.) Route by method string / matching BOTH
`Request::Client(ClientRequest::CreateMessage)` and `Request::Server(ServerRequest::CreateMessage)`
to the sampling handler; `ServerRequest::ElicitationCreate` → elicitation; `ServerRequest::ListRoots`
→ roots provider.

**Zone D (part of Zone B/initialize) — capability derivation (HOST-05)** at
`src/client/mod.rs:308`. Currently `initialize` stores caller-supplied capabilities verbatim:
```rust
self.capabilities = Some(capabilities.clone());  // :308
```
Before sending `InitializeRequest` (`:311-315`), OR the three host fields from the registry
(registry wins on merge — Pitfall 4). `ClientCapabilities` fields are
`sampling`/`elicitation`/`roots` (`src/types/capabilities.rs:25-36`), all
`Option<...Capabilities>`:
```rust
let mut capabilities = capabilities;
if self.host_registry.sampling.is_some() {
    capabilities.sampling.get_or_insert_with(Default::default);
}
// same for elicitation / roots
```
Do NOT gate the inbound handler on *server* capabilities — that is the legacy
`create_message` path's job (`:1848` `assert_capability("sampling", ...)`), which is the
INVERSE direction and stays untouched (HOST-06 anti-pattern in RESEARCH).

---

### `tests/client_host_roundtrip.rs` (new — duplex integration)

**Analog:** `tests/common/duplex.rs::call_via_server` (`:86-100`) + the round-trip assertion
style of `tests/handler_peer_integration.rs:24-69`.

Include the shared harness per-crate (convention documented in `duplex.rs:11`):
```rust
#![cfg(not(target_arch = "wasm32"))]
#[path = "common/duplex.rs"]
mod duplex;
```
`call_via_server` currently builds the client with plain `Client::new(client_t)` (`:91`)
and default caps (`:93`). Phase 106 needs a variant that registers host handlers on the
client before `initialize` — either add a `call_via_server_with_host(server, registry_fn, ...)`
helper to `duplex.rs`, or inline the client construction in the test:
```rust
// Build a Server whose tool calls extra.peer().sample(params) — mirror s43's ToolHandler
// (examples/s43_handler_peer_sample.rs:67-93) but with a REAL peer via Server::run.
let (client_t, server_t) = duplex::DuplexTransport::pair();
tokio::spawn(async move { let _ = server.run(server_t).await; });
let mut client = ClientBuilder::new(client_t)
    .on_sampling(MockHostHandler)
    .build();
client.initialize(ClientCapabilities::default()).await.unwrap();
let result = client.call_tool("t".into(), args).await.unwrap();
// server's sampling/createMessage was answered by MockHostHandler mid-call.
```
Assert style from `handler_peer_integration.rs`: canned `CreateMessageResult`, then
`assert_eq!(result..., ...)`. Cover HOST-01 (sampling), HOST-02 (elicitation), HOST-03
(roots) as three `#[tokio::test]`s.

**Response JSON shape** for the mock handler — camelCase per
`handler_peer_integration.rs:54-57`:
```rust
CreateMessageResult { content: Content::text("..."), model: "mock-model".into(), .. }
```

---

### `fuzz/fuzz_targets/client_host_dispatch.rs` (new)

**Analog:** `fuzz/fuzz_targets/fuzz_peer_handle.rs` (whole file, 26 lines) — the serde-boundary
fuzz pattern. Copy it verbatim and point it at the INBOUND params types the host dispatch
deserializes (`CreateMessageParams`, `ElicitRequestParams`), asserting no panic on
adversarial JSON:
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use pmcp::types::sampling::CreateMessageParams;
use pmcp::types::elicitation::ElicitRequestParams;
use serde_json::{from_slice, from_value, Value};

fuzz_target!(|data: &[u8]| {
    let Ok(json) = from_slice::<Value>(data) else { return; };
    let _ = from_value::<CreateMessageParams>(json.clone());
    let _ = from_value::<ElicitRequestParams>(json);
});
```
Register the target in `fuzz/Cargo.toml` alongside `fuzz_peer_handle`.

---

### `examples/s49_sampling_host.rs` (new — sampling host with mock LLM handler)

**Analog:** `examples/s43_handler_peer_sample.rs` (whole file) — the requesting-side mock;
plus `s30_tool_with_sampling.rs` and `s12_sampling_llm.rs` for sampling conventions.

Copy s43's file conventions exactly:
- Header `//! Run with: cargo run --example s49_sampling_host` (s43:15)
- `#![cfg(not(target_arch = "wasm32"))]` (s43:17)
- `use anyhow::Result;` + `#[tokio::main] async fn main()` (s43:21,96)
- A mock/canned handler mirroring `MockPeer` (s43:37-61), but implementing the NEW
  `HostSamplingHandler` returning `CreateMessageResult::new(Content::text("..."), "mock-model")`
  (s43:42-45).

Unlike s43 (in-process `MockPeer`, no transport), s49 should demonstrate the real nested
flow: a `Server` with a sampling-requesting tool + a `Client` built via
`ClientBuilder::new(t).on_sampling(MockLlm).build()` over the duplex transport, so
`client.call_tool(...)` triggers the host handler. This is the "runnable example" ALWAYS
requirement.

## Shared Patterns

### Handler trait definition (async, object-safe, Send+Sync)
**Source:** `src/server/traits.rs:24-69`
**Apply to:** all three new host traits (`src/client/host/*.rs`)
```rust
#[async_trait]
pub trait XHandler: Send + Sync {
    async fn handle_x(&self, params: XParams) -> Result<XResult>;
}
```
`async_trait` + `Send + Sync` is the universal convention (also `PeerHandle` in
`src/server/peer_impl.rs:53`). Required for object-safety and pmcp.run durable-host reuse.

### Consuming builder methods returning `Self`
**Source:** `src/client/mod.rs:2310-2405` (`enforce_strict_capabilities`, `with_middleware`,
`middleware_chain`)
**Apply to:** all `ClientBuilder::on_*` registration methods
```rust
pub fn setting(mut self, v: V) -> Self { self.field = v; self }
```
Thread the new field through `build()` exactly like `middleware_chain` (`:2414-2415`).

### JSONRPCResponse construction (result + error)
**Source:** `src/server/mod.rs:1331-1350` (`create_response`)
**Apply to:** the client's `dispatch_host_request` reply builder
```rust
JSONRPCResponse { jsonrpc: "2.0".to_string(), id, payload: ResponsePayload::Result(value) }
JSONRPCResponse { jsonrpc: "2.0".to_string(), id, payload: ResponsePayload::Error(JSONRPCError { code, message, data: None }) }
```
Use **-32601** (method-not-found) for unhandled request types (keeps the connection alive —
CONTEXT locked); **-32603** for handler-internal failures.

### Manual `Debug` for structs holding trait objects
**Source:** `src/server/elicitation.rs:30-37`; `src/client/mod.rs:122-133`; `:2291-2298`
**Apply to:** `ClientHostRegistry` (handlers aren't `Debug`) — print `has_*` booleans.

### Duplex round-trip test harness
**Source:** `tests/common/duplex.rs` (`DuplexTransport::pair`, `call_via_server`) +
`tests/handler_peer_integration.rs` assertion style
**Apply to:** `tests/client_host_roundtrip.rs` via `#[path = "common/duplex.rs"] mod duplex;`

### wasm-clean async primitives
**Source:** cfg gates in `src/server/elicitation.rs:11-14`; RESEARCH Pitfall 5
**Apply to:** all new host code — use `futures::future::BoxFuture` + `async_trait`, never
`tokio::spawn`; keep registry/traits cfg-agnostic. Verify with
`cargo check --target wasm32-unknown-unknown`. Scope the example + duplex test to
`#[cfg(not(target_arch = "wasm32"))]` (matches `duplex.rs:14`, `s43:17`).

## No Analog Found

None. Every file has a strong in-crate analog. The two genuinely NEW seams —
the **approval callback** (HOST-04) and **capability derivation from a registry**
(HOST-05) — have no direct prior art but are small, self-contained additions whose
surrounding scaffolding (builder methods, initialize flow) is fully patterned above.

| Seam | Role | Data Flow | Note |
|------|------|-----------|------|
| Approval callback (`ApprovalCallback`) | policy hook | request-response | New seam; shape given in RESEARCH Pattern 3. Default-allow closure. No existing analog for the callback *type*, but boxed-async-closure is idiomatic. |
| Registry→capability derivation | config transform | — | New logic in `initialize` (`:308`); no prior "derive caps from handlers" code. Test: registered ⇒ present, unregistered ⇒ absent. |

## Metadata

**Analog search scope:** `src/client/`, `src/server/` (traits, elicitation, peer_impl, roots,
mod dispatch/response), `src/types/` (protocol, capabilities, sampling), `src/shared/protocol_helpers.rs`,
`tests/common/`, `tests/handler_peer_integration.rs`, `examples/s*`, `fuzz/fuzz_targets/`
**Files scanned:** ~15 (targeted ranges; no full re-reads)
**Pattern extraction date:** 2026-07-17
</content>
</invoke>
