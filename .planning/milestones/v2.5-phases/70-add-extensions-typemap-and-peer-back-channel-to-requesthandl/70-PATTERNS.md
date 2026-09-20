# Phase 70: Add Extensions typemap and peer back-channel to RequestHandlerExtra — Pattern Map

**Mapped:** 2026-04-16
**Files analyzed:** 7 new + 5 modified (+12 struct-literal test sites in one modified file) = 12 distinct edit locations
**Analogs found:** 12 / 12 (all have close in-repo matches)

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/server/cancellation.rs` *(modify)* | model / request-context struct | per-request state container | (self — canonical site) | exact / in-place |
| `src/shared/cancellation.rs` *(modify)* | model (wasm-safe shadow) | per-request state container | `src/server/cancellation.rs` | exact |
| `src/shared/peer.rs` *(NEW)* | trait definition | request-response + pub-sub | `src/server/elicitation.rs:198-220` (`ElicitInput` trait) and `src/server/traits.rs` trait-hierarchy style | role-match |
| `src/server/peer_impl.rs` *(NEW)* | service impl (pub(crate)) | request-response via mpsc + oneshot | `src/server/elicitation.rs:21-190` (`ElicitationManager`) | exact (clone-adapt) |
| `src/server/core.rs` *(modify, 4 sites)* | controller / dispatch | request-response | (self — existing `RequestHandlerExtra::new(..).with_auth_context(..)` chain at `src/server/core.rs:411-418`) | exact / in-place |
| `src/server/mod.rs` *(modify, 5 sites)* | controller / dispatch (legacy) | request-response | `src/server/mod.rs:1055-1060` (the builder-chain at site #5) | exact / in-place |
| `src/server/workflow/prompt_handler.rs` *(modify, 12 struct-literal sites)* | test fixtures | compilation-only | `src/server/workflow/prompt_handler.rs:1055-1064` (site #1 is the template for the other 11) | exact / in-place |
| `tests/handler_extensions_properties.rs` *(NEW)* | test (proptest) | property | `tests/property_tests.rs:11-57` (proptest! block style) + `tests/state_machine_properties.rs:1-80` (module layout) | exact |
| `tests/handler_peer_integration.rs` *(NEW)* | test (integration) | request-response harness | `tests/test_roots.rs:1-60` (in-process `Server::builder()` + `#[tokio::test]`) | role-match |
| `fuzz/fuzz_targets/fuzz_peer_handle.rs` *(NEW)* | fuzz target | transform / parser | `fuzz/fuzz_targets/protocol_parsing.rs:1-55` | exact |
| `fuzz/Cargo.toml` *(modify)* | config | registration | `fuzz/Cargo.toml:27-39` (each `[[bin]]` block) | exact / in-place |
| `examples/s42_handler_extensions.rs` *(NEW)* | example (server) | request-response | `examples/s16_typed_tools.rs:1-80` + `examples/s30_tool_with_sampling.rs:195-229` (main fn + `Server::builder` + `.tool(..)` + `run_stdio`) | role-match |
| `examples/s43_handler_peer_sample.rs` *(NEW)* | example (server) | request-response with server-to-client back-channel | `examples/s30_tool_with_sampling.rs` (contrast target per research Question #8) | role-match |
| `Cargo.toml` *(modify, after line 503)* | config | registration | `Cargo.toml:495-498` + `500-503` (existing `[[example]]` entries for `s24_mcp_prompt_macro` and `s41_code_mode_graphql`) | exact / in-place |

---

## Pattern Assignments

### 1. `src/server/cancellation.rs` (model, per-request state container) — MODIFY

**Analog:** self (in-place extension). Reuse the existing Debug-redaction + builder-chain conventions already inside the file.

**Imports pattern — extend the existing block** (`src/server/cancellation.rs:1-11`):

```rust
use crate::error::Result;
use crate::server::progress::ProgressReporter;
use crate::types::{CancelledNotification, Notification};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::RwLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio_util::sync::CancellationToken;
```

**Additions required:**

```rust
// Plan 01 (always-on, both wasm and non-wasm):
// (no new `use` needed — refer as `http::Extensions` inline; the `http` crate is
//  already a direct dep at Cargo.toml:58, version 1.x)

// Plan 02 (non-wasm only):
#[cfg(not(target_arch = "wasm32"))]
use crate::shared::peer::PeerHandle;  // new module defined in src/shared/peer.rs
```

**Struct-extension pattern — copy shape of existing field additions** (`src/server/cancellation.rs:117-147`):

```rust
// CURRENT (8 fields)
#[derive(Clone)]
pub struct RequestHandlerExtra {
    pub cancellation_token: CancellationToken,
    pub request_id: String,
    pub session_id: Option<String>,
    pub auth_info: Option<crate::types::auth::AuthInfo>,
    pub auth_context: Option<crate::server::auth::AuthContext>,
    pub metadata: HashMap<String, String>,
    #[allow(dead_code)]
    pub progress_reporter: Option<Arc<dyn ProgressReporter>>,
    pub task_request: Option<serde_json::Value>,
}
```

**Proposed change (Plan 01 adds `extensions`, Plan 02 adds `peer`):**

```rust
#[derive(Clone)]
#[non_exhaustive]  // NEW — prevents future struct-literal break at 12 prompt_handler sites
pub struct RequestHandlerExtra {
    pub cancellation_token: CancellationToken,
    pub request_id: String,
    pub session_id: Option<String>,
    pub auth_info: Option<crate::types::auth::AuthInfo>,
    pub auth_context: Option<crate::server::auth::AuthContext>,
    pub metadata: HashMap<String, String>,
    #[allow(dead_code)]
    pub progress_reporter: Option<Arc<dyn ProgressReporter>>,
    pub task_request: Option<serde_json::Value>,
    /// Typed request-scoped state for middleware→handler transfer. [Plan 01]
    pub extensions: http::Extensions,
    /// Server-to-client back-channel for in-handler RPCs. [Plan 02]
    #[cfg(not(target_arch = "wasm32"))]
    pub peer: Option<Arc<dyn PeerHandle>>,
}
```

**Constructor pattern — copy from `::new` at lines 151-162:**

```rust
// CURRENT
impl RequestHandlerExtra {
    pub fn new(request_id: String, cancellation_token: CancellationToken) -> Self {
        Self {
            cancellation_token,
            request_id,
            session_id: None,
            auth_info: None,
            auth_context: None,
            metadata: HashMap::new(),
            progress_reporter: None,
            task_request: None,
        }
    }
```

**Extend with (Plan 01 + Plan 02):**

```rust
    pub fn new(request_id: String, cancellation_token: CancellationToken) -> Self {
        Self {
            cancellation_token,
            request_id,
            session_id: None,
            auth_info: None,
            auth_context: None,
            metadata: HashMap::new(),
            progress_reporter: None,
            task_request: None,
            extensions: http::Extensions::new(),
            #[cfg(not(target_arch = "wasm32"))]
            peer: None,
        }
    }
```

**Builder-method pattern — copy shape of `with_auth_context` at lines 177-183:**

```rust
// EXISTING TEMPLATE
pub fn with_auth_context(
    mut self,
    auth_context: Option<crate::server::auth::AuthContext>,
) -> Self {
    self.auth_context = auth_context;
    self
}
```

**New builder methods to add (same shape):**

```rust
// Plan 01:
pub fn extensions(&self) -> &http::Extensions { &self.extensions }
pub fn extensions_mut(&mut self) -> &mut http::Extensions { &mut self.extensions }

// Plan 02 (non-wasm only):
#[cfg(not(target_arch = "wasm32"))]
pub fn with_peer(mut self, peer: Arc<dyn PeerHandle>) -> Self {
    self.peer = Some(peer);
    self
}
#[cfg(not(target_arch = "wasm32"))]
pub fn peer(&self) -> Option<&Arc<dyn PeerHandle>> { self.peer.as_ref() }
```

**Default impl pattern — copy lines 276-294:**

```rust
// CURRENT
impl Default for RequestHandlerExtra {
    fn default() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
            request_id: uuid::Uuid::new_v4().to_string(),
            session_id: None,
            auth_info: None,
            auth_context: None,
            metadata: HashMap::new(),
            progress_reporter: None,
            task_request: None,
        }
    }
}
```

Add `extensions: http::Extensions::new()` plus `#[cfg(not(target_arch = "wasm32"))] peer: None,`.

**Debug-impl (redaction) pattern — copy lines 296-335:**

```rust
// CURRENT tail of Debug::fmt
f.debug_struct("RequestHandlerExtra")
    .field("cancellation_token", &self.cancellation_token)
    .field("request_id", &self.request_id)
    .field("session_id", &self.session_id)
    .field("auth_info", &self.auth_info)
    .field("auth_context", &self.auth_context)
    .field("metadata", &redacted_metadata)
    .field("task_request", &self.task_request.is_some())
    .finish()
```

**Extend with (Plan 01 + Plan 02):**

```rust
    .field("extensions", &self.extensions)  // http::Extensions Debug prints type names only — redaction-friendly
    .field(
        "peer",
        &self.peer.as_ref().map(|_| "Arc<dyn PeerHandle>"),  // non-wasm only
    )
    .finish()
```

**In-module unit test pattern — copy lines 352-375 (`#[cfg(test)] mod tests`):**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_cancel_token() {
        let manager = CancellationManager::new();
        let token = manager.create_token("test-request".to_string()).await;
        assert!(!token.is_cancelled());
        // ...
    }
}
```

New unit tests to add (Plan 01+02) into this module:

- `test_extensions_insert_overwrite_returns_old` (70-01-03)
- `test_peer_handle_trait_shape` (70-02-01)
- `test_peer_progress_notify_noop_without_reporter` (70-02-03)
- `test_peer_sample_respects_timeout` (70-02-04)

---

### 2. `src/shared/cancellation.rs` (model, wasm-safe shadow) — MODIFY [Plan 01 only]

**Analog:** self + mirror of `src/server/cancellation.rs`

**Current struct** (`src/shared/cancellation.rs:38-51`):

```rust
#[derive(Clone, Debug)]
pub struct RequestHandlerExtra {
    pub cancellation_token: CancellationToken,
    pub request_id: String,
    pub session_id: Option<String>,
    pub auth_info: Option<crate::types::auth::AuthInfo>,
    #[cfg(not(target_arch = "wasm32"))]
    pub auth_context: Option<crate::server::auth::AuthContext>,
}
```

**Current `::new`** (`src/shared/cancellation.rs:53-64`):

```rust
impl RequestHandlerExtra {
    pub fn new(request_id: String, cancellation_token: CancellationToken) -> Self {
        Self {
            cancellation_token,
            request_id,
            session_id: None,
            auth_info: None,
            #[cfg(not(target_arch = "wasm32"))]
            auth_context: None,
        }
    }
```

**Change:** Add `#[non_exhaustive]` + field `pub extensions: http::Extensions` + matching `extensions: http::Extensions::new()` in the struct literal inside `::new`. No peer field here — peer is non-wasm only and lives in `src/server/cancellation.rs`.

---

### 3. `src/shared/peer.rs` (trait definition, NEW FILE) — Plan 02

**Analog:** `src/server/elicitation.rs:198-220` (the `ElicitInput` extension trait) for the `#[async_trait]` object-safe trait shape; module-organization pattern from `src/shared/cancellation.rs` (module-level `#[cfg(not(target_arch = "wasm32"))]`).

**Imports pattern — copy from elicitation.rs:1-15 minus the manager-specific types:**

```rust
use crate::error::Result;
use crate::types::{CreateMessageParams, CreateMessageResult, ProgressToken};
// ListRootsResult comes from src/server/roots.rs:26
use async_trait::async_trait;
```

**Core trait pattern (no in-repo identical analog — use the ElicitInput shape from `src/server/elicitation.rs:198-220`):**

```rust
// TEMPLATE FROM src/server/elicitation.rs:199-200 (ElicitInput trait preamble)
/// Extension trait for tool handlers to elicit input.
#[async_trait::async_trait]
pub trait ElicitInput {
    // ...
}
```

**Proposed for `src/shared/peer.rs`:**

```rust
//! Peer back-channel trait for server-to-client RPCs from inside request handlers.

#![cfg(not(target_arch = "wasm32"))]

use crate::error::Result;
use crate::types::{CreateMessageParams, CreateMessageResult, ProgressToken};
use crate::server::roots::ListRootsResult;
use async_trait::async_trait;

/// Server-to-client back-channel accessible from inside request handlers.
///
/// Implementations route the outbound RPC to the client session that
/// originated the current inbound request. The trait is object-safe so
/// `RequestHandlerExtra` can hold `Option<Arc<dyn PeerHandle>>`.
#[async_trait]
pub trait PeerHandle: Send + Sync {
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult>;
    async fn list_roots(&self) -> Result<ListRootsResult>;
    async fn progress_notify(
        &self,
        token: ProgressToken,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()>;
}
```

Doctest required on the trait and each method (`rust,no_run` per CLAUDE.md doctest requirement).

---

### 4. `src/server/peer_impl.rs` (service impl, NEW FILE) — Plan 02

**Analog:** `src/server/elicitation.rs:21-119` (`ElicitationManager` struct + `elicit_input` method — the exact mpsc+oneshot+timeout pattern) PLUS `src/server/roots.rs:148-166` (`request_client_roots` for the list_roots adapter shape).

**Struct-state pattern — copy shape of `ElicitationManager` (lines 21-28):**

```rust
// TEMPLATE
pub struct ElicitationManager {
    pending: Arc<RwLock<HashMap<String, oneshot::Sender<ElicitResult>>>>,
    request_tx: Option<mpsc::Sender<ServerRequest>>,
    timeout_duration: Duration,
}
```

**Proposed `DispatchPeerHandle`:**

```rust
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct DispatchPeerHandle {
    request_tx: mpsc::Sender<ServerRequest>,
    pending: Arc<RwLock<HashMap<String, oneshot::Sender<serde_json::Value>>>>,
    session_id: Option<String>,
    timeout_duration: Duration,
    progress_reporter: Option<Arc<dyn ProgressReporter>>,
}
```

**Core back-channel method pattern — clone-adapt `ElicitationManager::elicit_input` (lines 65-119):**

```rust
// TEMPLATE (src/server/elicitation.rs:67-119)
#[allow(clippy::cognitive_complexity)]
pub async fn elicit_input(&self, request: ElicitRequestParams) -> Result<ElicitResult> {
    let request_tx = self.request_tx.as_ref().ok_or_else(|| {
        Error::protocol(ErrorCode::INTERNAL_ERROR, "Elicitation not configured")
    })?;
    let (tx, rx) = oneshot::channel();
    let elicitation_id = Self::next_elicitation_id();
    {
        let mut pending = self.pending.write().await;
        pending.insert(elicitation_id.clone(), tx);
    }
    let server_request = ServerRequest::ElicitationCreate(Box::new(request));
    if let Err(e) = request_tx.send(server_request).await {
        self.pending.write().await.remove(&elicitation_id);
        return Err(Error::protocol(
            ErrorCode::INTERNAL_ERROR,
            format!("Failed to send elicitation request: {e}"),
        ));
    }
    match timeout(self.timeout_duration, rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => Err(Error::protocol(ErrorCode::INTERNAL_ERROR, "Elicitation channel closed")),
        Err(_) => {
            self.pending.write().await.remove(&elicitation_id);
            Err(Error::protocol(ErrorCode::REQUEST_TIMEOUT, "Elicitation request timed out"))
        },
    }
}
```

Adapt to `sample`: replace `ElicitRequestParams`→`CreateMessageParams`, `ElicitResult`→`CreateMessageResult`, `ServerRequest::ElicitationCreate`→`ServerRequest::CreateMessage` (already plumbed at `src/server/mod.rs:3769-3782` per research §2).

**list_roots adapter pattern — copy from `src/server/roots.rs:148-166`:**

```rust
pub async fn request_client_roots<F, Fut>(&self, request_sender: F) -> Result<ListRootsResult>
where
    F: FnOnce(ServerRequest) -> Fut,
    Fut: std::future::Future<Output = Result<serde_json::Value>>,
{
    let request = ServerRequest::ListRoots;
    let response = request_sender(request).await?;
    serde_json::from_value(response).map_err(|e| {
        crate::error::Error::protocol_msg(format!("Invalid roots response: {}", e))
    })
}
```

For `DispatchPeerHandle::list_roots`, inline-adapt: emit `ServerRequest::ListRoots`, await the correlated oneshot (same pattern as `elicit_input`), then `serde_json::from_value` on the returned `Value`.

**progress_notify — delegate, no new plumbing.** Just call `self.progress_reporter.as_ref().ok_or_else(..).report_progress(progress, total, message).await`. Mirror the guard from `RequestHandlerExtra::report_progress` at `src/server/cancellation.rs:239-250`:

```rust
// TEMPLATE (src/server/cancellation.rs:239-250)
pub async fn report_progress(
    &self,
    progress: f64,
    total: Option<f64>,
    message: Option<String>,
) -> crate::Result<()> {
    if let Some(rep) = &self.progress_reporter {
        rep.report_progress(progress, total, message).await
    } else {
        Ok(())  // no-op when no reporter — this is the no-op-without-token success criterion (70-02-03)
    }
}
```

---

### 5. `src/server/core.rs` (controller / dispatch) — MODIFY, 4 sites — Plan 02

**Analog:** self — the existing chain already uses the builder-call pattern that the new `.with_peer(...)` adds one line to.

**Site 1 — `handle_call_tool` at line 411-418 (VERIFIED by Read):**

```rust
// CURRENT
let request_id = format!("tool_{}", req.name);
let mut extra = RequestHandlerExtra::new(
    request_id.clone(),
    self.cancellation_manager.create_token(request_id.clone()).await,
)
.with_auth_context(auth_context)
.with_task_request(req.task.clone());
```

**Proposed:** append `.with_peer(self.build_peer_handle(session_id))` to the chain.

**Site 2 — `handle_get_prompt` at line 574-580:**

```rust
// CURRENT
let extra = RequestHandlerExtra::new(
    request_id.clone(),
    self.cancellation_manager.create_token(request_id.clone()).await,
)
.with_auth_context(auth_context);
```

**Proposed:** same `.with_peer(...)` append.

**Site 3 — `handle_list_resources` at line 594-600:** optional per research (low value).

**Site 4 — `handle_read_resource` at line 636-642:** append `.with_peer(...)`.

---

### 6. `src/server/mod.rs` (controller / dispatch, legacy path) — MODIFY, 5 sites — Plan 02

**Analog:** self. The site-5 pattern at lines 1055-1060 is the template:

```rust
// CURRENT (verified by Read)
let mut extra = crate::server::cancellation::RequestHandlerExtra::new(
    request_id.to_string(),
    cancellation_token,
)
.with_auth_context(validated_auth_context)
.with_progress_reporter(progress_reporter);
```

**Proposed:** append `.with_peer(self.build_peer_handle_from_notification_tx(session_id))` — leveraging the already-present `self.notification_tx` channel per research §3.

Sites to update: lines 1055, 1193, 1228 (optional), 1301, 1358.

---

### 7. `src/server/workflow/prompt_handler.rs` (test fixtures) — MODIFY, 12 struct-literal sites

**Analog:** self — site #1 at lines 1055-1064 (VERIFIED by Read) is the template for the other 11.

**Current (the 12 identical-shape sites):**

```rust
let extra = RequestHandlerExtra {
    cancellation_token: Default::default(),
    request_id: "test-1".to_string(),
    session_id: None,
    auth_info: None,
    auth_context: None,
    metadata: std::collections::HashMap::new(),
    progress_reporter: None,
    task_request: None,
};
```

**Option A (recommended by research §4):** Mark struct `#[non_exhaustive]` AND update each of the 12 sites to switch to:

```rust
let extra = RequestHandlerExtra::new(
    "test-1".to_string(),
    Default::default(),  // CancellationToken::default()
);
// Use builder methods if any non-defaults are needed — these 12 test sites
// currently spell out only defaults, so `::new()` alone suffices.
```

Mechanical edit at the 12 exact lines: 1055, 1238, 1347, 1441, 1573, 1704, 1772, 1851, 1948, 2081, 2223, 2338.

**Plan split:** Plan 01 handles 6 sites (first half), Plan 02 handles the remaining 6 after peer field lands — OR Plan 01 does all 12 at once with `#[non_exhaustive]` + `::new()` switch (cleanest; recommended).

---

### 8. `tests/handler_extensions_properties.rs` (proptest, NEW FILE) — Plan 01

**Analog:** `tests/property_tests.rs:1-57` for the `proptest!` block scaffolding; `tests/state_machine_properties.rs:1-80` for the module layout + `prop_compose!` generator style.

**Imports pattern — copy from `tests/property_tests.rs:1-9`:**

```rust
//! Property-based tests for RequestHandlerExtra.extensions typemap + Clone preservation.
//!
//! ALWAYS Requirement: Property tests for all new features

use pmcp::RequestHandlerExtra;
use proptest::prelude::*;
```

**Proptest-block pattern — copy shape from `tests/property_tests.rs:15-43`:**

```rust
// TEMPLATE
proptest! {
    #[test]
    fn property_jsonrpc_roundtrip(
        id in prop::option::of(any::<i64>().prop_map(RequestId::Number)),
        // ...
    ) {
        // setup
        let request = JSONRPCRequest { /* ... */ };
        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: JSONRPCRequest = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(request.jsonrpc, deserialized.jsonrpc);
    }
}
```

**Tests to write (per VALIDATION.md task ids):**

- `prop_extensions_insert_get_roundtrip` (70-01-02): arbitrary `(String, u64)` key/value inserted into `extra.extensions_mut()`, retrieved by type via `extra.extensions().get::<(String, u64)>()`, assert equal.
- `prop_extra_clone_preserves_extensions` (70-01-04): insert value; clone `extra`; assert retrieval works on the clone.

---

### 9. `tests/handler_peer_integration.rs` (integration, NEW FILE) — Plan 02

**Analog:** `tests/test_roots.rs:1-60` (in-process `Server::builder()` + `#[tokio::test]`). This is the closest integration-harness pattern since the research-mentioned `tests/typed_tool_transport_e2e.rs` does NOT exist in the repo.

**Imports + harness pattern — copy from `tests/test_roots.rs:1-31`:**

```rust
//! Integration test for PeerHandle session routing (T-70-02).

use pmcp::Server;

#[tokio::test]
async fn test_sample_session_routing() {
    let server_a = Server::builder()
        .name("peer-test-a")
        .version("1.0.0")
        .build()
        .unwrap();

    let server_b = Server::builder()
        .name("peer-test-b")
        .version("1.0.0")
        .build()
        .unwrap();

    // Register a tool on each server that calls extra.peer().unwrap().sample(..)
    // Fire sample() on server_a and assert it routes back only through server_a's
    // mpsc channel (not server_b).
}
```

**Assertion pattern — mirror `tests/test_roots.rs:26-31`:**

```rust
assert_eq!(roots.len(), 2);
assert_eq!(roots[0].uri, "file:///home/user/project1");
```

---

### 10. `fuzz/fuzz_targets/fuzz_peer_handle.rs` (fuzz, NEW FILE) — Plan 03

**Analog:** `fuzz/fuzz_targets/protocol_parsing.rs:1-55` (complete template — imports, `fuzz_target!` macro, `from_slice<Value>` try-parse chain).

**Pattern — copy from `fuzz/fuzz_targets/protocol_parsing.rs:1-20`:**

```rust
// TEMPLATE
#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::{
    CallToolRequest, CallToolResult,
    // ...
};
use serde_json::{Value, from_slice, from_value};

fuzz_target!(|data: &[u8]| {
    if let Ok(json) = from_slice::<Value>(data) {
        let _ = from_value::<CallToolRequest>(json.clone());
        let _ = from_value::<CallToolResult>(json.clone());
        // ...
    }
});
```

**Proposed for `fuzz_peer_handle.rs`:**

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use pmcp::types::{CreateMessageParams, CreateMessageResult, ListRootsResult};
use serde_json::{Value, from_slice, from_value};

fuzz_target!(|data: &[u8]| {
    if let Ok(json) = from_slice::<Value>(data) {
        let _ = from_value::<CreateMessageParams>(json.clone());
        let _ = from_value::<CreateMessageResult>(json.clone());
        let _ = from_value::<ListRootsResult>(json.clone());
    }
});
```

---

### 11. `fuzz/Cargo.toml` (config registration) — MODIFY

**Analog:** `fuzz/Cargo.toml:27-39` — every existing `[[bin]]` block follows the same shape.

**Pattern to append:**

```toml
[[bin]]
name = "fuzz_peer_handle"
path = "fuzz_targets/fuzz_peer_handle.rs"
test = false
doc = false
bench = false
```

---

### 12. `examples/s42_handler_extensions.rs` (example, NEW FILE) — Plan 03

**Analog:** `examples/s16_typed_tools.rs:1-80` (imports + struct-style) + `examples/s30_tool_with_sampling.rs:195-229` (main-fn scaffolding).

**Import + preamble pattern — copy from `examples/s16_typed_tools.rs:1-17`:**

```rust
//! Example demonstrating RequestHandlerExtra.extensions typemap for
//! cross-middleware state transfer.
//!
//! Run with: cargo run --example s42_handler_extensions

use anyhow::Result;
use async_trait::async_trait;
use pmcp::{RequestHandlerExtra, Server, ServerCapabilities, ToolHandler};
use serde_json::{json, Value};
```

**Main fn scaffold pattern — copy from `examples/s30_tool_with_sampling.rs:195-229`:**

```rust
// TEMPLATE
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let server = Server::builder()
        .name("handler-extensions-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("inspect_extensions", InspectExtensionsTool)
        .build()?;

    server.run_stdio().await
}
```

**ToolHandler impl pattern — copy from `examples/s30_tool_with_sampling.rs:25-57`:**

```rust
#[async_trait]
impl ToolHandler for InspectExtensionsTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Middleware previously inserted a typed value:
        // extra.extensions_mut().insert(MyRequestContext { user_id: 42 });
        // Now retrieve it here:
        let ctx = extra.extensions().get::<MyRequestContext>();
        Ok(json!({ "content": [{"type":"text","text": format!("ctx={:?}", ctx)}], "isError": false }))
    }
}
```

---

### 13. `examples/s43_handler_peer_sample.rs` (example, NEW FILE) — Plan 03

**Analog:** `examples/s30_tool_with_sampling.rs` in its entirety — explicitly contrast against it per research §8 to highlight the new "call peer.sample() from inside the handler" ergonomics.

**Contrast demonstration — mirror `examples/s30_tool_with_sampling.rs:25-57` but replace `simulate_llm_summarization` with a real `peer.sample()` call:**

```rust
#[async_trait]
impl ToolHandler for SummarizeViaPeerTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        let text = args.get("text").and_then(|v| v.as_str())
            .ok_or_else(|| Error::validation("Missing 'text'"))?;

        let peer = extra.peer()
            .ok_or_else(|| Error::protocol_msg("peer not available — non-wasm server path only"))?;

        let result = peer.sample(CreateMessageParams {
            messages: vec![/* ... Summarize this: <text> ... */],
            // ...
        }).await?;

        Ok(json!({ "content": [{"type":"text","text": result.content_text()}], "isError": false }))
    }
}
```

---

### 14. `Cargo.toml` (example registration) — MODIFY

**Analog:** `Cargo.toml:500-503` — existing `s41_code_mode_graphql` entry (immediately above the `[[bench]]` block).

**Current block (VERIFIED by Read at 500-503):**

```toml
[[example]]
name = "s41_code_mode_graphql"
path = "examples/s41_code_mode_graphql.rs"
required-features = ["full"]

