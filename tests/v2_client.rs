//! Phase 113-05 (CLNT-01): live acceptance that the pmcp `Client` speaks v2.
//!
//! Every test here drives a REAL `pmcp::Client` over a REAL
//! `StreamableHttpTransport` against a REAL `StreamableHttpServer` on a loopback
//! TCP socket. This is the first end-to-end proof that pmcp's OWN client is
//! accepted by pmcp's OWN Phase-112 strict v2 header gate — RESEARCH Pitfall 7
//! measured the gap (the client transport emitted ZERO `Mcp-Method`/`Mcp-Name`
//! headers, so every v2 request from a pmcp client was rejected).
//!
//! # The assertion style is deliberate
//!
//! Most tests here assert only that the call SUCCEEDS. That is not a weak
//! assertion: the server under test runs `require_v2_headers` +
//! `cross_check_method` + `cross_check_name` + the header/`_meta` era matrix and
//! answers `-32020 HEADER_MISMATCH` at HTTP 400 for any disagreement. A success
//! therefore proves the client emitted `Mcp-Method` and `MCP-Protocol-Version`,
//! emitted a correct `Mcp-Name` wherever the method carries a routing name
//! (Phase 118 D-13 / D-18), and stamped `params._meta` with the v2 era signal. The
//! two tests that need to observe what the server RECEIVED use a thin recording
//! `ServerHttpMiddleware` rather than parsing logs.
//!
//! Servers are spawned with [`common::v2::spawn_default_config`] — the STATEFUL
//! `StreamableHttpServerConfig::default()` — so the per-request era gate (not a
//! build-time stateless config) is what makes these session-free (RESEARCH
//! Pitfall 1).
//!
//! Test reliability doctrine (carried from `tests/v2_required_headers.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN (`JoinHandle::abort()` after each
//! round-trip).
//!
//! # Structure
//!
//! Plan 07 (mock-transport MRTR) and plan 13 EXTEND this file. Helpers live at the
//! top, tests below, so later work APPENDS rather than rewrites.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, extensions_capabilities, spawn_default_config, spawn_with, GreetingPrompt,
    GreetingResource, SearchTool, V1, V2,
};

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::http_middleware::{
    ServerHttpContext, ServerHttpMiddleware, ServerHttpMiddlewareChain, ServerHttpRequest,
};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::server::Server;
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
use pmcp::shared::StreamableHttpTransport;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::ClientCapabilities;
use pmcp::{Client, ClientBuilder};
use pmcp::{RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use url::Url;

// ===========================================================================
// Helpers.
// ===========================================================================

/// A tool whose name is NOT header-safe, so `Mcp-Name` must travel in the
/// `=?base64?…?=` sentinel form (T-113-47). Decoding it on the server is plan
/// 04's work, which is why this plan depends on 113-04.
const NON_ASCII_TOOL: &str = "поиск-☂";

/// The `mem://` resource `build_v2_server` registers a handler for.
const RESOURCE_URI: &str = "mem://greeting";

/// A second trivial tool, registered under [`NON_ASCII_TOOL`].
struct UnicodeTool;

#[async_trait]
impl ToolHandler for UnicodeTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "answer": "unicode ok" }))
    }
}

/// The v2-opted-in harness server, plus a tool whose name needs sentinel
/// encoding.
fn build_server_with_unicode_tool() -> Server {
    Server::builder()
        .name("v2-client-harness")
        .version("1.0.0")
        .capabilities(extensions_capabilities())
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool("search", SearchTool)
        .tool(NON_ASCII_TOOL, UnicodeTool)
        .prompt("greeting", GreetingPrompt)
        .resources(GreetingResource)
        .build()
        .expect("server builds")
}

/// What a [`RecordingMiddleware`] observed about the requests that arrived.
#[derive(Debug, Default)]
struct Observed {
    /// Set when ANY request body carried `"method":"initialize"` or
    /// `"notifications/initialized"`.
    handshake: AtomicBool,
    /// Set when ANY request carried an inbound `Mcp-Session-Id` header.
    inbound_session_id: AtomicBool,
    /// Set when at least one request arrived at all (guards vacuous assertions).
    traffic_arrived: AtomicBool,
}

