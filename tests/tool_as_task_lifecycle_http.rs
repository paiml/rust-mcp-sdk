//! Tools-as-Tasks LIVE HTTP loopback acceptance gate (Phase 102, HTASK-03).
//!
//! This is the phase's end-to-end proof: a REAL HTTP round-trip (no in-process
//! duplex shim) that drives the full tasks/* lifecycle through a HIGH-LEVEL,
//! store-backed `pmcp::Server` served over `StreamableHttpServer` +
//! `StreamableHttpTransport`. Phase 101 could only test the task path via an
//! in-process duplex transport because the HTTP path rejected `tasks/*`; with
//! Plan 02 wired, this test replaces that shim with the live boundary — the
//! carried-from-Phase-101 rule that protocol-shape requirements are "resolved"
//! ONLY via a live round-trip.
//!
//! It mirrors every Phase 101 invariant over HTTP:
//! - finding #4 — `initialize` observes the auto-advertised `tasks` capability.
//! - id consistency (finding #3) — the wire `CreateTaskResult.task.taskId` is the
//!   store-minted id (not the tool-fabricated one), and `tasks/get` polls THAT id.
//! - finding #1 — typed, non-empty `tasks/result` from a persisted terminal
//!   `CallToolResult`, plus the specified `-32002` pending error before completion.
//!
//! Test reliability is a first-class requirement here (Codex HIGH concerns):
//! - EPHEMERAL PORT (Concern #9): the harness binds `127.0.0.1:0` and reads the
//!   actual bound address back from `StreamableHttpServer::start()` — NO hardcoded
//!   port, NO `:18765`.
//! - READINESS (Concern #10): `start()` binds the `TcpListener` BEFORE it returns,
//!   so the returned `local_addr` is already accepting connections — a deterministic
//!   readiness signal, no fixed sleep.
//! - SHUTDOWN (Concern #10): the spawned server `JoinHandle` is `abort()`ed after
//!   the client completes so the test process can never hang on a lingering server.
//!
//! Cross-owner isolation (Concern #7) is also asserted over the REAL HTTP boundary:
//! two clients inject distinct owners via the `x-pmcp-user-id` proxy header, and
//! owner B cannot `tasks/get` / `tasks/result` / `tasks/cancel` owner A's task.

#![cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]

use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::types::{ClientCapabilities, TaskSupport, ToolExecution};
use pmcp::{Client, ErrorCode, Server, ToolCallResponse};
use tokio::sync::Mutex;
use url::Url;

// ---------------------------------------------------------------------------
// Tool builders (mirror tests/tool_as_task_lifecycle.rs:135-179).
// ---------------------------------------------------------------------------

/// A `with_task_support` tool that completes synchronously: its handler returns a
/// Task-shaped value carrying a nested `result` (the terminal `CallToolResult`),
/// so the shared create path records create + `set_result` + Completed before
/// returning. Polling therefore observes `Completed` and `tasks/result` serves a
/// non-empty terminal `CallToolResult`.
fn completing_task_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "complete_now",
        serde_json::json!({ "type": "object" }),
        |_args: serde_json::Value, _extra| {
            Box::pin(async {
                Ok(serde_json::json!({
                    "taskId": "tool-fabricated",
                    "status": "completed",
                    "ttl": 60000,
                    "createdAt": "2026-06-21T00:00:00Z",
                    "lastUpdatedAt": "2026-06-21T00:00:00Z",
                    "result": {
                        "content": [ { "type": "text", "text": "terminal result payload" } ]
                    }
                }))
            })
        },
    )
    .with_description("A task tool that completes synchronously with content")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

/// A `with_task_support` tool that returns NO result content (only task metadata),
/// so the task stays genuinely pending — used to prove the specified `-32002`
/// not-completed error from `tasks/result`.
fn pending_task_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "stay_pending",
        serde_json::json!({ "type": "object" }),
        |_args: serde_json::Value, _extra| {
            Box::pin(async {
                Ok(serde_json::json!({
                    "taskId": "tool-fabricated",
                    "status": "working",
                    "ttl": 60000,
                    "createdAt": "2026-06-21T00:00:00Z",
                    "lastUpdatedAt": "2026-06-21T00:00:00Z"
                }))
            })
        },
    )
    .with_description("A task tool that stays pending (no terminal content)")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