[[bench]]
name = "comprehensive_benchmarks"
```

**Proposed — insert two new entries between the s41 block and `[[bench]]`:**

```toml
[[example]]
name = "s42_handler_extensions"
path = "examples/s42_handler_extensions.rs"

[[example]]
name = "s43_handler_peer_sample"
path = "examples/s43_handler_peer_sample.rs"
```

No `required-features` needed unless the examples use schema-generation (per research: keep them minimal).

---

## Shared Patterns (apply to multiple files)

### Shared Pattern A: `#[async_trait]` on object-safe traits

**Source:** `src/server/elicitation.rs:199` (`#[async_trait::async_trait] pub trait ElicitInput`), `src/server/mod.rs:200-255` (ToolHandler/PromptHandler/ResourceHandler/SamplingHandler all use `#[async_trait]`).

**Apply to:** new `PeerHandle` trait in `src/shared/peer.rs`.

**Canonical excerpt:**

```rust
#[async_trait]
pub trait PeerHandle: Send + Sync {
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult>;
    // ...
}
```

### Shared Pattern B: mpsc::Sender + oneshot::Sender + tokio::time::timeout (back-channel RPC)

**Source:** `src/server/elicitation.rs:21-119` (full reference impl).

**Apply to:** `DispatchPeerHandle::sample` and `DispatchPeerHandle::list_roots` in `src/server/peer_impl.rs`.

