//! Real `Server::run` + real `Client` proof for the Phase 108 Transport Actor
//! (D-01/D-02/D-03).
//!
//! Unlike `tests/client_host_roundtrip.rs` (which drives the server side by hand
//! with a raw pump because the OLD serialized loop could not answer an in-tool
//! `peer.sample()`), these cases run the STOCK high-level `Server::run` against a
//! STOCK `Client`. They prove the never-block transport actor:
//!
//!   * `sampling`   — a tool that awaits `extra.peer().sample()` completes.
//!   * `list_roots` — a tool that awaits `extra.peer().list_roots()` completes.
//!   * `saturation` — a SECOND `tools/call` queued while the first handler is
//!     parked on its sampling round-trip is still received and processed (the
//!     receive path never blocks on request execution / queue capacity).
//!   * `shutdown`   — closing the transport makes `run()` return without hanging.
//!   * `with_tools` — (Task 3) end-to-end `sample_with_tools` carrying a
//!     `ToolUse` block, added alongside the Task 2 `WithTools` surface.
//!   * `elicit`     — (Phase 118.1 plan 09, D-07) a tool that awaits
//!     `extra.peer().elicit()` receives the host's `ElicitResult` intact, and a
//!     `Decline` arrives as a decline rather than as an empty approval.
//!
//! # Why `elicit` is proved HERE, on the in-process loop, and not over HTTP
//!
//! 118-07 MEASURED the peer handle to be PRESENT under `Server::run()` and
//! ABSENT only under `StreamableHttpServer`. Proving `elicit` on the stock loop
//! therefore separates two questions that would otherwise be confounded: does
//! the elicit METHOD work, and does the HTTP TRANSPORT carry a peer at all.
//! Plan 10 wires the transport; if its HTTP twin of these cases fails, this file
//! is the baseline that says the transport is the cause.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::client::host::{
    HostElicitationHandler, HostSamplingHandler, HostSamplingHandlerWithTools,
};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::elicitation::{ElicitAction, ElicitRequestParams, ElicitResult};
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResult, CreateMessageResultWithTools, SamplingMessage,
    SamplingMessageContent,
};
use pmcp::types::{ClientCapabilities, Content, JSONRPCResponse, Request, RequestId, Role};
use pmcp::{ClientBuilder, RequestHandlerExtra, Result, Server, ToolHandler};

// ---------------------------------------------------------------------------
// Host sampling handler answering with a canned single-content completion.
// ---------------------------------------------------------------------------

struct CannedSampling {
    model: String,
}

#[async_trait]
impl HostSamplingHandler for CannedSampling {
    async fn handle_create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResult> {
        Ok(CreateMessageResult::new(
            Content::text("ok"),
            self.model.clone(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Host elicitation handler answering one prompt with accept and one with decline.
// ---------------------------------------------------------------------------

/// The two prompts [`CannedElicitation`] branches on.
///
/// `ElicitTool` takes its message from its tool arguments, so ONE registered
/// tool drives both outcomes and each test below names the branch it exercises
/// rather than relying on argument order.
const ELICIT_ACCEPT_MESSAGE: &str = "which environment?";
const ELICIT_DECLINE_MESSAGE: &str = "may I deploy to production?";

/// The field the accept branch collects, asserted on by the accept test.
const ELICIT_ACCEPT_ENV: &str = "staging";

struct CannedElicitation;

#[async_trait]
impl HostElicitationHandler for CannedElicitation {
    async fn handle_elicitation(&self, params: ElicitRequestParams) -> Result<ElicitResult> {
        let ElicitRequestParams::Form { message, .. } = &params else {
            panic!("fixture only drives the form shape, got {params:?}");
        };
        // A decline returns `content: None`, and that is the shape that matters
        // for D-07: the refusal must reach the tool AS a decline — not as an
        // `Err`, and not as an accept carrying an empty form. A server that
        // reads a refusal as approval is precisely the spoofing failure the
        // plan's threat model calls out (T-118.1-09-01).
        if message == ELICIT_DECLINE_MESSAGE {
            return Ok(ElicitResult {
                action: ElicitAction::Decline,
                content: None,
            });
        }
        Ok(ElicitResult {
            action: ElicitAction::Accept,
            content: Some(HashMap::from([(
                "env".to_string(),
                json!(ELICIT_ACCEPT_ENV),
            )])),
        })
    }
}

// ---------------------------------------------------------------------------
// Server tools that call back into the client via the peer handle.
// ---------------------------------------------------------------------------

/// Tool that awaits `extra.peer().sample()` and echoes the model name.
struct SamplerTool;

#[async_trait]
impl ToolHandler for SamplerTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra
            .peer()
            .expect("peer must be attached on the stock loop")
            .clone();
        let params = CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "summarize".to_string(),
                meta: None,
            },
        )]);
        let result = peer.sample(params).await?;
        Ok(json!(format!("sampled:{}", result.model)))
    }
}

