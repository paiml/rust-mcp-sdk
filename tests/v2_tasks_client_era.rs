//! Phase 114-19 (TASK-02 / TASK-03 / TASK-04): the CLIENT half of the v2 tasks
//! surface, driven by RAW RESPONSE FRAMES.
//!
//! # Why raw frames and not a live server
//!
//! `tests/v2_tasks_shapes.rs` (114-11) already proves what the pmcp SERVER
//! EMITS, over a real socket, on raw bytes. If this suite also went through a
//! live pmcp server it would prove only that pmcp agrees with itself: a
//! simultaneous change on both sides would make the pair VACUOUSLY green while
//! every other conformant peer broke.
//!
//! So every decoding test here feeds the client a canned JSON-RPC `result`
//! object through a scripted transport. Where a byte shape is shared with
//! `tests/v2_tasks_shapes.rs` (the v2 emitter) or `tests/v1_tasks_golden.rs`
//! (the FROZEN v1 wire), the literal is taken from that file and the comment
//! above it NAMES the counterpart — so a future one-sided edit is visibly a
//! contract change rather than a local test fix.
//!
//! # What each pair proves
//!
//! Every v2 property has a v1 CONTROL beside it, because the whole point of the
//! change is that it is ERA-SCOPED. A v2 assertion that passes while v1 silently
//! moved would be a regression this file must catch, not miss. In each pair the
//! NEGATIVE / ABSENT case is written FIRST (the ordinary result before the task
//! handle, the refusal before the success), so a reader meets the discriminator
//! before the happy path.
//!
//! # No HTTP, no feature gate beyond `not(wasm32)`
//!
//! The suite exercises `pmcp::Client` over a hand-written `Transport`, so it
//! needs neither `streamable-http` nor `http-client` and runs under every
//! feature set the gate builds. `not(target_arch = "wasm32")` is the only cfg:
//! the tests are `#[tokio::test]`.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use pmcp::client::{ToolCallResponse, WaitForTaskOptions};
use pmcp::shared::protocol_helpers::create_request;
use pmcp::shared::Transport;
use pmcp::types::mrtr::{InputResponse, InputResponses};
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
use pmcp::types::roots::ListRootsResult;
use pmcp::types::tasks::{TaskDetailV2, TaskStatus};
use pmcp::types::{
    ClientCapabilities, JSONRPCResponse, RequestId, TransportMessage, LATEST_PROTOCOL_VERSION,
};
use pmcp::{Client, ClientBuilder};
use serde_json::{json, Value};

// ===========================================================================
// The scripted transport.
// ===========================================================================

/// What the client actually put on the wire.
#[derive(Debug, Default)]
struct Wire {
    /// Every REQUEST method, in order.
    methods: Vec<String>,
    /// Every frame — requests AND notifications. The "zero sends" assertions
    /// read this, because a notification is bytes on the wire too.
    frames: usize,
}

/// A transport that answers by METHOD from a canned script and records what it
/// was asked.
///
/// It serves BOTH client paths deliberately: `send` (the typed v1 path) and
/// `send_raw` (the raw v2 path). A regression that made a v2 client fall back to
/// the typed path — or a v1 client start emitting raw frames — still gets
/// answered here, and the recorded method sequence is identical either way, so
/// the assertions below are about the METHOD the client chose and never about
/// which encoder it used.
#[derive(Debug, Clone)]
struct ScriptedTransport {
    /// Per-method queues of `result` objects. An exhausted queue REPEATS its
    /// last entry, so a poll loop that ticks more than the script expects does
    /// not become a spurious failure.
    script: Arc<Mutex<HashMap<String, VecDeque<Value>>>>,
    wire: Arc<Mutex<Wire>>,
    inbox: Arc<Mutex<VecDeque<TransportMessage>>>,
}

