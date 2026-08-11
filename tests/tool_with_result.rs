//! TOUT-01 D-03 sugar-layer acceptance gate (Phase 104, Plan 04).
//!
//! Task 1 — `ServerBuilder::tool_with_result`: a closure returning a full
//! [`CallToolResult`] lands on the wire VERBATIM (its top-level `_meta` and
//! un-stringified `content` preserved), so a closure author can attach task
//! augmentation in one call without hand-writing a `ToolHandler`.
//!
//! Task 2 — `RequestHandlerExtra::set_result_meta`: an existing Payload-path
//! handler retrofits `_meta` with one call, round-tripping through the
//! encapsulated `Arc<std::sync::Mutex>` slot; merge precedence (handler-set key
//! overwrites same-name key, unrelated widget/native keys preserved), repeated
//! accumulation, no-op-when-never-called, and — FLIPPED by D-06 (Phase 118.1
//! plan 09) — drained-onto-`ToolOutput::Result` as well. That last case used to
//! assert the opposite (ignored on the verbatim path); the raw-wire twin of the
//! new contract, covering both dispatchers, lives in
//! `tests/tool_output_result_http.rs`.
//!
//! Both dispatchers are reachable, but these tests drive the high-level
//! `pmcp::Server` over an in-process duplex transport via a real `pmcp::Client`
//! (the sanctioned alternative — `Server::handle_request` is private).

#![cfg(all(not(target_arch = "wasm32"), feature = "schema-generation"))]

