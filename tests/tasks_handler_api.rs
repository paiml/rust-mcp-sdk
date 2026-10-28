//! What a task handler can learn and do (pmcp 2.22.7 field report F-1, F-2, F-6).
//!
//! - F-2: `extra.is_task_request()` follows the era's own trigger, and
//!   `extra.will_create_task()` reports the dispatcher's create gate before the
//!   handler runs, so a handler never returns a task handle the client cannot
//!   poll.
//! - F-1: `extra.on_task_created(..)` hands the hook the STORE-minted id and
//!   owner, which is what a background worker needs.
//! - F-6: `TaskStore::set_status_message` reports progress on a working task,
//!   and `tasks/get` shows it.
//!
//! Every scenario runs over real loopback HTTP through the high-level `Server`.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use common::v2::{
    post, spawn_default_config, teardown, v1_body, v1_session, v2_body_with_caps,
    v2_body_with_client_extensions, v2_headers, OptionalBearer, Resp, V1, V2,
};
use pmcp::server::task_store::{CreatedTask, InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::tasks::TaskStatus;
use pmcp::types::{TaskSupport, ToolExecution};
use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use serde_json::{json, Value};
use tokio::sync::mpsc;

const TOOL: &str = "probe";
const SUBJECT: &str = "alice";

/// What the handler saw, recorded so the test can compare it with what the
/// dispatcher then did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    is_task_request: bool,
    will_create_task: bool,
}

/// A `TaskSupport::Optional` tool that records what it sees, registers an
/// `on_task_created` hook forwarding the created task, and returns a
/// task-shaped value.
fn probe_tool(
    seen: Arc<Mutex<Vec<Seen>>>,
    created: mpsc::UnboundedSender<CreatedTask>,
) -> impl ToolHandler {
    TypedTool::new_with_schema(
        TOOL,
        json!({ "type": "object" }),
        move |_args: Value, extra: RequestHandlerExtra| {
            seen.lock().unwrap().push(Seen {
                is_task_request: extra.is_task_request(),
                will_create_task: extra.will_create_task(),
            });
            let tx = created.clone();
            extra.on_task_created(move |task| {
                let _ = tx.send(task);
            });
            Box::pin(async { Ok(json!({ "taskId": "tool-fabricated", "status": "working" })) })
        },
    )
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))
}

struct Fixture {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
    store: Option<Arc<InMemoryTaskStore>>,
    seen: Arc<Mutex<Vec<Seen>>>,
    created: mpsc::UnboundedReceiver<CreatedTask>,
}

impl Fixture {
    async fn start(with_store: bool) -> Self {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (tx, created) = mpsc::unbounded_channel();
        let mut builder = Server::builder()
            .name("task-handler-api")
            .version("1.0.0")
            .with_supported_protocol_versions([
                ProtocolVersion(V1.to_string()),
                ProtocolVersion(V2.to_string()),
            ])
            .tool(TOOL, probe_tool(Arc::clone(&seen), tx))
            .auth_provider(OptionalBearer);
        let store = with_store.then(|| Arc::new(InMemoryTaskStore::new()));
        if let Some(store) = &store {
            builder = builder.task_store(Arc::clone(store) as Arc<dyn TaskStore>);
        }
        let server = builder.build().expect("server builds");
        let (addr, handle) = spawn_default_config(server).await;
        Self {
            addr,
            handle,
            store,
            seen,
            created,
        }
    }

    fn seen(&self) -> Seen {
        let seen = self.seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "the tool ran exactly once: {seen:?}");
        seen[0]
    }

    async fn stop(self) {
        teardown(self.handle, ()).await;
    }
}

fn auth() -> Vec<(String, String)> {
    vec![("authorization".to_string(), format!("Bearer {SUBJECT}"))]
}

/// v1 headers: the bearer plus a session from a real v1 handshake, which the
/// default (stateful) config requires on every later v1 request.
async fn v1_headers(addr: SocketAddr) -> Vec<(String, String)> {
    let mut headers = auth();
    headers.push((
        "mcp-session-id".to_string(),
        v1_session(addr, "tasks_handler_api").await,
    ));
    headers
}

fn call_params(with_v1_task_field: bool) -> Value {
    let mut params = json!({ "name": TOOL, "arguments": {} });
    if with_v1_task_field {
        params["task"] = json!({});
    }
    params
}

async fn v1_call(addr: SocketAddr, headers: &[(String, String)], with_task_field: bool) -> Resp {
    post(
        addr,
        headers,
        &v1_body("tools/call", json!(1), call_params(with_task_field)),
    )
    .await
}

/// A v2 `tools/call` that does or does not declare the tasks extension on the
/// call itself. The non-declaring body still declares the three
/// MRTR capabilities, so the declaration is the only difference.
async fn v2_call(addr: SocketAddr, declare: bool, with_v1_task_field: bool) -> Resp {
    let mut headers = v2_headers("tools/call", TOOL);
    headers.extend(auth());
    let params = call_params(with_v1_task_field);
    let body = if declare {
        v2_body_with_client_extensions("tools/call", json!(1), params, &[TASKS_EXTENSION_KEY])
    } else {
        v2_body_with_caps(
            "tools/call",
            json!(1),
            params,
            json!({ "elicitation": {}, "sampling": {}, "roots": {} }),
        )
    };
    post(addr, &headers, &body).await
}