impl ScriptedTransport {
    fn new() -> Self {
        Self {
            script: Arc::new(Mutex::new(HashMap::new())),
            wire: Arc::new(Mutex::new(Wire::default())),
            inbox: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Append one `result` object to `method`'s queue.
    fn on(self, method: &str, result: Value) -> Self {
        self.script
            .lock()
            .expect("no panic while holding")
            .entry(method.to_string())
            .or_default()
            .push_back(result);
        self
    }

    /// The recorded request-method sequence.
    fn methods(&self) -> Vec<String> {
        self.wire
            .lock()
            .expect("no panic while holding")
            .methods
            .clone()
    }

    /// The number of frames sent so far (requests + notifications).
    fn frames(&self) -> usize {
        self.wire.lock().expect("no panic while holding").frames
    }

    /// Record a frame and enqueue the scripted answer for `method`.
    fn answer(&self, method: &str, id: RequestId) {
        {
            let mut wire = self.wire.lock().expect("no panic while holding");
            wire.methods.push(method.to_string());
            wire.frames += 1;
        }
        let result = {
            let mut script = self.script.lock().expect("no panic while holding");
            let queue = script
                .get_mut(method)
                .unwrap_or_else(|| panic!("the script has no entry for {method}"));
            if queue.len() > 1 {
                queue.pop_front().expect("non-empty")
            } else {
                queue.front().cloned().expect("at least one entry")
            }
        };
        self.inbox
            .lock()
            .expect("no panic while holding")
            .push_back(TransportMessage::Response(JSONRPCResponse::success(
                id, result,
            )));
    }
}

#[async_trait]
impl Transport for ScriptedTransport {
    async fn send(&mut self, message: TransportMessage) -> pmcp::Result<()> {
        match message {
            // The v1 typed path. `create_request` is the SAME conversion the
            // client uses, so the method string recorded here is the one that
            // would have gone on the wire — not a re-derivation.
            TransportMessage::Request { id, request } => {
                let method = create_request(id.clone(), request).method;
                self.answer(&method, id);
            },
            TransportMessage::Notification(_) => {
                self.wire.lock().expect("no panic while holding").frames += 1;
            },
            TransportMessage::Response(_) => {},
        }
        Ok(())
    }

    async fn receive(&mut self) -> pmcp::Result<TransportMessage> {
        self.inbox
            .lock()
            .expect("no panic while holding")
            .pop_front()
            .ok_or_else(|| pmcp::Error::internal("the scripted transport has nothing to deliver"))
    }

    async fn close(&mut self) -> pmcp::Result<()> {
        Ok(())
    }

    fn transport_type(&self) -> &'static str {
        "scripted"
    }

    fn supports_negotiated_protocol_version(&self) -> bool {
        true
    }

    async fn send_raw(&mut self, body: Vec<u8>) -> pmcp::Result<()> {
        let frame: Value = serde_json::from_slice(&body).expect("the client sends valid JSON");
        let method = frame["method"]
            .as_str()
            .expect("every request frame carries a method")
            .to_string();
        let id: RequestId =
            serde_json::from_value(frame["id"].clone()).expect("every request carries an id");
        self.answer(&method, id);
        Ok(())
    }
}

// ===========================================================================
// Clients.
// ===========================================================================

/// A v2 client, initialized (which on v2 sends NOTHING) and with no
/// `server/discover` performed — so `assert_capability` defers to the server,
/// which is the posture a stateless v2 caller actually has.
async fn v2_client(transport: ScriptedTransport) -> Client<ScriptedTransport> {
    let mut client = ClientBuilder::new(transport)
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))
        .expect("2026-07-28 is selectable")
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("a v2 initialize is local and infallible");
    client
}

/// The `initialize` result a v1 client is answered with: tools AND tasks
/// advertised, so `assert_capability("tasks", ..)` passes on the v1 arm.
fn v1_initialize_result() -> Value {
    json!({
        "protocolVersion": LATEST_PROTOCOL_VERSION,
        "capabilities": { "tools": {}, "tasks": {} },
        "serverInfo": { "name": "scripted", "version": "1.0.0" },
    })
}

