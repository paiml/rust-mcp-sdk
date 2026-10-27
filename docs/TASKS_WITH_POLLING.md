# Tasks with Polling

MCP Tasks let servers manage long-running operations that outlive a single request/response cycle. Instead of SSE-based notifications, the client polls for status updates on a timer -- stateless, simple, and compatible with serverless deployments.

In v2.0, task detection is **requestor-driven**: the SDK returns `CreateTaskResult` only when the client sends a `task` field in the `tools/call` request. Without it, the tool result is wrapped as a normal `CallToolResult`. This is capability-based negotiation, not client sniffing.

## Recommended Server Setup

Register a task-capable tool (`with_task_support`) and back it with a `TaskStore` via `task_store(...)`. The SDK serves `tasks/get | result | list | cancel` typed from your store — **you never hand-write `tasks/*` wire JSON**, and the `tasks` capability is auto-advertised because a store backs it.

```rust
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::{ToolExecution, TaskSupport};
use serde_json::json;
use std::sync::Arc;

let task_tool = TypedTool::new_with_schema(
    "long_analysis",
    json!({ "type": "object" }),
    |_args, _extra| Box::pin(async { Ok(json!({ "content": [] })) }),
)
.with_description("Run a long analysis as an MCP Task")
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;

let server = ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool("long_analysis", task_tool)
    .task_store(store)   // presence of a store auto-advertises the `tasks` capability
    .build()?;
```