/// Tool that awaits `extra.peer().list_roots()` and echoes the root count.
struct RootsTool;

#[async_trait]
impl ToolHandler for RootsTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra.peer().expect("peer must be attached").clone();
        let roots = peer.list_roots().await?;
        Ok(json!(format!("roots:{}", roots.roots.len())))
    }
}

/// Tool that awaits `extra.peer().sample_with_tools()` and reports the first
/// `tool_use` block it received (Task 3 / AGNT-04 proof).
struct SamplerWithToolsTool;

#[async_trait]
impl ToolHandler for SamplerWithToolsTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra.peer().expect("peer must be attached").clone();
        let params = CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "pick a tool".to_string(),
                meta: None,
            },
        )]);
        let result: CreateMessageResultWithTools = peer.sample_with_tools(params).await?;
        let tool_use = result
            .content
            .iter()
            .find_map(|c| match c {
                SamplingMessageContent::ToolUse { id, name, .. } => Some(format!("{name}#{id}")),
                _ => None,
            })
            .unwrap_or_else(|| "none".to_string());
        Ok(json!(format!("tooluse:{tool_use}")))
    }
}

/// Tool that awaits `extra.peer().elicit()` and echoes the action the host
/// returned plus the field it collected (D-07 end-to-end proof).
///
/// The `message` is taken from the tool arguments so ONE tool can drive both the
/// accept and the decline case — the host handler branches on it.
struct ElicitTool;

#[async_trait]
impl ToolHandler for ElicitTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra.peer().expect("peer must be attached").clone();
        let message = args
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(ELICIT_ACCEPT_MESSAGE)
            .to_string();
        let answer = peer
            .elicit(ElicitRequestParams::Form {
                message,
                requested_schema: json!({
                    "type": "object",
                    "properties": { "env": { "type": "string" } },
                    "required": ["env"],
                }),
            })
            .await?;
        let env = answer
            .content
            .as_ref()
            .and_then(|c| c.get("env"))
            .and_then(Value::as_str)
            .unwrap_or("<none>");
        Ok(json!(format!("elicit:{:?}:{env}", answer.action)))
    }
}

/// Trivial tool that returns immediately (no peer round-trip). Used to prove a
/// second request is drained + processed while another handler is parked.
struct FastTool;

#[async_trait]
impl ToolHandler for FastTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!("fast-done"))
    }
}

fn build_server() -> Server {
    Server::builder()
        .name("peer-roundtrip-server")
        .version("0.1.0")
        .tool("sampler", SamplerTool)
        .tool("roots", RootsTool)
        .tool("sampler_with_tools", SamplerWithToolsTool)
        .tool("elicit", ElicitTool)
        .tool("fast", FastTool)
        .build()
        .expect("server builds")
}

fn result_text(result: &pmcp::types::CallToolResult) -> String {
    serde_json::to_value(result).unwrap().to_string()
}

// ---------------------------------------------------------------------------
// (a) In-tool peer.sample() completes on the stock loop.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn in_tool_sample_completes_on_stock_loop() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling(CannedSampling {
            model: "host-model".to_string(),
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool("sampler".to_string(), json!({})),
    )
    .await
    .expect("call must not hang")
    .expect("tools/call succeeds");

    assert!(
        result_text(&result).contains("sampled:host-model"),
        "tool must observe the host completion model: {}",
        result_text(&result)
    );

    drop(client);
    server_handle.abort();
}