/// A v1 client that completed a real handshake over the scripted transport.
async fn v1_client(transport: ScriptedTransport) -> Client<ScriptedTransport> {
    let transport = transport.on("initialize", v1_initialize_result());
    let mut client = ClientBuilder::new(transport).build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("the scripted initialize succeeds");
    client
}

// ===========================================================================
// Byte-shape fixtures.
// ===========================================================================

/// The FLAT v2 create result.
///
/// COUNTERPART: `tests/v2_tasks_shapes.rs` ::
/// `v2_create_task_result_is_flat_and_carries_all_required_fields`, which
/// asserts this exact key set on the raw bytes a real pmcp server emits — five
/// required `Task` fields at the TOP level, no `task` wrapper, `ttlMs` present
/// (required AND nullable), and `resultType: "task"`. Change one side and this
/// comment is your notice that the other side must move too.
fn v2_create_result() -> Value {
    json!({
        "taskId": "task-v2-0001",
        "status": "working",
        "createdAt": "2026-07-28T00:00:00Z",
        "lastUpdatedAt": "2026-07-28T00:00:00Z",
        "ttlMs": 60000,
        "pollIntervalMs": 50,
        "resultType": "task",
        "_meta": { "io.modelcontextprotocol/related-task": { "taskId": "task-v2-0001" } },
    })
}

/// The NESTED v1 create result.
///
/// COUNTERPART: `tests/v1_tasks_golden.rs` :: `ROUTER_GET_WORKING` / the
/// `router_task` fixture and the `STORE_CREATE` golden — v1 wraps under `task`
/// and spells `ttl` / `pollInterval`. That file pins these bytes for the
/// SERVER; this one pins that the client still decodes them.
fn v1_create_result() -> Value {
    json!({
        "task": v1_task("working"),
        "_meta": { "io.modelcontextprotocol/related-task": { "taskId": "router-task-0001" } },
    })
}

/// The v1 `Task` body, spelled exactly as `tests/v1_tasks_golden.rs`'s
/// `router_task` spells it.
fn v1_task(status: &str) -> Value {
    json!({
        "taskId": "router-task-0001",
        "status": status,
        "ttl": 60000,
        "createdAt": "2026-01-01T00:00:00Z",
        "lastUpdatedAt": "2026-01-01T00:00:01Z",
        "pollInterval": 5000,
    })
}

/// A flat v2 `tasks/get` body with `status` and an optional detail key.
///
/// COUNTERPART: `tests/v2_tasks_shapes.rs` :: `v2_tasks_get_on_a_working_task_is_flat`
/// and its three `..._inlines_..._on_...` siblings, which assert the same flat
/// base plus exactly one status-conditional key, on raw server bytes.
fn v2_get_result(status: &str, detail: Option<(&str, Value)>) -> Value {
    let mut object = json!({
        "taskId": "task-v2-0001",
        "status": status,
        "createdAt": "2026-07-28T00:00:00Z",
        "lastUpdatedAt": "2026-07-28T00:00:09Z",
        "ttlMs": 60000,
        "pollIntervalMs": 50,
        "resultType": "complete",
    });
    if let Some((key, value)) = detail {
        object
            .as_object_mut()
            .expect("object")
            .insert(key.to_string(), value);
    }
    object
}

/// The `inputRequests` map a paused v2 task inlines at the TOP level (row 23).
fn input_requests_fixture() -> Value {
    json!({ "where": { "method": "roots/list" } })
}