/// A thin recording wrapper at the HTTP boundary.
///
/// Preferred over log parsing (and over a tool-handler wrapper) because the two
/// facts under observation — "was `initialize` ever sent?" and "did an inbound
/// `Mcp-Session-Id` arrive?" — live at the transport layer, not in a handler.
struct RecordingMiddleware {
    observed: Arc<Observed>,
}

#[async_trait]
impl ServerHttpMiddleware for RecordingMiddleware {
    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        _context: &ServerHttpContext,
    ) -> pmcp::Result<()> {
        self.observed.traffic_arrived.store(true, Ordering::SeqCst);
        if request.get_header(MCP_SESSION_ID).is_some() {
            self.observed
                .inbound_session_id
                .store(true, Ordering::SeqCst);
        }
        let method = serde_json::from_slice::<Value>(&request.body)
            .ok()
            .and_then(|body| {
                body.get("method")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
        if matches!(
            method.as_deref(),
            Some("initialize" | "notifications/initialized")
        ) {
            self.observed.handshake.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Spawn the harness server with a [`RecordingMiddleware`] installed.
async fn spawn_recording(server: Server) -> (SocketAddr, JoinHandle<()>, Arc<Observed>) {
    let observed = Arc::new(Observed::default());
    let mut chain = ServerHttpMiddlewareChain::new();
    chain.add(Arc::new(RecordingMiddleware {
        observed: observed.clone(),
    }));
    // The STATEFUL default config (a live `session_id_generator`), so what makes
    // these round trips session-free is the PER-REQUEST era gate, not a
    // build-time stateless branch (RESEARCH Pitfall 1).
    let config = StreamableHttpServerConfig {
        http_middleware: Some(Arc::new(chain)),
        ..StreamableHttpServerConfig::default()
    };
    let (addr, handle) = spawn_with(server, config).await;
    (addr, handle, observed)
}

/// A `StreamableHttpTransport` pointed at `addr`.
fn transport_for(addr: SocketAddr) -> StreamableHttpTransport {
    let url = Url::parse(&format!("http://{addr}/")).expect("loopback URL parses");
    StreamableHttpTransport::new(StreamableHttpTransportConfigBuilder::new(url).build())
}

/// A pmcp client that OPTED INTO `2026-07-28` — no handshake, v2 headers,
/// per-request `_meta`, no session.
fn v2_client(addr: SocketAddr) -> Client<StreamableHttpTransport> {
    ClientBuilder::new(transport_for(addr))
        .with_protocol_version(ProtocolVersion(V2.to_string()))
        .expect("2026-07-28 is selectable")
        .build()
}

/// A pmcp client that made NO era selection — today's behavior, handshake and all.
fn v1_client(addr: SocketAddr) -> Client<StreamableHttpTransport> {
    ClientBuilder::new(transport_for(addr)).build()
}

// ===========================================================================
// Tests.
// ===========================================================================

/// A v2 `tools/call` is ACCEPTED by the strict Phase-112 header gate.
///
/// Success is the assertion: a missing or mismatched `Mcp-Method` / `Mcp-Name` /
/// `MCP-Protocol-Version`, or a missing `params._meta` era signal, is a 400.
#[tokio::test]
async fn emits_required_headers() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client.call_tool("search".to_string(), json!({})).await;

    handle.abort();
    assert!(
        result.is_ok(),
        "a v2 pmcp client must be accepted by pmcp's own v2 header gate: {result:?}"
    );
}

/// The client half of the empty-`Mcp-Name` rule.
///
/// `tools/list` carries no routing name, so since Phase 118 D-13 the server would
/// accept the request with or without `Mcp-Name` — the empty value this client
/// emits is discarded by `require_v2_headers`. What still makes this test
/// load-bearing is the `_meta` era signal: omit it and the header/`_meta` matrix
/// 400s the request as a `HEADER_MISMATCH`. Neither struct has a `_meta` field, so
/// this only passes because the v2 frame is assembled and stamped by the client
/// itself.
#[tokio::test]
async fn nameless_method_accepted() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client.list_tools(None).await;

    handle.abort();
    let tools = result.expect("a v2 tools/list must be accepted");
    assert!(
        tools.tools.iter().any(|tool| tool.name == "search"),
        "the listing must be the real one: {:?}",
        tools.tools
    );
}

/// `Mcp-Name` for `resources/read` comes from `params.uri`, NOT `params.name`.
///
/// A `ReadResourceRequest` has no `name` field, so a client that read the wrong
/// key would send an empty `Mcp-Name` and fail the server's body cross-check.
#[tokio::test]
async fn mcp_name_from_uri_for_resources_read() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client.read_resource(RESOURCE_URI.to_string()).await;

    handle.abort();
    let read = result.expect("a v2 resources/read must be accepted");
    assert!(!read.contents.is_empty(), "the handler must have run");
}

/// A non-header-safe tool name round-trips through the `=?base64?…?=` sentinel.
///
/// This proves the client ENCODER and the server DECODER agree — the reason this
/// plan depends on 113-04, which shipped the decode half.
#[tokio::test]
async fn mcp_name_sentinel_for_non_ascii() {
    let (addr, handle) = spawn_default_config(build_server_with_unicode_tool()).await;
    let client = v2_client(addr);

    let result = client
        .call_tool(NON_ASCII_TOOL.to_string(), json!({}))
        .await;

    handle.abort();
    assert!(
        result.is_ok(),
        "a sentinel-encoded Mcp-Name must survive the server's cross-check: {result:?}"
    );
}

/// A v2 client completes a `tools/call` having sent NO `initialize` and no
/// `notifications/initialized`.
#[tokio::test]
async fn no_initialize_on_v2() {
    let (addr, handle, observed) = spawn_recording(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client.call_tool("search".to_string(), json!({})).await;

    handle.abort();
    assert!(result.is_ok(), "the call must succeed: {result:?}");
    assert!(
        observed.traffic_arrived.load(Ordering::SeqCst),
        "the recording middleware must have seen the traffic (guard against a vacuous pass)"
    );
    assert!(
        !observed.handshake.load(Ordering::SeqCst),
        "v2 has no handshake — neither initialize nor notifications/initialized may be sent"
    );
}

/// A v2 client never puts `Mcp-Session-Id` on the wire (T-113-06 / HTTP-01).
#[tokio::test]
async fn no_session_id_from_v2_client() {
    let (addr, handle, observed) = spawn_recording(build_v2_server()).await;
    let client = v2_client(addr);

    // Two round trips: if the server ever handed out a session id, a naive
    // client would echo it on the second request.
    let first = client.call_tool("search".to_string(), json!({})).await;
    let second = client.list_tools(None).await;

    handle.abort();
    assert!(first.is_ok(), "first call must succeed: {first:?}");
    assert!(second.is_ok(), "second call must succeed: {second:?}");
    assert!(
        !observed.inbound_session_id.load(Ordering::SeqCst),
        "no v2 request may carry Mcp-Session-Id, on any round trip"
    );
}

/// Regression guard for the `assert_capability` blocker.
///
/// `server_capabilities` is populated only by `initialize`, which v2 does not
/// have. Before this plan every `call_tool` on a v2 client failed LOCALLY, before
/// a byte was sent. This client never calls `server_discover`.
#[tokio::test]
async fn capability_check_does_not_block_v2() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    assert!(
        client.get_server_capabilities().is_none(),
        "this test is only meaningful with nothing observed"
    );
    let result = client.call_tool("search".to_string(), json!({})).await;

    handle.abort();
    assert!(
        result.is_ok(),
        "a v2 client must not fail a capability check it could not possibly have learned: {result:?}"
    );
}

/// `server/discover` — the v2 replacement for the `initialize` handshake.
///
/// It is EXPLICIT (pmcp never calls it implicitly, and never to CHOOSE an era —
/// D-08), and it STORES the projection, after which capability enforcement is as
/// strict as v1's.
#[tokio::test]
async fn server_discover_from_v2_client() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let mut client = v2_client(addr);

    let discovered = client.server_discover().await;
    let discovered = match discovered {
        Ok(value) => value,
        Err(error) => {
            handle.abort();
            panic!("server/discover must succeed on a v2 client: {error:?}");
        },
    };

    assert_eq!(discovered.protocol_version, V2);
    assert!(
        discovered.capabilities.tools.is_some(),
        "the projection must carry the server's real capabilities: {:?}",
        discovered.capabilities
    );
    assert!(
        client.get_server_capabilities().is_some(),
        "server_discover must STORE what it learned"
    );

    // And enforcement now runs against the DISCOVERED capabilities.
    let result = client.call_tool("search".to_string(), json!({})).await;
    handle.abort();
    assert!(
        result.is_ok(),
        "a discovered `tools` capability must let the call through: {result:?}"
    );
}

