# Phase 55: Tasks with Polling - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement MCP Tasks with status polling over streamable HTTP. Add tasks/create, tasks/get, tasks/list, tasks/cancel methods. TaskStore trait with in-memory implementation in the SDK. Task TTL and auto-expiration. Task variables (PMCP extension). Reconcile SDK types with pmcp-tasks crate types. No SSE-based task notifications — polling is the async pattern.

</domain>

<decisions>
## Implementation Decisions

### Type reconciliation
- **D-01:** SDK types (`src/types/tasks.rs`) are the canonical source of truth
- ~~D-02: pmcp-tasks crate re-exports or converts from SDK types~~ — **Deferred:** pmcp-tasks refactoring is a follow-up phase. This phase establishes SDK types as canonical; pmcp-tasks adaptation happens separately.
- **D-03:** Fix the ttl serialization divergence — SDK types must match MCP spec (ttl present as null, not omitted)
- **D-04:** Add utility methods from pmcp-tasks (is_terminal(), can_transition_to()) to SDK TaskStatus

### Polling API contract
- **D-05:** Simple interval polling — client calls tasks/get repeatedly on a timer (2-5 seconds)
- **D-06:** Server returns current Task with status field. Client stops when status is terminal
- **D-07:** No long-polling, no SSE notifications — stateless request/response only
- **D-08:** Polling is the pattern for long-running agent tasks, replacing complex SSE

### Crate relationship
- **D-09:** SDK provides TaskStore trait + InMemoryTaskStore (for dev/testing)
- **D-10:** pmcp-tasks provides production backends (DynamoDB, Redis) — separate crate
- **D-11:** Users start simple with in-memory, add backends when scaling to production
- **D-12:** Follow TypeScript SDK pattern: tasks are in core with store interfaces + in-memory impl

### Task lifecycle scope — ALL included
- **D-13:** tasks/create, tasks/get, tasks/cancel (core trio)
- **D-14:** tasks/list (discovery — list all tasks for current session)
- **D-15:** Task TTL and auto-expiration (configurable, prevents zombie tasks)
- ~~D-16: Task variables~~ — **Deferred:** PMCP extension stays in pmcp-tasks crate per D-10/D-12 (TypeScript SDK pattern = no variables in core). Not in ROADMAP success criteria.

### Claude's Discretion
- TaskStore trait method signatures (sync vs async, error types)
- How the builder integrates TaskStore (existing .task_router() pattern or new .task_store() method)
- InMemoryTaskStore implementation details (locking, cleanup strategy)
- Polling interval recommendation in docs

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Task design
- `docs/design/tasks-feature-design.md` — Original tasks feature design with DynamoDB backend, task variables
- `crates/pmcp-tasks/src/store/backend.rs` — Existing StorageBackend trait (reference for TaskStore design)
- `crates/pmcp-tasks/src/types/` — Existing task types to reconcile with SDK types

### SDK types (Phase 54)
- `src/types/tasks.rs` — Canonical task wire types (Task, TaskStatus, CreateTaskResult, etc.)
- `src/types/capabilities.rs` — ServerTasksCapability, ClientTasksCapability
- `src/types/protocol/mod.rs` — ClientRequest task variants (TasksGet, TasksList, TasksCancel, TasksCreate)

### TypeScript SDK reference
- `~/Development/mcp/sdk/typescript-sdk/packages/server/src/experimental/tasks/` — TypeScript TaskStore/TaskMessageQueue interfaces

### Existing integration
- `src/server/builder.rs` — Builder with .task_router() method (line 606+)
- `src/server/tasks.rs` — Current TaskRouter trait in SDK

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/pmcp-tasks/src/store/backend.rs` — StorageBackend trait with get, create, update, list, delete
- `crates/pmcp-tasks/src/store/memory.rs` — InMemoryStore implementation (can be adapted)
- `src/server/builder.rs` — Builder already has `.task_router()` method accepting Arc<dyn TaskRouter>
- `src/types/tasks.rs` — All wire types already defined from Phase 54

### Established Patterns
- Feature flags for optional functionality (tasks should be behind a `tasks` feature flag)
- Builder pattern for server construction
- async_trait for all async handler traits
- parking_lot::RwLock for synchronization (used throughout SDK)

### Integration Points
- `src/server/builder.rs` — Where .task_store() or updated .task_router() connects
- `src/server/mod.rs` — Where task request handling is dispatched (ClientRequest::TasksGet, etc.)
- `src/types/protocol/mod.rs` — ClientRequest enum already has task variants

</code_context>

<specifics>
## Specific Ideas

- Tasks will likely replace complex SSE as the standard async pattern in MCP
- Follow TypeScript SDK's approach: tasks in core with store interfaces + in-memory impl
- The existing pmcp-tasks DynamoDB/Redis backends should continue working after type reconciliation
- Task variables are a PMCP extension for shared state between client and server during long-running tasks

</specifics>

<deferred>
## Deferred Ideas

- TaskMessageQueue (TypeScript SDK concept for side-channel message delivery) — future phase if needed
- Task resumption after server restart (requires persistent store) — users get this by plugging in DynamoDB backend
- Task progress notifications via SSE — explicitly de-prioritized per v2.0 direction

</deferred>

---

*Phase: 55-tasks-with-polling*
*Context gathered: 2026-03-20*
