# Phase 73: Typed client helpers + list_all pagination (PARITY-CLIENT-01) - Pattern Map

**Mapped:** 2026-04-21
**Files analyzed:** 10 (4 new + 6 modified)
**Analogs found:** 10 / 10 (100%)
**Anchor:** pmcp @ 2.5.0 HEAD of `main`

All anchors verified against `/Users/guy/Development/mcp/sdk/rust-mcp-sdk` HEAD.

---

## File Classification

| File | New/Modify | Role | Data Flow | Closest Analog | Match Quality |
|------|-----------|------|-----------|----------------|---------------|
| `src/client/options.rs` | create | config struct module | config (static) | `src/types/tools.rs:18-20` (`#[non_exhaustive]` struct) + `src/shared/protocol.rs` `ProtocolOptions` | role-match (new config type, mirroring existing `ProtocolOptions` pattern) |
| `tests/list_all_pagination.rs` | create | integration test | request-response (paginated loop) | `tests/oauth_dcr_integration.rs:1-40` (integration-test scaffolding) + `src/client/mod.rs:1987-2027` (`MockTransport`-driven flow) | role-match |
| `fuzz/fuzz_targets/list_all_cursor_loop.rs` | create | fuzz target | adversarial input | `fuzz/fuzz_targets/dcr_response_parser.rs:1-17` (minimal) + `fuzz/fuzz_targets/jsonrpc_handling.rs:1-147` (Arbitrary-derive) | exact (minimal) |
| `examples/09_typed_client_helpers.rs` | create | example | request-response | `examples/c02_client_tools.rs:1-131` | exact |
| `src/client/mod.rs` | modify | client impl + tests | request-response + pagination loop | self — add to existing `impl<T: Transport> Client<T>`; test-module mirrors existing `mod tests` at `src/client/mod.rs:1833-2027` | exact (same file) |
| `src/lib.rs` | modify | re-export | static | self — `src/lib.rs:54` existing `pub use client::{Client, ClientBuilder, ToolCallResponse};` | exact |
| `Cargo.toml` (root) | modify | version bump | static | self — `Cargo.toml:3` | exact |
| `CHANGELOG.md` | modify | docs | static | self — `CHANGELOG.md:8-41` Phase 74 entry | exact |
| `crates/mcp-tester/Cargo.toml`, `cargo-pmcp/Cargo.toml`, plus `examples/25-oauth-basic/Cargo.toml`, `examples/test-basic/Cargo.toml`, `crates/pmcp-tasks/Cargo.toml`, `crates/pmcp-server/Cargo.toml`, `crates/pmcp-server/pmcp-server-lambda/Cargo.toml` | modify | dep-pin bump | static | self — grep returned 7 files pinning `pmcp = "2.5.0"` (see Grep output below) | exact |
| `README.md` | modify | docs (feature list) | static | `README.md:211-222` "Key Features" bullets | exact |

---

## Pattern Assignments

### 1. `src/client/options.rs` (new module — `ClientOptions` config struct)

**Analog A — `#[non_exhaustive]` struct pattern:** `src/types/tools.rs:18-20`

```rust
// Source: src/types/tools.rs:18-22 [VERIFIED]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Human-readable title for the tool
    #[serde(skip_serializing_if = "Option::is_none")]
```

**Analog B — plain `Debug + Clone + Default` config struct:** `src/shared/protocol.rs` — `ProtocolOptions` (field-update idiom used at `src/client/mod.rs:146-152`):

```rust
// Source: src/client/mod.rs:146-152 [VERIFIED] — usage pattern for ClientOptions callers
let options = ProtocolOptions {
    enforce_strict_capabilities: false,
    debounced_notification_methods: vec![
        "notifications/progress".to_string(),
        "notifications/message".to_string(),
    ],
};
```

**Pattern to mirror — the complete new module body (≤40 lines):**

