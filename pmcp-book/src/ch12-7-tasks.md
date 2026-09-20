# Chapter 12.7: MCP Tasks -- Long-Running Operations

> **Note on vocabulary**: **v1** means the MCP protocol revision `2025-11-25` and **v2** means `2026-07-28`. These are *protocol eras*, not crate versions — the crate is always written with its name attached ("pmcp 2.18"), so a bare "2.18" can never be mistaken for a protocol revision. One pmcp binary speaks both. Unless a passage says otherwise, this chapter describes the **v1** task surface; the v2 differences are collected in [Era delta (v1 vs v2)](#era-delta-v1-vs-v2) near the end, and the full v2 migration story lives in [Chapter 12.17: Migrating to MCP 2026-07-28 (v2)](ch12-17-migrating-to-mcp-2026-07-28.md).

When a tool takes five seconds, request/response is fine. When it takes five minutes -- deploying infrastructure, processing a large dataset, running a multi-step pipeline -- the caller needs more than silence followed by a result. MCP Tasks solve this with a stateless polling model that works everywhere from local development to serverless Lambda functions.

This chapter covers the design rationale, the protocol flow, and how to integrate tasks into your PMCP server.

> **Read this first.** PMCP ships a *typed, correct-by-construction* task path: you register a task-capable tool plus a `TaskStore`, and the SDK serves `tasks/get | result | list | cancel` for you from typed structs. **You never hand-write `tasks/*` wire JSON.** Start with the [Recommended Pattern](#recommended-pattern-tools-as-tasks) below. The hand-rolled `pmcp-tasks` / `TaskRouter` approach shown later in this chapter is the **legacy experimental path** — kept for reference, but not what you should reach for in new code.

---

## Recommended Pattern: Tools as Tasks

The clean way to expose a long-running tool as an async MCP **Task** is to declare task support on the tool and back it with a `TaskStore`. The SDK does the rest — it mints the task id, advertises the `tasks` capability, and serializes every `tasks/*` response from typed structs (`GetTaskResult`, `CreateTaskResult`, `CallToolResult`). Because both server and client speak the same typed shapes, an entire class of silent wire-shape bugs (mismatched ids, hollow capabilities, malformed `tasks/result` payloads) becomes impossible.

### Server

The task path lives on `ServerCoreBuilder` / `ServerCore`:

```rust,ignore
use std::sync::Arc;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::{TaskSupport, ToolExecution};
use serde_json::json;

let schema = json!({ "type": "object" });

let task_tool = TypedTool::new_with_schema("summarize", schema, |_args, _extra| {
    Box::pin(async {
        // ... do the work, return a CallToolResult-shaped value ...
        Ok(json!({ "content": [{ "type": "text", "text": "done" }] }))
    })
})
.with_description("Summarize asynchronously as an MCP Task")
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;

let server = ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool("summarize", task_tool)
    .task_store(store)   // presence of a store auto-advertises the `tasks` capability
    .build()?;
```

Four things the SDK guarantees here:

1. **You never hand-write `tasks/*` JSON.** The SDK serializes every response from typed structs, so server and client agree by construction.
2. **`task_store(...)` auto-advertises the `tasks` capability.** A `TaskSupport::Required` tool with **no** store (or router) makes `build()` return an `Err` — no hollow capability that advertises endpoints which cannot work.
3. **The store mints the task id.** The id returned to the client is the store-minted id, and it is exactly the id the client polls.
4. **`tasks/result` is served typed from the store** (with `TaskRouter` fall-through for the legacy path).

### Client

```rust,ignore
use pmcp::ToolCallResponse;

// 1) Call the tool as a task — get back the store-minted id.
let task_id = match client
    .call_tool_with_task("summarize".to_string(), serde_json::json!({}))
    .await?
{
    ToolCallResponse::Task(task) => task.task_id,
    ToolCallResponse::Result(_) => unreachable!("Required task tool always creates a task"),
};

// 2) Poll the typed `Task` until it reaches a terminal status.
let mut task = client.tasks_get(&task_id).await?;
while !task.status.is_terminal() {
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    task = client.tasks_get(&task_id).await?;
}

// 3) Fetch the typed terminal `CallToolResult`.
let result = client.tasks_result(&task_id).await?;
```

`client.tasks_list(cursor)` and `client.tasks_cancel(&task_id)` round out the typed client surface.

> **Scope caveat (current SDK).** Task dispatch and the `TaskStore` live on `ServerCoreBuilder` / `ServerCore`. The high-level `pmcp::Server` (and `StreamableHttpServer`) does **not** yet carry a `TaskStore` — wire the task path through `ServerCore` for now. For a complete, runnable end-to-end round-trip (initialize → call-with-task → poll → result), see the reference example [`examples/s45_tool_as_task_lifecycle.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s45_tool_as_task_lifecycle.rs).

The rest of this chapter explains *why* the polling model exists, the task state machine, and the legacy `pmcp-tasks` path you may still encounter in older code.

---

## Why Tasks

The MCP protocol's default flow is synchronous: client sends `tools/call`, server does work, server returns `CallToolResult`. This works until it does not. Long-running operations break the model in three ways:

1. **Connection timeouts.** HTTP gateways, load balancers, and Lambda runtimes impose response deadlines. A 30-second tool call that takes 90 seconds simply fails.

2. **No intermediate visibility.** The client has no way to know whether the operation is 10% done, stuck, or waiting for input. Progress notifications (Chapter 12) help for connected transports, but they require a persistent connection -- something serverless environments do not have.

3. **No recovery from disconnects.** If the client drops and reconnects, the in-flight result is lost. There is no way to say "what happened to my request?"

Tasks replace the single request/response exchange with a two-phase pattern:

- **Phase 1: Accept.** The server receives the request, creates a durable task record, and immediately returns a `CreateTaskResult` with a task ID and polling metadata.
- **Phase 2: Poll.** The client calls `tasks/get` at intervals until the task reaches a terminal status, then retrieves the result with `tasks/result`.

Because the task state lives in a store (not in connection state), this model is compatible with stateless deployments. A Lambda function can create the task on one invocation and serve the result on a completely different invocation minutes later.

```
Phase 1: Accept                          Phase 2: Poll

Client                Server              Client                Server
  |                      |                  |                      |
  |-- tools/call ------->|                  |-- tasks/get -------->|
  |   { task: {} }       |                  |   { taskId }         |
  |                      |                  |                      |
  |   (server creates    |                  |<-- { status:         |
  |    task record,      |                  |      "working" } ----|
  |    starts background |                  |                      |
  |    work)             |                  |   (wait pollInterval)|
  |                      |                  |                      |
  |<- CreateTaskResult --|                  |-- tasks/get -------->|
  |   { task: {          |                  |                      |
  |     taskId,          |                  |<-- { status:         |
  |     status: working, |                  |      "completed" } --|
  |     pollInterval     |                  |                      |
  |   }}                 |                  |-- tasks/result ----->|
  |                      |                  |                      |
                                            |<-- CallToolResult --|
```

This is the same model used by cloud APIs (AWS Step Functions, Azure Durable Functions) and is specifically designed for environments where SSE or WebSocket connections are not available or not reliable.

---

## The Polling Model

The full lifecycle of a task-augmented tool call looks like this:

1. The client calls `tools/call` with a `task` field in the request params:
   ```json
   {
     "method": "tools/call",
     "params": {
       "name": "deploy_service",
       "arguments": { "region": "us-east-1" },
       "task": {}
     }
   }
   ```

2. The server sees `params.task` and knows this is a task-augmented request. It creates a task record, starts background processing, and returns immediately:
   ```json
   {
     "result": {
       "task": {
         "taskId": "786512e2-9e0d-44bd-8f29-789f320fe840",
         "status": "working",
         "statusMessage": "Deployment started",
         "createdAt": "2025-11-25T10:30:00Z",
         "lastUpdatedAt": "2025-11-25T10:30:00Z",
         "ttl": 60000,
         "pollInterval": 5000
       }
     }
   }
   ```

3. The client polls `tasks/get` using the task ID, respecting the suggested `pollInterval`:
   ```json
   { "method": "tasks/get", "params": { "taskId": "786512e2-..." } }
   ```

4. Once the task reaches a terminal status (`completed`, `failed`, or `cancelled`), the client calls `tasks/result` to retrieve the actual operation result:
   ```json
   { "method": "tasks/result", "params": { "taskId": "786512e2-..." } }
   ```

5. The response to `tasks/result` is the same `CallToolResult` the client would have received from a synchronous call, plus `_meta` linking it back to the task:
   ```json
   {
     "result": {
       "content": [{ "type": "text", "text": "Deployed to us-east-1" }],
       "_meta": {
         "io.modelcontextprotocol/related-task": {
           "taskId": "786512e2-..."
         }
       }
     }
   }
   ```

The client can also list all its tasks with `tasks/list` and cancel an in-progress task with `tasks/cancel`.

---

## Legacy: Setting Up TaskStore via `pmcp-tasks`

> **Legacy / experimental path.** The sections from here through "Writing Dual-Path Tool Handlers" describe the older `pmcp-tasks` + `TaskRouter` approach, where you hand-construct the store, wire it with `with_task_store(...)`, and emit `task` JSON by hand inside the handler. New code should prefer the [Recommended Pattern](#recommended-pattern-tools-as-tasks) above (`task_store(...)` + `with_task_support`), which removes the hand-rolled JSON entirely. The material below is retained because you may encounter it in existing servers and because the production backends (DynamoDB, Redis) documented here are still the way to persist tasks.

Tasks need persistent storage. The `pmcp-tasks` crate provides the `TaskStore` trait and two ready-made backends:

| Backend | Crate Feature | Use Case |
|---------|--------------|----------|
| `InMemoryTaskStore` | (default) | Development, tests, single-process servers |
| `DynamoDbBackend` | `dynamodb` | AWS Lambda, serverless production |
| `RedisBackend` | `redis` | Long-running server deployments |

### In-Memory (Development)

```rust
use pmcp_tasks::store::memory::InMemoryTaskStore;
use pmcp_tasks::store::{StoreConfig, TaskStore};
use pmcp_tasks::security::TaskSecurityConfig;
use std::sync::Arc;

let store: Arc<dyn TaskStore> = Arc::new(
    InMemoryTaskStore::new()
        .with_config(StoreConfig::default())
        .with_security(
            TaskSecurityConfig::default().with_allow_anonymous(true)
        )
        .with_poll_interval(3000), // suggest 3s polling
);
```

`InMemoryTaskStore` uses `DashMap` for concurrent access and is perfectly fine for development and testing. Tasks disappear when the process exits -- that is intentional for dev.

### DynamoDB (Production / Serverless)

For Lambda deployments where each invocation is a separate process, you need external storage:

```toml
[dependencies]
pmcp-tasks = { version = "0.1", features = ["dynamodb"] }
```

```rust
use pmcp_tasks::store::dynamodb::DynamoDbBackend;
use pmcp_tasks::store::generic::GenericTaskStore;
use pmcp_tasks::security::TaskSecurityConfig;
use std::sync::Arc;

let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
let dynamo_client = aws_sdk_dynamodb::Client::new(&aws_config);

let backend = DynamoDbBackend::new(dynamo_client, "mcp-tasks".to_string());
let store = Arc::new(
    GenericTaskStore::new(backend)
        .with_security(TaskSecurityConfig::default()),
);
```

All domain logic -- state machine validation, owner isolation, variable merge, TTL enforcement -- lives in `GenericTaskStore`. The backend is a dumb key-value store. This means switching from in-memory to DynamoDB requires zero changes to your tool handlers.

### Wiring the Store to the Server (legacy `with_task_store`)

> The `with_task_store(Arc<dyn TaskRouter>)` builder method shown here is the **legacy experimental** wiring. It advertises under `experimental.tasks` and expects you to emit `task` JSON by hand. The recommended path is `task_store(Arc<dyn TaskStore>)` (see the [Recommended Pattern](#recommended-pattern-tools-as-tasks)), which advertises the standard top-level `tasks` capability and serves every `tasks/*` response typed.

The `pmcp-tasks` crate ships a `TaskRouterImpl` that bridges any `TaskStore` to the `TaskRouter` trait the SDK consumes. Wrap the store in `TaskRouterImpl`, then register it with `ServerCoreBuilder::with_task_store(...)`:

```rust
use pmcp::server::builder::ServerCoreBuilder;
use pmcp_tasks::TaskRouterImpl;
use std::sync::Arc;

let router = Arc::new(TaskRouterImpl::new(store));

let server = ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .with_task_store(router)  // enables tasks/get, tasks/list, tasks/cancel, tasks/result
    .tool("deploy_service", DeployTool { /* ... */ })
    .build()?;
```

`with_task_store(...)` automatically advertises `experimental.tasks` on `ServerCapabilities` so clients know the server supports the tasks protocol. You do not need to set capabilities manually.

---

## Declaring Task Support on Tools

Each tool declares whether it supports task augmentation via the `execution` field on `ToolInfo`. There are three levels:

| Level | Meaning | Client Behavior |
|-------|---------|----------------|
| `Forbidden` (default) | Tool does not support tasks | Client must not include `task` in request |
| `Optional` | Tool supports both sync and async paths | Client may include `task` or omit it |
| `Required` | Tool only works as a task | Client must include `task` in request |

### Setting Task Support

```rust
use pmcp_tasks::{ToolExecution, TaskSupport};
use pmcp::types::protocol::ToolInfo;
use serde_json::json;

let tool = ToolInfo::new(
    "deploy_service",
    Some("Deploy a service to the specified region".to_string()),
    json!({
        "type": "object",
        "properties": {
            "region": { "type": "string" }
        },
        "required": ["region"]
    }),
)
.with_execution(ToolExecution {
    task_support: TaskSupport::Optional,
});
```

This produces an `execution` field in the `tools/list` response:

```json
{
  "name": "deploy_service",
  "description": "Deploy a service to the specified region",
  "inputSchema": { ... },
  "execution": {
    "taskSupport": "optional"
  }
}
```

**Guidance on choosing a level:**

- Use `Forbidden` (default) for fast tools that always complete within a few seconds.
- Use `Optional` for tools that might be fast or slow depending on input. The handler checks at runtime whether the client requested a task.
- Use `Required` for tools that are inherently long-running and always need background execution (large data processing, multi-step deployments).

---

## Writing Dual-Path Tool Handlers

When a tool has `TaskSupport::Optional`, the handler must support both paths: synchronous (return result directly) and asynchronous (create task, return immediately). The `RequestHandlerExtra` tells you which path the client wants.

```rust
use async_trait::async_trait;
use pmcp::error::Result;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::ToolHandler;
use serde_json::{json, Value};
use std::sync::Arc;
use pmcp_tasks::store::TaskStore;
use pmcp_tasks::context::TaskContext;
use pmcp_tasks::security::resolve_owner_id;

struct DeployTool {
    task_store: Arc<dyn TaskStore>,
}

#[async_trait]
impl ToolHandler for DeployTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        let region = args["region"].as_str().unwrap_or("us-east-1");

        if extra.is_task_request() {
            // --- Async path: create task, return immediately ---
            // Resolve owner from the auth context (OAuth subject -> client ID ->
            // session ID -> DEFAULT_LOCAL_OWNER). When anonymous access is
            // disabled on the store, an unauthenticated request is rejected
            // before reaching the handler.
            let auth = extra.auth_context();
            let subject = auth.map(|a| a.subject.as_str());
            let client_id = auth.and_then(|a| a.client_id.as_deref());
            let session_id = extra.session_id.as_deref();
            let owner_id = resolve_owner_id(subject, client_id, session_id);

            let record = self.task_store
                .create(&owner_id, "tools/call", Some(60_000))
                .await
                .map_err(|e| pmcp::Error::internal(e.to_string()))?;

            let task_id = record.task.task_id.clone();
            let store = self.task_store.clone();
            let region = region.to_string();
            let owner_for_ctx = owner_id.clone();

            // Spawn the actual work in the background
            tokio::spawn(async move {
                let ctx = TaskContext::new(store, task_id, owner_for_ctx);
                match do_deploy(&region).await {
                    Ok(result) => {
                        let _ = ctx.complete(result).await;
                    }
                    Err(e) => {
                        let _ = ctx.fail(e.to_string()).await;
                    }
                }
            });

            // Return CreateTaskResult immediately
            Ok(json!({
                "task": {
                    "taskId": record.task.task_id,
                    "status": "working",
                    "statusMessage": "Deployment started",
                    "createdAt": record.task.created_at,
                    "lastUpdatedAt": record.task.last_updated_at,
                    "ttl": 60000,
                    "pollInterval": 5000
                }
            }))
        } else {
            // --- Sync path: do the work and return the result ---
            let result = do_deploy(region).await
                .map_err(|e| pmcp::Error::internal(e.to_string()))?;
            Ok(result)
        }
    }
}