/// Build the HIGH-LEVEL, store-backed `Server` that this phase makes HTTP-serveable.
///
/// This is the capability Plan 02 added: `Server::builder().tool(t).task_store(store)`
/// now auto-advertises `tasks` and serves the full lifecycle over `Server::handle_request`
/// (the exact entrypoint `StreamableHttpServer` calls) — NO `ServerCore` shim.
fn build_task_server() -> pmcp::Result<Server> {
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    Server::builder()
        .name("s46-http-task-server")
        .version("1.0.0")
        .tool("complete_now", completing_task_tool())
        .tool("stay_pending", pending_task_tool())
        .task_store(store)
        .build()
}

/// Stand the server up over REAL HTTP and return the read-back bound address plus
/// the server `JoinHandle`.
///
/// EPHEMERAL PORT (Concern #9): binds `127.0.0.1:0` and reads the assigned address
/// back from `start()` — no hardcoded port. READINESS (Concern #10): `start()` binds
/// the listener BEFORE returning, so the returned address is already accepting — a
/// deterministic readiness signal (no sleep).
async fn spawn_http_task_server() -> pmcp::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let server = Arc::new(Mutex::new(build_task_server()?));
    // EPHEMERAL PORT: bind :0 and let the OS assign a free port.
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback addr");
    let http_server = StreamableHttpServer::new(bind_addr, server);
    // start() binds the TcpListener before returning, so `bound` is already listening.
    let (bound, server_handle) = http_server.start().await?;
    Ok((bound, server_handle))
}

/// Build an HTTP client transport targeting the read-back bound address, optionally
/// injecting an owner identity via the `x-pmcp-user-id` proxy header (used for the
/// cross-owner isolation case). `enable_json_response: true` keeps the loopback on the
/// single-response JSON path.
fn http_client(
    bound: SocketAddr,
    owner: Option<&str>,
) -> pmcp::Result<Client<StreamableHttpTransport>> {
    // Builder, not a struct literal: `session_id` / `on_resumption_token` are
    // `v1-compat`-only, so a literal naming them breaks the `full-v2` build.
    let mut builder = StreamableHttpTransportConfigBuilder::new(
        Url::parse(&format!("http://{bound}")).map_err(|e| pmcp::Error::Internal(e.to_string()))?,
    )
    .enable_json_response();
    if let Some(id) = owner {
        builder = builder.with_header("x-pmcp-user-id", id);
    }
    let config = builder.build();
    Ok(Client::new(StreamableHttpTransport::new(config)))
}

// ---------------------------------------------------------------------------
// Live HTTP round-trip: the full lifecycle + all Phase 101 invariants over HTTP.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_http_round_trip_typed_lifecycle_id_consistency_and_capability() -> pmcp::Result<()> {
    let (bound, server_handle) = spawn_http_task_server().await?;

    // Guard everything so the server handle is ALWAYS aborted, even on assertion
    // failure (Concern #10 — no lingering HTTP server, no process hang).
    let outcome = run_lifecycle_over_http(bound).await;

    // SHUTDOWN (Concern #10): abort the spawned server task after the client is done.
    server_handle.abort();
    // The aborted task resolves to a `Cancelled` JoinError; anything else means the
    // server's serve future finished early with a real error we must surface.
    match server_handle.await {
        Ok(()) => {},
        Err(e) if e.is_cancelled() => {},
        Err(e) => panic!("HTTP server task ended unexpectedly: {e}"),
    }

    outcome
}