/// A terminal `CallToolResult`, as a v2 `completed` task inlines it.
fn inlined_tool_result(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

// ===========================================================================
// 1. The `tools/call` create decode — v2 discriminator, then v1 control.
// ===========================================================================

/// NEGATIVE CASE FIRST: `resultType: "complete"` is an ordinary tool result even
/// when the body is TASK-SHAPED.
///
/// This is the shape-sniffing regression guard, and the mitigation for T-114-99:
/// on v2 the branch is chosen by the discriminator the server wrote, never by
/// "does this look like a task?". The body below carries `taskId`, `status` and
/// every other create-result key — a shape-sniffing decoder returns a task
/// handle for it and the caller then polls an id that is not a task.
#[tokio::test]
async fn v2_complete_result_type_decodes_to_an_ordinary_tool_result() {
    let mut task_shaped = v2_create_result();
    task_shaped["resultType"] = json!("complete");
    let transport = ScriptedTransport::new().on("tools/call", task_shaped);
    let client = v2_client(transport).await;

    let response = client
        .call_tool_with_task("search".to_string(), json!({}))
        .await
        .expect("a complete result decodes");

    assert!(
        matches!(response, ToolCallResponse::Result(_)),
        "a task-SHAPED body with resultType=complete must NOT become a task handle"
    );
}

/// The FLAT v2 create result becomes a task handle, with the two renames applied.
#[tokio::test]
async fn v2_flat_create_result_decodes_to_a_task_handle() {
    let transport = ScriptedTransport::new().on("tools/call", v2_create_result());
    let client = v2_client(transport).await;

    let response = client
        .call_tool_with_task("search".to_string(), json!({}))
        .await
        .expect("a flat create result decodes");

    let ToolCallResponse::Task(task) = response else {
        panic!("resultType=task must decode to a task handle");
    };
    assert_eq!(task.task_id, "task-v2-0001");
    assert_eq!(task.status, TaskStatus::Working);
    assert_eq!(task.ttl, Some(60000), "ttlMs must land on ttl");
    assert_eq!(
        task.poll_interval,
        Some(50),
        "pollIntervalMs must land on pollInterval"
    );
}

/// v1 CONTROL: the NESTED create envelope still decodes, unchanged.
#[tokio::test]
async fn v1_nested_create_result_still_decodes() {
    let transport = ScriptedTransport::new().on("tools/call", v1_create_result());
    let client = v1_client(transport).await;

    let response = client
        .call_tool_with_task("search".to_string(), json!({}))
        .await
        .expect("the v1 nested create result decodes");

    let ToolCallResponse::Task(task) = response else {
        panic!("a v1 CreateTaskResult must decode to a task handle");
    };
    assert_eq!(task.task_id, "router-task-0001");
    assert_eq!(task.ttl, Some(60000));
    assert_eq!(task.poll_interval, Some(5000));
}

// ===========================================================================
// 2. `tasks/get` — flat on v2, nested on v1.
// ===========================================================================

/// The flat v2 payload maps `ttlMs` onto `ttl` and `pollIntervalMs` onto
/// `pollInterval`, so an existing poll loop keeps working verbatim.
#[tokio::test]
async fn v2_tasks_get_flat_payload_maps_ttl_ms_onto_ttl() {
    let transport = ScriptedTransport::new().on("tasks/get", v2_get_result("working", None));
    let client = v2_client(transport).await;

    let task = client.tasks_get("task-v2-0001").await.expect("decodes");

    assert_eq!(task.task_id, "task-v2-0001");
    assert_eq!(task.status, TaskStatus::Working);
    assert_eq!(task.ttl, Some(60000), "ttlMs -> ttl");
    assert_eq!(
        task.poll_interval,
        Some(50),
        "pollIntervalMs -> pollInterval"
    );
}

/// v1 CONTROL: the NESTED `{"task": …}` payload still decodes, and the v1 key
/// spellings still land on the same fields.
#[tokio::test]
async fn v1_tasks_get_still_decodes_the_nested_payload() {
    let transport = ScriptedTransport::new().on("tasks/get", json!({ "task": v1_task("working") }));
    let client = v1_client(transport).await;

    let task = client.tasks_get("router-task-0001").await.expect("decodes");

    assert_eq!(task.task_id, "router-task-0001");
    assert_eq!(task.ttl, Some(60000));
    assert_eq!(task.poll_interval, Some(5000));
}

// ===========================================================================
// 3. The status-conditional detail, reachable WITHOUT a second round trip.
// ===========================================================================

/// A `completed` task inlines its terminal `result`.
#[tokio::test]
async fn v2_tasks_get_inlines_result_on_completed() {
    let transport = ScriptedTransport::new().on(
        "tasks/get",
        v2_get_result(
            "completed",
            Some(("result", inlined_tool_result("the answer is 42"))),
        ),
    );
    let client = v2_client(transport).await;

    let detailed = client
        .tasks_get_detailed("task-v2-0001")
        .await
        .expect("decodes");

    let TaskDetailV2::Completed { result } = detailed.detail() else {
        panic!("a completed task must carry the Completed detail");
    };
    assert_eq!(
        result["content"][0]["text"], "the answer is 42",
        "the inlined result must be reachable without a tasks/result round trip"
    );
}

/// A `failed` task inlines the JSON-RPC `error`.
#[tokio::test]
async fn v2_tasks_get_inlines_error_on_failed() {
    let transport = ScriptedTransport::new().on(
        "tasks/get",
        v2_get_result(
            "failed",
            Some((
                "error",
                json!({ "code": -32603, "message": "the worker died mid-flight" }),
            )),
        ),
    );
    let client = v2_client(transport).await;

    let detailed = client
        .tasks_get_detailed("task-v2-0001")
        .await
        .expect("decodes");

    let TaskDetailV2::Failed { error } = detailed.detail() else {
        panic!("a failed task must carry the Failed detail");
    };
    assert_eq!(error["message"], "the worker died mid-flight");
}

/// An `input_required` task inlines a TOP-LEVEL `inputRequests` — row 23, read
/// from the client side.
#[tokio::test]
async fn v2_tasks_get_inlines_input_requests_on_input_required() {
    let transport = ScriptedTransport::new().on(
        "tasks/get",
        v2_get_result(
            "input_required",
            Some(("inputRequests", input_requests_fixture())),
        ),
    );
    let client = v2_client(transport).await;

    let detailed = client
        .tasks_get_detailed("task-v2-0001")
        .await
        .expect("decodes");

    let TaskDetailV2::InputRequired { input_requests } = detailed.detail() else {
        panic!("an input_required task must carry the InputRequired detail");
    };
    assert!(
        input_requests.contains_key("where"),
        "the server-recorded key must survive the client decode: {input_requests:?}"
    );
}

// ===========================================================================
// 4. The EMPTY acknowledgements.
// ===========================================================================

/// A bare `{}` cancel result is a SUCCESS, not a decode error.
///
/// v2's `CancelTaskResult = Result`: there is no task body to unwrap, and the
/// v1 `CancelTaskResult` decode fails outright against it. `tasks_cancel_ack`
/// is the one-round-trip primitive; `tasks_cancel` keeps its `Task` return by
/// re-reading, which is asserted here as a SECOND send rather than assumed.
#[tokio::test]
async fn v2_empty_cancel_ack_is_not_a_decode_error() {
    let transport = ScriptedTransport::new()
        .on("tasks/cancel", json!({}))
        .on("tasks/get", v2_get_result("working", None));
    let client = v2_client(transport.clone()).await;

    client
        .tasks_cancel_ack("task-v2-0001")
        .await
        .expect("an EMPTY ack must decode as success");
    assert_eq!(
        transport.methods(),
        vec!["tasks/cancel".to_string()],
        "the ack primitive performs exactly one round trip"
    );

    let task = client
        .tasks_cancel("task-v2-0001")
        .await
        .expect("cancel + re-read succeeds");
    assert_eq!(
        transport.methods(),
        vec![
            "tasks/cancel".to_string(),
            "tasks/cancel".to_string(),
            "tasks/get".to_string()
        ],
        "tasks_cancel re-reads rather than synthesising a status"
    );
    assert_eq!(
        task.status,
        TaskStatus::Working,
        "cancellation is cooperative: the re-read reports what the server says, \
         not an invented `cancelled`"
    );
}

/// A bare `{}` update result is a SUCCESS, not a decode error.
#[tokio::test]
async fn v2_empty_update_ack_is_not_a_decode_error() {
    let transport = ScriptedTransport::new().on("tasks/update", json!({}));
    let client = v2_client(transport.clone()).await;

    client
        .tasks_update("task-v2-0001", answers())
        .await
        .expect("an EMPTY ack must decode as success");

    assert_eq!(transport.methods(), vec!["tasks/update".to_string()]);
}

/// One answer, keyed exactly as the server keyed its request.
fn answers() -> InputResponses {
    let mut responses = InputResponses::new();
    responses.insert(
        "where".to_string(),
        InputResponse::Roots(Box::new(ListRootsResult { roots: vec![] })),
    );
    responses
}

// ===========================================================================
// 5. The RETIRED methods fail fast, with ZERO bytes on the wire.
// ===========================================================================

/// `tasks/result` is refused LOCALLY on v2 — the send counter stays at 0.
#[tokio::test]
async fn v2_tasks_result_fails_fast_with_zero_sends() {
    let transport = ScriptedTransport::new();
    let client = v2_client(transport.clone()).await;
    let before = transport.frames();

    let error = client
        .tasks_result("task-v2-0001")
        .await
        .expect_err("tasks/result does not exist on 2026-07-28");

    assert_eq!(
        transport.frames(),
        before,
        "a local refusal must send NOTHING"
    );
    assert!(error.is_retired_on_v2(), "{error}");
    assert_eq!(error.retired_method(), Some("tasks/result"));
    assert_eq!(
        error.retired_replacement(),
        Some("tasks/get"),
        "the refusal must name the replacement, which is the whole point of \
         answering locally"
    );
}

/// `tasks/list` is refused LOCALLY on v2 — the send counter stays at 0.
#[tokio::test]
async fn v2_tasks_list_fails_fast_with_zero_sends() {
    let transport = ScriptedTransport::new();
    let client = v2_client(transport.clone()).await;
    let before = transport.frames();

    let error = client
        .tasks_list(None)
        .await
        .expect_err("tasks/list does not exist on 2026-07-28");

    assert_eq!(
        transport.frames(),
        before,
        "a local refusal must send NOTHING"
    );
    assert!(error.is_retired_on_v2(), "{error}");
    assert_eq!(error.retired_method(), Some("tasks/list"));
    assert_ne!(
        error.retired_replacement(),
        Some("tasks/get"),
        "there is no v2 list, and tasks/get is not one"
    );
}

/// v1 CONTROL for both refusals: v1 still SERVES `tasks/result` and `tasks/list`.
///
/// Without this, "the retirement works" would be indistinguishable from "the two
/// methods are broken everywhere".
#[tokio::test]
async fn v1_tasks_result_and_tasks_list_still_serve() {
    let transport = ScriptedTransport::new()
        .on("tasks/result", inlined_tool_result("v1 terminal"))
        .on("tasks/list", json!({ "tasks": [v1_task("working")] }));
    let client = v1_client(transport.clone()).await;

    let result = client
        .tasks_result("router-task-0001")
        .await
        .expect("v1 serves tasks/result");
    assert_eq!(result.content.len(), 1);

    let list = client.tasks_list(None).await.expect("v1 serves tasks/list");
    assert_eq!(list.tasks.len(), 1);

    assert_eq!(
        transport.methods(),
        vec![
            "initialize".to_string(),
            "tasks/result".to_string(),
            "tasks/list".to_string()
        ]
    );
}

/// A v2 client that HAS observed the server's capabilities, and found no tasks
/// extension in them.
///
/// `server_discover` is what moves `assert_capability` from "defer to the
/// server" to "enforce locally" on v2, so it has to actually run.
async fn v2_client_with_no_tasks_extension(
    transport: ScriptedTransport,
) -> Client<ScriptedTransport> {
    let transport = transport.on(
        "server/discover",
        json!({
            "protocolVersion": PROTOCOL_VERSION_2026_07_28,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "scripted", "version": "1.0.0" },
        }),
    );
    let mut client = ClientBuilder::new(transport)
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))
        .expect("2026-07-28 is selectable")
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("a v2 initialize is local and infallible");
    client
        .server_discover()
        .await
        .expect("the scripted discover succeeds");
    client
}