// ---------------------------------------------------------------------------
// (b) In-tool peer.list_roots() completes on the stock loop.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn in_tool_list_roots_completes_on_stock_loop() {
    use pmcp::types::roots::{ListRootsResult, Root};

    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_roots(|| async {
            Ok(ListRootsResult {
                roots: vec![
                    Root {
                        uri: "file:///a".to_string(),
                        name: Some("a".to_string()),
                    },
                    Root {
                        uri: "file:///b".to_string(),
                        name: Some("b".to_string()),
                    },
                ],
            })
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool("roots".to_string(), json!({})),
    )
    .await
    .expect("call must not hang")
    .expect("tools/call succeeds");

    assert!(
        result_text(&result).contains("roots:2"),
        "tool must observe the two host roots: {}",
        result_text(&result)
    );

    drop(client);
    server_handle.abort();
}

// ---------------------------------------------------------------------------
// (c) SATURATION: a second request queued while the first handler is parked on
// its sampling round-trip is still received and processed.
//
// Driven at the raw transport level because the high-level `Client` issues one
// request at a time; here we interleave two `tools/call`s by hand so the second
// lands while the worker is blocked awaiting the sampling answer.
// ---------------------------------------------------------------------------

/// Build an inbound client->server `Request` from method + params, bypassing the
/// `#[non_exhaustive]` request structs via deserialization.
fn client_req(method: &str, params: Value) -> Request {
    let mut obj = serde_json::Map::new();
    obj.insert("method".to_string(), Value::from(method));
    obj.insert("params".to_string(), params);
    let cr = serde_json::from_value(Value::Object(obj)).expect("valid ClientRequest");
    Request::Client(Box::new(cr))
}

fn init_params() -> Value {
    json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": { "name": "raw-test-client", "version": "1.0.0" }
    })
}

#[tokio::test]
async fn second_request_is_processed_while_first_handler_parks() {
    let (mut client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    // Handshake.
    client_t
        .send(TransportMessage::Request {
            id: RequestId::from(0i64),
            request: client_req("initialize", init_params()),
        })
        .await
        .unwrap();

    // Read until the initialize response arrives.
    loop {
        if let TransportMessage::Response(r) = client_t.receive().await.unwrap() {
            if r.id == RequestId::from(0i64) {
                break;
            }
        }
    }

    // Issue call #1 (sampler — will park on peer.sample) then call #2 (fast).
    // Both frames go out BEFORE we answer the sampling request, so #2 must be
    // drained off the wire and queued while the single worker is parked on #1.
    client_t
        .send(TransportMessage::Request {
            id: RequestId::from(1i64),
            request: client_req("tools/call", json!({ "name": "sampler", "arguments": {} })),
        })
        .await
        .unwrap();
    client_t
        .send(TransportMessage::Request {
            id: RequestId::from(2i64),
            request: client_req("tools/call", json!({ "name": "fast", "arguments": {} })),
        })
        .await
        .unwrap();

    // Now drive the client side: answer the inbound sampling request, then
    // collect both tool responses. If the receive path blocked while the worker
    // parked, the server could never read our sampling answer -> timeout.
    let mut got_1 = false;
    let mut got_2 = false;
    let driver = async {
        while !(got_1 && got_2) {
            match client_t.receive().await.unwrap() {
                TransportMessage::Request { id, request: _ } => {
                    // The only inbound request is the server's sampling call.
                    let answer = CreateMessageResult::new(Content::text("done"), "host-model");
                    client_t
                        .send(TransportMessage::Response(JSONRPCResponse::success(
                            id,
                            serde_json::to_value(&answer).unwrap(),
                        )))
                        .await
                        .unwrap();
                },
                TransportMessage::Response(r) => {
                    if r.id == RequestId::from(1i64) {
                        got_1 = true;
                    } else if r.id == RequestId::from(2i64) {
                        got_2 = true;
                    }
                },
                TransportMessage::Notification(_) => {},
            }
        }
    };

    tokio::time::timeout(Duration::from_secs(5), driver)
        .await
        .expect("both tool calls must complete (no deadlock)");

    assert!(got_1 && got_2, "both queued requests must be answered");
    server_handle.abort();
}

// ---------------------------------------------------------------------------
// (d) SHUTDOWN: closing the transport makes run() return.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_returns_when_transport_closes() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move { server.run(server_t).await });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling(CannedSampling {
            model: "host-model".to_string(),
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");
    let _ = client
        .call_tool("sampler".to_string(), json!({}))
        .await
        .expect("tools/call succeeds");

    // Drop the client -> the server's transport.receive() errors -> the actor
    // breaks -> run() returns.
    drop(client);

    let run_result = tokio::time::timeout(Duration::from_secs(5), server_handle)
        .await
        .expect("run() must return after the transport closes")
        .expect("server task joins");
    assert!(run_result.is_ok(), "run() returns Ok on clean shutdown");
}