fn result_of(resp: &Resp) -> &Value {
    resp.body
        .get("result")
        .unwrap_or_else(|| panic!("expected a success result, got {}", resp.raw))
}

/// The handle came back as an ordinary tool result: no protocol-level task.
fn assert_not_a_task(resp: &Resp) {
    let result = result_of(resp);
    assert!(result.get("task").is_none(), "{}", resp.raw);
    assert!(result.get("taskId").is_none(), "{}", resp.raw);
    assert!(result.get("content").is_some(), "{}", resp.raw);
}

#[tokio::test]
async fn v1_task_call_hands_the_minted_task_to_the_hook() {
    let mut fx = Fixture::start(true).await;
    let headers = v1_headers(fx.addr).await;
    let resp = v1_call(fx.addr, &headers, true).await;

    let minted = result_of(&resp)["task"]["taskId"]
        .as_str()
        .unwrap_or_else(|| panic!("a v1 create result nests the task: {}", resp.raw))
        .to_string();
    assert_ne!(minted, "tool-fabricated");
    assert_eq!(
        fx.seen(),
        Seen {
            is_task_request: true,
            will_create_task: true
        }
    );

    let created = fx
        .created
        .try_recv()
        .expect("the hook ran before the reply");
    assert_eq!(created.task.task_id, minted, "the hook gets the polled id");
    assert_eq!(created.owner_id, SUBJECT);
    assert_eq!(created.task.status, TaskStatus::Working);

    // A worker holding that id and owner can drive the task the client polls.
    let store = fx.store.clone().expect("store");
    store
        .set_status_message(&minted, &created.owner_id, Some("step 1/2".into()))
        .await
        .expect("progress on a working task");
    let get = |id: &str| v1_body("tasks/get", json!(2), json!({ "taskId": id }));
    let polled = post(fx.addr, &headers, &get(&minted)).await;
    assert_eq!(
        result_of(&polled)["task"]["status"],
        "working",
        "{}",
        polled.raw
    );
    assert_eq!(
        result_of(&polled)["task"]["statusMessage"],
        "step 1/2",
        "{}",
        polled.raw
    );

    store
        .update_status(&minted, &created.owner_id, TaskStatus::Completed, None)
        .await
        .expect("complete");
    let polled = post(fx.addr, &headers, &get(&minted)).await;
    assert_eq!(
        result_of(&polled)["task"]["status"],
        "completed",
        "{}",
        polled.raw
    );
    fx.stop().await;
}

#[tokio::test]
async fn v1_plain_call_is_synchronous_and_drops_the_hook() {
    let mut fx = Fixture::start(true).await;
    let headers = v1_headers(fx.addr).await;
    let resp = v1_call(fx.addr, &headers, false).await;
    assert_not_a_task(&resp);
    assert_eq!(
        fx.seen(),
        Seen {
            is_task_request: false,
            will_create_task: false
        }
    );
    assert!(fx.created.try_recv().is_err(), "no task, so no hook call");
    fx.stop().await;
}

#[tokio::test]
async fn v2_declared_call_hands_the_minted_task_to_the_hook() {
    let mut fx = Fixture::start(true).await;
    let resp = v2_call(fx.addr, true, false).await;
    let minted = result_of(&resp)["taskId"]
        .as_str()
        .unwrap_or_else(|| panic!("a v2 create result is flat: {}", resp.raw))
        .to_string();
    assert_eq!(
        fx.seen(),
        Seen {
            is_task_request: true,
            will_create_task: true
        }
    );
    let created = fx.created.try_recv().expect("the hook ran");
    assert_eq!(created.task.task_id, minted);
    assert_eq!(created.owner_id, SUBJECT);
    fx.stop().await;
}

/// The Claude Code shape from F-2: the client declares the tasks extension on
/// the connection but not on `tools/call`. The handler must be told no, so it
/// does not hand the model an unpollable task handle.
#[tokio::test]
async fn v2_call_without_the_declaration_is_not_a_task_request() {
    let mut fx = Fixture::start(true).await;
    let resp = v2_call(fx.addr, false, false).await;
    assert_not_a_task(&resp);
    assert_eq!(
        fx.seen(),
        Seen {
            is_task_request: false,
            will_create_task: false
        }
    );
    assert!(fx.created.try_recv().is_err());
    fx.stop().await;
}

/// On 2026-07-28 the v1 `task` field is not a trigger. `is_task_request` used
/// to read it in every era.
#[tokio::test]
async fn v2_ignores_the_v1_task_field() {
    let mut fx = Fixture::start(true).await;
    let resp = v2_call(fx.addr, false, true).await;
    assert_not_a_task(&resp);
    assert_eq!(
        fx.seen(),
        Seen {
            is_task_request: false,
            will_create_task: false
        }
    );
    assert!(fx.created.try_recv().is_err());
    fx.stop().await;
}

/// The client asked, but no store backs tasks: `is_task_request` says yes and
/// `will_create_task` says no, which is exactly the case the second exists for.
#[tokio::test]
async fn without_a_store_the_client_asks_but_no_task_is_created() {
    let mut fx = Fixture::start(false).await;
    let headers = v1_headers(fx.addr).await;
    let resp = v1_call(fx.addr, &headers, true).await;
    assert_not_a_task(&resp);
    assert_eq!(
        fx.seen(),
        Seen {
            is_task_request: true,
            will_create_task: false
        }
    );
    assert!(fx.created.try_recv().is_err());
    fx.stop().await;
}