/// A client that never opted in is byte-identical to today: full `initialize`
/// handshake against the SAME server, then a normal `tools/call`.
#[tokio::test]
async fn v1_client_unchanged() {
    let (addr, handle, observed) = spawn_recording(build_v2_server()).await;
    let mut client = v1_client(addr);

    let init = client.initialize(ClientCapabilities::default()).await;
    let init = match init {
        Ok(value) => value,
        Err(error) => {
            handle.abort();
            panic!("a v1 client must still handshake: {error:?}");
        },
    };
    assert_eq!(init.protocol_version.as_str(), V1);

    let result = client.call_tool("search".to_string(), json!({})).await;
    handle.abort();

    assert!(result.is_ok(), "a v1 tools/call must succeed: {result:?}");
    assert!(
        observed.handshake.load(Ordering::SeqCst),
        "the v1 path MUST still send initialize"
    );
}

/// Not one of the nine required tests, but the cheapest proof that the whole
/// prompt surface is reachable too: `prompts/get` derives `Mcp-Name` from
/// `params.name`, the third row of the shared table.
#[tokio::test]
async fn mcp_name_from_name_for_prompts_get() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client
        .get_prompt("greeting".to_string(), std::collections::HashMap::new())
        .await;

    handle.abort();
    let prompt = result.expect("a v2 prompts/get must be accepted");
    // Assert on something the handler actually produces. The previous
    // `is_empty() || !is_empty()` was a tautology and pinned nothing.
    assert_eq!(
        prompt.description.as_deref(),
        Some("greeting"),
        "the response must come from the registered GreetingPrompt handler"
    );
}