```rust
//! Client configuration options.
//!
//! [`ClientOptions`] is the additive surface for configuring a [`crate::Client`]
//! beyond the protocol-level [`crate::shared::ProtocolOptions`]. This type is
//! marked `#[non_exhaustive]` so future knobs (see Phase 73 deferred ideas —
//! StrictMode / typed-output) can be added without a breaking change.

/// Client-level configuration.
///
/// Constructed via [`ClientOptions::default`] + field-update syntax. From outside
/// the `pmcp` crate the struct literal is forbidden by `#[non_exhaustive]`, so
/// callers must always spread `..Default::default()`.
///
/// # Examples
///
/// ```rust
/// use pmcp::ClientOptions;
///
/// let opts = ClientOptions { max_iterations: 50, ..Default::default() };
/// assert_eq!(opts.max_iterations, 50);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ClientOptions {
    /// Maximum number of pagination iterations `list_all_*` helpers will make
    /// before returning [`crate::Error::Validation`]. Default: 100.
    pub max_iterations: usize,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self { max_iterations: 100 }
    }
}
```

**Callers / downstream:**
- `src/client/mod.rs` — adds `options: ClientOptions` field to `Client<T>` struct (after line 70), initializes in every constructor (`new`, `with_info`, `with_options`, and new `with_client_options`), and threads through `Clone` impl at `src/client/mod.rs:1815-1831`.
- `src/lib.rs:54` — add `ClientOptions` to existing `pub use client::{…}` re-export.
- Every rustdoc doctest for `list_all_*` that overrides `max_iterations` MUST use `ClientOptions { max_iterations: N, ..Default::default() }` per Landmine #7.

---

### 2. `src/client/mod.rs` — new methods + new constructor + test modules

#### 2a. New constructor — resolve `with_options` collision (Open Q1)

**Existing (collision target):** `src/client/mod.rs:159-177`

```rust
// Source: src/client/mod.rs:159-177 [VERIFIED] — DO NOT overload this name
pub fn with_options(
    transport: T,
    client_info: Implementation,
    options: ProtocolOptions,
) -> Self {
    Self {
        transport: Arc::new(RwLock::new(transport)),
        protocol: Arc::new(RwLock::new(Protocol::new(options))),
        middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
        capabilities: None,
        server_capabilities: None,
        server_version: None,
        instructions: None,
        initialized: false,
        info: client_info,
        notification_tx: None,
        active_requests: Arc::new(RwLock::new(HashMap::new())),
    }
}
```

**Analog to mirror (neighboring constructor):** `src/client/mod.rs:121-135` `with_info`:

```rust
// Source: src/client/mod.rs:121-135 [VERIFIED] — template for new constructor
pub fn with_info(transport: T, client_info: Implementation) -> Self {
    Self {
        transport: Arc::new(RwLock::new(transport)),
        protocol: Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default()))),
        middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
        capabilities: None,
        server_capabilities: None,
        server_version: None,
        instructions: None,
        initialized: false,
        info: client_info,
        notification_tx: None,
        active_requests: Arc::new(RwLock::new(HashMap::new())),
    }
}
```

**Recommended name:** `Client::with_client_options(transport: T, options: ClientOptions) -> Self` (matches `with_info` / `with_options` sibling pattern; parallel adjective).

Existing `new`, `with_info`, `with_options`, and `ClientBuilder::build` (line 1803-1812) all need to initialize the new `options` field to `ClientOptions::default()` inside their struct literal. `Clone` impl at line 1815-1831 must also include the new field.

#### 2b. Typed call helpers — `call_tool_typed` / `_with_task` / `_and_poll` / `get_prompt_typed`

**Analog (delegate target):** `src/client/mod.rs:416-441` (`call_tool`) — full body shown in RESEARCH.md §"Code Examples".

**Pattern to mirror — thin serialize-and-delegate:**

```rust
// New method; delegate to existing call_tool at src/client/mod.rs:416-441
pub async fn call_tool_typed<A: Serialize + ?Sized>(
    &self,
    name: impl Into<String>,
    args: &A,
) -> Result<CallToolResult> {
    let value = serde_json::to_value(args).map_err(|e| {
        Error::validation(format!("call_tool_typed arguments: {e}"))
    })?;
    self.call_tool(name.into(), value).await
}
```

Same shape for:
- `call_tool_typed_with_task` → delegates to `call_tool_with_task` at `src/client/mod.rs:463-497`
- `call_tool_typed_and_poll` → delegates to `call_tool_and_poll` at `src/client/mod.rs:620-697` (carries extra `max_polls: usize` arg)
- `get_prompt_typed` → delegates to `get_prompt` at `src/client/mod.rs:825-849` BUT must apply D-06 per-variant coercion before building the `HashMap<String, String>`:

```rust
// Per CONTEXT.md D-06 — Value → HashMap<String, String>
pub async fn get_prompt_typed<A: Serialize + ?Sized>(
    &self,
    name: impl Into<String>,
    args: &A,
) -> Result<GetPromptResult> {
    let value = serde_json::to_value(args).map_err(|e| {
        Error::validation(format!("get_prompt_typed arguments: {e}"))
    })?;
    let obj = match value {
        serde_json::Value::Object(map) => map,
        _ => return Err(Error::validation(
            "prompts/get arguments must serialize to a JSON object",
        )),
    };
    let mut out: HashMap<String, String> = HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        match v {
            serde_json::Value::Null => continue,
            serde_json::Value::String(s) => { out.insert(k, s); }
            serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
                out.insert(k, v.to_string());
            }
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                let nested = serde_json::to_string(&v).map_err(|e| {
                    Error::validation(format!("get_prompt_typed nested arg {k}: {e}"))
                })?;
                out.insert(k, nested);
            }
        }
    }
    self.get_prompt(name.into(), out).await
}
```

**Pitfall guard (Pitfall 1):** DO NOT use blanket `v.to_string()` for string values — `Value::String("hi").to_string()` returns `"\"hi\""` (quoted). Strings must be extracted via the `Value::String(s)` arm and inserted as the inner `String` directly.

#### 2c. `list_all_*` auto-pagination helpers (four methods)

**Analog (delegate target per method):**

| New method | Single-page delegate | Source |
|------------|----------------------|--------|
| `list_all_tools()` → `Vec<ToolInfo>` | `list_tools(cursor)` | `src/client/mod.rs:339-357` (`.tools`, `.next_cursor`) |
| `list_all_prompts()` → `Vec<PromptInfo>` | `list_prompts(cursor)` | `src/client/mod.rs:749-767` (`.prompts`, `.next_cursor`) |
| `list_all_resources()` → `Vec<ResourceInfo>` | `list_resources(cursor)` | `src/client/mod.rs:891-909` (`.resources`, `.next_cursor`) |
| `list_all_resource_templates()` → `Vec<ResourceTemplate>` | `list_resource_templates(cursor)` | `src/client/mod.rs:948-969` (`.resource_templates`, `.next_cursor`) |

**Pattern to mirror — bounded cursor loop (per Pitfall 2, terminate only on `None`):**

```rust
// Loops over list_tools at src/client/mod.rs:339-357
pub async fn list_all_tools(&self) -> Result<Vec<ToolInfo>> {
    let cap = self.options.max_iterations;
    let mut out: Vec<ToolInfo> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..cap {
        let page = self.list_tools(cursor).await?;
        out.extend(page.tools);
        match page.next_cursor {
            None => return Ok(out),
            Some(next) => cursor = Some(next),
        }
    }
    Err(Error::validation(format!(
        "list_all_tools exceeded max_iterations cap of {cap} pages"
    )))
}
```

Same body for the other three — swap `list_tools`/`.tools`/`ToolInfo` for the per-method triple.

**Anti-pattern (see RESEARCH.md):** do NOT re-implement `send_request`/`ResponsePayload` inside `list_all_*` — always delegate; do NOT treat `Some("")` as terminal.

#### 2d. In-file test module for typed helpers + list_all

**Analog (test module skeleton):** `src/client/mod.rs:1833-1899` (`mod tests` with `MockTransport`):

```rust
// Source: src/client/mod.rs:1833-1891 [VERIFIED] — reusable MockTransport
#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Transport;
    use crate::types::{
        jsonrpc::{JSONRPCError, ResponsePayload},
        JSONRPCResponse, ProgressNotification, ProgressToken, TransportMessage,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct MockTransport {
        responses: Arc<Mutex<Vec<TransportMessage>>>,
        sent_messages: Arc<Mutex<Vec<TransportMessage>>>,
    }

    impl MockTransport {
        fn new() -> Self { /* ... */ }
        fn with_responses(responses: Vec<TransportMessage>) -> Self { /* ... */ }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.sent_messages.lock().unwrap().push(message);
            Ok(())
        }
        async fn receive(&mut self) -> Result<TransportMessage> {
            self.responses.lock().unwrap().pop()
                .ok_or_else(|| Error::protocol_msg("No more responses"))
        }
        async fn close(&mut self) -> Result<()> { Ok(()) }
    }
    // ... existing #[tokio::test]s follow
}
```

**Analog (single-test body for `list_tools`):** `src/client/mod.rs:1987-2027`:

```rust
// Source: src/client/mod.rs:1987-2027 [VERIFIED] — template for list_all_tools tests
#[tokio::test]
async fn test_list_tools() {
    let init_response = TransportMessage::Response(JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id: RequestId::from(1i64),
        payload: ResponsePayload::Result(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "test-server", "version": "1.0.0" }
        })),
    });
    let tools_response = TransportMessage::Response(JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id: RequestId::from(2i64),
        payload: ResponsePayload::Result(json!({
            "tools": [{ "name": "test-tool", "description": "Test tool", "inputSchema": {} }]
        })),
    });
    let transport = MockTransport::with_responses(vec![tools_response, init_response]);
    let mut client = Client::new(transport);
    let _ = client.initialize(ClientCapabilities::minimal()).await;
    let result = client.list_tools(None).await;
    assert!(result.is_ok());
    let tools = result.unwrap();
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name, "test-tool");
}
```

**Critical:** `MockTransport::with_responses` pops from the **end** of the `Vec` (line 1884, `responses.lock().unwrap().pop()` = `Vec::pop` = last element). Therefore the response sequence must be **pushed in REVERSE ORDER** — see line 2015 where `vec![tools_response, init_response]` yields `init` first, then `tools`. For N-page `list_all` tests: push pages as `vec![page_n, ..., page_1, init_response]`.

**Callers / downstream:** may optionally extend `MockTransport` with a helper `fn with_paginated_tools(pages: Vec<Vec<ToolInfo>>) -> Self` (per RESEARCH.md Validation Architecture Wave 0) to avoid hand-spelling the reverse-ordered response vec per test.

---

### 3. `src/lib.rs` — re-export `ClientOptions`

**Analog:** `src/lib.rs:54` existing line:

```rust
// Source: src/lib.rs:54 [VERIFIED]
pub use client::{Client, ClientBuilder, ToolCallResponse};
```

**Pattern to mirror:** change to

```rust
pub use client::{Client, ClientBuilder, ClientOptions, ToolCallResponse};
```

**Callers / downstream:** every new rustdoc/example/doctest that references `ClientOptions` relies on this pub-use.

---

### 4. `tests/list_all_pagination.rs` (new integration test file)

**Analog A — integration-test scaffolding:** `tests/oauth_dcr_integration.rs:1-32` (shown above).

**Analog B — `MockTransport`-driven client flow:** `src/client/mod.rs:1987-2027` (full `test_list_tools` body shown above). Note: the integration test lives **outside** `src/client/mod.rs`, so `MockTransport` needs either (a) be marked `pub` in a `#[cfg(any(test, feature = "test-utils"))]` guard or (b) be duplicated inside `tests/list_all_pagination.rs`. The RESEARCH.md recommendation at §"Wave 0 Gaps" — "all unit + integration tests can live in the existing `src/client/mod.rs` `mod tests` block" — means **this file may be unnecessary**; default to co-locating all pagination tests in `src/client/mod.rs` `mod tests`. If a separate file is kept, duplicate a minimal `MockTransport` inline (≈40 LOC).