**Canonical excerpt:**

```rust
let (tx, rx) = oneshot::channel();
let correlation_id = /* ... */;
self.pending.write().await.insert(correlation_id.clone(), tx);
request_tx.send(server_request).await?;
match timeout(self.timeout_duration, rx).await {
    Ok(Ok(response)) => Ok(response),
    Ok(Err(_)) => Err(Error::protocol(ErrorCode::INTERNAL_ERROR, "channel closed")),
    Err(_) => Err(Error::protocol(ErrorCode::REQUEST_TIMEOUT, "timed out")),
}
```

### Shared Pattern C: `RequestHandlerExtra::new(...).with_xxx(...).with_yyy(...)` builder chain

**Source:** `src/server/core.rs:411-418` (the cleanest chain), mirrored 8 more times in `src/server/core.rs` and `src/server/mod.rs`.

**Apply to:** all 9 dispatch sites — append `.with_peer(...)` to each (Plan 02).

**Canonical excerpt:**

```rust
let extra = RequestHandlerExtra::new(request_id.clone(), cancellation_token)
    .with_auth_context(auth_context)
    .with_task_request(req.task.clone())
    .with_peer(self.build_peer_handle(session_id));  // NEW
```

### Shared Pattern D: `#[cfg(not(target_arch = "wasm32"))]` field/method gating