// ===========================================================================
// Phase 113-07 (CLNT-02): the MRTR gather->resend loop, over a MOCK transport.
// ===========================================================================
//
// # Why a mock and not the live server above
//
// No handler in this repo can emit an `input_required` result on demand until
// plan 09 ships the egress hardening, and plan 07 deliberately does NOT depend
// on plan 09 (Codex Plan-07 HIGH #2). The live client<->server MRTR acceptance
// is plan 11's, which depends on both. What this suite must prove is the
// CLIENT's own behavior against an adversarial script: fresh ids, stale-field-free
// resends, verbatim `requestState`, the all-or-nothing fold, the terminal
// non-`input_required` case, and the bound — none of which needs a real server.
//
// The mock records the frames the client actually put on the wire, so every
// assertion is against observed bytes rather than against log output.

mod mrtr {
    use super::*;

    use std::collections::{HashMap, VecDeque};
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;

    use pmcp::client::host::HostElicitationHandler;
    use pmcp::shared::Transport;
    use pmcp::types::elicitation::{ElicitAction, ElicitRequestParams, ElicitResult};
    use pmcp::types::roots::{ListRootsResult, Root};
    use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
    use pmcp::types::{Content, JSONRPCResponse, MrtrOutcome, RequestId, TransportMessage};

    // ----------------------------------------------------------------------
    // The mock transport.
    // ----------------------------------------------------------------------

    /// A transport that replays a canned sequence of JSON-RPC RESULTS and
    /// records every frame the client sent.
    ///
    /// Deliberately answers on the RAW (`send_raw`) path only: that is the only
    /// path a v2 client uses, so a regression that quietly fell back to the
    /// typed `send` would fail here rather than pass silently.
    #[derive(Debug, Clone)]
    struct MockV2Transport {
        /// `result` objects served in send order.
        script: Arc<Vec<Value>>,
        /// When the script runs out, repeat its last entry forever.
        repeat_last: bool,
        /// Every request frame the client put on the wire, in order.
        sent: Arc<Mutex<Vec<Value>>>,
        /// Responses waiting to be `receive`d.
        inbox: Arc<Mutex<VecDeque<TransportMessage>>>,
    }

