//! D-14 live-HTTP `_meta`-at-top-level acceptance gate (Phase 104, TOUT-04).
//!
//! This is the phase's wire-shape regression lock: a REAL HTTP round-trip (no
//! in-process shim) that drives a `tools/call` against a high-level, store-less
//! `pmcp::Server` whose tool returns
//! [`ToolOutput::Result`](pmcp::ToolOutput::Result) carrying a `CallToolResult` — the
//! SEP-1686 task-augmented result path (Plan 02/04). It consumes the REAL
//! dispatch output over `StreamableHttpServer` + `StreamableHttpTransport` and
//! asserts on the RAW wire JSON that:
//!
//! 1. `result._meta` is present at the result TOP LEVEL (the `_meta[related-task]`
//!    envelope survives dispatch — a `_meta`-sniffing client detects the task); AND
//! 2. `result.content[0].text`, if present, does NOT contain a stringified `_meta`
//!    (the agent-lake double-wrap bug — the whole `CallToolResult` serialized into
//!    `content[0].text` — must NEVER reappear).
//!
//! We read the raw JSON via the transport's `TransportMessage::Response` payload,
//! which carries the `tools/call` result as an untyped `serde_json::Value` — NOT
//! deserialized into `CallToolResult` — so the wire shape is observed faithfully
//! and never from a hand-authored fixture (the note's ask #4).
//!
//! Test reliability (carried from the Phase 102 HTTP harness):
//! - EPHEMERAL PORT — binds `127.0.0.1:0`, reads the bound address back from
//!   `StreamableHttpServer::start()` (no hardcoded port).
//! - READINESS — `start()` binds the listener before returning (no fixed sleep).
//! - SHUTDOWN — the spawned server `JoinHandle` is `abort()`ed after the round-trip.
//!
//! # D-06 (Phase 118.1 plan 09): the `set_result_meta` drain on this same path
//!
//! The verbatim arm used to return BEFORE the dispatchers drained the
//! handler-authored `_meta` slot, so `extra.set_result_meta(..)` was silently
//! discarded for any handler returning [`ToolOutput::Result`]. G-3's elicitation
//! wiring runs straight through this arm, so the drop is load-bearing rather
//! than cosmetic. The cases below extend this file — the one that already owns
//! the raw-wire assertion for this exact path — with:
//!
//! * `Server` (over REAL HTTP) — a handler-set key reaches `result._meta`;
//! * handler-key-wins — a handler key COLLIDING with one the envelope already
//!   carries overwrites it, while the envelope's unrelated key survives;
//! * `ServerCore` — the twin dispatcher, read at its rawest (`ResponsePayload`);
//! * no-opt-in — a handler that never calls `set_result_meta` emits exactly the
//!   envelope it authored, key-for-key (no injected keys, no regression).

#![cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::jsonrpc::ResponsePayload;
use pmcp::types::tasks::{TaskMetadata, RELATED_TASK_META_KEY};
use pmcp::types::{
    CallToolRequest, CallToolResult, ClientCapabilities, ClientRequest, Content, Implementation,
    InitializeRequest, Request, RequestId,
};
use pmcp::{RequestHandlerExtra, Server, ToolHandler, ToolOutput};
use tokio::sync::Mutex;
use url::Url;

/// The task id the tool stamps into the related-task `_meta` envelope.
const RELATED_TASK_ID: &str = "t-http";

/// A tool whose `handle_output` returns [`ToolOutput::Result`] — a fully-formed
/// `CallToolResult` carrying a top-level `_meta[related-task]` envelope that must
/// reach the wire VERBATIM.
struct AugmentedResultTool;

impl AugmentedResultTool {
    /// The verbatim envelope: `content = [text("done")]` + related-task `_meta`.
    fn envelope() -> CallToolResult {
        CallToolResult::new(vec![Content::text("done")])
            .with_related_task(TaskMetadata::new(RELATED_TASK_ID))
    }
}

#[async_trait]
impl ToolHandler for AugmentedResultTool {
    async fn handle(
        &self,
        _args: serde_json::Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        // Serialize fallback for non-dispatch callers; the dispatch path uses
        // `handle_output` below.
        Ok(serde_json::to_value(Self::envelope())?)
    }

