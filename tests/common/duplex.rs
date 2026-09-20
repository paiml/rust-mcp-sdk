//! Shared in-process duplex transport + call helpers for dispatcher tests.
//!
//! Extracted from the (previously copy-pasted) harness in
//! `tests/tool_output_passthrough.rs` / `tests/tool_with_result.rs` et al.:
//! an mpsc-backed client<->server [`Transport`] pair, plus helpers that drive
//! a `tools/call` through a real [`Client`] against either a high-level
//! [`Server`] (own transport loop) or a `ServerCore` [`ProtocolHandler`]
//! (request/response pump).
//!
//! Each file in `tests/` is compiled as a separate integration crate; include
//! this module per-crate via `#[path = "common/duplex.rs"] mod duplex;`.
//!
//! # Two seams, and why the second one exists
//!
//! [`call_via_server`] / [`call_via_core`] drive a real [`Client`], which runs
//! an `initialize` handshake and emits NO per-request `_meta` protocol-version
//! signal. Against the default (v1-only) accept-list that is exactly the v1
//! path — and it is the ONLY thing those two helpers can prove. A test that
//! calls them and then claims "v2" is claiming nothing.
//!
//! The era-aware seam below ([`call_tool_request`], [`raw_via_core`],
//! [`raw_via_server`], [`assert_v2_witness`]) exists so a test can send a
//! request that actually carries the era signal, against a server that actually
//! opted in, and then MEASURE which era the dispatcher resolved. Added by Phase
//! 115 plan 04 (SCHM-02) after a cross-AI review found that the pre-review test
//! design would have run every "v2" assertion as v1.
//!
//! Because this file is `#[path]`-included per test crate, every helper here is
//! compiled once per including file, and an including file that uses only some
//! of them would otherwise trip `dead_code` under `RUSTFLAGS=-D warnings`. The
//! file-level `#![allow(dead_code)]` below is what makes an unused helper
//! harmless rather than a `make lint` failure.

#![allow(dead_code)]
#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::core::ProtocolHandler;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{CallToolResult, ClientCapabilities};
use pmcp::{Client, Error, Result, Server};
use serde_json::Value;
use tokio::sync::mpsc;

// The era-aware seam needs the reserved `_meta` keys, which live behind
// `pmcp::testing` (folded into `full`, which every Phase 115 test command
// uses). Gated imports rather than a gated submodule so the helpers stay
// callable as `duplex::name(..)` from every including file.
#[cfg(feature = "testing")]
use pmcp::testing::META_PROTOCOL_VERSION;
#[cfg(feature = "testing")]
use pmcp::types::jsonrpc::{JSONRPCResponse, RequestId, ResponsePayload};
#[cfg(feature = "testing")]
use pmcp::types::protocol::{
    Era, ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
#[cfg(feature = "testing")]
use pmcp::types::{ClientRequest, Request, RequestMeta};
#[cfg(feature = "testing")]
use serde_json::json;

/// One half of an in-process duplex transport. The client side sends Requests
/// and receives Responses; the server side does the reverse.
#[derive(Debug)]
pub struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    /// Create a connected client/server transport pair.
    pub fn pair() -> (Self, Self) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                tx: client_tx,
                rx: client_rx,
                connected: true,
            },
            Self {
                tx: server_tx,
                rx: server_rx,
                connected: true,
            },
        )
    }
}

#[async_trait]
impl Transport for DuplexTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.tx
            .send(message)
            .map_err(|_| Error::internal("duplex peer dropped"))
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| Error::internal("duplex peer closed"))
    }

    async fn close(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> &'static str {
        "in-process-duplex"
    }
}

/// Drive a `tools/call` through a real [`Client`] against a high-level
/// [`Server`] running its own transport loop.
pub async fn call_via_server(server: Server, name: &str, args: Value) -> CallToolResult {
    let (client_t, server_t) = DuplexTransport::pair();
    tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });
    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes against server");
    client
        .call_tool(name.to_string(), args)
        .await
        .expect("tools/call succeeds against server")
}