    impl MockV2Transport {
        fn new(script: Vec<Value>, repeat_last: bool) -> Self {
            Self {
                script: Arc::new(script),
                repeat_last,
                sent: Arc::new(Mutex::new(Vec::new())),
                inbox: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        /// The frames observed so far.
        fn sent(&self) -> Vec<Value> {
            self.sent.lock().expect("no panic while holding").clone()
        }

        /// The `params` object of the `index`-th frame the client sent.
        fn params(&self, index: usize) -> Value {
            self.sent()
                .get(index)
                .unwrap_or_else(|| panic!("no frame at index {index}"))
                .get("params")
                .cloned()
                .expect("a v2 frame always carries params")
        }
    }

    #[async_trait]
    impl Transport for MockV2Transport {
        async fn send(&mut self, _message: TransportMessage) -> pmcp::Result<()> {
            panic!("a v2 client must not use the TYPED send path");
        }

        async fn receive(&mut self) -> pmcp::Result<TransportMessage> {
            self.inbox
                .lock()
                .expect("no panic while holding")
                .pop_front()
                .ok_or_else(|| pmcp::Error::internal("the mock script is exhausted"))
        }

        async fn close(&mut self) -> pmcp::Result<()> {
            Ok(())
        }

        fn supports_negotiated_protocol_version(&self) -> bool {
            true
        }

        async fn send_raw(&mut self, body: Vec<u8>) -> pmcp::Result<()> {
            let frame: Value = serde_json::from_slice(&body).expect("the client sends valid JSON");
            let id: RequestId =
                serde_json::from_value(frame["id"].clone()).expect("every request carries an id");
            let index = {
                let mut sent = self.sent.lock().expect("no panic while holding");
                sent.push(frame);
                sent.len() - 1
            };
            let result = self
                .script
                .get(index)
                .or_else(|| self.repeat_last.then(|| self.script.last()).flatten())
                .unwrap_or_else(|| panic!("the script has no entry for send #{index}"))
                .clone();
            self.inbox
                .lock()
                .expect("no panic while holding")
                .push_back(TransportMessage::Response(JSONRPCResponse::success(
                    id, result,
                )));
            Ok(())
        }
    }

    // ----------------------------------------------------------------------
    // Script fixtures.
    // ----------------------------------------------------------------------

    /// An `input_required` result carrying `inputRequests` and a `requestState`.
    fn input_required(entries: &Value, request_state: Option<&str>) -> Value {
        let mut result = json!({
            "resultType": "input_required",
            "inputRequests": entries,
        });
        if let Some(state) = request_state {
            result["requestState"] = json!(state);
        }
        result
    }

    /// A completed `tools/call` result.
    fn complete() -> Value {
        json!({ "resultType": "complete", "content": [{ "type": "text", "text": "done" }] })
    }

    fn elicitation_entry() -> Value {
        json!({ "method": "elicitation/create", "params": { "message": "who?", "requestedSchema": {} } })
    }

    fn sampling_entry() -> Value {
        json!({ "method": "sampling/createMessage", "params": { "messages": [], "maxTokens": 16 } })
    }

    fn roots_entry() -> Value {
        json!({ "method": "roots/list" })
    }

    // ----------------------------------------------------------------------
    // Stub host handlers with invocation counters.
    // ----------------------------------------------------------------------

    struct CountingElicitation {
        calls: Arc<AtomicUsize>,
        action: ElicitAction,
    }

    #[async_trait]
    impl HostElicitationHandler for CountingElicitation {
        async fn handle_elicitation(
            &self,
            _params: ElicitRequestParams,
        ) -> pmcp::Result<ElicitResult> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut content = HashMap::new();
            content.insert("user_name".to_string(), json!("ada"));
            Ok(ElicitResult {
                action: self.action,
                content: matches!(self.action, ElicitAction::Accept).then_some(content),
            })
        }
    }

    struct CountingSampling {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl pmcp::client::host::HostSamplingHandler for CountingSampling {
        async fn handle_create_message(
            &self,
            _params: CreateMessageParams,
        ) -> pmcp::Result<CreateMessageResult> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CreateMessageResult::new(
                Content::text("sampled"),
                "mock-model",
            ))
        }
    }