    async fn handle_output(
        &self,
        _args: serde_json::Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ToolOutput> {
        // Verbatim: the handler owns the full envelope (Plan 02/04, D-04a).
        Ok(ToolOutput::Result(Self::envelope()))
    }
}

/// Build a high-level, store-LESS `Server` exposing the augmented-result tool.
///
/// No `task_store` here: this is a plain synchronous tool result that merely
/// POINTS at a related task via `_meta` — exactly the SEP-1686 shape whose wire
/// survival this gate locks.
fn build_server() -> pmcp::Result<Server> {
    Server::builder()
        .name("tool-output-result-http")
        .version("1.0.0")
        .tool("augmented", AugmentedResultTool)
        .tool("meta_drain", MetaDrainTool)
        .build()
}

/// The `ServerCore` twin of [`build_server`], exposing the SAME two tools.
///
/// Driven through [`ProtocolHandler::handle_request`] directly rather than over
/// a transport: that is the rawest possible read of the dispatcher's output — a
/// `serde_json::Value` straight off `ResponsePayload::Result`, never a
/// deserialized `CallToolResult` (a round-trip through the typed struct would
/// hide an emission bug, which is the whole point of this file).
fn build_core() -> pmcp::Result<Arc<dyn ProtocolHandler>> {
    Ok(Arc::new(
        ServerCoreBuilder::new()
            .name("tool-output-result-core")
            .version("1.0.0")
            .tool("augmented", AugmentedResultTool)
            .tool("meta_drain", MetaDrainTool)
            .build()?,
    ))
}

/// Stand the server up over REAL HTTP; return the read-back bound address + handle.
///
/// EPHEMERAL PORT + READINESS: bind `127.0.0.1:0` and read the assigned address
/// back from `start()`, which binds the listener before returning (no sleep).
async fn spawn_http_server() -> pmcp::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let server = Arc::new(Mutex::new(build_server()?));
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback addr");
    let http_server = StreamableHttpServer::new(bind_addr, server);
    let (bound, handle) = http_server.start().await?;
    Ok((bound, handle))
}

/// Build the pmcp HTTP transport pointed at `bound` (single-response JSON path).
fn http_transport(bound: SocketAddr) -> pmcp::Result<StreamableHttpTransport> {
    // Builder, not a struct literal: `session_id` / `on_resumption_token` are
    // `v1-compat`-only, so a literal naming them breaks the `full-v2` build.
    let config = StreamableHttpTransportConfigBuilder::new(
        Url::parse(&format!("http://{bound}")).map_err(|e| pmcp::Error::Internal(e.to_string()))?,
    )
    .enable_json_response()
    .build();
    Ok(StreamableHttpTransport::new(config))
}