**Source:** `src/shared/cancellation.rs:49-50` (gates `auth_context` field), `src/shared/cancellation.rs:79-86` (gates `with_auth_context` method).

**Apply to:** every `peer`-related field, method, trait, and impl in Plan 02. The `extensions` field in Plan 01 is NOT gated (works on wasm too, per research §1 & §6).

**Canonical excerpt:**

```rust
#[cfg(not(target_arch = "wasm32"))]
pub auth_context: Option<crate::server::auth::AuthContext>,
// ...
#[cfg(not(target_arch = "wasm32"))]
pub fn with_auth_context(
    mut self,
    auth_context: Option<crate::server::auth::AuthContext>,
) -> Self {
    self.auth_context = auth_context;
    self
}
```

### Shared Pattern E: Debug-redaction on sensitive fields

**Source:** `src/server/cancellation.rs:296-335` (full redaction logic with `SENSITIVE_KEYS` list).

**Apply to:** Debug impl update for the new `extensions` field (print type names only — already the `http::Extensions` default per research §1) and the new `peer` field (print `"Arc<dyn PeerHandle>"` placeholder — not the inner handle).

**Canonical excerpt (tail):**

```rust
.field("metadata", &redacted_metadata)
.field("task_request", &self.task_request.is_some())
// NEW
.field("extensions", &self.extensions)  // http::Extensions Debug prints type names only
.field("peer", &self.peer.as_ref().map(|_| "Arc<dyn PeerHandle>"))
.finish()
```