use async_trait::async_trait;
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::tasks::{TaskMetadata, RELATED_TASK_META_KEY};
use pmcp::types::{CallToolResult, ClientCapabilities, Content, ToolInfo};
use pmcp::{Client, Error, RequestHandlerExtra, Result, Server, ToolHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// In-process duplex transport (client <-> server), mpsc-backed.
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    fn pair() -> (Self, Self) {
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

/// Drive a `tools/call` through a real `pmcp::Client` against a high-level
/// `Server` running its own transport loop.
async fn call_via_server(server: Server, name: &str, args: Value) -> CallToolResult {
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

// ---------------------------------------------------------------------------
// Task 1: tool_with_result — verbatim full CallToolResult.
// ---------------------------------------------------------------------------

#[derive(Deserialize, JsonSchema)]
struct StartArgs {
    /// The job name to start.
    job: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_with_result_lands_verbatim_meta_on_wire() {
    let server = Server::builder()
        .name("tool-with-result-server")
        .version("1.0.0")
        .tool_with_result("start_job", |args: StartArgs, _extra| {
            Box::pin(async move {
                Ok(
                    CallToolResult::new(vec![Content::text(format!("started {}", args.job))])
                        .with_related_task(TaskMetadata::new("t1")),
                )
            })
        })
        .build()
        .expect("server builds");

    let result = call_via_server(server, "start_job", json!({ "job": "backfill" })).await;
    let v = serde_json::to_value(&result).expect("serialize CallToolResult");

    // Top-level related-task _meta survives verbatim.
    assert_eq!(
        v["_meta"][RELATED_TASK_META_KEY]["taskId"], "t1",
        "top-level _meta[related-task].taskId must survive verbatim"
    );

    // Content is the handler's verbatim text — NOT a stringified envelope.
    let text = v["content"][0]["text"]
        .as_str()
        .expect("content[0].text is a string");
    assert_eq!(
        text, "started backfill",
        "content must be the handler's verbatim text, not a stringified value"
    );
    assert!(
        !text.contains(RELATED_TASK_META_KEY) && !text.contains("_meta"),
        "content must NOT be a stringified envelope (double-wrap bug)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_with_result_deserializes_typed_input() {
    // A closure that echoes a typed field proves TIn deserialization happens.
    let server = Server::builder()
        .name("typed-input-server")
        .version("1.0.0")
        .tool_with_result("echo_job", |args: StartArgs, _extra| {
            Box::pin(async move { Ok(CallToolResult::new(vec![Content::text(args.job)])) })
        })
        .build()
        .expect("server builds");

    let result = call_via_server(server, "echo_job", json!({ "job": "hello-typed" })).await;
    let v = serde_json::to_value(&result).expect("serialize");
    assert_eq!(v["content"][0]["text"], "hello-typed");
}

/// `tool_with_result_and_description` advertises the description in
/// `tools/list` AND keeps the verbatim `ToolOutput::Result` semantics (WR-04:
/// the flagship migration sugar must not register description-less tools).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_with_result_and_description_advertises_description() {
    const DESCRIPTION: &str = "Starts a background export job";
    let server = Server::builder()
        .name("desc-server")
        .version("1.0.0")
        .tool_with_result_and_description("start_job", DESCRIPTION, |args: StartArgs, _extra| {
            Box::pin(async move {
                Ok(CallToolResult::new(vec![Content::text(args.job)])
                    .with_related_task(TaskMetadata::new("t1")))
            })
        })
        .build()
        .expect("server builds");

    let (client_t, server_t) = DuplexTransport::pair();
    tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });
    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes against server");

    // The description lands in tools/list.
    let tools = client.list_tools(None).await.expect("tools/list succeeds");
    let tool = tools
        .tools
        .iter()
        .find(|t| t.name == "start_job")
        .expect("start_job must be advertised");
    assert_eq!(
        tool.description.as_deref(),
        Some(DESCRIPTION),
        "tool_with_result_and_description must advertise the description in tools/list"
    );

    // And the verbatim Result semantics are unchanged.
    let result = client
        .call_tool("start_job".to_string(), json!({ "job": "backfill" }))
        .await
        .expect("tools/call succeeds");
    let v = serde_json::to_value(&result).expect("serialize");
    assert_eq!(v["_meta"][RELATED_TASK_META_KEY]["taskId"], "t1");
    assert_eq!(v["content"][0]["text"], "backfill");
}

// ---------------------------------------------------------------------------
// Task 2: RequestHandlerExtra::set_result_meta — Payload-path _meta retrofit.
// ---------------------------------------------------------------------------

/// A Payload-path tool (returns a plain Value) that stamps a single `_meta` key
/// via `set_result_meta` — the one-call retrofit for an existing handler.
fn meta_setting_tool() -> impl ToolHandler {
    TypedTool::new_with_schema(
        "set_meta",
        json!({ "type": "object" }),
        |_args: Value, extra: RequestHandlerExtra| {
            Box::pin(async move {
                let mut m = serde_json::Map::new();
                m.insert("x".to_string(), json!(1));
                extra.set_result_meta(m);
                Ok(json!({ "ok": true }))
            })
        },
    )
}

/// A Payload-path tool that calls `set_result_meta` TWICE — proving repeated
/// calls accumulate into the same slot and a later colliding key wins.
fn repeated_meta_tool() -> impl ToolHandler {
    TypedTool::new_with_schema(
        "repeat_meta",
        json!({ "type": "object" }),
        |_args: Value, extra: RequestHandlerExtra| {
            Box::pin(async move {
                let mut first = serde_json::Map::new();
                first.insert("k".to_string(), json!(1));
                first.insert("keep".to_string(), json!("a"));
                extra.set_result_meta(first);

                let mut second = serde_json::Map::new();
                second.insert("k".to_string(), json!(2)); // collides -> later wins
                second.insert("added".to_string(), json!("b"));
                extra.set_result_meta(second);

                Ok(json!({ "ok": true }))
            })
        },
    )
}

/// A plain Payload-path tool that never touches `set_result_meta` — proves no
/// `_meta` is injected (no regression for existing handlers).
fn plain_tool() -> impl ToolHandler {
    TypedTool::new_with_schema(
        "plain",
        json!({ "type": "object" }),
        |_args: Value, _extra| Box::pin(async { Ok(json!({ "answer": 42 })) }),
    )
}

/// A widget tool (its metadata `_meta` carries widget + `openai/toolInvocation/*`
/// keys, so the dispatch tail pre-populates the result `_meta`) whose handler
/// also calls `set_result_meta`, colliding on ONE widget key and adding another.
/// Proves handler-key-wins on collision while unrelated widget keys survive.
struct WidgetCollisionTool;

#[async_trait]
impl ToolHandler for WidgetCollisionTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        let mut m = serde_json::Map::new();
        m.insert(
            "openai/toolInvocation/foo".to_string(),
            json!("handler-wins"),
        );
        m.insert("custom".to_string(), json!(99));
        extra.set_result_meta(m);
        Ok(json!({ "answer": 1 }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(
            ToolInfo::new("widget_collide", None, json!({ "type": "object" }))
                // Triggers widget_meta() so with_widget_enrichment fires.
                .with_meta_entry("openai/outputTemplate", json!("ui://widget"))
                // Copied verbatim into the result _meta by with_widget_enrichment.
                .with_meta_entry("openai/toolInvocation/foo", json!("widget-original"))
                .with_meta_entry("openai/toolInvocation/keep", json!("preserved")),
        )
    }
}

/// A `tool_with_result` (verbatim `ToolOutput::Result`) tool that ALSO calls
/// `set_result_meta`.
///
/// FLIPPED by D-06 (Phase 118.1 plan 09). This fixture used to lock the OPPOSITE
/// contract — the slot was IGNORED on the Result path — and the assertion below
/// was `_meta` is absent. That drop was load-bearing rather than intentional:
/// G-3's server-to-client elicitation wiring runs straight through this arm, so
/// a handler that retrofitted `_meta` with one call silently shipped none. The
/// verbatim arm still bypasses response middleware, the task create-path gate
/// and the text-wrap tail; it now ALSO drains the handler's own `_meta` slot,
/// which is authored by the same handler at the same trust level as the envelope
/// it returns.
fn result_path_drains_set_meta_tool(server_name: &str) -> Server {
    Server::builder()
        .name(server_name)
        .version("1.0.0")
        .tool_with_result("verbatim_drain", |_args: StartArgs, extra| {
            Box::pin(async move {
                let mut m = serde_json::Map::new();
                m.insert("drained".to_string(), json!(true));
                extra.set_result_meta(m);
                Ok(CallToolResult::new(vec![Content::text("owned")]))
            })
        })
        .build()
        .expect("server builds")
}

fn server_with(name: &str, tool_name: &str, handler: impl ToolHandler + 'static) -> Server {
    Server::builder()
        .name(name)
        .version("1.0.0")
        .tool(tool_name, handler)
        .build()
        .expect("server builds")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_result_meta_key_lands_on_payload_result() {
    let server = server_with("set-meta-server", "set_meta", meta_setting_tool());
    let result = call_via_server(server, "set_meta", json!({})).await;
    let v = serde_json::to_value(&result).expect("serialize");
    assert_eq!(
        v["_meta"]["x"], 1,
        "handler-set _meta key must land on the Payload-built result"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_result_meta_repeated_calls_accumulate_and_later_wins() {
    let server = server_with("repeat-meta-server", "repeat_meta", repeated_meta_tool());
    let result = call_via_server(server, "repeat_meta", json!({})).await;
    let v = serde_json::to_value(&result).expect("serialize");
    assert_eq!(v["_meta"]["k"], 2, "later colliding key must win");
    assert_eq!(
        v["_meta"]["keep"], "a",
        "non-colliding key from the first call must survive"
    );
    assert_eq!(
        v["_meta"]["added"], "b",
        "non-colliding key from the second call must survive"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_result_meta_collision_overwrites_widget_key_preserves_unrelated() {
    let server = server_with(
        "widget-collide-server",
        "widget_collide",
        WidgetCollisionTool,
    );
    let result = call_via_server(server, "widget_collide", json!({})).await;
    let v = serde_json::to_value(&result).expect("serialize");
    // Handler-set key overwrites the same-name widget key...
    assert_eq!(
        v["_meta"]["openai/toolInvocation/foo"], "handler-wins",
        "handler-set key must overwrite the same-name widget _meta key"
    );
    // ...unrelated widget key preserved (never a whole-map replace)...
    assert_eq!(
        v["_meta"]["openai/toolInvocation/keep"], "preserved",
        "unrelated widget _meta key must be preserved"
    );
    // ...and the new handler key is added.
    assert_eq!(
        v["_meta"]["custom"], 99,
        "new handler-set key must be added alongside preserved widget keys"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_set_result_meta_injects_nothing() {
    let server = server_with("plain-server", "plain", plain_tool());
    let result = call_via_server(server, "plain", json!({})).await;
    let v = serde_json::to_value(&result).expect("serialize");
    assert!(
        v.get("_meta").is_none_or(Value::is_null),
        "a handler that never calls set_result_meta must inject no _meta"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_result_meta_drains_onto_tool_output_result_path() {
    let server = result_path_drains_set_meta_tool("verbatim-drain-server");
    let result = call_via_server(server, "verbatim_drain", json!({ "job": "x" })).await;
    let v = serde_json::to_value(&result).expect("serialize");
    // The handler still owns its CONTENT verbatim — D-06 touches `_meta` only.
    assert_eq!(v["content"][0]["text"], "owned");
    assert_eq!(
        v["_meta"]["drained"],
        json!(true),
        "D-06: set_result_meta must reach the wire on the ToolOutput::Result path too, got: {v}"
    );
}