/// A v2 client whose `server/discover` projection carries NO tasks extension is
/// refused by `assert_capability` before a `tasks/get` is sent (114-06).
#[tokio::test]
async fn v2_undeclared_tasks_get_is_refused_before_the_wire() {
    let transport = ScriptedTransport::new();
    let client = v2_client_with_no_tasks_extension(transport.clone()).await;
    let before = transport.frames();

    let error = client
        .tasks_get("task-v2-0001")
        .await
        .expect_err("an un-negotiated tasks call must be refused");

    assert_eq!(
        transport.frames(),
        before,
        "the refusal must be LOCAL — zero additional frames"
    );
    assert!(
        error.to_string().contains("io.modelcontextprotocol/tasks"),
        "the v2 refusal names the extension key: {error}"
    );
}

/// The SAME gate on `tasks/update`, whose payload would otherwise leak to a
/// server that never advertised the extension (T-114-103).
///
/// Deliberately its OWN test rather than a second half of the one above: the two
/// methods reach `assert_capability` through different call chains, and a
/// control that removes the gate from one of them must fail exactly one test.
/// Folded together, "the `tasks/get` gate is gone" and "the `tasks/update` gate
/// is gone" produced the identical single-test failure set and could only be
/// told apart by reading a panic MESSAGE.
#[tokio::test]
async fn v2_undeclared_tasks_update_is_refused_before_the_wire() {
    let transport = ScriptedTransport::new();
    let client = v2_client_with_no_tasks_extension(transport.clone()).await;
    let before = transport.frames();

    let error = client
        .tasks_update("task-v2-0001", answers())
        .await
        .expect_err("an un-negotiated tasks/update must be refused");

    assert_eq!(
        transport.frames(),
        before,
        "the refusal must be LOCAL — zero additional frames"
    );
    assert!(
        error.to_string().contains("tasks/update"),
        "the refusal names the method it refused: {error}"
    );
}