### Shared Pattern F: `#[tokio::test]` unit-test with `Server::builder()` in-process fixture

**Source:** `tests/test_roots.rs:5-31` (complete template).

**Apply to:** `tests/handler_peer_integration.rs` (new integration harness).

**Canonical excerpt:**

```rust
#[tokio::test]
async fn test_server_roots_registration() {
    let server = Server::builder()
        .name("test-server")
        .version("1.0.0")
        .build()
        .unwrap();
    // ...
    let roots = server.get_roots().await;
    assert_eq!(roots.len(), 2);
}
```

### Shared Pattern G: no-op-when-reporter-absent (progress guard)

**Source:** `src/server/cancellation.rs:239-250` (`RequestHandlerExtra::report_progress`).

**Apply to:** `DispatchPeerHandle::progress_notify` — satisfies success criterion 70-02-03 (no-op without reporter).

**Canonical excerpt:**

```rust
if let Some(rep) = &self.progress_reporter {
    rep.report_progress(progress, total, message).await
} else {
    Ok(())  // silent no-op — the documented behavior
}
```

---

## No Analog Found

None. Every new file has a close in-repo analog per the table above. Two clarifications:

1. The `tests/typed_tool_transport_e2e.rs` file referenced in research §7 does NOT exist in the repo — it was misremembered. The closest real harness is `tests/test_roots.rs` (in-process `Server::builder()` + `#[tokio::test]`), which is the correct analog to use for `tests/handler_peer_integration.rs`.
2. The existing proptest scaffolding style comes from `tests/property_tests.rs` + `tests/state_machine_properties.rs` — neither perfectly matches the RequestHandlerExtra domain, but both are sufficient templates for the `proptest!` macro shape and module layout.