// ---------------------------------------------------------------------------
// (e) WithTools end-to-end (AGNT-04): a tool that awaits
// peer.sample_with_tools() receives a ToolUse block from a WithTools host
// handler, intact, on the stock loop.
// ---------------------------------------------------------------------------

/// `WithTools` host handler answering with a `tool_use` block.
struct ToolUseSampling;

#[async_trait]
impl HostSamplingHandlerWithTools for ToolUseSampling {
    async fn handle_create_message_with_tools(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools> {
        Ok(CreateMessageResultWithTools::new(
            "tool-model",
            Role::Assistant,
            vec![SamplingMessageContent::ToolUse {
                name: "search".to_string(),
                id: "call-42".to_string(),
                input: json!({ "q": "rust" }),
                meta: None,
            }],
        ))
    }
}

#[tokio::test]
async fn in_tool_sample_with_tools_preserves_tool_use_end_to_end() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling_with_tools(ToolUseSampling)
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool("sampler_with_tools".to_string(), json!({})),
    )
    .await
    .expect("call must not hang")
    .expect("tools/call succeeds");

    // The ToolUse block (name + id) survives to the server-side
    // CreateMessageResultWithTools.
    assert!(
        result_text(&result).contains("tooluse:search#call-42"),
        "tool_use block (name + id) must survive end-to-end: {}",
        result_text(&result)
    );

    drop(client);
    server_handle.abort();
}

// ---------------------------------------------------------------------------
// (f) In-tool peer.elicit() completes on the stock loop — accept and decline.
//
// The pair is the point. An `elicit` that could only ever report success would
// satisfy a single accept-only test while being useless (and dangerous) for the
// case the method exists to serve, so the decline branch is asserted as
// specifically as the accept branch.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn in_tool_elicit_accept_completes_on_stock_loop() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_elicitation(CannedElicitation)
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool(
            "elicit".to_string(),
            json!({ "message": ELICIT_ACCEPT_MESSAGE }),
        ),
    )
    .await
    .expect("call must not hang")
    .expect("tools/call succeeds");

    // Both halves matter: the ACTION the host chose survived the round trip,
    // and so did the CONTENT it collected. Asserting only the action would pass
    // against an `elicit` that dropped the form fields on the floor.
    assert!(
        result_text(&result).contains(&format!("elicit:Accept:{ELICIT_ACCEPT_ENV}")),
        "tool must observe the host's accept and its collected field: {}",
        result_text(&result)
    );

    drop(client);
    server_handle.abort();
}

#[tokio::test]
async fn in_tool_elicit_decline_reaches_the_tool_as_a_decline() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_elicitation(CannedElicitation)
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool(
            "elicit".to_string(),
            json!({ "message": ELICIT_DECLINE_MESSAGE }),
        ),
    )
    .await
    .expect("call must not hang")
    .expect("a declined elicitation is a successful round trip, not a tools/call error");

    let text = result_text(&result);
    assert!(
        text.contains("elicit:Decline:<none>"),
        "a decline must arrive as a decline carrying no form content: {text}"
    );
    assert!(
        !text.contains("Accept"),
        "a decline must never be observable as an accept: {text}"
    );

    drop(client);
    server_handle.abort();
}