// ===========================================================================
// 6. The poll loop's terminal step is ERA-SPLIT.
// ===========================================================================

/// v2: the terminal result comes from the INLINE payload, and `tasks/result`
/// is NEVER called.
#[tokio::test]
async fn v2_wait_for_task_never_calls_tasks_result() {
    let transport = ScriptedTransport::new().on(
        "tasks/get",
        v2_get_result(
            "completed",
            Some(("result", inlined_tool_result("inline terminal"))),
        ),
    );
    let client = v2_client(transport.clone()).await;

    let result = client
        .wait_for_task("task-v2-0001", WaitForTaskOptions::default())
        .await
        .expect("the poll reaches terminal");

    assert_eq!(result.content.len(), 1);
    let methods = transport.methods();
    assert!(
        methods.contains(&"tasks/get".to_string()),
        "the loop must poll: {methods:?}"
    );
    assert!(
        !methods.contains(&"tasks/result".to_string()),
        "tasks/result is RETIRED on v2 and must never be called: {methods:?}"
    );
}

/// v1 CONTROL for the test above: v1 still performs the second `tasks/result`
/// round trip, so the change is proven ERA-SCOPED rather than global.
#[tokio::test]
async fn v1_wait_for_task_still_calls_tasks_result() {
    let transport = ScriptedTransport::new()
        .on("tasks/get", json!({ "task": v1_task("completed") }))
        .on("tasks/result", inlined_tool_result("v1 terminal"));
    let client = v1_client(transport.clone()).await;

    let result = client
        .wait_for_task("router-task-0001", WaitForTaskOptions::default())
        .await
        .expect("the poll reaches terminal");

    assert_eq!(result.content.len(), 1);
    let methods = transport.methods();
    assert!(
        methods.contains(&"tasks/result".to_string()),
        "v1 must still fetch the persisted result: {methods:?}"
    );
}

