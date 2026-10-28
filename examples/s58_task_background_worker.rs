//! # Server example: hand a task's work to a background worker
//!
//! A long-running tool that runs as an MCP Task when the client can poll for
//! it, and synchronously when it cannot. It shows the three handler-side task
//! APIs together:
//!
//! - [`RequestHandlerExtra::will_create_task`] tells the handler, before it
//!   does any work, whether returning a task-shaped value will produce a task
//!   the client can poll. When it is `false` the handler does the work inline,
//!   so a client that cannot poll never receives a task handle.
//! - [`RequestHandlerExtra::on_task_created`] hands the background worker the
//!   **store-minted** task id and owner. The store mints the id after the
//!   handler returns, so this hook is the only way a worker can learn it.
//! - [`TaskStore::set_status_message`] reports progress ("step 2/4") on a task
//!   that stays `working`, which `update_status` cannot do because the state
//!   machine rejects `working -> working`.
//!
//! The client calls the tool twice over real HTTP: once asking for a task (it
//! polls `tasks/get`, printing each progress message, then reads
//! `tasks/result`) and once without (it gets the result directly).
//!
//! Every step is a hard assertion, not just printed output.
//!
//! Run with: `cargo run --example s58_task_background_worker --features full`

#![cfg(not(target_arch = "wasm32"))]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::task_store::{CreatedTask, InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::types::tasks::TaskStatus;
use pmcp::types::{CallToolResult, ClientCapabilities, Content, TaskSupport, ToolExecution};
use pmcp::{Client, Error, RequestHandlerExtra, Server, ToolCallResponse};
use serde_json::{json, Value};
use url::Url;

const STEPS: u32 = 4;

/// The long-running work, reporting progress through `report` after each step.
async fn run_backtest<F, Fut>(mut report: F) -> String
where
    F: FnMut(u32) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    for step in 1..=STEPS {
        tokio::time::sleep(Duration::from_millis(20)).await;
        report(step).await;
    }
    format!("backtest finished: {STEPS} windows evaluated")
}

/// The background worker: the minted id and owner arrive in `created`.
async fn worker(store: Arc<dyn TaskStore>, created: CreatedTask) {
    let id = created.task.task_id;
    let owner = created.owner_id;
    let summary = run_backtest(|step| {
        let (store, id, owner) = (Arc::clone(&store), id.clone(), owner.clone());
        async move {
            let message = format!("step {step}/{STEPS}");
            if let Err(e) = store.set_status_message(&id, &owner, Some(message)).await {
                eprintln!("progress update failed: {e}");
            }
        }
    })
    .await;

    let result = CallToolResult::new(vec![Content::text(summary)]);
    if let Err(e) = store.set_result(&id, &owner, result).await {
        eprintln!("storing the result failed: {e}");
        return;
    }
    if let Err(e) = store
        .update_status(&id, &owner, TaskStatus::Completed, None)
        .await
    {
        eprintln!("completing the task failed: {e}");
    }
}

fn backtest_tool(store: Arc<dyn TaskStore>) -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "backtest",
        json!({ "type": "object" }),
        move |_args: Value, extra: RequestHandlerExtra| {
            let store = Arc::clone(&store);
            if extra.will_create_task() {
                // Asynchronous path: the dispatcher will create a task, so hand
                // the work to a worker keyed by the id the client will poll.
                extra.on_task_created(move |created| {
                    tokio::spawn(worker(store, created));
                });
                return Box::pin(async {
                    Ok(json!({ "taskId": "assigned-by-the-store", "status": "working" }))
                });
            }
            // Synchronous path: this client cannot poll, so do the work inline.
            // A plain value; the SDK wraps it into the tool result.
            Box::pin(async {
                let summary = run_backtest(|_| async {}).await;
                Ok(json!({ "summary": summary }))
            })
        },
    )
    .with_description("Run a backtest; long, so it runs as a task when the client can poll")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    let server = Server::builder()
        .name("task-background-worker-example")
        .version("1.0.0")
        .tool("backtest", backtest_tool(Arc::clone(&store)))
        .task_store(store)
        .build()?;

    // `new_concurrent`: a long call must not hold up the polls for it.
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback addr");
    let (bound, server_handle) = StreamableHttpServer::new_concurrent(bind_addr, Arc::new(server))
        .start()
        .await?;
    println!("task server listening on http://{bound}");

    let result = run_client(bound).await;
    server_handle.abort();
    result
}

async fn run_client(bound: SocketAddr) -> pmcp::Result<()> {
    let config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{bound}")).map_err(|e| Error::Internal(e.to_string()))?,
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: None,
    };
    let mut client = Client::new(StreamableHttpTransport::new(config));
    client.initialize(ClientCapabilities::default()).await?;

    // 1) Ask for a task: the handler hands off and returns at once.
    let task_id = match client
        .call_tool_with_task("backtest".to_string(), json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => {
            return Err(Error::internal("expected a task, got a synchronous result"))
        },
    };
    println!("1) task created with the store-minted id {task_id}");

    // 2) Poll, printing each progress message the worker reports.
    let mut task = client.tasks_get(&task_id).await?;
    let mut messages = Vec::new();
    for _ in 0..200 {
        if let Some(message) = &task.status_message {
            if messages.last() != Some(message) {
                println!("2) {} - {message}", task.status);
                messages.push(message.clone());
            }
        }
        if task.status.is_terminal() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        task = client.tasks_get(&task_id).await?;
    }
    if task.status != TaskStatus::Completed {
        return Err(Error::internal(format!(
            "the worker must complete the task the client polls; status is {}",
            task.status
        )));
    }
    if messages.is_empty() {
        return Err(Error::internal("progress messages must reach tasks/get"));
    }

    // 3) Read the worker's result.
    let result = client.tasks_result(&task_id).await?;
    println!("3) tasks/result: {:?}", result.content);
    if result.content.is_empty() {
        return Err(Error::internal(
            "tasks/result must carry the worker's result",
        ));
    }

    // 4) Without asking for a task, the same tool answers synchronously.
    let direct = client.call_tool("backtest".to_string(), json!({})).await?;
    println!("4) synchronous call: {:?}", direct.content);
    if direct.content.is_empty() {
        return Err(Error::internal("the synchronous path returns the result"));
    }

    println!("background-worker task lifecycle OK");
    Ok(())
}