**Pattern to mirror (if kept):**

```rust
//! Integration tests for Client::list_all_* auto-pagination.
//!
//! Drives a MockTransport with a scripted multi-page response sequence and
//! asserts: (a) aggregation across pages preserves order, (b) termination on
//! next_cursor: None, (c) max_iterations cap enforcement.
#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;
use pmcp::{
    shared::Transport,
    types::{jsonrpc::ResponsePayload, JSONRPCResponse, RequestId, TransportMessage},
    Client, ClientCapabilities, ClientOptions, Error, Result,
};
use serde_json::json;
use std::sync::{Arc, Mutex};

// ... MockTransport copy from src/client/mod.rs:1847-1891 ...

#[tokio::test]
async fn list_all_tools_aggregates_three_pages() { /* ... */ }

#[tokio::test]
async fn list_all_tools_terminates_on_none_cursor() { /* ... */ }

#[tokio::test]
async fn list_all_tools_rejects_on_max_iterations_exceeded() { /* ... */ }
```

---

### 5. `fuzz/fuzz_targets/list_all_cursor_loop.rs` (new fuzz target)

**Analog A — minimal structure:** `fuzz/fuzz_targets/dcr_response_parser.rs:1-17` (shown above).

**Analog B — Arbitrary-derive for structured adversarial inputs:** `fuzz/fuzz_targets/jsonrpc_handling.rs:1-147`.