The builder:
- Sets the standard top-level `ServerCapabilities.tasks` automatically (list, cancel, tools/call) **because a store backs the endpoints** — a `TaskSupport::Required` tool with **no** store makes `build()` return `Err` (no hollow capability)
- Dispatches `tasks/get`, `tasks/list`, `tasks/cancel`, and `tasks/result` through your store, serialized from typed structs
- Binds each task to its owner, the authenticated OAuth subject (isolation needs authentication; see [Owner Resolution](#owner-resolution))
- Mints the task id in the store and returns it to the client; returns `CreateTaskResult` or `CallToolResult` based on the client's request

Both `ServerCoreBuilder` and the high-level `Server::builder()` take `task_store(...)`, so the same setup works behind `StreamableHttpServer`. For a complete runnable round-trip, see [`examples/s45_tool_as_task_lifecycle.rs`](../examples/s45_tool_as_task_lifecycle.rs).

> **Long work belongs in a task, not behind a timeout.** A host that gives up on a slow synchronous `tools/call` does not necessarily stop the handler; on Cloud Run, handlers have been observed running about two minutes after the client left. On the streamable-HTTP transport, build the server with `StreamableHttpServer::new_concurrent` so one long call doesn't hold up the others.

> **Legacy path:** `with_task_store(Arc<dyn TaskRouter>)` (wrapping a store in `pmcp_tasks::TaskRouterImpl`) is the older experimental wiring; it advertises under `experimental.tasks` and expects hand-emitted `task` JSON. Prefer `task_store(...)` + `with_task_support` for new code.

## Recommended Client Walkthrough

The typed client never touches `tasks/*` JSON either:

```rust
use pmcp::ToolCallResponse;

// 1) Call the tool as a task — receive the STORE-MINTED id.
let task_id = match client
    .call_tool_with_task("long_analysis".to_string(), serde_json::json!({}))
    .await?
{
    ToolCallResponse::Task(task) => task.task_id,
    ToolCallResponse::Result(_) => unreachable!("Required task tool always creates a task"),
};

// 2) Poll the typed `Task` until it reaches a terminal status.
let mut task = client.tasks_get(&task_id).await?;
while !task.status.is_terminal() {
    tokio::time::sleep(std::time::Duration::from_millis(task.poll_interval.unwrap_or(2000))).await;
    task = client.tasks_get(&task_id).await?;
}

// 3) Fetch the typed terminal `CallToolResult`.
let result = client.tasks_result(&task_id).await?;
```

`client.tasks_list(cursor)` and `client.tasks_cancel(&task_id)` complete the typed client surface.

## Declaring Task Support on Tools

Every tool that participates in the task lifecycle must declare its `TaskSupport` level via `ToolExecution`:

```rust
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::{ToolExecution, TaskSupport};
use serde_json::json;

// Required: tool MUST be called with task augmentation
let tool = TypedTool::new_with_schema(
    "long_analysis",
    json!({"type": "object"}),
    |args, extra| Box::pin(async move { /* ... */ Ok(json!({})) }),
)
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

// Optional: tool works with or without task augmentation
let tool = TypedTool::new_with_schema(
    "flexible_query",
    json!({"type": "object"}),
    |args, extra| Box::pin(async move { /* ... */ Ok(json!({})) }),
)
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional));

// Forbidden: tool never creates tasks (default when execution is omitted)
let tool = TypedTool::new_with_schema(
    "quick_lookup",
    json!({"type": "object"}),
    |args, extra| Box::pin(async move { /* ... */ Ok(json!({})) }),
)
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Forbidden));
```

The three levels:

| TaskSupport | Meaning |
|-------------|---------|
| `Required` | Tool always produces a task. If the client omits the `task` field, the SDK logs a warning and falls through to `CallToolResult` for compatibility. |
| `Optional` | Tool can work both ways. Use `extra.is_task_request()` in the handler to branch. |
| `Forbidden` | Tool never creates tasks. The SDK ignores any `task` field on the request. |

## Capability Negotiation

An MCP server should not ask "who is the client?" It should ask "what capabilities has the client declared?"

The `task` field in the `tools/call` request IS the per-request capability signal. No session state is needed, no capability handshake during `initialize`. The SDK enforces this with a four-part check before returning `CreateTaskResult`:

1. `task_store` is configured on the server
2. The tool declares `taskSupport` of `Required` or `Optional`
3. The client sent a `task` field in the request (explicit task-augmented call)
4. The tool handler returned a Task-shaped value (has `taskId` + `status`)

When any condition is not met, the SDK falls through to `CallToolResult`. This means:
- Task-aware clients (Claude Desktop, custom agents) get `CreateTaskResult` with polling
- Non-task-aware clients (ChatGPT, older clients) get `CallToolResult` with text content

The tool handler itself decides what to return based on `extra.is_task_request()`.

## How Polling Works

### Task-Aware Client (sends `task` field)

```
Client                              Server
  |                                    |
  |-- tools/call { task: {...} } ----->|
  |<-- CreateTaskResult { task } ------|  (status: working)
  |                                    |
  |-- tasks/get { task_id } ---------->|  (poll every 2-5s)
  |<-- Task { status: working } -------|
  |                                    |
  |-- tasks/get { task_id } ---------->|
  |<-- Task { status: completed } -----|
  |                                    |
  |   (client stops polling)           |
```

### Non-Task-Aware Client (no `task` field)

```
Client                              Server
  |                                    |
  |-- tools/call (no task field) ----->|
  |<-- CallToolResult { text } --------|  (task_id embedded in text)
  |                                    |
  |-- tools/call get_task_result ----->|  (fallback polling tool)
  |<-- CallToolResult { text } --------|  (status: working)
  |                                    |
  |-- tools/call get_task_result ----->|
  |<-- CallToolResult { text } --------|  (status: completed + result)
  |                                    |
  |   (client stops polling)           |
```

The `Task` response includes `poll_interval` (milliseconds) to guide the client's polling frequency. Terminal statuses (`completed`, `failed`, `cancelled`) signal the client to stop polling.

## Using `extra.is_task_request()` in a Tool Handler

The `RequestHandlerExtra.is_task_request()` method is the primary tool for building dual-path handlers. It returns `true` when the client sent a `task` field in the request.

```rust
use pmcp::{TaskStore, RequestHandlerExtra};
use pmcp_tasks::InMemoryTaskStore;
use pmcp_tasks::task::TaskStatus;
use serde_json::{json, Value};
use std::sync::Arc;

struct FlexibleTool {
    task_store: Arc<InMemoryTaskStore>,
}

// Inside the tool's execute method:
async fn execute(
    &self,
    args: Value,
    extra: RequestHandlerExtra,
) -> pmcp::Result<Value> {
    if extra.is_task_request() {
        // Async path: create task, spawn background work, return immediately
        let task = self.task_store.create("local", Some(300_000)).await?;
        let task_id = task.task_id.clone();
        let store = self.task_store.clone();

        tokio::spawn(async move {
            // ... do expensive work ...
            let _ = store.update_status(
                &task_id, "local",
                TaskStatus::Completed,
                Some("Done processing".into()),
            ).await;
        });

        // Return task-shaped value; SDK wraps as CreateTaskResult
        Ok(json!({
            "taskId": task.task_id,
            "status": "working"
        }))
    } else {
        // Sync path: do the work inline, return CallToolResult text
        let result = do_work_inline(&args).await;
        Ok(json!({ "result": result }))
    }
}
```

This pattern is the foundation for `TaskSupport::Optional` tools. The same tool serves both task-aware and non-task-aware clients without separate implementations.

## Client Compatibility

### The `get_task_result` Fallback Tool Pattern

Non-task-aware clients (like ChatGPT) cannot send the `task` field or call `tasks/get`. For these clients, a recommended server-side pattern is to expose a fallback polling tool:

```rust
// This is a server-implemented pattern, NOT a built-in SDK feature.
// The server registers a regular tool that wraps task store reads.

fn get_task_result_tool(store: Arc<InMemoryTaskStore>) -> TypedTool {
    TypedTool::new_with_schema(
        "get_task_result",
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task ID to check" }
            },
            "required": ["task_id"]
        }),
        move |args: Value, _extra| {
            let store = store.clone();
            Box::pin(async move {
                let task_id = args["task_id"].as_str()
                    .ok_or_else(|| pmcp::Error::validation("task_id required"))?;
                let task = store.get(task_id, "local").await?;
                Ok(serde_json::to_value(&task)?)
            })
        },
    )
    .with_description("Check the status and result of a background task")
    // Forbidden: this tool itself should NOT create tasks
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Forbidden))
}
```

The dual-path flow for `TaskSupport::Optional` tools in serverless deployments:

| Client sends `task` | Tool behavior | Response type |
|---------------------|---------------|---------------|
| Yes | Create task, return immediately | `CreateTaskResult` -- client polls `tasks/get` |
| No | Create task anyway, return task info as text | `CallToolResult` -- client calls `get_task_result` tool |

The SDK does not provide `get_task_result` as a built-in because it is a server policy decision (naming, access control, what fields to expose). Servers implement it as a regular tool.

## Task Status State Machine

```
                 +--------------+
                 |   Working    |
                 +------+-------+
                   +----+-----+------------+
                   |    |     |            |
                   v    |     v            v
          +------------+|  +--------+  +-----------+
          |InputRequired||  |Completed|  | Cancelled |
          +------+-----+|  +---------+  +-----------+
                 |      |                     ^
                 +------+     +-------+       |
                              | Failed |       |
                              +--------+       |
                   InputRequired --------------+
```

- **Working** -> InputRequired, Completed, Failed, Cancelled
- **InputRequired** -> Working, Completed, Failed, Cancelled
- **Completed, Failed, Cancelled** -> (terminal, no transitions)
- Self-transitions (Working -> Working) are rejected

```rust
use pmcp::types::tasks::TaskStatus;

assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Completed));
assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Working));
assert!(TaskStatus::Failed.is_terminal());
```

## TaskStore Trait

The `TaskStore` trait defines 7 methods for task lifecycle management:

| Method | Description |
|--------|-------------|
| `create(owner_id, ttl)` | Create a new task in `Working` state |
| `get(task_id, owner_id)` | Retrieve a task (owner-scoped) |
| `update_status(task_id, owner_id, status, message)` | Transition to a new status |
| `list(owner_id, cursor)` | List tasks with cursor pagination |
| `cancel(task_id, owner_id)` | Cancel a task |
| `cleanup_expired()` | Remove expired tasks |
| `config()` | Get store configuration |

All methods enforce **owner isolation**: accessing a task owned by someone else returns `NotFound` (never reveals existence).

### Implementing a Custom TaskStore

```rust
use pmcp::server::task_store::{TaskStore, TaskStoreError, StoreConfig};
use pmcp::types::tasks::{Task, TaskStatus};
use async_trait::async_trait;

struct MyDatabaseStore { /* ... */ }

#[async_trait]
impl TaskStore for MyDatabaseStore {
    async fn create(&self, owner_id: &str, ttl: Option<u64>) -> Result<Task, TaskStoreError> {
        // INSERT INTO tasks ...
        todo!()
    }

    async fn get(&self, task_id: &str, owner_id: &str) -> Result<Task, TaskStoreError> {
        // SELECT FROM tasks WHERE id = ? AND owner_id = ?
        todo!()
    }

    // ... implement remaining methods ...
    # async fn update_status(&self, _: &str, _: &str, _: TaskStatus, _: Option<String>) -> Result<Task, TaskStoreError> { todo!() }
    # async fn list(&self, _: &str, _: Option<&str>) -> Result<(Vec<Task>, Option<String>), TaskStoreError> { todo!() }
    # async fn cancel(&self, _: &str, _: &str) -> Result<Task, TaskStoreError> { todo!() }
    # async fn cleanup_expired(&self) -> Result<usize, TaskStoreError> { todo!() }
    # fn config(&self) -> &StoreConfig { todo!() }
}
```

`InMemoryTaskStore` is the only implementation in the SDK. Its tasks don't survive a restart and aren't shared between instances, so any deployment that runs more than one instance (every Lambda deployment, any container service that scales out) needs an implementation like the one above over a shared database.

The DynamoDB and Redis backends in the `pmcp-tasks` crate implement that crate's **own** `TaskStore` trait and plug in only through the legacy `with_task_store(TaskRouterImpl)` path. They do not implement this trait, and `task_store(...)` doesn't accept them.

## Configuration

`StoreConfig` controls task store behavior:

```rust
use pmcp::StoreConfig;
use pmcp::InMemoryTaskStore;

let store = InMemoryTaskStore::with_config(StoreConfig {
    default_ttl_ms: Some(300_000),      // 5 minutes (default: 1 hour)
    max_ttl_ms: Some(3_600_000),        // 1 hour max (default: 24 hours)
    default_poll_interval_ms: 3000,     // 3 seconds (default: 5 seconds)
    max_tasks_per_owner: 50,            // Per-owner limit (default: 100)
});
```

| Setting | Default | Description |
|---------|---------|-------------|
| `default_ttl_ms` | 3,600,000 (1h) | Applied when `create()` receives `None` for TTL |
| `max_ttl_ms` | 86,400,000 (24h) | Requested TTL values are clamped to this ceiling |
| `default_poll_interval_ms` | 5,000 (5s) | Suggested to clients in `Task.poll_interval` |
| `max_tasks_per_owner` | 100 | Active (non-expired) tasks per owner |

## Cleanup

`InMemoryTaskStore` does not run automatic background cleanup. Call `cleanup_expired()` periodically if you use TTL:

```rust
use std::sync::Arc;
use pmcp::InMemoryTaskStore;

let store = Arc::new(InMemoryTaskStore::new());
let cleanup_store = store.clone();

tokio::spawn(async move {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    loop {
        interval.tick().await;
        let _ = cleanup_store.cleanup_expired().await;
    }
});
```

Expired tasks are filtered from `get()`, `list()`, and the `max_tasks_per_owner` count, so cleanup primarily reclaims memory.

## Error Handling

`TaskStoreError` variants map to appropriate SDK error types:

| TaskStoreError | SDK Error | JSON-RPC | When |
|----------------|-----------|----------|------|
| `NotFound` | `Error::not_found` | -32602 | Task ID doesn't exist or wrong owner |
| `InvalidTransition` | `Error::validation` | -32602 | Invalid state machine transition |
| `Expired` | `Error::not_found` | -32602 | Task TTL elapsed (same as NotFound for privacy) |
| `Internal` | `Error::internal` | -32603 | Quota exceeded, storage failure |

## Owner Resolution

When a request arrives, the server resolves the task owner from the auth context:

| Request | Auth provider | Owner ID |
|---|---|---|
| Authenticated (v1 or v2) | any | `subject` (the `sub` claim) |
| Unauthenticated v1 (2025-11-25) | any | `"local"`, shared by every such caller |
| Unauthenticated v2 (2026-07-28) | configured | none: task creation is refused |
| Unauthenticated v2 (2026-07-28) | none | `""`, shared by every such caller |

The owner is never the OAuth `client_id`: that identifies the client application, so keying on it would put every user of the same application in one bucket.

With OAuth, User A therefore cannot see or cancel User B's tasks. Without it, every caller shares a bucket, so anyone holding a task id can read that task's status and result. Task ids are random v4 UUIDs, but nothing else protects them. Configure an auth provider before storing results that are not public. `InMemoryTaskStore` does not refuse the shared owners.

The legacy `TaskRouter` path resolves owners differently (subject, then client ID, then session ID), and `pmcp-tasks`' stores refuse the shared `"local"` owner unless `TaskSecurityConfig::with_allow_anonymous(true)` is set.

## Architecture

```
+--------------------------------------------------+
|                   pmcp (SDK)                      |
|                                                   |
|  types/tasks.rs     Task, TaskStatus (wire types) |
|  types/tools.rs     ToolExecution, TaskSupport    |
|  server/cancellation.rs  is_task_request()        |
|  server/core        Dispatch + capability neg.    |
|                     CreateTaskResult / CallToolResult
|                     routing based on request       |
+--------------------------------------------------+
          |
          |  (optional, for production)
          v
+--------------------------------------------------+
|              pmcp-tasks (extension)                |
|                                                   |
|  InMemoryTaskStore   TaskRouterImpl               |
|  DynamoDB backend    Redis backend                |
|  Task variables      Task result storage          |
|  Security config     Owner binding                |
+--------------------------------------------------+
```

The SDK provides everything needed for development and testing. `pmcp-tasks` serves the legacy `TaskRouter` path only; on the recommended `task_store(...)` path, production persistence means implementing `pmcp::server::task_store::TaskStore` over your own database.