/// Drive a `tools/call` through a real [`Client`] against a `ServerCore`
/// served by a request/response pump over the duplex transport.
pub async fn call_via_core(
    core: Arc<dyn ProtocolHandler>,
    name: &str,
    args: Value,
) -> CallToolResult {
    let (client_t, mut server_t) = DuplexTransport::pair();
    tokio::spawn(async move {
        while let Ok(message) = server_t.receive().await {
            if let TransportMessage::Request { id, request } = message {
                let response = core.handle_request(id, request, None).await;
                if server_t
                    .send(TransportMessage::Response(response))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });
    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes against core");
    client
        .call_tool(name.to_string(), args)
        .await
        .expect("tools/call succeeds against core")
}

// ===========================================================================
// Era-aware seam (Phase 115 plan 04, SCHM-02).
//
// Everything below is gated on `testing`: the reserved `_meta` keys are
// re-exported through `pmcp::testing`, and this harness sources them from the
// crate rather than re-spelling them.
// ===========================================================================

/// The spec spelling of the per-request reserved-metadata object.
///
/// Matches the `#[serde(rename = "_meta", alias = "meta")]` Phase 113 (D-113-A)
/// pinned onto `CallToolRequest`, so a request built here deserializes through
/// the same field the wire uses. `tests/common/v2.rs` carries the twin constant
/// for the raw-HTTP harness; the two files are separate `#[path]` modules and
/// cannot share one.
#[cfg(feature = "testing")]
const REQUEST_META_KEY: &str = "_meta";

/// The v2 result-envelope discriminator key.
///
/// pmcp's own `crate::types::mrtr::RESULT_TYPE_KEY` is `pub(crate)`, so an
/// integration-test crate cannot read it; this is the one wire spelling here
/// that has to be a literal. It is asserted on, never emitted, so a drift shows
/// up as a failing witness rather than a wrong request.
#[cfg(feature = "testing")]
const RESULT_TYPE_KEY: &str = "resultType";

/// The accept-list of a server that has opted into the v2 era while keeping v1.
///
/// Both entries come from pmcp's own constants — never string literals — so the
/// harness cannot drift from the crate. Pass to
/// `ServerBuilder::with_supported_protocol_versions` /
/// `ServerCoreBuilder::with_supported_protocol_versions`. WITHOUT this opt-in,
/// `resolve_ingress_protocol_context` returns `Ok(None)` before it ever looks at
/// `_meta` (D-04), so a v2-signalling request is served as v1 and every "v2"
/// assertion in the calling test is vacuous.
#[cfg(feature = "testing")]
pub fn v2_accept_list() -> Vec<ProtocolVersion> {
    vec![
        ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
        ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
    ]
}

/// Build a `tools/call` [`Request`] carrying (or deliberately omitting) the
/// per-request era signal.
///
/// For [`Era::V2`] the `params` object carries a `_meta` object built through
/// pmcp's OWN [`RequestMeta`] serialization, exactly as `tests/common/v2.rs`
/// builds it for the raw-HTTP harness, so the reserved-key spelling round-trips
/// what the server deserializes. For [`Era::V1`] the `params` object carries no
/// `_meta` key at all — which is what a real v1 client sends, and what makes the
/// v1 half a contrast rather than a claim.
///
/// The request is built by DESERIALIZING the real wire shape rather than by
/// struct literal: `CallToolRequest` is `#[non_exhaustive]` and its `new()` has
/// no `_meta` seam, so a test crate cannot construct one with `_meta` set. Going
/// through `serde_json::from_value::<ClientRequest>` is both the only correct
/// route and the one that exercises the D-113-A `_meta` spelling instead of
/// bypassing it.
///
/// `params` is assembled through a `serde_json::Map` rather than the `json!`
/// macro because the macro BORROWS its interpolated values, which would leave
/// `args` a pass-by-value-but-not-consumed parameter — the same reason
/// `tests/common/v2.rs`'s `jsonrpc_envelope` builds its envelope by hand.
#[cfg(feature = "testing")]
pub fn call_tool_request(name: &str, args: Value, era: Era) -> Request {
    let mut params = serde_json::Map::new();
    params.insert("name".to_string(), Value::String(name.to_string()));
    params.insert("arguments".to_string(), args);
    era_signalling_request("tools/call", params, era)
}

/// The shared body of [`call_tool_request`] and [`read_resource_request`].
///
/// Stamps the era signal onto `params` and deserializes the wire envelope. The
/// two public builders differ ONLY in their method string and which params keys
/// they seed, so everything after that point — the reserved `_meta` spelling,
/// the `RequestMeta` serialization, the `from_value::<ClientRequest>` route —
/// lives here once. A second copy is how the `_meta` key spelling drifts between
/// two builders that are supposed to be testing the same ingress.
///
/// Deliberately PRIVATE. Making it public would hand callers an era-aware
/// builder for ANY method string, including the four list methods whose typed
/// requests silently DROP `_meta` — the exact misleading affordance
/// [`read_resource_request`]'s docs explain must not exist.
#[cfg(feature = "testing")]
fn era_signalling_request(
    method: &str,
    mut params: serde_json::Map<String, Value>,
    era: Era,
) -> Request {
    if matches!(era, Era::V2) {
        let meta =
            RequestMeta::new().with_meta(META_PROTOCOL_VERSION, json!(PROTOCOL_VERSION_2026_07_28));
        let meta = serde_json::to_value(&meta).expect("request meta serializes");
        params.insert(REQUEST_META_KEY.to_string(), meta);
    }
    let mut envelope = serde_json::Map::new();
    envelope.insert("method".to_string(), Value::String(method.to_string()));
    envelope.insert("params".to_string(), Value::Object(params));
    let client_request: ClientRequest = serde_json::from_value(Value::Object(envelope))
        .unwrap_or_else(|e| panic!("`{method}` request deserializes into ClientRequest ({e})"));
    Request::Client(Box::new(client_request))
}

/// Build a `resources/read` [`Request`] carrying (or deliberately omitting) the
/// per-request era signal.
///
/// The `resources/read` sibling of [`call_tool_request`], in the same shape and
/// for the same reasons: [`Era::V2`] gets a `params._meta` built through pmcp's
/// OWN [`RequestMeta`] serialization, [`Era::V1`] gets no `_meta` key at all,
/// and the request is DESERIALIZED from the wire shape rather than built by
/// struct literal because `ReadResourceRequest` is `#[non_exhaustive]` with no
/// `_meta` seam on its constructor.
///
/// # Why `resources/read`, and why it has no list-method sibling
///
/// MEASURED, not assumed: `extract_request_meta_value`
/// (`src/server/core.rs:3960-4026`) matches EXHAUSTIVELY and returns the `_meta`
/// object for exactly three [`ClientRequest`] variants — `CallTool`,
/// `GetPrompt` and `ReadResource`. Every other variant, INCLUDING `ListTools`,
/// `ListPrompts`, `ListResources` and `ListResourceTemplates`, yields `None`.
/// Of the six `2026-07-28` `CacheableResult` extenders, `resources/read` is
/// therefore the ONLY one whose typed request can carry an era signal into the
/// in-process `ServerCore` route at all.
///
/// **An era-aware builder for any of those four list methods would be actively
/// MISLEADING, and must not be added here.** Their request structs have no
/// `_meta` field, serde drops the key on deserialization (none of them sets
/// `deny_unknown_fields`), the request resolves as v1, and a caller would get a
/// silently-v1 response under a v2-shaped call. The obvious names for such
/// builders are deliberately NOT written anywhere in this file, so that a plain
/// `grep` for them is a working detector rather than a hit on this warning —
/// the same device `tests/v1_lists_golden.rs:432-439` uses to keep its
/// not-opted-in invariant greppable. The rustdoc at
/// `src/server/core.rs:3971-3991` records why the structs are not widened
/// instead: adding a `pub` field to a constructible `pub` struct is a MAJOR
/// semver break (`cargo semver-checks` `constructible_struct_adds_field`) and
/// the v2.5 milestone is scoped additive. The four list methods reach v2 over
/// the streamable-HTTP transport instead, through
/// `Server::resolve_raw_meta_protocol_context`, which reads the RAW body and has
/// FULL method coverage — that is where `tests/v2_caching_hints.rs` covers them,
/// and the bound itself is asserted at
/// `v2_caching_hints_list_methods_cannot_reach_v2_through_the_typed_dispatch_route`.
#[cfg(feature = "testing")]
pub fn read_resource_request(uri: &str, era: Era) -> Request {
    let mut params = serde_json::Map::new();
    params.insert("uri".to_string(), Value::String(uri.to_string()));
    era_signalling_request("resources/read", params, era)
}

/// Send one already-built request straight at a `ServerCore` and return the RAW
/// [`JSONRPCResponse`].
///
/// The `None` third argument to `handle_request` is the `auth_context` — it has
/// NOTHING to do with the protocol era. The era is resolved INSIDE the
/// dispatcher, from the server's accept-list plus this request's `_meta`. That
/// misreading is what made the pre-review version of this suite silently assert
/// v1 behaviour under v2 test names.
#[cfg(feature = "testing")]
pub async fn raw_via_core(core: Arc<dyn ProtocolHandler>, request: Request) -> JSONRPCResponse {
    core.handle_request(RequestId::from(1i64), request, None)
        .await
}

/// Run the v1 `initialize` handshake against a `ServerCore`, returning its raw
/// response.
///
/// MEASURED, not assumed (Phase 115 plan 04): `ServerCore` gates every
/// non-`initialize` `ClientRequest` behind `v1_initialize_gate_applies`
/// (`src/server/core.rs:3431`), which returns `true` for a v1 / non-opted-in
/// request on a non-stateless core — so a v1 `tools/call` sent as the FIRST
/// message gets `-32002 "Server not initialized"` instead of a result. A v2
/// request needs no handshake (the same predicate returns `false` for
/// `Some(Era::V2)`), so the v2 half of a suite calls [`raw_via_core`] directly
/// and only the v1 half needs this. That asymmetry is itself evidence the era
/// reached the dispatcher.
#[cfg(feature = "testing")]
pub async fn initialize_via_core(core: &Arc<dyn ProtocolHandler>) -> JSONRPCResponse {
    let request: ClientRequest = serde_json::from_value(json!({
        "method": "initialize",
        "params": {
            "protocolVersion": LATEST_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "duplex-harness", "version": "0.0.0" },
        },
    }))
    .expect("initialize request deserializes into ClientRequest");
    core.handle_request(
        RequestId::from(0i64),
        Request::Client(Box::new(request)),
        None,
    )
    .await
}

/// Send one already-built request at a high-level [`Server`] over the duplex
/// transport and return the RAW [`JSONRPCResponse`].
///
/// No [`Client`] and no `initialize`: the request goes on the wire as the FIRST
/// message. MEASURED (Phase 115 plan 04): the high-level `Server` has no
/// initialize gate at all on its dispatch path — only `ServerCore` does (see
/// [`initialize_via_core`]) — so this works for BOTH eras.
///
/// Non-response frames (server-initiated notifications) are skipped rather than
/// treated as the answer.
#[cfg(feature = "testing")]
pub async fn raw_via_server(server: Server, request: Request) -> JSONRPCResponse {
    let (mut client_t, server_t) = DuplexTransport::pair();
    tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });
    client_t
        .send(TransportMessage::Request {
            id: RequestId::from(1i64),
            request,
        })
        .await
        .expect("client half sends the request");
    loop {
        if let TransportMessage::Response(response) = client_t
            .receive()
            .await
            .expect("server answers the request")
        {
            return response;
        }
    }
}