    /// A v2 client over the mock, with an accepting elicitation handler whose
    /// invocations are counted.
    fn client_with_elicitation(
        transport: MockV2Transport,
        calls: Arc<AtomicUsize>,
        action: ElicitAction,
    ) -> Client<MockV2Transport> {
        ClientBuilder::new(transport)
            .with_protocol_version(ProtocolVersion(V2.to_string()))
            .expect("2026-07-28 is selectable")
            .on_elicitation(CountingElicitation { calls, action })
            .build()
    }

    /// A v2 client over the mock with NO host handlers at all.
    fn bare_client(transport: MockV2Transport) -> Client<MockV2Transport> {
        ClientBuilder::new(transport)
            .with_protocol_version(ProtocolVersion(V2.to_string()))
            .expect("2026-07-28 is selectable")
            .build()
    }

    // ----------------------------------------------------------------------
    // Tests.
    // ----------------------------------------------------------------------

    /// All three input kinds are fulfilled from the registry in ONE round, and
    /// the resend carries the server's keys plus the echoed `requestState`.
    #[tokio::test]
    async fn mrtr_three_kinds() {
        let entries = json!({
            "who": elicitation_entry(),
            "what": sampling_entry(),
            "where": roots_entry(),
        });
        let transport = MockV2Transport::new(
            vec![input_required(&entries, Some("state-1")), complete()],
            false,
        );
        let elicit_calls = Arc::new(AtomicUsize::new(0));
        let sample_calls = Arc::new(AtomicUsize::new(0));
        let client = ClientBuilder::new(transport.clone())
            .with_protocol_version(ProtocolVersion(V2.to_string()))
            .expect("2026-07-28 is selectable")
            .on_elicitation(CountingElicitation {
                calls: elicit_calls.clone(),
                action: ElicitAction::Accept,
            })
            .on_sampling(CountingSampling {
                calls: sample_calls.clone(),
            })
            .on_roots(|| async {
                Ok(ListRootsResult {
                    roots: vec![Root {
                        uri: "file:///tmp".to_string(),
                        name: None,
                    }],
                })
            })
            .build();

        let outcome = client
            .call_tool_mrtr("search".to_string(), json!({}))
            .await
            .expect("the loop completes");
        assert!(matches!(outcome, MrtrOutcome::Complete(_)));

        assert_eq!(transport.sent().len(), 2, "exactly one resend");
        assert_eq!(elicit_calls.load(Ordering::SeqCst), 1);
        assert_eq!(sample_calls.load(Ordering::SeqCst), 1);

        let retry = transport.params(1);
        let responses = retry["inputResponses"]
            .as_object()
            .expect("the resend carries inputResponses as a params sibling");
        assert_eq!(responses.len(), 3);
        for key in ["who", "what", "where"] {
            assert!(
                responses.contains_key(key),
                "the SERVER's key {key} must be preserved verbatim: {responses:?}"
            );
        }
        assert_eq!(
            retry["requestState"], "state-1",
            "requestState is echoed verbatim"
        );
        // ...and the original params survive the splice untouched.
        assert_eq!(retry["name"], "search");
    }

    /// Three rounds with an EVOLVING `requestState`, each echoed verbatim.
    #[tokio::test]
    async fn mrtr_multi_round() {
        let entries = json!({ "who": elicitation_entry() });
        let transport = MockV2Transport::new(
            vec![
                input_required(&entries, Some("state-1")),
                input_required(&entries, Some("state-2")),
                complete(),
            ],
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let client =
            client_with_elicitation(transport.clone(), calls.clone(), ElicitAction::Accept);

        let outcome = client
            .call_tool_mrtr("search".to_string(), json!({}))
            .await
            .expect("three rounds complete");
        assert!(matches!(outcome, MrtrOutcome::Complete(_)));

        assert_eq!(transport.sent().len(), 3);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "one handler invocation per LOGICAL round"
        );
        assert_eq!(transport.params(1)["requestState"], "state-1");
        assert_eq!(transport.params(2)["requestState"], "state-2");
    }