**Pattern to mirror:**

```rust
//! Fuzz target: Client::list_all_tools cursor loop.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run list_all_cursor_loop`.
//!
//! Invariants:
//!   1. Loop MUST terminate within ClientOptions::max_iterations for any
//!      adversarial cursor sequence (empty strings, very long strings,
//!      repeated values, A->B->A cycles).
//!   2. Cap-exceeded MUST yield Error::Validation, never panic.
//!   3. None-cursor MUST terminate cleanly with Ok(accumulator).

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
struct FuzzCursorSeq {
    // Each entry: Some(next) continues, None terminates.
    cursors: Vec<Option<String>>,
    max_iterations: u8, // clamped 1..=200 inside target
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(seq) = FuzzCursorSeq::arbitrary(&mut u) else { return };
    // Build scripted MockTransport, drive list_all_tools, assert
    // Result is Ok(_) OR Err(Error::Validation(_)) — never panic, never
    // any other Error variant.
    // ...
});
```

**Registration:** add `[[bin]] name = "list_all_cursor_loop"` stanza to `fuzz/Cargo.toml` mirroring lines 92-97:

```toml
# Source: fuzz/Cargo.toml:92-97 [VERIFIED]
[[bin]]
name = "dcr_response_parser"
path = "fuzz_targets/dcr_response_parser.rs"
test = false
doc = false
bench = false
```

**Callers / downstream:** the fuzz target depends on `MockTransport` being visible. RESEARCH.md Open Q3 flags this as optional; the pragmatic choice is a self-contained `MockTransport` inlined in the fuzz target.

---

### 6. Property-test extensions (either `tests/property_tests.rs` extend or new file)

**Analog:** `tests/property_tests.rs:1-80` (shown above) — `use proptest::prelude::*;` + `proptest! { #[test] fn property_* (...) { ... } }` macro block.

**Pattern to mirror:**

```rust
// In tests/property_tests.rs (extend) OR tests/client_parity_properties.rs (new)
use pmcp::{Client, ClientOptions, Error};
use proptest::prelude::*;

proptest! {
    /// D-01 delegation-equivalence: call_tool_typed(name, &arg) produces the
    /// same wire request as call_tool(name, serde_json::to_value(&arg).unwrap()).
    #[test]
    fn prop_call_tool_typed_delegation(
        name in "[a-zA-Z_][a-zA-Z0-9_]{0,32}",
        args in prop::collection::hash_map("[a-z]{1,8}", any::<i32>(), 0..5),
    ) {
        // ... drive two MockTransports, compare sent_messages ...
    }

    /// D-12 flat-concatenation: for any paginated sequence of ToolInfo vecs
    /// (N <= cap), list_all_tools returns their in-order concatenation.
    #[test]
    fn prop_list_all_tools_flat_concatenation(
        pages in prop::collection::vec(
            prop::collection::vec("[a-z]{1,8}", 0..3),
            1..10,
        ),
    ) {
        // ... drive MockTransport with scripted next_cursor chain ...
    }

    /// D-10 cap enforcement: server emitting cap+1 pages with Some(_) cursors
    /// MUST produce Error::Validation.
    #[test]
    fn prop_list_all_tools_cap_enforced(
        cap in 1usize..20,
    ) {
        // ... always-Some cursor stream of length cap+1 ...
    }
}
```

---

### 7. `examples/09_typed_client_helpers.rs` (new — filename per Landmine #2)

**Analog:** `examples/c02_client_tools.rs:1-131` (full body shown above).

**Pattern to mirror (preamble + shape):**

```rust
//! Example: Typed call helpers and auto-paginating list helpers.
//!
//! This example demonstrates the Phase 73 additions:
//! - `Client::call_tool_typed` with a #[derive(Serialize)] struct
//! - `Client::list_all_tools` across a multi-page server
//! - `Client::get_prompt_typed` with struct-to-HashMap coercion
//! - Custom `ClientOptions { max_iterations: 50, ..Default::default() }`
//!
//! Filename note: numeric prefix `09` rather than `c09` to match the phase
//! prompt; if the planner aligns with the c-series convention it should
//! become `c09_client_list_all.rs` (c08 is already `c08_oauth_dcr.rs`).

use pmcp::{Client, ClientCapabilities, ClientOptions, StdioTransport};
use serde::Serialize;

#[derive(Serialize)]
struct Search { query: String, limit: u32 }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("pmcp=info").init();

    let transport = StdioTransport::new();
    let opts = ClientOptions { max_iterations: 50, ..Default::default() };
    let mut client = Client::with_client_options(transport, opts);
    client.initialize(ClientCapabilities::minimal()).await?;

    // Typed call_tool — no manual json!() boilerplate
    let _ = client.call_tool_typed(
        "search",
        &Search { query: "rust mcp".into(), limit: 10 },
    ).await?;

    // Auto-paginating list
    let all_tools = client.list_all_tools().await?;
    println!("tools total: {}", all_tools.len());

    Ok(())
}
```

**Registration:** add `[[example]]` stanza to root `Cargo.toml` mirroring lines 229-231:

```toml
# Source: Cargo.toml:229-231 [VERIFIED]
[[example]]
name = "c02_client_tools"
path = "examples/c02_client_tools.rs"
```

**README entry:** add to `examples/README.md` under `### Tools, Resources, Prompts` (after `c04_client_prompts` block at lines 280-283). Also add `c08_oauth_dcr` which is currently un-registered in README — optional housekeeping.

**Filename recommendation:** per RESEARCH.md Landmine #2, prefer `c09_client_list_all.rs` for c-series consistency. The phase prompt's `09_typed_client_helpers.rs` (no `c` prefix) breaks the role-prefix convention documented at `examples/README.md:7-16`. **Planner should use `c09_client_list_all.rs`.**

---

### 8. `Cargo.toml` (root) — pmcp version bump

**Analog:** `Cargo.toml:3`:

```toml
# Source: Cargo.toml:3 [VERIFIED]
version = "2.5.0"
```

**Pattern:** bump to `version = "2.6.0"` — additive-only minor per RESEARCH.md Breaking-Change Check.

---

### 9. Dep-pin bumps in downstream `Cargo.toml`s

**Grep result (verbatim) — all 7 files pinning `pmcp = "2.5.0"`:**

```
examples/25-oauth-basic/Cargo.toml:24: pmcp = { version = "2.5.0", path = "../../", features = ["streamable-http"] }
cargo-pmcp/Cargo.toml:38:              pmcp = { version = "2.5.0", path = "..", features = ["streamable-http", "oauth"] }
examples/test-basic/Cargo.toml:13:     pmcp = { version = "2.5.0", path = "../../", features = ["http"] }
crates/pmcp-tasks/Cargo.toml:10:       pmcp = { version = "2.5.0", path = "../..", default-features = false }
crates/pmcp-tasks/Cargo.toml:32:       pmcp = { version = "2.5.0", path = "../..", features = ["full"] }
crates/pmcp-server/Cargo.toml:30:      pmcp = { version = "2.5.0", path = "../../", features = ["streamable-http"] }
crates/pmcp-server/pmcp-server-lambda/Cargo.toml:17: pmcp = { version = "2.5.0", path = "../../..", features = ["streamable-http"] }
crates/mcp-tester/Cargo.toml:21:       pmcp = { version = "2.5.0", path = "../../", features = ["streamable-http", "oauth"] }
```

**Pattern to mirror:** replace `"2.5.0"` → `"2.6.0"` in all eight lines (seven files). Per CONTEXT.md "Integration Points" and RESEARCH.md §"Installation": these crates don't exercise the new APIs, so dep-pin bump is mechanical and may be deferred — but the CLAUDE.md Release Workflow rule says "Downstream crates that pin a bumped dependency must also be bumped." Planner should include these in Plan 3 (release coordination).

The phase prompt lists only `crates/mcp-tester/Cargo.toml` and `cargo-pmcp/Cargo.toml` explicitly; the other five are easy-to-miss — **flag them to the planner.**

---

### 10. `CHANGELOG.md` — new `## [2.6.0]` entry

**Analog:** `CHANGELOG.md:8-32` (Phase 74 / 2.5.0 entry):

```markdown
## [2.5.0] - 2026-04-21

### Added

- **pmcp 2.5.0 — Dynamic Client Registration (RFC 7591) support in `OAuthHelper`** (Phase 74).
  `OAuthConfig` gains `client_name: Option<String>` and `dcr_enabled: bool` (default: `true`).
  ...
- **`OAuthHelper::authorize_with_details()` + `AuthorizationResult` struct** (Phase 74,
  Blocker #6): returns the full set of OAuth artifacts ...
- **cargo-pmcp 0.9.0 — `cargo pmcp auth` command group** (Phase 74, Plan 02).
  ...

### Changed

- **BREAKING (minor-within-v2.x window):** ...
```

**Pattern to mirror:** new top-of-file entry (above the 2.5.0 block) with sections `### Added` (list the 9 new methods + `ClientOptions` + `with_client_options` + the new example) and `### Fixed` (REQUIREMENTS.md §55 `call_prompt_typed` → `get_prompt_typed` doc correction). No `### Changed` / `### Removed` — phase is purely additive.

---

### 11. `README.md` — add new typed/list_all bullet to Key Features

**Analog:** `README.md:211-222`:

```markdown
**Key Features:**
- **Type-Safe Tools**: Automatic JSON schema generation from Rust types
- **Multiple Transports**: stdio, HTTP/SSE, WebSocket, WASM
- **OAuth Support**: Full auth context pass-through
- **Workflows**: Multi-step orchestration with array indexing support
- **MCP Apps**: Rich HTML UI widgets with live preview and browser DevTools
- **MCP Tasks**: Shared client/server state with task lifecycle management
- **Tower Middleware**: DNS rebinding protection, CORS, security headers
- **Performance**: 16x faster than TypeScript, SIMD-accelerated parsing
- **Quality**: Zero `unwrap()`, comprehensive error handling

**Latest Version:** `pmcp = "2.0"`
```

**Pattern:** add a bullet such as `- **Typed Client Helpers**: \`call_tool_typed\`, \`get_prompt_typed\`, and auto-paginating \`list_all_*\` helpers with bounded safety cap`. Also update `**Latest Version:** \`pmcp = "2.0"\`` → `\`pmcp = "2.6"\`` (currently still reads "2.0" — pre-existing drift; optional housekeeping).

---

## Shared Patterns

### Request-response + `ResponsePayload::{Result, Error}` match

**Source:** every `Client::*` method in `src/client/mod.rs` (template example at `src/client/mod.rs:339-357` for `list_tools`).

**Apply to:** this phase does **NOT** duplicate this pattern. New helpers delegate to existing single-page methods that already implement it. Verified in RESEARCH.md §"Anti-Patterns to Avoid".

### Error construction

**Source:** `src/error/mod.rs:214-217`:

```rust
// Source: src/error/mod.rs:214-217 [VERIFIED]
pub fn validation(message: impl Into<String>) -> Self {
    Self::Validation(message.into())
}
```

**Apply to:** every serialize failure (D-02) AND every cap-exceeded branch (D-10). No new `Error` variants needed.

### `#[non_exhaustive]` struct + field-update idiom

**Source:** `src/types/tools.rs:18-20` (and many sibling types).

**Apply to:** `ClientOptions` struct + every call-site (examples, doctests, tests) that constructs it with a non-default field.

### Doctest convention (Phase 66)

**Source:** `src/client/mod.rs:140-157` (doctest on `with_options`) — every public `Client` method carries a `rust,no_run` or `rust` doctest demonstrating construction + one call.

**Apply to:** all 4 typed + 4 list_all + 1 new-constructor method must carry a doctest (8 methods + 1 constructor = 9 new doctests). The `get_prompt_typed` doctest MUST use D-07's `#[derive(Serialize)] struct SummaryArgs { topic: String, length: u32 }` example.

### `MockTransport`-based `#[tokio::test]` block

**Source:** `src/client/mod.rs:1833-1891` (scaffolding) + `src/client/mod.rs:1987-2027` (canonical test case).

**Apply to:** all 13 unit/integration tests called out in RESEARCH.md Validation Architecture table (Wave 0). Remember the **reverse-order push** quirk.

---

## No Analog Found

None. Every file in the phase has a close existing analog. `ClientOptions` as a whole-new config struct is the furthest from the codebase, but `ProtocolOptions` (same `Debug + Clone + Default` shape) and `ToolAnnotations` (`#[non_exhaustive]` usage) together cover the template cleanly.

---

## Metadata

**Analog search scope:**
- `src/client/mod.rs` (full — all existing `call_*` / `list_*` / `get_*` methods, `ClientBuilder`, `MockTransport`)
- `src/types/protocol/mod.rs` (Cursor alias)
- `src/types/tools.rs`, `src/types/resources.rs`, `src/types/prompts.rs` (item types)
- `src/error/mod.rs` (validation constructor)
- `src/lib.rs` (re-export list)
- `src/server/typed_tool.rs` (generic-bound template)
- `examples/c0*.rs` (example template + numbering)
- `tests/property_tests.rs`, `tests/oauth_dcr_integration.rs` (test framework templates)
- `fuzz/fuzz_targets/*.rs` + `fuzz/Cargo.toml` (fuzz target templates + registration)
- `Cargo.toml` (root) + 7 workspace-member `Cargo.toml`s pinning pmcp
- `CHANGELOG.md`, `README.md`, `examples/README.md` (doc surfaces)

**Files read:** ~12 distinct files, ~1800 total lines of verified-at-HEAD source.
**Pattern extraction date:** 2026-04-21 (pmcp 2.5.0 HEAD of `main`).