---

## Metadata

**Analog search scope:** `src/server/`, `src/shared/`, `tests/`, `fuzz/fuzz_targets/`, `examples/`, `Cargo.toml`, `fuzz/Cargo.toml`
**Files scanned (Read/Grep):** 12 distinct files plus 3 directory listings
**Pattern extraction date:** 2026-04-16

---

## PATTERN MAPPING COMPLETE

**Phase:** 70 — Add Extensions typemap and peer back-channel to RequestHandlerExtra
**Files classified:** 14 distinct edit locations (7 new + 5 modified + 2 registration-config)
**Analogs found:** 14 / 14

### Coverage

- Files with exact analog (same role + same data flow, often in-place self-extension): 10
- Files with role-match analog (same role, slightly different data flow): 4
- Files with no analog: 0

### Key Patterns Identified

1. **`RequestHandlerExtra` struct extension follows in-place add-field-plus-builder-method pattern** — every existing field (auth_info, auth_context, progress_reporter, task_request) uses the same shape (`pub field: Option<T>` + `pub fn with_field(mut self, ...) -> Self`). The new `extensions` and `peer` fields copy this exact shape.
2. **`PeerHandle` has a complete in-repo blueprint in `ElicitationManager`** — mpsc::Sender + HashMap of oneshot::Sender (keyed by correlation ID) + `tokio::time::timeout`. `DispatchPeerHandle::sample` clone-adapts `ElicitationManager::elicit_input` with `ElicitRequestParams`→`CreateMessageParams` substitution.
3. **All 9 dispatch sites use the identical `RequestHandlerExtra::new(..).with_xxx(..)` builder chain** — appending `.with_peer(..)` is a one-line mechanical edit per site.
4. **12 struct-literal test sites in `prompt_handler.rs` are ALL identical-shape** (first site at line 1055 is the template) — switching them to `::new()` + `#[non_exhaustive]` on the struct is the cleanest recommendation.
5. **wasm-gating convention is well-established** (`#[cfg(not(target_arch = "wasm32"))]` on field/method/impl) — peer field follows the same convention; extensions field deliberately skips gating since `http::Extensions` compiles on wasm32.

### File Created

`.planning/phases/70-add-extensions-typemap-and-peer-back-channel-to-requesthandl/70-PATTERNS.md`

### Ready for Planning

Pattern mapping complete. Planner can now reference analog file paths (`src/server/elicitation.rs:21-119`, `src/server/core.rs:411-418`, `src/server/cancellation.rs:117-335`, `tests/test_roots.rs:1-31`, `tests/property_tests.rs:1-57`, `examples/s30_tool_with_sampling.rs:25-229`, `fuzz/fuzz_targets/protocol_parsing.rs:1-55`, `Cargo.toml:500-503`, `fuzz/Cargo.toml:27-39`) and concrete code excerpts in each plan's `<read_first>` and `<action>` blocks.
