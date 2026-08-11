//! G-9 / CONF-08 — the v1 handshake's `capabilities` must reach the handler.
//!
//! The v1 `initialize` handshake writes the client's advertised capabilities to a
//! SERVER-LEVEL `RwLock` (`src/server/mod.rs` / `src/server/core.rs`
//! `handle_initialize`). A handler reads them through
//! `RequestHandlerExtra::client_capabilities()`, which reads only
//! `protocol_context.client_capabilities` — a field `resolve_protocol_context`
//! populates ONLY from the v2 `_meta` reserved key. The two never met, so a
//! server-side capability gate read permanently `None` under v1.
//!
//! # What this file measures, and what it deliberately does not
//!
//! Every v1 case drives a REAL `initialize` handshake over the wire. Nothing here
//! constructs a `ProtocolContext` by hand — a fence that synthesises the very
//! value it is checking restates the rule instead of measuring it, which is the
//! failure shape Phase 115 reopened on four times.
//!
//! The advertised capability set is DISTINCTIVE
//! ([`distinctive_capabilities`]) — `roots.listChanged: true`, a `sampling`
//! object and a namespaced `experimental` marker. `ClientCapabilities::default()`
//! serialises to `{}`, so a fold that fabricated a default could not
//! accidentally satisfy any assertion below.
//!
//! Coverage, one case per disposition from the plan-08 site table:
//!
//! | Case | Sites exercised |
//! |---|---|
//! | [`v1_handshake_capabilities_reach_tool_prompt_and_resource_handlers`] | high-level `Server` FOLD sites (`tools/call`, `prompts/get`, `resources/read`) |
//! | [`v1_handshake_capabilities_reach_the_server_core_dispatcher`] | `ServerCore` FOLD sites (the twin) |
//! | [`v1_handshake_capabilities_reach_the_thread_then_fold_sites`] | `resources/list` on BOTH dispatchers + `sampling/createMessage` on `Server` — the three THREAD-THEN-FOLD sites |
//! | [`v2_meta_client_capabilities_still_win_over_a_v1_handshake`] | non-regression: the fold is ONE-DIRECTIONAL |
//! | [`no_handshake_and_no_meta_yields_none`] | the no-signal row: `None`, never a fabricated default |
//!
//! No case covers `src/server/wasm_core.rs`: it constructs no
//! `RequestHandlerExtra` at all (it dispatches on method strings and serves only
//! `initialize` / `tools/list` / `tools/call`), and on `wasm32`
//! `RequestHandlerExtra` is a zero-field stub with no `client_capabilities()`
//! accessor to assert on. That site is a measured NO-OP, recorded in the plan-08
//! SUMMARY rather than fenced here.
//!
//! **Security — self-reported, not for authorization:** everything captured here
//! is client-supplied and informational ONLY. These assertions prove the value
//! is DELIVERED; they must never be read as proof it is TRUSTWORTHY. Real
//! identity binds to the OAuth token.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::server::core::ProtocolHandler;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::testing::{META_CLIENT_CAPABILITIES, META_PROTOCOL_VERSION};
use pmcp::types::jsonrpc::{JSONRPCResponse, RequestId, ResponsePayload};
use pmcp::types::protocol::{
    ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::types::{
    ClientCapabilities, ClientRequest, Content, CreateMessageResult, GetPromptResult,
    ListResourcesResult, ReadResourceResult, Request, RootsCapabilities, SamplingCapabilities,
};
use pmcp::{
    PromptHandler, ResourceHandler, Result, SamplingHandler, Server, ServerBuilder,
    ServerCapabilities, ToolHandler,
};

use duplex::DuplexTransport;

/// Every await in this file is bounded by this. A dispatcher that never answers
/// must fail the test, not hang the suite.
const DEADLINE: Duration = Duration::from_secs(5);

// ===========================================================================
// The DISTINCTIVE advertised capability set.
// ===========================================================================

/// The capability set every v1 handshake in this file advertises.
///
/// Deliberately NOT `ClientCapabilities::default()`: that serialises to `{}`, so
/// a bug that fabricated a default where nothing was advertised would satisfy an
/// equality assertion against it. Three independent signals are set so a partial
/// fold (say, one that carried only `roots`) also fails.
fn distinctive_capabilities() -> ClientCapabilities {
    let mut caps = ClientCapabilities::default();
    caps.roots = Some(RootsCapabilities { list_changed: true });
    caps.sampling = Some(SamplingCapabilities {
        models: Some(vec!["g9-model".to_string()]),
        context: None,
        tools: None,
    });
    caps.experimental = Some(HashMap::from([(
        "io.pmcp.test/g9-marker".to_string(),
        json!("v1-handshake"),
    )]));
    caps
}

/// A SECOND, clearly different capability set, used as the v2 `_meta` value in
/// the non-regression case so "the `_meta` value won" is distinguishable from
/// "the handshake value won".
fn v2_meta_capabilities() -> ClientCapabilities {
    let mut caps = ClientCapabilities::default();
    caps.experimental = Some(HashMap::from([(
        "io.pmcp.test/g9-marker".to_string(),
        json!("v2-meta"),
    )]));
    caps
}

/// Canonical JSON for a capability set, so comparisons do not depend on
/// `ClientCapabilities` deriving `PartialEq` (it does not).
fn canonical(caps: &ClientCapabilities) -> Value {
    serde_json::to_value(caps).expect("ClientCapabilities serialises")
}

// ===========================================================================
// Observation slots + capturing handlers.
// ===========================================================================

/// What one handler invocation saw on its `RequestHandlerExtra`.
#[derive(Clone, Debug)]
struct Observed {
    /// `extra.client_capabilities()`, canonicalised; `None` when the accessor
    /// returned `None`.
    capabilities: Option<Value>,
    /// `extra.era()`, rendered — recorded so a failure message says which era
    /// the dispatcher resolved.
    era: Option<String>,
}

impl Observed {
    fn of(extra: &pmcp::RequestHandlerExtra) -> Self {
        Self {
            capabilities: extra.client_capabilities().map(canonical),
            era: extra.era().map(|era| format!("{era:?}")),
        }
    }
}

type Slot = Arc<Mutex<Option<Observed>>>;

fn slot() -> Slot {
    Arc::new(Mutex::new(None))
}

/// One observation slot per capturing handler, NAMED.
///
/// The builders below would otherwise take five consecutive `Slot` parameters,
/// where swapping `read` and `list` compiles clean and silently points an
/// assertion at the wrong handler — the exact false-green this fence exists to
/// rule out.
struct Slots {
    tool: Slot,
    prompt: Slot,
    read: Slot,
    list: Slot,
    sampling: Slot,
}

impl Slots {
    fn new() -> Self {
        Self {
            tool: slot(),
            prompt: slot(),
            read: slot(),
            list: slot(),
            sampling: slot(),
        }
    }
}

fn observed(slot: &Slot, what: &str) -> Observed {
    slot.lock()
        .expect("observation slot is not poisoned")
        .clone()
        .unwrap_or_else(|| panic!("{what} handler never ran"))
}

fn record(slot: &Slot, extra: &pmcp::RequestHandlerExtra) {
    *slot.lock().expect("observation slot is not poisoned") = Some(Observed::of(extra));
}

struct CapturingTool(Slot);

#[async_trait]
impl ToolHandler for CapturingTool {
    async fn handle(&self, _args: Value, extra: pmcp::RequestHandlerExtra) -> Result<Value> {
        record(&self.0, &extra);
        Ok(json!("ok"))
    }
}

struct CapturingPrompt(Slot);

#[async_trait]
impl PromptHandler for CapturingPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        extra: pmcp::RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        record(&self.0, &extra);
        Ok(GetPromptResult::new(vec![], None))
    }
}