/// Extract the raw JSON-RPC `result` Value from a received response message.
fn expect_result_value(msg: TransportMessage) -> pmcp::Result<serde_json::Value> {
    match msg {
        TransportMessage::Response(resp) => match resp.payload {
            ResponsePayload::Result(value) => Ok(value),
            ResponsePayload::Error(err) => Err(pmcp::Error::internal(format!(
                "expected a result over HTTP, got JSON-RPC error: {}",
                err.message
            ))),
        },
        other => Err(pmcp::Error::internal(format!(
            "expected a Response message, got: {other:?}"
        ))),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_output_result_carries_meta_at_top_level_over_http() -> pmcp::Result<()> {
    let (bound, server_handle) = spawn_http_server().await?;

    // Guard the round-trip so the server handle is ALWAYS aborted, even on failure.
    let outcome = run_round_trip(bound).await;

    // SHUTDOWN: abort the spawned server task (no lingering listener, no hang).
    server_handle.abort();
    match server_handle.await {
        Ok(()) => {},
        Err(e) if e.is_cancelled() => {},
        Err(e) => panic!("HTTP server task ended unexpectedly: {e}"),
    }

    outcome
}

async fn run_round_trip(bound: SocketAddr) -> pmcp::Result<()> {
    let mut transport = http_transport(bound)?;

    // 1) initialize over HTTP (establishes the session + protocol version that the
    //    transport reuses on subsequent sends).
    transport
        .send(TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("tout-04-gate", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        })
        .await?;
    let _init = transport.receive().await?;

    // 2) tools/call over the REAL transport -> consume the RAW dispatch output.
    transport
        .send(TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
                "augmented",
                serde_json::json!({}),
            )))),
        })
        .await?;
    let result = expect_result_value(transport.receive().await?)?;

    // ASSERTION 1 — `_meta` survives at the result TOP LEVEL.
    let meta = result.get("_meta").ok_or_else(|| {
        pmcp::Error::internal(format!(
            "result._meta must be present at top level over HTTP; got: {result}"
        ))
    })?;
    assert!(
        meta.get("io.modelcontextprotocol/related-task").is_some(),
        "result._meta must carry the related-task envelope, got: {meta}"
    );
    assert_eq!(
        meta["io.modelcontextprotocol/related-task"]["taskId"].as_str(),
        Some(RELATED_TASK_ID),
        "related-task taskId must equal the tool's minted id over HTTP"
    );

    // ASSERTION 2 — content[0].text (if any) must NOT be a stringified envelope.
    if let Some(text) = result
        .get("content")
        .and_then(|c| c.get(0))
        .and_then(|c0| c0.get("text"))
        .and_then(serde_json::Value::as_str)
    {
        assert_eq!(
            text, "done",
            "content[0].text must be the real text, not a wrapped envelope"
        );
        assert!(
            !text.contains("_meta"),
            "content[0].text must NOT contain a stringified `_meta` (the double-wrap bug), got: {text}"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// D-06: the `set_result_meta` drain on the verbatim `ToolOutput::Result` path.
// ---------------------------------------------------------------------------

/// A key the ENVELOPE carries and the handler never touches — must survive the
/// merge (proves the drain merges, never whole-map replaces).
const ENVELOPE_ONLY_KEY: &str = "com.example/envelope-only";

/// The value stamped under [`ENVELOPE_ONLY_KEY`].
const ENVELOPE_ONLY_VALUE: &str = "kept";

/// A key ONLY `set_result_meta` supplies — must appear on the wire.
const HANDLER_ONLY_KEY: &str = "com.example/handler-only";

/// The value stamped under [`HANDLER_ONLY_KEY`].
const HANDLER_ONLY_VALUE: &str = "added";

/// The related-task id the ENVELOPE carries; the handler collides with it and
/// must WIN, so seeing this id on the wire means the drain lost the collision.
const ENVELOPE_TASK_ID: &str = "t-envelope-loses";

/// The related-task id `set_result_meta` supplies for the SAME key.
const HANDLER_TASK_ID: &str = "t-handler-wins";

/// A tool that returns [`ToolOutput::Result`] (the verbatim arm) AND calls
/// `extra.set_result_meta(..)`, so the two `_meta` sources collide on purpose.
///
/// The envelope carries `{ENVELOPE_ONLY_KEY, related-task = ENVELOPE_TASK_ID}`;
/// the handler-authored slot carries `{HANDLER_ONLY_KEY,
/// related-task = HANDLER_TASK_ID}`. Correct behaviour is the UNION with
/// handler-key-wins on the collision.
struct MetaDrainTool;

impl MetaDrainTool {
    /// The envelope the handler owns, before any drain.
    fn envelope() -> CallToolResult {
        let mut meta = serde_json::Map::new();
        meta.insert(
            ENVELOPE_ONLY_KEY.to_string(),
            serde_json::Value::String(ENVELOPE_ONLY_VALUE.to_string()),
        );
        // `with_meta` sets the map; `with_related_task` then PRESERVES it while
        // adding the colliding related-task key.
        CallToolResult::new(vec![Content::text("drained")])
            .with_meta(meta)
            .with_related_task(TaskMetadata::new(ENVELOPE_TASK_ID))
    }

    /// The handler-authored `_meta` pushed through `extra.set_result_meta`.
    fn handler_meta() -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();
        meta.insert(
            HANDLER_ONLY_KEY.to_string(),
            serde_json::Value::String(HANDLER_ONLY_VALUE.to_string()),
        );
        meta.insert(
            RELATED_TASK_META_KEY.to_string(),
            serde_json::json!({ "taskId": HANDLER_TASK_ID }),
        );
        meta
    }
}

#[async_trait]
impl ToolHandler for MetaDrainTool {
    async fn handle(
        &self,
        _args: serde_json::Value,
        extra: RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        extra.set_result_meta(Self::handler_meta());
        Ok(serde_json::to_value(Self::envelope())?)
    }

    async fn handle_output(
        &self,
        _args: serde_json::Value,
        extra: RequestHandlerExtra,
    ) -> pmcp::Result<ToolOutput> {
        // Interior mutability: the dispatcher cloned the slot handle BEFORE
        // `extra` moved in here, so this call stays observable after we return.
        extra.set_result_meta(Self::handler_meta());
        Ok(ToolOutput::Result(Self::envelope()))
    }
}

/// Assert the drained union on a RAW wire `result` object.
fn assert_drained_union(result: &serde_json::Value) {
    let meta = result.get("_meta").unwrap_or_else(|| {
        panic!("result._meta must be present after the D-06 drain; got: {result}")
    });

    assert_eq!(
        meta.get(HANDLER_ONLY_KEY).and_then(serde_json::Value::as_str),
        Some(HANDLER_ONLY_VALUE),
        "a handler-only `set_result_meta` key must reach the wire on the verbatim path, got: {meta}"
    );
    assert_eq!(
        meta.get(ENVELOPE_ONLY_KEY)
            .and_then(serde_json::Value::as_str),
        Some(ENVELOPE_ONLY_VALUE),
        "the envelope's unrelated key must SURVIVE the merge (never a whole-map replace), got: {meta}"
    );
    assert_eq!(
        meta[RELATED_TASK_META_KEY]["taskId"].as_str(),
        Some(HANDLER_TASK_ID),
        "handler-key-wins: the handler's colliding key must overwrite the envelope's, got: {meta}"
    );
}

/// Drive one `tools/call` at a `ServerCore` and return the RAW result `Value`.
///
/// `ServerCore` gates non-`initialize` v1 requests behind its initialize gate,
/// so the handshake runs first (its response is discarded — this file asserts
/// only on the `tools/call` payload).
async fn call_tool_via_core(
    core: &Arc<dyn ProtocolHandler>,
    tool: &str,
) -> pmcp::Result<serde_json::Value> {
    let init = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
        Implementation::new("tout-04-core-gate", "1.0.0"),
        ClientCapabilities::default(),
    ))));
    let _init = core.handle_request(RequestId::from(0i64), init, None).await;

    let call = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
        tool,
        serde_json::json!({}),
    ))));
    let response = core.handle_request(RequestId::from(1i64), call, None).await;
    match response.payload {
        ResponsePayload::Result(value) => Ok(value),
        ResponsePayload::Error(err) => Err(pmcp::Error::internal(format!(
            "expected a result from ServerCore, got JSON-RPC error: {}",
            err.message
        ))),
    }
}