async fn do_deploy(region: &str) -> std::result::Result<Value, Box<dyn std::error::Error + Send>> {
    // Simulate deployment work
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Successfully deployed to {}", region)
        }]
    }))
}
```

The key pattern: `extra.is_task_request()` returns `true` when the client included `task` in the request params. The same tool handler serves both task-aware and non-task-aware clients without any branching at the protocol level.

---

## Capability Negotiation

The task field in the request is the per-request capability signal. This is a deliberate design choice.

Consider the alternative: the server could store "this client supports tasks" during initialization and check a session flag on every request. But that requires session state, which breaks in serverless environments where each request might hit a different Lambda instance with no shared memory.

Instead, the protocol uses a stateless signal:

- **Client wants a task:** includes `"task": {}` in the request params.
- **Client wants a synchronous result:** omits the `task` field.

The server adapts its response format -- `CreateTaskResult` vs `CallToolResult` -- based solely on what is in the current request. No session lookup, no stored preferences.

> An MCP server should not ask "who is the client?" It should ask "what capabilities has the client declared?"

This principle applies beyond tasks. It is the same pattern used for progress tokens (`_meta.progressToken`), sampling, and elicitation. Each request carries its own capability declarations, making every request self-describing and every server interaction stateless.

### Server Capability Advertisement

The server side of negotiation happens during initialization. With the recommended `task_store(...)` path, PMCP advertises task support under the **standard top-level `tasks` capability** (`ServerCapabilities.tasks`) as soon as a store backs the endpoints — you do not set it manually:

```json
{
  "capabilities": {
    "tasks": {
      "list": {},
      "cancel": {},
      "requests": {
        "tools": {
          "call": {}
        }
      }
    }
  }
}
```

This tells clients: "I support tasks for `tools/call`, and I support `tasks/list` and `tasks/cancel`." Clients that understand tasks will include the `task` field when calling tools that advertise `TaskSupport::Optional` or `TaskSupport::Required`.

The advertisement follows the backend, not tool metadata alone: it appears only when a store (or, on the legacy path, a router) is present. A tool declaring `TaskSupport::Required` with **no** backend is a build-time error rather than a hollow capability — `build()` returns an `Err`. (The legacy `with_task_store(router)` path instead advertises under `experimental.tasks`.)

---

## Client Compatibility

Not all clients understand tasks. Claude Desktop does. ChatGPT (as of this writing) does not. Your server should handle both gracefully.

### Task-Aware Clients

Task-aware clients follow the polling protocol directly:

1. See `execution.taskSupport` on a tool in `tools/list`.
2. Include `"task": {}` in `tools/call` for that tool.
3. Receive `CreateTaskResult`, extract `taskId` and `pollInterval`.
4. Poll `tasks/get` until the status is terminal.
5. Call `tasks/result` to get the final `CallToolResult`.

### Non-Task-Aware Clients (Fallback)

When a client does not include `task` in the request, the handler takes the synchronous path. For tools with `TaskSupport::Optional`, this means the client gets a regular `CallToolResult` after the full operation completes. If the operation is long, the client simply waits (or times out).

For a better experience with non-task-aware clients, you can provide a separate polling tool as a fallback:

```rust
use pmcp::server::SyncTool;
use serde_json::json;