/// A v2 task that reaches `failed` surfaces its INLINED JSON-RPC error as a
/// typed client error, not as an empty success.
///
/// `CallToolResult::content` carries `#[serde(default)]`, so a permissive decode
/// of a failed task's payload produces a perfectly well-formed EMPTY success —
/// which is why this needs its own test rather than being implied by the one
/// above.
#[tokio::test]
async fn v2_failed_task_surfaces_its_inlined_error() {
    let transport = ScriptedTransport::new().on(
        "tasks/get",
        v2_get_result(
            "failed",
            Some((
                "error",
                json!({ "code": -32603, "message": "the worker died mid-flight" }),
            )),
        ),
    );
    let client = v2_client(transport).await;

    let error = client
        .wait_for_task("task-v2-0001", WaitForTaskOptions::default())
        .await
        .expect_err("a failed task must not be reported as a success");

    assert!(
        error.to_string().contains("the worker died mid-flight"),
        "the inlined error message must survive: {error}"
    );
}

// ===========================================================================
// 7. The input-supplying poller.
// ===========================================================================

/// The responder answers the paused task, `tasks/update` delivers it, and the
/// loop resumes to terminal — all on methods that exist on v2.
#[tokio::test]
async fn v2_wait_for_task_with_inputs_answers_and_resumes() {
    let transport = ScriptedTransport::new()
        .on(
            "tasks/get",
            v2_get_result(
                "input_required",
                Some(("inputRequests", input_requests_fixture())),
            ),
        )
        .on(
            "tasks/get",
            v2_get_result(
                "completed",
                Some(("result", inlined_tool_result("resumed and done"))),
            ),
        )
        .on("tasks/update", json!({}));
    let client = v2_client(transport.clone()).await;

    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_in_callback = Arc::clone(&seen);
    let result = client
        .wait_for_task_with_inputs("task-v2-0001", Default::default(), move |requests| {
            let seen = Arc::clone(&seen_in_callback);
            async move {
                seen.lock()
                    .expect("no panic")
                    .extend(requests.keys().cloned());
                Ok(answers())
            }
        })
        .await
        .expect("the loop resumes past the pause");

    assert_eq!(result.content.len(), 1);
    assert_eq!(
        *seen.lock().expect("no panic"),
        vec!["where".to_string()],
        "the responder must receive the task's own inputRequests"
    );
    assert_eq!(
        transport.methods(),
        vec![
            "tasks/get".to_string(),
            "tasks/update".to_string(),
            "tasks/get".to_string()
        ],
        "gather -> update -> resume, with no tasks/result anywhere"
    );
}