/// Borrow the `result` object of a successful response, panicking with the full
/// response on an error payload or a non-object result.
#[cfg(feature = "testing")]
pub fn result_object(response: &JSONRPCResponse) -> &serde_json::Map<String, Value> {
    match &response.payload {
        ResponsePayload::Result(value) => value
            .as_object()
            .unwrap_or_else(|| panic!("expected an object result, got: {value}")),
        ResponsePayload::Error(error) => {
            panic!("expected a Result payload, got error: {error:?}")
        },
    }
}

/// Assert that the dispatcher actually resolved [`Era::V2`] for this request.
///
/// `inject_v2_result_envelope` (`src/server/core.rs`) calls
/// `own_reserved_result_fields` — the only writer of `resultType` — ONLY inside
/// its `Some(Era::V2)` branch, so the key's presence is in-band, server-minted
/// proof of the resolved era, not a restatement of what the test intended. A
/// test that asserts v2 behaviour WITHOUT calling this is asserting nothing: the
/// same request against a non-opted-in server is served as v1 and every
/// downstream assertion still passes.
///
/// (Phase 115 moved the era gate DOWN: the function no longer early-returns on a
/// non-v2 context, because the caching-hint projection above it runs on both
/// eras. Only the ENVELOPE half is v2-gated now — which is still exactly what
/// this witness reads.)
///
/// `ctx` names the call site so a failure says which dispatcher and which
/// payload shape lost its era.
#[cfg(feature = "testing")]
pub fn assert_v2_witness(response: &JSONRPCResponse, ctx: &str) {
    let result = result_object(response);
    assert!(
        result.contains_key(RESULT_TYPE_KEY),
        "{ctx}: no `{RESULT_TYPE_KEY}` in the result, so the dispatcher did NOT resolve Era::V2 \
         for this request — the server is probably not opted in via \
         `with_supported_protocol_versions(v2_accept_list())`, or the request carries no \
         `_meta` protocol-version signal. Result was: {result:?}"
    );
}

/// The mirror of [`assert_v2_witness`]: assert the dispatcher resolved v1.
///
/// Absence of `resultType` is proof of v1 for the same reason its presence is
/// proof of v2 — the v2 envelope injector is the key's only writer.
#[cfg(feature = "testing")]
pub fn assert_no_v2_witness(response: &JSONRPCResponse, ctx: &str) {
    let result = result_object(response);
    assert!(
        !result.contains_key(RESULT_TYPE_KEY),
        "{ctx}: found `{RESULT_TYPE_KEY}` in the result, so the dispatcher resolved Era::V2 for \
         a request that was supposed to be served as v1. Result was: {result:?}"
    );
}

/// Deserialize a raw response's result into a [`CallToolResult`] so assertions
/// stay at the typed level.
///
/// Unknown keys (including the v2 envelope's `resultType`) are ignored by serde,
/// which is why the same helper works on both eras.
#[cfg(feature = "testing")]
pub fn call_tool_result_of(response: &JSONRPCResponse) -> CallToolResult {
    let result = Value::Object(result_object(response).clone());
    serde_json::from_value(result.clone())
        .unwrap_or_else(|e| panic!("result deserializes into CallToolResult ({e}): {result}"))
}