let get_task = SyncTool::new("get_task_result", |args| {
    let task_id = args["task_id"].as_str()
        .ok_or_else(|| pmcp::Error::validation("task_id required"))?;

    // Look up the task in the store and return its status/result
    // (In practice, this would be async and use the TaskStore)
    Ok(json!({
        "task_id": task_id,
        "status": "completed",
        "result": "Deployment finished successfully"
    }))
})
.with_description(
    "Check the status of a long-running task. \
     Use this when a tool returns a task_id instead of a direct result."
);
```

With this approach, non-task-aware clients like ChatGPT receive a `CallToolResult` containing the task ID as text, and the LLM can decide to call `get_task_result` to poll for completion. The LLM acts as the polling loop.

---

## Task Status State Machine

Tasks follow a strict state machine. The `TaskStatus` enum has five variants, three of which are terminal (no further transitions allowed):

```
                     +──────────────+
                     │   Working    │ <── initial state
                     +──────┬───────+
                            │
               +────────────┼────────────+
               v            │            v
      +────────────────+    │    +───────────────+
      │ InputRequired  │────+    │   (terminal)  │
      +────────────────+         │  +-----------+ │
               │                 │  | Completed | │
               +────────────────>│  | Failed    | │
                                 │  | Cancelled | │
                                 │  +-----------+ │
                                 +───────────────+