/// `Server` dispatcher, over REAL HTTP: the handler-authored `_meta` survives
/// the verbatim `ToolOutput::Result` arm, with handler-key-wins precedence.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_result_meta_survives_the_verbatim_path_over_http() -> pmcp::Result<()> {
    let (bound, server_handle) = spawn_http_server().await?;

    let outcome = run_meta_drain_round_trip(bound).await;

    server_handle.abort();
    match server_handle.await {
        Ok(()) => {},
        Err(e) if e.is_cancelled() => {},
        Err(e) => panic!("HTTP server task ended unexpectedly: {e}"),
    }

    outcome
}

async fn run_meta_drain_round_trip(bound: SocketAddr) -> pmcp::Result<()> {
    let mut transport = http_transport(bound)?;

    transport
        .send(TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("tout-04-gate", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        })
        .await?;
    let _init = transport.receive().await?;

    transport
        .send(TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
                "meta_drain",
                serde_json::json!({}),
            )))),
        })
        .await?;
    let result = expect_result_value(transport.receive().await?)?;

    assert_drained_union(&result);

    // The verbatim arm still owns its CONTENT — the drain touches `_meta` only.
    assert_eq!(
        result["content"][0]["text"].as_str(),
        Some("drained"),
        "the drain must not disturb the handler's verbatim content, got: {result}"
    );

    Ok(())
}

/// `ServerCore` dispatcher (the twin): same drain, same precedence.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_result_meta_survives_the_verbatim_path_on_core() -> pmcp::Result<()> {
    let core = build_core()?;
    let result = call_tool_via_core(&core, "meta_drain").await?;

    assert_drained_union(&result);
    assert_eq!(
        result["content"][0]["text"].as_str(),
        Some("drained"),
        "the drain must not disturb the handler's verbatim content, got: {result}"
    );

    Ok(())
}

/// NO-REGRESSION: a handler that never calls `set_result_meta` emits exactly the
/// `_meta` it authored — same key count, same key — so the drain injects nothing
/// on the opt-out path. Asserted here on the `ServerCore` twin; the
/// `Server`/HTTP half is
/// [`tool_output_result_carries_meta_at_top_level_over_http`].
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_opt_in_handler_emits_its_envelope_meta_unchanged_on_core() -> pmcp::Result<()> {
    let core = build_core()?;
    let result = call_tool_via_core(&core, "augmented").await?;

    let meta = result
        .get("_meta")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("result._meta must be an object, got: {result}"));

    assert_eq!(
        meta.len(),
        1,
        "a handler that never opts into set_result_meta must emit EXACTLY its own \
         envelope keys — no injected keys: {meta:?}"
    );
    assert_eq!(
        meta[RELATED_TASK_META_KEY]["taskId"].as_str(),
        Some(RELATED_TASK_ID),
        "the sole key must be the envelope's own related-task, got: {meta:?}"
    );

    Ok(())
}