/// The actual round-trip body, factored out so the caller can always abort the
/// server handle afterward regardless of the result.
async fn run_lifecycle_over_http(bound: SocketAddr) -> pmcp::Result<()> {
    let mut client = http_client(bound, None)?;

    // finding #4: initialize over HTTP sees the auto-advertised `tasks` capability.
    let init = client.initialize(ClientCapabilities::default()).await?;
    assert!(
        init.capabilities.tasks.is_some(),
        "server must auto-advertise the `tasks` capability over HTTP (HTASK-03)"
    );

    // call(tool with task) over HTTP -> CreateTaskResult with the store-minted id.
    let response = client
        .call_tool_with_task("complete_now".to_string(), serde_json::json!({}))
        .await?;
    let client_task_id = match response {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => {
            return Err(pmcp::Error::internal(
                "expected a created task, got a sync result",
            ))
        },
    };
    assert_ne!(
        client_task_id, "tool-fabricated",
        "wire id must be the store-minted id over HTTP, not a tool-fabricated one"
    );

    // Poll tasks/get over HTTP until terminal; the polled id must equal the client id
    // (finding #3 — proves it is the store-minted, pollable id, not a 404).
    let mut polled = client.tasks_get(&client_task_id).await?;
    for _ in 0..50 {
        assert_eq!(
            polled.task_id, client_task_id,
            "polled task id must equal the client-observed (store-minted) id over HTTP"
        );
        if polled.status.is_terminal() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        polled = client.tasks_get(&client_task_id).await?;
    }
    assert!(
        polled.status.is_terminal(),
        "task should reach a terminal status over HTTP (synchronous completion)"
    );

    // finding #1: typed, non-empty CallToolResult from tasks/result over HTTP.
    let result = client.tasks_result(&client_task_id).await?;
    assert!(
        !result.content.is_empty(),
        "tasks/result must carry the persisted non-empty terminal content over HTTP"
    );

    // finding #1 (MED): a genuinely-pending task's tasks/result returns the SPECIFIED
    // `-32002` not-completed error over HTTP — the frozen pending shape survives HTTP framing.
    let pending = client
        .call_tool_with_task("stay_pending".to_string(), serde_json::json!({}))
        .await?;
    let pending_id = match pending {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => {
            return Err(pmcp::Error::internal("expected a created (pending) task"))
        },
    };
    let err = client
        .tasks_result(&pending_id)
        .await
        .expect_err("pending tasks/result must be an error over HTTP, not Ok");
    assert_eq!(
        err.error_code(),
        Some(ErrorCode(-32002)),
        "pending tasks/result must return the specified -32002 not-completed error over HTTP, got: {err}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-owner isolation over the REAL HTTP boundary (Concern #7, T-102-12).
// ---------------------------------------------------------------------------

/// Two clients inject distinct owners via the `x-pmcp-user-id` proxy header (the
/// `Server` derives the task owner from the resulting `AuthContext.subject`). Owner
/// B must NOT be able to `tasks/get`, `tasks/result`, or `tasks/cancel` owner A's
/// task — proving owner-scoped IDOR protection holds across the live HTTP boundary,
/// not just at the in-crate `Server::handle_request` layer (Plan 02's
/// `tasks_cross_owner_isolation`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_http_cross_owner_isolation() -> pmcp::Result<()> {
    let (bound, server_handle) = spawn_http_task_server().await?;

    let outcome = run_cross_owner_over_http(bound).await;

    server_handle.abort();
    match server_handle.await {
        Ok(()) => {},
        Err(e) if e.is_cancelled() => {},
        Err(e) => panic!("HTTP server task ended unexpectedly: {e}"),
    }

    outcome
}

async fn run_cross_owner_over_http(bound: SocketAddr) -> pmcp::Result<()> {
    // Owner A creates a task over HTTP.
    let mut alice = http_client(bound, Some("alice"))?;
    alice.initialize(ClientCapabilities::default()).await?;
    let alice_task = match alice
        .call_tool_with_task("complete_now".to_string(), serde_json::json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => {
            return Err(pmcp::Error::internal("expected a created task for alice"))
        },
    };

    // Owner A can read its own task (sanity — the harness/owner-scoping works).
    let alice_view = alice.tasks_get(&alice_task).await?;
    assert_eq!(
        alice_view.task_id, alice_task,
        "owner A must retain access to its own task over HTTP"
    );

    // Owner B (distinct `x-pmcp-user-id`) must NOT see owner A's task.
    let mut bob = http_client(bound, Some("bob"))?;
    bob.initialize(ClientCapabilities::default()).await?;

    bob.tasks_get(&alice_task)
        .await
        .expect_err("owner B must NOT be able to tasks/get owner A's task over HTTP");
    bob.tasks_result(&alice_task)
        .await
        .expect_err("owner B must NOT be able to tasks/result owner A's task over HTTP");
    bob.tasks_cancel(&alice_task)
        .await
        .expect_err("owner B must NOT be able to tasks/cancel owner A's task over HTTP");

    Ok(())
}