```

### Status Descriptions

| Status | Meaning | Next States |
|--------|---------|-------------|
| `Working` | The operation is actively being processed | `InputRequired`, `Completed`, `Failed`, `Cancelled` |
| `InputRequired` | The server needs additional input from the client before it can proceed | `Working`, `Completed`, `Failed`, `Cancelled` |
| `Completed` | The operation finished successfully (terminal) | None |
| `Failed` | The operation did not complete successfully (terminal) | None |
| `Cancelled` | The operation was cancelled before completion (terminal) | None |

### Transition Rules

- Self-transitions are rejected (`Working` -> `Working` is invalid).
- Terminal states reject all transitions.
- `InputRequired` can return to `Working` once the client provides the needed input.

The `TaskStatus` type enforces these rules:

```rust
use pmcp_tasks::TaskStatus;

let status = TaskStatus::Working;
assert!(status.can_transition_to(&TaskStatus::Completed));   // valid
assert!(!status.can_transition_to(&TaskStatus::Working));     // self-transition rejected

let terminal = TaskStatus::Completed;
assert!(!terminal.can_transition_to(&TaskStatus::Working));   // terminal, cannot transition
assert!(terminal.is_terminal());
```

The store layer also enforces transitions atomically -- if two concurrent requests try to transition the same task, one will succeed and the other will receive an `InvalidTransition` error.

### Using TaskContext for Transitions

The `TaskContext` wrapper provides convenience methods for each transition:

```rust
use pmcp_tasks::context::TaskContext;
use serde_json::json;

