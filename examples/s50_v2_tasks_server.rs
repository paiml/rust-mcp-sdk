//! Example: an MCP 2026-07-28 (v2) TASKS server that PAUSES for input and is
//! resumed by `tasks/update`.
//!
//! Run this server with:
//! ```bash
//! cargo run --example s50_v2_tasks_server --features full
//! ```
//!
//! Optionally pass a bind address: `cargo run --example s50_v2_tasks_server
//! --features full -- 127.0.0.1:9100`. Then, in another terminal, run the paired
//! AGENT client, which drives the whole lifecycle autonomously:
//! ```bash
//! cargo run --example s51_v2_tasks_agent --features full
//! ```
//!
//! # What this demonstrates
//!
//! - **Extension negotiation on `server/discover`.** v2 has no `initialize`, so
//!   a client learns what this server supports by asking `server/discover`. The
//!   `tasks` capability appears there as `capabilities.extensions`
//!   `["io.modelcontextprotocol/tasks"]` — configuring a `TaskStore` is what
//!   advertises it, and nothing else has to be set.
//! - **The server-directed create trigger.** On v2 a `tools/call` becomes a task
//!   because the CLIENT DECLARED the tasks extension on that request — not
//!   because it sent a `task` field (that is the v1 trigger, and v2 ignores it).
//!   A client that did not declare gets an ordinary `CallToolResult` and never
//!   sees a task handle.
//! - **`tasks/get` INLINING.** One v2 `tasks/get` answers with the task AND its
//!   status-conditional detail in the same payload: `inputRequests` while
//!   paused, `result` once completed, `error` once failed. There is no second
//!   round trip and no `tasks/result` to make.
//! - **`tasks/update`.** The client's answers arrive on this method, are decoded
//!   against the kinds THIS SERVER recorded, and resume the task in one atomic
//!   write. The acknowledgement is an EMPTY object — it claims nothing about the
//!   task's status, because the task may still be moving.
//! - **The v2 retirements.** `tasks/list` and `tasks/result` are GONE on
//!   2026-07-28. This server answers both `-32601`; the enumeration primitive was
//!   removed as a security improvement, and the payload `tasks/result` used to
//!   serve is now inlined by `tasks/get`.
//!
//! # What a successful run prints
//!
//! On startup: a `PMCP_REQUEST_STATE_KEY is not set` WARNING from pmcp, then the
//! banner below, ending in `Press Ctrl+C to stop the server`. The warning is
//! EXPECTED and irrelevant here — it is about resuming multi-round-trip
//! `tools/call`s across load-balanced instances, and this server never mints a
//! `requestState`; a task is resumed by its id, which the store owns. See
//! `s47_v2_stateless_mrtr` for that contract.
//!
//! Then, once `s51_v2_tasks_agent` runs against it, one `worker:` line per task
//! it completes:
//!
//! ```text
//!   worker: task <id> received its input, completing it
//! ```
//!
//! If no `worker:` line appears while the agent is running, the input never
//! reached the store and the agent will fail with a poll timeout rather than
//! quietly passing.
//!
//! # Who does the work: the SDK owns the PROTOCOL, you own the WORK
//!
//! A paused task is resumed by whatever code is doing the work — the SDK has no
//! opinion about it, and deliberately so. The tool handler has already returned
//! by the time the input arrives, so it cannot be the thing that finishes the
//! job. This example plays that role with a small in-process worker
//! ([`run_worker`]) that watches the store; a production server would hand the
//! task to a queue, a Lambda, or a durable workflow. What matters is that the
//! protocol half — pause, deliver, resume — is entirely the SDK's.
//!
//! # Why the IN-CRATE `InMemoryTaskStore`, and not `pmcp-tasks`
//!
//! `pmcp-tasks`' `GenericTaskStore` (the DynamoDB/Redis-shaped backend) REFUSES
//! the anonymous owner while its `allow_anonymous` flag is `false`, which is its
//! default — it rejects both the empty principal and the legacy `"local"` bucket.
//! This example has no auth provider, so every caller here IS the anonymous
//! owner: pairing it with that backend would fail the very first time anyone ran
//! it. The in-crate [`InMemoryTaskStore`] has no such check, which makes it the
//! right store for a no-auth demo and the WRONG one for production. For the
//! production-backend story see 114-07's
//! `anonymous_owner_is_refused_by_default_on_this_backend` test.
//!
//! # SHARED OWNER BUCKET — say it out loud
//!
//! This server configures NO auth provider, so every v2 caller resolves to the
//! same anonymous owner ([`SHARED_ANONYMOUS_OWNER`], the empty principal) and
//! therefore shares ONE task bucket. Any caller can read, feed and cancel any
//! task any other caller created. That is acceptable here because the server
//! binds to loopback and exists to be read, and it is NOT acceptable anywhere
//! else.
//!
//! A real deployment configures OAuth so the owner is the authenticated `sub`.
//! pmcp then scopes every `tasks/*` operation to it and answers a cross-owner
//! request with the SAME `-32602` it answers for an id that never existed, so
//! task ids cannot be probed for existence.

