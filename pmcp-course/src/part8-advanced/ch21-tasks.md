# MCP Tasks: Long-Running Operations

Some operations take too long to fit inside a single request-response cycle. A satellite imagery analysis tool might need 45 seconds. A compliance report generator might need two minutes. An ETL pipeline might need ten. When the work outlasts the transport timeout, you need a different pattern.

MCP Tasks solve this with a two-phase model: the server accepts the request immediately (Phase 1), then the client polls for the result (Phase 2). This chapter teaches you how to build servers that support both synchronous and task-based execution, letting the client decide which path to take.

## Learning Objectives

By the end of this chapter, you will be able to:

- Explain why polling is preferred over SSE for serverless deployments
- Use the recommended pattern: register a `with_task_support` tool + a `TaskStore` on `ServerCoreBuilder` and let the SDK serve `tasks/*` typed -- so you never hand-write `tasks/*` wire JSON
- Drive the typed client round-trip: `call_tool_with_task` → poll `tasks_get` → `tasks_result`
- Declare `TaskSupport::Optional` on tools using `.with_execution()`
- Write a dual-path tool handler that branches on `extra.is_task_request()` (the advanced manual alternative)
- Build a `get_task_result` fallback tool for clients that do not support tasks natively
- Describe how capability negotiation works at the per-request level, and why the server's `tasks` capability follows its backend

## Why Tasks Matter for Enterprise MCP

Enterprise MCP deployments face a fundamental tension: many valuable operations are inherently slow, but the MCP protocol's default behavior assumes synchronous request-response.

```
+---------------------------------------------------------------------+
|                 The Timeout Problem                                   |
+---------------------------------------------------------------------+
|                                                                       |
|  Operation                        Typical Duration    Sync Feasible?  |
|  ================================ =================== ==============  |
|  Database query                   50ms - 2s           Yes             |
|  API call with retry              1s - 10s            Maybe           |
|  Report generation (PDF)          10s - 60s           Unlikely        |
|  Satellite imagery analysis       30s - 120s          No              |
|  ETL pipeline stage               1min - 30min        No              |
|  ML model training checkpoint     5min - hours        No              |
|                                                                       |
|  Lambda timeout: 15 minutes max                                       |
|  Streamable HTTP timeout: varies by proxy/CDN                         |
|  Client patience: varies by UX expectations                           |
|                                                                       |
+---------------------------------------------------------------------+
```

Without tasks, a slow tool call either times out (bad UX, wasted compute) or forces the server to use WebSockets with server-sent events (incompatible with serverless). Tasks give you a third option: accept immediately, work in the background, let the client poll.

## The Polling Model vs SSE Notifications

MCP defines two mechanisms for tracking long-running work:

| Mechanism | Transport | Serverless Compatible | Client Complexity |
|-----------|-----------|----------------------|-------------------|
| **Polling** (`tasks/get`) | Any (HTTP, stdio, WS) | Yes | Low -- just retry on interval |
| **SSE notifications** | Streamable HTTP only | No -- requires persistent connection | Higher -- must hold open connection |

Polling wins for serverless because each poll is a stateless HTTP request. The server does not need to hold a connection open. It does not need to remember which client is listening. It reads task state from the store, returns it, and shuts down.

```
Polling model (serverless-friendly):

Client                    Lambda                    DynamoDB
  │                         │                          │
  │── tools/call ──────────>│                          │
  │                         │── create task ──────────>│
  │<── CreateTaskResult ────│                          │
  │                         │  (Lambda exits)          │
  │                         │                          │
  │── tasks/get ───────────>│  (new Lambda)            │
  │                         │── read task ────────────>│
  │<── {status: working} ───│                          │
  │                         │  (Lambda exits)          │
  │                         │                          │
  │── tasks/get ───────────>│  (new Lambda)            │
  │                         │── read task ────────────>│
  │<── {status: completed} ─│                          │
  │                         │                          │
  │── tasks/result ────────>│  (new Lambda)            │
  │                         │── read result ──────────>│
  │<── CallToolResult ──────│                          │
```