// In a background task handler:
async fn process_with_context(ctx: &TaskContext) -> Result<(), Box<dyn std::error::Error>> {
    // Do some work...
    ctx.set_variable("progress", json!(50)).await?;

    // Need user input?
    ctx.require_input("Please confirm the deployment target").await?;

    // After receiving input, resume:
    ctx.resume().await?;

    // Finish successfully:
    ctx.complete(json!({"deployed": true})).await?;

    // Or if something goes wrong:
    // ctx.fail("Connection to deployment target lost").await?;

    // Or if the client cancels:
    // ctx.cancel().await?;

    Ok(())
}
```

---

## Durable and replay consumers

The [polling model](#the-polling-model) above assumes an ordinary client that
sleeps between `tasks/get` calls. A **durable / replay-shaped** consumer — a
Temporal-style workflow, or the pmcp.run durable poller that motivated this
section — cannot sleep or loop freely: on replay, the runtime re-executes the
workflow function from the top and must observe the *same* decisions it made the
first time. Blocking sleeps and hidden loops are non-deterministic under replay,
so these consumers need the poll *decision* as a small, pure primitive they can
drive one step at a time.

That primitive is `Task::poll_decision()`, paired with `resolve_poll_interval()`.
For a runnable, compiling version of the loop below, see
`examples/s48_durable_poll_decision.rs`
(`cargo run --example s48_durable_poll_decision --features full`).

### The typed-accessors-without-the-loop pattern

A durable runtime exposes two building blocks: `ctx.step(...)`, which runs an
effect **once** and memoizes its result into the workflow history (so replays
return the recorded value instead of re-running the effect), and `ctx.wait(...)`,
which suspends the workflow deterministically instead of sleeping a thread. You
compose the classifier out of those instead of calling a blocking
poll-to-terminal helper:

```rust,ignore
// Illustrative only — `ctx` is your durable runtime's step/wait context.
loop {
    // ONE memoized step: the network tasks/get AND its serde decode happen
    // inside ctx.step, so the workflow history records the decoded Task.
    let task = ctx.step("tasks/get", || client.tasks_get(&task_id)).await?;

    // Pure, per-poll, no I/O — safe to run on every replay.
    match task.poll_decision() {
        TaskPollDecision::Terminal { status } => {
            // Terminal carries ONLY the status. Fetch the payload separately
            // (see below) — as its own memoized step.
            break status;
        }
        TaskPollDecision::InputRequired => {
            // NOT terminal: route to elicitation, then resume polling. Never
            // fetch a result here.
            handle_input_required(&task_id).await?;
        }
        TaskPollDecision::InProgress { poll_hint } => {
            // Deterministic suspend instead of a thread sleep.
            let interval = resolve_poll_interval(None, poll_hint);
            ctx.wait(Duration::from_millis(interval)).await;
        }
        // TaskPollDecision is #[non_exhaustive] — carry a wildcard (see semver
        // note below) and treat an unrecognized decision as "keep polling".
        _ => ctx.wait(Duration::from_millis(resolve_poll_interval(None, None))).await,
    }
}
```

### Replay determinism (scoped precisely)

`poll_decision()` is replay-deterministic **only** as a pure function over an
**already-deserialized** `Task`. The determinism guarantee stops at the struct
boundary: the network `tasks/get` call and the serde decode of its response are
**not** pure, so they must sit **inside** the runtime's memoized step (the
`ctx.step("tasks/get", ...)` above). Once the `Task` is decoded and recorded in
history, classifying it is a pure `match` that yields the same
`TaskPollDecision` on every replay.

A corollary: an unknown or future task status fails at **deserialization**, before
classification ever runs. `poll_decision()` never sees an unrecognized status —
by the time it is called, the `Task` has already been decoded into the known
`TaskStatus` variants (or the decode step has already errored inside `ctx.step`).

### Semver: two distinct claims

These are separate facts; do not conflate them:

- **`TaskStatus` is exhaustive today.** The five variants (`Working`,
  `InputRequired`, `Completed`, `Failed`, `Cancelled`) are the complete set, and
  `poll_decision()` matches all of them exhaustively.
- **`TaskPollDecision` is `#[non_exhaustive]`.** This is a *future-proofing
  affordance* — it lets the SDK add a decision variant in a future release
  without a breaking change, which is why external `match`es must carry a
  wildcard arm. It is **not** present runtime graceful handling of unknown
  statuses (there are none to handle — see the determinism note above). The
  wildcard exists for source-compatibility across SDK versions, nothing more.