    /// `requestState` with NO `inputRequests` is server-side load shedding: the
    /// client resends immediately and invokes NO handler.
    #[tokio::test]
    async fn mrtr_load_shedding() {
        let transport = MockV2Transport::new(
            vec![
                json!({ "resultType": "input_required", "requestState": "shed-1" }),
                complete(),
            ],
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let client =
            client_with_elicitation(transport.clone(), calls.clone(), ElicitAction::Accept);

        let outcome = client
            .call_tool_mrtr("search".to_string(), json!({}))
            .await
            .expect("a state-only retry completes");
        assert!(matches!(outcome, MrtrOutcome::Complete(_)));

        assert_eq!(transport.sent().len(), 2);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "load shedding asks no questions, so no handler may run"
        );
        let retry = transport.params(1);
        assert_eq!(retry["requestState"], "shed-1");
        assert!(
            retry.get("inputResponses").is_none(),
            "nothing was asked, so nothing may be answered: {retry}"
        );
    }

    /// With NO registered handler the additive method returns the
    /// `input_required` result as a VALUE, and does not resend.
    #[tokio::test]
    async fn no_handler_returns_outcome() {
        let transport = MockV2Transport::new(
            vec![input_required(
                &json!({ "who": elicitation_entry() }),
                Some("state-1"),
            )],
            false,
        );
        let client = bare_client(transport.clone());

        let outcome = client
            .call_tool_mrtr("search".to_string(), json!({}))
            .await
            .expect("an unfulfillable result is not an error on the *_mrtr path");
        let MrtrOutcome::InputRequired(result) = outcome else {
            panic!("expected InputRequired");
        };
        assert_eq!(result.request_state.as_deref(), Some("state-1"));
        assert!(result.input_requests.is_some());
        assert_eq!(
            transport.sent().len(),
            1,
            "the client must NOT resend what it cannot answer"
        );
    }

    /// The SAME scenario through the EXISTING `call_tool` is a typed error
    /// carrying the full result — explicitly NOT an empty `CallToolResult`.
    #[tokio::test]
    async fn no_handler_existing_method_returns_typed_error() {
        let transport = MockV2Transport::new(
            vec![input_required(
                &json!({ "who": elicitation_entry() }),
                Some("state-1"),
            )],
            false,
        );
        let client = bare_client(transport.clone());

        let outcome = client.call_tool("search".to_string(), json!({})).await;
        let error = match outcome {
            Ok(result) => panic!(
                "an input_required result must NOT deserialize into a CallToolResult \
                 (content is #[serde(default)], so this would be a silently EMPTY success): \
                 {result:?}"
            ),
            Err(error) => error,
        };
        assert!(
            error.is_input_required_unfulfilled(),
            "the error must be programmatically distinguishable: {error}"
        );
        let recovered = error
            .input_required_result()
            .expect("the full result must be recoverable");
        assert_eq!(recovered.request_state.as_deref(), Some("state-1"));
        assert!(
            recovered.input_requests.is_some(),
            "the inputRequests the client could not answer must survive"
        );
        assert_eq!(transport.sent().len(), 1, "no resend");
    }