#![cfg(not(target_arch = "wasm32"))]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::task_store::{InMemoryTaskStore, TaskInputSnapshot, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::content::Content;
use pmcp::types::elicitation::ElicitRequestParams;
use pmcp::types::protocol::{
    ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::types::tasks::TaskStatus;
use pmcp::types::{
    CallToolResult, InputRequest, InputRequests, InputResponse, TaskSupport, ToolExecution,
};
use pmcp::Server;
use serde_json::{json, Value};
use tokio::sync::Mutex;

/// The task-capable tool this example exposes.
const TOOL_NAME: &str = "research";

/// The `inputRequests` key the tool asks under, and the field its elicitation
/// schema wants.
///
/// The key is SERVER-ASSIGNED and opaque to the client; reusing one spelling for
/// both is a readability choice, not a protocol rule.
const TOPIC_KEY: &str = "topic";

/// Where the server binds when `argv[1]` is absent.
const DEFAULT_ADDR: &str = "127.0.0.1:8150";

/// The owner every caller resolves to on a server with NO auth provider.
///
/// The empty string is the anonymous principal. It is named here rather than
/// spelled `""` at three call sites so the shared-bucket caveat in this file's
/// header has something concrete to point at. A deployment with OAuth sees the
/// authenticated `sub` here instead, and the worker below would then have to
/// carry the owner with the work rather than assume one.
const SHARED_ANONYMOUS_OWNER: &str = "";

/// How often the worker looks for tasks whose input has arrived.
const WORKER_TICK_MS: u64 = 25;

/// The TTL the tool requests for its task, in milliseconds.
const TASK_TTL_MS: u64 = 300_000;

/// The `taskId` the handler fabricates, and which the store THROWS AWAY.
///
/// This is the single most surprising thing about writing a task handler, so the
/// value says so in its own name. See [`research_task_value`].
const DISCARDED_TASK_ID: &str = "discarded-the-store-mints-the-real-one";

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Example setup: pmcp's own WARNINGs stay visible (the unset
    // PMCP_REQUEST_STATE_KEY notice is one of them, and swallowing an SDK
    // warning in an example teaches people to swallow it in production).
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=warn")
        .init();

    let requested: SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string())
        .parse()?;

    // ---- The store. ----
    //
    // ONE `Arc` serves two roles: the SDK reads it to answer `tasks/*`, and the
    // worker below writes to it to finish the work. Configuring it is also what
    // advertises the tasks extension on `server/discover` — there is no separate
    // capability switch to remember.
    let store = Arc::new(InMemoryTaskStore::new());
    let worker_store: Arc<dyn TaskStore> = store.clone();
    let server_store: Arc<dyn TaskStore> = store;

    let task_tool = TypedTool::new_with_schema(
        TOOL_NAME,
        json!({ "type": "object", "properties": {} }),
        |_args: Value, _extra| Box::pin(async { research_task_value() }),
    )
    .with_description("Research a topic asynchronously, asking which topic first")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

    // The accept-list is what opts this server into 2026-07-28. Listing the
    // 2025-11-25 version alongside it keeps v1 clients working: the era is
    // negotiated PER REQUEST, so one binary serves both. On v1 this same tool
    // still works — with the v1 `task` trigger, and with `tasks/list` and
    // `tasks/result` still served.
    let server = Server::builder()
        .name("s50-v2-tasks-server")
        .version("1.0.0")
        .with_supported_protocol_versions([
            ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ])
        .tool(TOOL_NAME, task_tool)
        .task_store(server_store)
        .build()?;

    let http = StreamableHttpServer::with_config(
        requested,
        Arc::new(Mutex::new(server)),
        StreamableHttpServerConfig::default(),
    );
    let (addr, server_handle) = http.start().await?;

    let worker_handle = tokio::spawn(run_worker(worker_store));

    print_instructions(addr);

    // A server, not a one-shot script: run until signalled.
    let outcome = server_handle.await;
    worker_handle.abort();
    outcome?;
    Ok(())
}