### Terminal → a separate `tasks/result` step

The `Terminal { status }` decision carries only the status. To retrieve the
final `CallToolResult`, the consumer issues a **separate** `tasks/result` call —
itself a memoized durable step:

```rust,ignore
let result = ctx.step("tasks/result", || client.tasks_result(&task_id)).await?;
```

A durable consumer **must not** fetch a result on the `input_required` path: an
`input_required` task is not terminal and has no result yet. The `s48` example
guards the `tasks/result` fetch behind a terminal flag so that path is
unreachable — mirror that guard in your own workflow.

### When NOT to use the blocking waiter

> ⚠️ **Do not wrap the blocking poll-to-terminal helper inside a replay/durable
> workflow.** The `Client::wait_for_task` convenience owns the entire polling
> lifecycle — it sleeps, loops, and blocks until the task is terminal. That is
> exactly the non-deterministic behavior a replay runtime cannot tolerate: the
> sleep durations and loop iterations are not recorded in history, so a replay
> would diverge. It is the right tool for an ordinary synchronous client, and the
> wrong tool inside `ctx.step`/`ctx.wait`. Durable consumers should drive the
> per-poll classifier shown above instead, exactly as `s48` demonstrates.

---

## Configuration

### StoreConfig

`StoreConfig` controls storage limits and TTL behavior. These defaults are designed for production safety:

| Setting | Default | Description |
|---------|---------|-------------|
| `max_variable_size_bytes` | 1,048,576 (1 MB) | Maximum size of the serialized variable payload per task |
| `default_ttl_ms` | 3,600,000 (1 hour) | Applied when a task is created without an explicit TTL |
| `max_ttl_ms` | 86,400,000 (24 hours) | Upper bound on TTL; requests for longer are clamped |
| `max_variable_depth` | 10 | Maximum JSON nesting depth (prevents depth-bomb attacks) |
| `max_string_length` | 65,536 (64 KB) | Maximum length of any single string value in variables |

```rust
use pmcp_tasks::store::StoreConfig;

let config = StoreConfig {
    max_variable_size_bytes: 512_000,     // 500 KB
    default_ttl_ms: Some(1_800_000),      // 30 minutes
    max_ttl_ms: Some(7_200_000),          // 2 hours
    max_variable_depth: 5,
    max_string_length: 32_768,            // 32 KB
};

let store = InMemoryTaskStore::new()
    .with_config(config);
```

### Poll Interval

The `pollInterval` field in task responses tells clients how frequently to call `tasks/get`. Set it based on your expected task duration:

| Task Duration | Suggested Poll Interval |
|--------------|------------------------|
| < 10 seconds | 1,000 ms (1 second) |
| 10-60 seconds | 3,000 ms (3 seconds) |
| 1-10 minutes | 5,000 ms (5 seconds) |
| > 10 minutes | 10,000-30,000 ms |

```rust
let store = InMemoryTaskStore::new()
    .with_poll_interval(5000);  // 5 seconds
```

### Security Configuration

Owner isolation ensures that one client cannot see or modify another client's tasks. The store resolves owner identity from the auth context (OAuth subject, client ID, or session ID):

```rust
use pmcp_tasks::security::TaskSecurityConfig;

// Production: require authenticated owners
let security = TaskSecurityConfig::default();

// Development: allow anonymous access
let security = TaskSecurityConfig::default()
    .with_allow_anonymous(true);
```

When `allow_anonymous` is `false` (the default), every task operation requires a valid owner ID derived from the request's auth context. This prevents a public client from reading tasks created by an authenticated user.

---

## Era delta (v1 vs v2)

Everything above describes tasks as MCP **v1** (`2025-11-25`) defines them. MCP **v2** (`2026-07-28`) moved Tasks out of the core specification and onto the Extensions Track, and that move is observable on the wire in four places: where the capability is negotiated, what triggers task creation, which `tasks/*` methods a server answers, and what a create response looks like.

> **These wire values are provisional.** The Tasks extension is still published only as a `draft/` schema upstream (`modelcontextprotocol/ext-tasks`) — no versioned schema directory, no tagged release. Every v2 Tasks value pmcp emits is read from that draft, so it may change before the extension is final. The SDK says the same thing to Rust readers at the definition site: see the `This value is PRE-FINAL` note on `TASKS_EXTENSION_KEY` in `src/types/capabilities.rs`. Read this section as a description of what pmcp does today, not as a stable contract — do not pin production behaviour to it yet.

### Where task support is negotiated