    /// A DECLINED elicitation is not a fulfilled input: no resend, and the
    /// caller receives the `input_required` result.
    #[tokio::test]
    async fn declined_elicitation_returns_outcome() {
        let transport = MockV2Transport::new(
            vec![input_required(
                &json!({ "who": elicitation_entry() }),
                Some("state-1"),
            )],
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let client =
            client_with_elicitation(transport.clone(), calls.clone(), ElicitAction::Decline);

        let outcome = client
            .call_tool_mrtr("search".to_string(), json!({}))
            .await
            .expect("a decline is a normal outcome, not a transport error");
        assert!(matches!(outcome, MrtrOutcome::InputRequired(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the handler DID run");
        assert_eq!(
            transport.sent().len(),
            1,
            "the user said no — the client must not answer on their behalf"
        );
    }

    /// Spec MUST: the JSON-RPC id differs between the initial request and every
    /// retry. Asserted on the ids the mock actually OBSERVED.
    #[tokio::test]
    async fn retry_uses_new_id() {
        let entries = json!({ "who": elicitation_entry() });
        let transport = MockV2Transport::new(
            vec![
                input_required(&entries, Some("s1")),
                input_required(&entries, Some("s2")),
                complete(),
            ],
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let client = client_with_elicitation(transport.clone(), calls, ElicitAction::Accept);

        client
            .call_tool_mrtr("search".to_string(), json!({}))
            .await
            .expect("the loop completes");

        let ids: Vec<Value> = transport
            .sent()
            .iter()
            .map(|frame| frame["id"].clone())
            .collect();
        assert_eq!(ids.len(), 3);
        for (i, left) in ids.iter().enumerate() {
            for right in ids.iter().skip(i + 1) {
                assert_ne!(left, right, "ids must be pairwise distinct: {ids:?}");
            }
        }
    }

    /// The resend carries EXACTLY the current round's MRTR fields — no key and
    /// no `requestState` from an earlier round survives (T-113-28).
    #[tokio::test]
    async fn retry_carries_no_stale_mrtr_fields() {
        let transport = MockV2Transport::new(
            vec![
                input_required(&json!({ "round_one_key": elicitation_entry() }), Some("s1")),
                input_required(&json!({ "round_two_key": elicitation_entry() }), Some("s2")),
                complete(),
            ],
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let client = client_with_elicitation(transport.clone(), calls, ElicitAction::Accept);

        client
            .call_tool_mrtr("search".to_string(), json!({}))
            .await
            .expect("the loop completes");

        let third = transport.params(2);
        let responses = third["inputResponses"]
            .as_object()
            .expect("round 3 answers round 2");
        assert_eq!(
            responses.keys().collect::<Vec<_>>(),
            vec!["round_two_key"],
            "round 3 must carry EXACTLY round 2's values: {responses:?}"
        );
        assert_eq!(third["requestState"], "s2");
    }

    /// A server that always re-elicits trips the bound: exactly `limit` sends,
    /// then a programmatically distinguishable error. No handler runs for the
    /// round that trips it.
    #[tokio::test]
    async fn round_limit() {
        let transport = MockV2Transport::new(
            vec![input_required(
                &json!({ "who": elicitation_entry() }),
                Some("forever"),
            )],
            true,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let client = ClientBuilder::new(transport.clone())
            .with_protocol_version(ProtocolVersion(V2.to_string()))
            .expect("2026-07-28 is selectable")
            .mrtr_round_limit(3)
            .on_elicitation(CountingElicitation {
                calls: calls.clone(),
                action: ElicitAction::Accept,
            })
            .build();

        let error = client
            .call_tool("search".to_string(), json!({}))
            .await
            .expect_err("a looping server must not loop the client forever");
        assert!(
            error.is_mrtr_round_limit_exceeded(),
            "the bound must be distinguishable: {error}"
        );
        assert_eq!(error.mrtr_round_limit(), Some(3));
        assert_eq!(
            transport.sent().len(),
            3,
            "exactly `limit` requests may leave the client"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "one handler invocation per round, and none after the bound trips"
        );
    }

    /// The default bound is 8 with no builder call.
    #[tokio::test]
    async fn default_round_limit_is_eight() {
        let transport = MockV2Transport::new(
            vec![input_required(
                &json!({ "who": elicitation_entry() }),
                Some("forever"),
            )],
            true,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let client = client_with_elicitation(transport.clone(), calls, ElicitAction::Accept);

        let error = client
            .call_tool("search".to_string(), json!({}))
            .await
            .expect_err("the default bound still trips");
        assert_eq!(error.mrtr_round_limit(), Some(8));
        assert_eq!(transport.sent().len(), 8);
    }

    /// Any `resultType` other than `input_required` is TERMINAL — including
    /// Phase 114's `"task"`, which this build has never heard of. The loop
    /// composes with later result types without modification.
    #[tokio::test]
    async fn non_input_required_result_type_is_terminal() {
        let transport = MockV2Transport::new(
            vec![json!({
                "resultType": "task",
                "content": [{ "type": "text", "text": "queued" }]
            })],
            false,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let client =
            client_with_elicitation(transport.clone(), calls.clone(), ElicitAction::Accept);

        let outcome = client
            .call_tool_mrtr("search".to_string(), json!({}))
            .await
            .expect("an unknown result type is returned, not retried");
        assert!(matches!(outcome, MrtrOutcome::Complete(_)));
        assert_eq!(transport.sent().len(), 1, "no retry");
        assert_eq!(calls.load(Ordering::SeqCst), 0, "no handler may run");
    }
}