/// The task-shaped value the tool returns to DECLARE a pause.
///
/// # The handler's `taskId` is DISCARDED — read this before writing your own
///
/// `store.create()` mints the canonical id INSIDE dispatch, AFTER this function
/// has already returned, so a handler cannot know the id the client will poll.
/// The `taskId` here exists only because the create gate requires a task-SHAPED
/// value; the store's id wins on the wire and in `_meta.relatedTask`. Never
/// derive anything from a handler-fabricated id, and never hand one to a client.
///
/// # What makes this a PAUSE rather than a completion
///
/// `status: "input_required"` plus an `inputRequests` object. Dispatch re-extracts
/// that map and records it against the STORE-minted id, so the handle this call
/// returns is ALREADY paused and pollable: the client's very first `tasks/get`
/// shows `input_required` and inlines exactly this request set. A value carrying
/// a `result` instead would complete synchronously; a value with neither would
/// stay `working`.
///
/// `createdAt` / `lastUpdatedAt` are deliberately ABSENT. The store owns the
/// task record's timestamps, and a handler that invented them would be
/// publishing a second, disagreeing clock.
///
/// The requests are built from the TYPED [`InputRequests`] and serialized, rather
/// than hand-written as wire JSON, so the `elicitation/create` method spelling
/// comes from the SDK's own enum.
fn research_task_value() -> pmcp::Result<Value> {
    let mut requests = InputRequests::new();
    requests.insert(
        TOPIC_KEY.to_string(),
        InputRequest::Elicitation(Box::new(ElicitRequestParams::Form {
            message: "Which topic should I research?".to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": { TOPIC_KEY: { "type": "string" } },
                "required": [TOPIC_KEY],
            }),
        })),
    );
    let requests =
        serde_json::to_value(requests).map_err(|error| pmcp::Error::internal(error.to_string()))?;

    Ok(json!({
        "taskId": DISCARDED_TASK_ID,
        "status": "input_required",
        "ttl": TASK_TTL_MS,
        "inputRequests": requests,
    }))
}

/// The APPLICATION half of the loop: finish tasks whose input has arrived.
///
/// This is not SDK machinery and there is no SDK hook it should have been
/// written against. `tasks/update` delivered the client's answers and moved the
/// task to `working`; something has to turn `working` into `completed`, and that
/// something is the code that does the work. Here it is a 25 ms poll over the
/// store because the store is a `HashMap`; in a real deployment it is whatever
/// already owns the job.
///
/// It reads the delivered answers through
/// [`TaskStore::task_input_snapshot`] — the OWNER-SCOPED accessor, which is the
/// only supported way to reach them. `inputResponses` never appears in a
/// `tasks/get` payload, so a client cannot read back what it (or anyone else)
/// answered.
///
/// Every store call is tolerated rather than unwrapped: a task can expire or be
/// cancelled between the `list` and the write, and a worker that panicked on
/// that would take the server down for a race it should simply skip.
async fn run_worker(store: Arc<dyn TaskStore>) {
    loop {
        tokio::time::sleep(Duration::from_millis(WORKER_TICK_MS)).await;

        let Ok((tasks, _cursor)) = store.list(SHARED_ANONYMOUS_OWNER, None).await else {
            continue;
        };

        for task in tasks {
            // `input_required` means the answers have not arrived; anything
            // terminal means this task is already finished.
            if task.status != TaskStatus::Working {
                continue;
            }
            let Ok(snapshot) = store
                .task_input_snapshot(&task.task_id, SHARED_ANONYMOUS_OWNER)
                .await
            else {
                continue;
            };
            if !snapshot.is_complete() {
                continue;
            }

            println!(
                "  worker: task {} received its input, completing it",
                task.task_id
            );

            let result = finish(&snapshot);
            if store
                .set_result(&task.task_id, SHARED_ANONYMOUS_OWNER, result)
                .await
                .is_err()
            {
                continue;
            }
            let _ = store
                .update_status(
                    &task.task_id,
                    SHARED_ANONYMOUS_OWNER,
                    TaskStatus::Completed,
                    None,
                )
                .await;
        }
    }
}