/// One handler, TWO slots: `resources/read` and `resources/list` are separate
/// `RequestHandlerExtra` construction sites with different dispositions (FOLD vs
/// THREAD-THEN-FOLD), so they need separate observations.
struct CapturingResource {
    read: Slot,
    list: Slot,
}

#[async_trait]
impl ResourceHandler for CapturingResource {
    async fn read(
        &self,
        _uri: &str,
        extra: pmcp::RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        record(&self.read, &extra);
        Ok(ReadResourceResult::new(vec![Content::text("ok")]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        extra: pmcp::RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        record(&self.list, &extra);
        Ok(ListResourcesResult::new(vec![]))
    }
}

struct CapturingSampling(Slot);

#[async_trait]
impl SamplingHandler for CapturingSampling {
    async fn create_message(
        &self,
        _params: pmcp::types::CreateMessageParams,
        extra: pmcp::RequestHandlerExtra,
    ) -> Result<CreateMessageResult> {
        record(&self.0, &extra);
        Ok(CreateMessageResult::new(Content::text("ok"), "g9-model"))
    }
}

// ===========================================================================
// Wire helpers — REAL requests, deserialized from the wire shape.
// ===========================================================================

/// Build a [`Request`] by DESERIALIZING the wire envelope.
///
/// `CallToolRequest` and friends are `#[non_exhaustive]` with no `_meta` seam on
/// their constructors, so an integration-test crate cannot build one with `_meta`
/// set. Going through `from_value::<ClientRequest>` is both the only route and
/// the one that exercises the real `_meta` field spelling.
///
/// The envelope is assembled through a `serde_json::Map` rather than the `json!`
/// macro because the macro BORROWS its interpolated values, which would leave
/// `params` a pass-by-value-but-not-consumed parameter — the same reason
/// `tests/common/duplex.rs`'s `era_signalling_request` builds its envelope by
/// hand.
fn client_request(method: &str, params: Value) -> Request {
    let mut envelope = serde_json::Map::new();
    envelope.insert("method".to_string(), Value::String(method.to_string()));
    envelope.insert("params".to_string(), params);
    let parsed: ClientRequest = serde_json::from_value(Value::Object(envelope))
        .unwrap_or_else(|e| panic!("`{method}` deserializes into ClientRequest ({e})"));
    Request::Client(Box::new(parsed))
}

/// The v1 `initialize` params advertising `caps`.
fn initialize_params(caps: &ClientCapabilities) -> Value {
    json!({
        "protocolVersion": LATEST_PROTOCOL_VERSION,
        "capabilities": canonical(caps),
        "clientInfo": { "name": "g9-fence", "version": "0.0.0" },
    })
}

/// A `params._meta` carrying the v2 era signal plus a `clientCapabilities`
/// declaration, built from the SHIPPED reserved-key constants.
fn v2_meta(caps: &ClientCapabilities) -> Value {
    json!({
        META_PROTOCOL_VERSION: PROTOCOL_VERSION_2026_07_28,
        META_CLIENT_CAPABILITIES: canonical(caps),
    })
}

fn assert_ok(response: &JSONRPCResponse, what: &str) {
    if let ResponsePayload::Error(error) = &response.payload {
        panic!("{what} must succeed, got JSON-RPC error: {error:?}");
    }
}

/// The three FOLD-site probes, as ONE table.
///
/// `meta` splices a `params._meta` into each — the only difference between the
/// v1 and v2 cases below — so the method names and params literals are spelled
/// once here instead of once per case. Renaming the fixture resource, or adding
/// a fourth probe, is a one-line edit.
fn probes(meta: Option<&Value>) -> [(&'static str, Value); 3] {
    let mut probes = [
        ("tools/call", json!({ "name": "probe", "arguments": {} })),
        (
            "prompts/get",
            json!({ "name": "greeting", "arguments": {} }),
        ),
        ("resources/read", json!({ "uri": "mem://greeting" })),
    ];
    if let Some(meta) = meta {
        for (_, params) in &mut probes {
            params["_meta"] = meta.clone();
        }
    }
    probes
}

/// One handler saw exactly the v1 handshake's advertised set.
///
/// The shared body of every FOLD-site assertion: reading it out of a single
/// place is what keeps the four call sites from drifting in what they claim.
fn assert_carries(cell: &Slot, what: &str, expected: &Value) {
    let seen = observed(cell, what);
    assert_eq!(
        seen.capabilities.as_ref(),
        Some(expected),
        "{what}: extra.client_capabilities() must carry the v1 handshake's advertised set \
         (era seen: {:?})",
        seen.era
    );
}

// ===========================================================================
// Driver A — the high-level `Server` over a real duplex transport.
// ===========================================================================

/// Raw v1 wire driver: no `Client`, so `derive_host_capabilities` cannot rewrite
/// the advertised set before it reaches the server (it forces
/// `sampling`/`roots`/`elicitation` to `None` unless a host handler is
/// registered, which would silently flatten the distinctive set this fence
/// depends on).
struct ServerDriver {
    transport: DuplexTransport,
    handle: tokio::task::JoinHandle<()>,
    next_id: i64,
}

impl ServerDriver {
    fn spawn(server: Server) -> Self {
        let (client_t, server_t) = DuplexTransport::pair();
        let handle = tokio::spawn(async move {
            let _ = server.run(server_t).await;
        });
        Self {
            transport: client_t,
            handle,
            next_id: 1,
        }
    }

    async fn call(&mut self, method: &str, params: Value) -> JSONRPCResponse {
        let id = RequestId::from(self.next_id);
        self.next_id += 1;
        tokio::time::timeout(
            DEADLINE,
            self.transport.send(TransportMessage::Request {
                id: id.clone(),
                request: client_request(method, params),
            }),
        )
        .await
        .unwrap_or_else(|_| panic!("`{method}` send deadline"))
        .unwrap_or_else(|e| panic!("`{method}` send failed ({e})"));

        loop {
            let message = tokio::time::timeout(DEADLINE, self.transport.receive())
                .await
                .unwrap_or_else(|_| panic!("`{method}` response deadline"))
                .unwrap_or_else(|e| panic!("`{method}` receive failed ({e})"));
            if let TransportMessage::Response(response) = message {
                if response.id == id {
                    return response;
                }
            }
        }
    }

    /// Drive every [`probes`] entry, asserting each is served without error.
    async fn probe_all(&mut self, meta: Option<&Value>, context: &str) {
        for (method, params) in probes(meta) {
            let response = self.call(method, params).await;
            assert_ok(&response, &format!("{method} [{context}]"));
        }
    }

    /// Ordered teardown: drop the socket FIRST so `run()` observes a closed
    /// transport, then abort, then await the join handle.
    async fn shutdown(self) {
        let Self {
            transport, handle, ..
        } = self;
        drop(transport);
        handle.abort();
        let _ = tokio::time::timeout(DEADLINE, handle).await;
    }
}

fn build_server(slots: &Slots, accept_list: Option<Vec<ProtocolVersion>>) -> Server {
    let builder: ServerBuilder = Server::builder()
        .name("g9-fence-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::default())
        .tool("probe", CapturingTool(slots.tool.clone()))
        .prompt("greeting", CapturingPrompt(slots.prompt.clone()))
        .resources(CapturingResource {
            read: slots.read.clone(),
            list: slots.list.clone(),
        })
        .sampling(CapturingSampling(slots.sampling.clone()));
    let builder = match accept_list {
        Some(versions) => builder.with_supported_protocol_versions(versions),
        None => builder,
    };
    builder.build().expect("server builds")
}

// ===========================================================================
// Driver B — the `ServerCore` twin, entered through its REAL dispatch root.
// ===========================================================================

fn build_core(slots: &Slots) -> Arc<dyn ProtocolHandler> {
    Arc::new(
        pmcp::server::builder::ServerCoreBuilder::new()
            .name("g9-fence-core")
            .version("1.0.0")
            .tool("probe", CapturingTool(slots.tool.clone()))
            .prompt("greeting", CapturingPrompt(slots.prompt.clone()))
            .resources(CapturingResource {
                read: slots.read.clone(),
                list: slots.list.clone(),
            })
            // Explicit, never defaulted: `build()` resolves an unset value by
            // ENVIRONMENT auto-detection, and a stateless core skips the
            // initialize gate this case depends on.
            .stateless_mode(false)
            .build()
            .expect("core builds"),
    )
}

async fn core_call(
    core: &Arc<dyn ProtocolHandler>,
    id: i64,
    method: &str,
    params: Value,
) -> JSONRPCResponse {
    tokio::time::timeout(
        DEADLINE,
        core.handle_request(RequestId::from(id), client_request(method, params), None),
    )
    .await
    .unwrap_or_else(|_| panic!("`{method}` deadline against ServerCore"))
}

/// The [`ServerDriver::probe_all`] twin for the `ServerCore` root: the SAME
/// [`probes`] table, ids allocated from `first_id` because `core_call` has no
/// connection to carry a counter on.
async fn probe_all_via_core(core: &Arc<dyn ProtocolHandler>, first_id: i64, context: &str) {
    for (offset, (method, params)) in probes(None).into_iter().enumerate() {
        let id = first_id + i64::try_from(offset).expect("probe count fits in i64");
        let response = core_call(core, id, method, params).await;
        assert_ok(&response, &format!("{method} [{context}]"));
    }
}

// ===========================================================================
// 1. The three high-level `Server` FOLD sites.
// ===========================================================================

#[tokio::test]
async fn v1_handshake_capabilities_reach_tool_prompt_and_resource_handlers() {
    let slots = Slots::new();
    let server = build_server(
        &slots,
        // Default accept-list: v1-only. `resolve_ingress_protocol_context`
        // returns `Ok(None)` for EVERY request here (D-04), which is exactly the
        // shape G-9 leaves broken.
        None,
    );
    let mut driver = ServerDriver::spawn(server);

    let advertised = distinctive_capabilities();
    let init = driver
        .call("initialize", initialize_params(&advertised))
        .await;
    assert_ok(&init, "v1 initialize");

    driver.probe_all(None, "Server").await;
    driver.shutdown().await;

    let expected = canonical(&advertised);
    assert_carries(&slots.tool, "tools/call", &expected);
    assert_carries(&slots.prompt, "prompts/get", &expected);
    assert_carries(&slots.read, "resources/read", &expected);
}

// ===========================================================================
// 2. The `ServerCore` twin.
// ===========================================================================

#[tokio::test]
async fn v1_handshake_capabilities_reach_the_server_core_dispatcher() {
    let slots = Slots::new();
    let core = build_core(&slots);

    let advertised = distinctive_capabilities();
    let init = core_call(&core, 0, "initialize", initialize_params(&advertised)).await;
    assert_ok(&init, "v1 initialize against ServerCore");

    probe_all_via_core(&core, 1, "ServerCore").await;

    let expected = canonical(&advertised);
    assert_carries(&slots.tool, "ServerCore tools/call", &expected);
    assert_carries(&slots.prompt, "ServerCore prompts/get", &expected);
    assert_carries(&slots.read, "ServerCore resources/read", &expected);
}

// ===========================================================================
// 3. The three THREAD-THEN-FOLD sites.
// ===========================================================================

#[tokio::test]
async fn v1_handshake_capabilities_reach_the_thread_then_fold_sites() {
    let advertised = distinctive_capabilities();
    let expected = canonical(&advertised);

    // --- Site 7 + 9: `Server::handle_list_resources` and
    // `Server::handle_create_message`, neither of which took a
    // `ProtocolContext` parameter before this plan.
    let slots = Slots::new();
    let server = build_server(&slots, None);
    let mut driver = ServerDriver::spawn(server);
    let init = driver
        .call("initialize", initialize_params(&advertised))
        .await;
    assert_ok(&init, "v1 initialize");

    let list_resources = driver.call("resources/list", json!({})).await;
    assert_ok(&list_resources, "resources/list");
    let create_message = driver
        .call(
            "sampling/createMessage",
            json!({ "messages": [], "maxTokens": 16 }),
        )
        .await;
    assert_ok(&create_message, "sampling/createMessage");
    driver.shutdown().await;

    assert_carries(&slots.list, "Server resources/list", &expected);
    assert_carries(&slots.sampling, "Server sampling/createMessage", &expected);

    // --- Site 3: `ServerCore::handle_list_resources`, the twin.
    let core_slots = Slots::new();
    let core = build_core(&core_slots);
    let init = core_call(&core, 0, "initialize", initialize_params(&advertised)).await;
    assert_ok(&init, "v1 initialize against ServerCore");
    let list_resources = core_call(&core, 1, "resources/list", json!({})).await;
    assert_ok(&list_resources, "resources/list against ServerCore");

    assert_carries(&core_slots.list, "ServerCore resources/list", &expected);
}

// ===========================================================================
// 4. NON-REGRESSION — the fold is ONE-DIRECTIONAL.
// ===========================================================================

/// A v2 request's `_meta` `clientCapabilities` must still win, even on a server
/// whose v1 handshake lock holds a DIFFERENT set. A bidirectional fold would let
/// a stale handshake value overwrite a fresh per-request declaration
/// (T-118.1-08-03).
#[tokio::test]
async fn v2_meta_client_capabilities_still_win_over_a_v1_handshake() {
    let slots = Slots::new();
    let server = build_server(&slots, Some(duplex::v2_accept_list()));
    let mut driver = ServerDriver::spawn(server);

    // The handshake plants a DIFFERENT set on the server-level lock.
    let handshake = distinctive_capabilities();
    let init = driver
        .call("initialize", initialize_params(&handshake))
        .await;
    assert_ok(&init, "v1 initialize on a v2-opted-in server");

    let per_request = v2_meta_capabilities();
    driver.probe_all(Some(&v2_meta(&per_request)), "v2").await;
    driver.shutdown().await;

    let expected = canonical(&per_request);
    let handshake_json = canonical(&handshake);
    for (what, cell) in [
        ("tools/call", &slots.tool),
        ("prompts/get", &slots.prompt),
        ("resources/read", &slots.read),
    ] {
        let seen = observed(cell, what);
        assert_eq!(
            seen.era.as_deref(),
            Some("V2"),
            "v2 {what}: the dispatcher must have resolved Era::V2, otherwise this case asserts \
             nothing about the v2 path"
        );
        assert_eq!(
            seen.capabilities.as_ref(),
            Some(&expected),
            "v2 {what}: the per-request `_meta` clientCapabilities must win over the v1 \
             handshake value ({handshake_json})"
        );
    }
}

// ===========================================================================
// 5. NO SIGNAL — `None`, never a fabricated default.
// ===========================================================================

/// With no handshake and no `_meta`, `client_capabilities()` must stay `None`. A
/// fold that invented `ClientCapabilities::default()` would make a
/// capability gate silently permissive — worse than the bug it replaces
/// (T-118.1-08-04).
#[tokio::test]
async fn no_handshake_and_no_meta_yields_none() {
    let slots = Slots::new();
    // The high-level `Server` has NO initialize gate on its dispatch path, so a
    // `tools/call` sent as the FIRST frame is served with the lock still empty.
    let server = build_server(&slots, None);
    let mut driver = ServerDriver::spawn(server);

    driver.probe_all(None, "no handshake").await;
    let list_resources = driver.call("resources/list", json!({})).await;
    assert_ok(&list_resources, "resources/list with no handshake");

    driver.shutdown().await;

    for (what, cell) in [
        ("tools/call", &slots.tool),
        ("prompts/get", &slots.prompt),
        ("resources/read", &slots.read),
        ("resources/list", &slots.list),
    ] {
        let seen = observed(cell, what);
        assert_eq!(
            seen.capabilities, None,
            "{what}: with no handshake and no `_meta`, client_capabilities() must be None, not a \
             fabricated default"
        );
        assert_eq!(
            seen.era, None,
            "{what}: a server that never saw a handshake must not gain a synthesised era either"
        );
    }
}