On v1, task support is a field of the capability object, exactly as [Server Capability Advertisement](#server-capability-advertisement) above describes. On v2 it is an entry in the **extensions map**, keyed by the reverse-DNS identifier `io.modelcontextprotocol/tasks`, whose value is always the empty object.

| Era | Server advertises under | Client declares under |
|---|---|---|
| v1 (`2025-11-25`) | `capabilities.tasks` (legacy router path: `experimental.tasks`) | the per-request `task` field on `tools/call` |
| v2 (`2026-07-28`) | `capabilities.extensions["io.modelcontextprotocol/tasks"]` | `params._meta["io.modelcontextprotocol/clientCapabilities"].extensions["io.modelcontextprotocol/tasks"]` |

Both spellings and both negotiation directions are documented on `TASKS_EXTENSION_KEY` in `src/types/capabilities.rs`. Tasks moved to `extensions` precisely *because* they left the core specification — the v1 `capabilities.tasks` field has no v2 counterpart.

### The create trigger changes, and each era ignores the other's signal

This is the part most likely to surprise a reader arriving from [Capability Negotiation](#capability-negotiation) above. On v1 the per-request `task` field is the signal that a call should become a task. **On v2 that field is not consulted at all** — it does not exist in the v2 extension — and the client's own extension declaration on that request is what opens the create gate instead. Symmetrically, a v1-era request that carries an extension declaration is unaffected by it.

| Negotiated era | What triggers task creation | The other era's signal |
|---|---|---|
| v1 / no era code | `CallToolRequest.task` is present | a declaration is ignored |
| v2 | the client declared `io.modelcontextprotocol/tasks` on this request | the `task` field is ignored |

The gate is a conformance requirement rather than a pmcp preference: a v2 server must not hand a task handle to a client that never declared it could read one. See `create_gate` and `maybe_build_task_created` in `src/server/task_dispatch.rs`. The stateless, per-request character of the negotiation is unchanged — only the field that carries it moves.

### Which `tasks/*` methods each era serves

| Method | v1 (`2025-11-25`) | v2 (`2026-07-28`) |
|---|---|---|
| `tasks/get` | served | served |
| `tasks/cancel` | served | served |
| `tasks/result` | served | **retired** — answers `-32601` |
| `tasks/list` | served | **retired** — answers `-32601` |
| `tasks/update` | — | served |

The per-era sets are documented on `Tool::with_task_support` in `src/types/tools.rs`; `tasks/update` is defined in `src/types/mrtr.rs`.

**`tasks/list` was retired for a security reason, not as a simplification.** The SDK states it at the type itself, on `ListTasksResult` in `src/types/tasks.rs`: the removal is a security improvement, because *with no enumeration primitive a server cannot leak the existence of one caller's tasks to another*. On v1, a server whose owner scoping is wrong exposes an enumeration surface; on v2 there is no method that returns a list, so that surface does not exist to get wrong.

`tasks/result` is retired alongside it. On v2 a terminal task's outcome is read inline from the same `tasks/get` payload that reports its status, so the separate result call has nothing left to do. `tasks/update` is the new method in the set: it is how a client delivers answers back to a task that paused to ask for input.

### The create response shape

The v1 create response is the nested envelope this chapter shows throughout — `CreateTaskResult`, i.e. `{"task": { … }}` — and it is frozen in that shape. The v2 create response is **flat**: the task's own fields sit at the top level of `result`, carrying the envelope discriminator `resultType: "task"`. Both eras attach the related-task pointer under `_meta`. See `v1_create_result_value` and `v2_create_result_value` in `src/server/task_dispatch.rs`.

### Runnable v2 examples

Two examples exercise the v2 surface end to end. Start the server, then run the agent against it in a second terminal:

```bash
cargo run --example s50_v2_tasks_server --features full
cargo run --example s51_v2_tasks_agent --features full
```

`s51_v2_tasks_agent` is an executable assertion rather than a printout: it exits non-zero if any step diverges from what is documented here. Between them the pair demonstrates explicit `server/discover` negotiation, the paused-task round trip through `tasks/update`, an undeclaring client correctly receiving an ordinary `CallToolResult`, and both retirements refused from both sides.

---

## Summary

MCP Tasks extend the request/response model with durable, pollable operations that survive disconnects and work in stateless deployments.

**Key concepts:**

- **Recommended pattern:** declare `with_task_support` on the tool and register a `TaskStore` with `task_store(...)`. The SDK mints the id, auto-advertises the `tasks` capability, and serves every `tasks/*` response typed — you never hand-write `tasks/*` wire JSON. See [`examples/s45_tool_as_task_lifecycle.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s45_tool_as_task_lifecycle.rs).
- **Two-phase flow:** `tools/call` returns a task, `tasks/get` polls status, `tasks/result` retrieves the outcome.
- **Stateless negotiation:** The `task` field in each request is the capability signal. No session state required.
- **Dual-path handlers:** `extra.is_task_request()` lets the same tool handler serve both task-aware and non-task-aware clients.
- **Strict state machine:** Five statuses with validated transitions. Terminal states are final.
- **Storage backends:** `InMemoryTaskStore` for dev, DynamoDB or Redis for production. Swap backends without changing handler code.

**Related chapters:**

- Chapter 12 covers progress reporting and cancellation for connected transports.
- Chapter 10.3 covers Streamable HTTP, which pairs well with tasks for serverless deployments.
- Chapter 16 covers deployment strategies, including Lambda configurations where tasks are essential.

**Crate reference:** [`pmcp-tasks` on crates.io](https://crates.io/crates/pmcp-tasks)