/// Turn the delivered answers into the task's terminal [`CallToolResult`].
///
/// A DECLINED elicitation is a legitimate answer, not a protocol fault, so it
/// completes the task with an `isError` result rather than leaving it paused
/// forever. A task nobody can finish is worse than a task that reports why.
fn finish(snapshot: &TaskInputSnapshot) -> CallToolResult {
    match answered_topic(snapshot) {
        Some(topic) => CallToolResult::new(vec![Content::Text {
            text: format!("research on {topic}: 3 sources reviewed, no contradictions found"),
        }]),
        None => CallToolResult::error(vec![Content::Text {
            text: "no usable topic was supplied, so there was nothing to research".to_string(),
        }]),
    }
}

/// Read the topic out of the delivered `inputResponses`, if one was supplied.
///
/// Returns `None` for a missing, declined or wrong-shaped answer. Everything in
/// a snapshot's `input_responses` arrived from a CLIENT and must be validated
/// exactly like tool arguments — the fact that it came through a task does not
/// make it more trustworthy.
///
/// The VARIANT, on the other hand, is trustworthy: `tasks/update` decoded the
/// value against the kind THIS server recorded, so an `Elicitation` here really
/// was answered against an `elicitation/create` request. A client cannot choose
/// how its own answer is typed.
fn answered_topic(snapshot: &TaskInputSnapshot) -> Option<String> {
    let InputResponse::Elicitation(result) = snapshot.input_responses.get(TOPIC_KEY)? else {
        return None;
    };
    result
        .content
        .as_ref()?
        .get(TOPIC_KEY)?
        .as_str()
        .map(str::to_string)
}

/// Print the bound address, the negotiated versions and the paired client
/// command.
fn print_instructions(addr: SocketAddr) {
    println!();
    println!("=============================================================");
    println!("  v2 (2026-07-28) TASKS SERVER — pause, update, resume");
    println!("=============================================================");
    println!("  Listening on : {addr}");
    println!("  Endpoint     : http://{addr}");
    println!(
        "  Versions     : {LATEST_PROTOCOL_VERSION} (v1) and {PROTOCOL_VERSION_2026_07_28} (v2)"
    );
    println!("  Tool         : {TOOL_NAME} (TaskSupport::Required)");
    println!("  Extension    : {TASKS_EXTENSION_KEY}");
    println!("  Store        : InMemoryTaskStore (no auth provider — SHARED");
    println!("                 owner bucket; see this example's header)");
    println!("-------------------------------------------------------------");
    println!("  A v2 caller that DECLARES the tasks extension and calls");
    println!("  {TOOL_NAME} receives a task handle that is ALREADY paused on an");
    println!("  elicitation for \"{TOPIC_KEY}\". Answering it with tasks/update");
    println!("  resumes the task; a worker in this process then completes it.");
    println!();
    println!("  tasks/list and tasks/result are RETIRED on 2026-07-28 and this");
    println!("  server answers both -32601.");
    println!("-------------------------------------------------------------");
    println!("  Now run the paired autonomous agent:");
    println!("    cargo run --example s51_v2_tasks_agent --features full -- {addr}");
    println!("=============================================================");
    println!();
    println!("Press Ctrl+C to stop the server");
}