Each arrow to Lambda is a fresh invocation, possibly on a different instance. The `TaskStore`, backed by DynamoDB, Redis or another external store, provides the continuity that the stateless compute layer cannot. That only works if the store is shared: an in-memory store on one instance is invisible to the next.

## TaskStore: The External State Bridge

The `TaskStore` trait is the abstraction that makes tasks work across stateless invocations. It manages the lifecycle of task records -- creation, status transitions, pagination, expiration, and cleanup.

```rust
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore, StoreConfig};
use std::sync::Arc;

// For development and testing: in-memory store
let store = Arc::new(InMemoryTaskStore::new());

// For production: your own `impl TaskStore` over a shared database
// let store = Arc::new(MyDynamoDbTaskStore::new(client, table_name));
```

The SDK provides `InMemoryTaskStore` for development. It is the only `TaskStore` implementation in the SDK, and its tasks don't survive a restart or reach other instances. For Lambda or any deployment that scales out, implement `pmcp::server::task_store::TaskStore` over a shared database. The required methods are `create`, `get`, `update_status`, `list`, `cancel`, `cleanup_expired` and `config`, and every lookup is scoped to the owner id.

> **What about `pmcp-tasks`?** Its DynamoDB and Redis backends implement that crate's own `TaskStore` trait. They plug into the legacy `with_task_store(TaskRouterImpl)` path, not into `task_store(...)`.

> **Recommended pattern (read the next section with this in mind).** The clean way to expose a tool as a Task is to declare `with_task_support` on the tool and register the store with `task_store(...)` on `ServerCoreBuilder`. The SDK then mints the task id, auto-advertises the `tasks` capability, and serves `tasks/get | result | list | cancel` typed from the store -- **you never hand-write `tasks/*` wire JSON**. The runnable reference is [`examples/s45_tool_as_task_lifecycle.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s45_tool_as_task_lifecycle.rs). The high-level `Server::builder()` takes `task_store(...)` too, so the same pattern works behind `StreamableHttpServer`.

| Backend | Plugs into | Use Case |
|---------|-----------|----------|
| `InMemoryTaskStore` (`pmcp`) | `task_store(...)` | Development, testing, single-instance servers |
| Your own `impl TaskStore` over a shared database | `task_store(...)` | AWS Lambda, serverless, multi-instance |
| `GenericTaskStore<DynamoDbBackend>` / `<RedisBackend>` (`pmcp-tasks`) | legacy `with_task_store(TaskRouterImpl)` only | Existing servers on the router path |

**Owner isolation needs authentication.** Each task is served only to its owner, the authenticated OAuth subject. Unauthenticated callers share one bucket (`"local"` on v1, `""` on a v2 server with no auth provider), so on a server without auth anyone holding a task id can read the task. Configure an auth provider before storing anything private.

**Key point:** The `TaskStore` is not just storage -- it enforces the MCP state machine. Transitions like `completed -> working` are rejected at the store level, not in your handler code. This means you cannot accidentally corrupt task state regardless of how your handler logic is structured.

## Chapter Contents

This chapter has two hands-on sections and an exercise set:

1. **[Task Lifecycle and Polling](./ch21-01-lifecycle.md)** -- Set up a TaskStore, declare task support on tools, write a dual-path handler, and implement the `get_task_result` fallback tool pattern

2. **[Capability Negotiation](./ch21-02-capability-negotiation.md)** -- Understand how per-request capability signals work, the three client profiles, and why no session state is needed for task negotiation

3. **[Chapter 21 Exercises](./ch21-exercises.md)** -- Practice adding task support to existing tools, building dual-path handlers, and designing task support strategies

## Knowledge Check

Before continuing, make sure you can answer:

- What problem does the two-phase task model solve that synchronous tool calls cannot?
- Why is polling better than SSE notifications for serverless deployments?
- What role does the `TaskStore` play in a stateless server architecture?

---

*Continue to [Task Lifecycle and Polling](./ch21-01-lifecycle.md) ->*