/// The input-supplying poller is v2-ONLY and refuses a v1 connection with zero
/// bytes on the wire.
#[tokio::test]
async fn v1_wait_for_task_with_inputs_is_refused_with_zero_sends() {
    let transport = ScriptedTransport::new();
    let client = v1_client(transport.clone()).await;
    let before = transport.frames();

    let error = client
        .wait_for_task_with_inputs("router-task-0001", Default::default(), |_requests| async {
            Ok(answers())
        })
        .await
        .expect_err("tasks/update does not exist on v1");

    assert_eq!(
        transport.frames(),
        before,
        "the refusal must be LOCAL — zero additional frames"
    );
    assert!(
        error.to_string().contains("2026-07-28"),
        "the refusal names the era it needs: {error}"
    );
}

/// `wait_for_task` itself is UNCHANGED for a caller that supplied no responder:
/// `input_required` is still an immediate, actionable error.
#[tokio::test]
async fn wait_for_task_without_a_responder_still_errors_on_input_required() {
    let transport = ScriptedTransport::new().on(
        "tasks/get",
        v2_get_result(
            "input_required",
            Some(("inputRequests", input_requests_fixture())),
        ),
    );
    let client = v2_client(transport.clone()).await;

    let error = client
        .wait_for_task("task-v2-0001", WaitForTaskOptions::default())
        .await
        .expect_err("a pause with no responder is an error");

    assert!(
        error.to_string().contains("input_required"),
        "the message must say WHY it stopped: {error}"
    );
    assert!(
        !transport.methods().contains(&"tasks/update".to_string()),
        "wait_for_task must not deliver input it was never given"
    );
}
