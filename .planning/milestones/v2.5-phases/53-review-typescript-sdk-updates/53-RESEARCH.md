# Phase 53: Review TypeScript SDK Updates - Research

**Researched:** 2026-03-19
**Domain:** Comparative SDK analysis (TypeScript MCP SDK v2.0.0-alpha.0 vs Rust PMCP SDK v1.20.0)
**Confidence:** HIGH

## Summary

This research compares the TypeScript MCP SDK v2.0.0-alpha.0 (packages split into `@modelcontextprotocol/core`, `server`, `client`, and middleware packages) against the Rust PMCP SDK v1.20.0 across six domains: protocol version negotiation, conformance testing, MCP Apps, Tasks, framework adapters, and package structure.

The TypeScript SDK has made significant architectural changes in v2: (1) split into core/server/client/middleware packages, (2) added experimental task support with `TaskStore`/`TaskMessageQueue` interfaces and polling, (3) created framework middleware packages for Express/Hono/Node.js, (4) introduced a conformance test infrastructure using the `@modelcontextprotocol/conformance` CLI, and (5) moved to Zod v4 for all schema validation. The TypeScript SDK also tracks the DRAFT-2026-v1 spec in `spec.types.ts` while `types.ts` uses `2025-11-25` as LATEST.

The Rust SDK already leads in several areas: MCP Apps with multi-platform adapters (ChatGPT, Claude Desktop, ext-apps), server-side auth (JWT, Cognito, OIDC), Tasks implementation with DynamoDB/Redis backends and PMCP-specific task variables, and outputSchema/structuredContent support. The primary gaps are: lack of formal conformance testing, older protocol version negotiation (missing `2025-11-25`), and no framework adapter layer (Tower/Axum).

**Primary recommendation:** Focus implementation effort on (1) updating protocol version support to include `2025-11-25`, (2) porting conformance test patterns to mcp-tester, and (3) designing a Tower middleware adapter. MCP Apps and Tasks are already ahead of TypeScript.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Support the latest 3 protocol versions: 2025-11-25, 2025-06-18, 2025-03-26
- Don't support 2024 protocol versions -- they're pre-production
- Extend mcp-tester with a `conformance` command for MCP spec compliance testing
- Goal: any MCP server (Rust or not) can validate spec compliance via `mcp-tester conformance <url>`
- Full gap analysis across three MCP Apps dimensions: spec alignment, host-side APIs, DX ergonomics
- Compare both Tasks implementations for API shape compatibility
- Keep feature flags approach (not crate split) for package structure
- Framework adapters: Claude's discretion on which frameworks to recommend

### Claude's Discretion
- Exact format of the gap analysis report (markdown table vs prose vs both)
- Priority scoring methodology for recommendations
- Which TypeScript conformance tests to port first
- Framework adapter recommendations (Tower middleware + Axum adapter suggested in CONTEXT.md)

### Deferred Ideas (OUT OF SCOPE)
- WebSocket transport for Rust SDK (full bidirectional)
- Cross-runtime support (WASM target for browser clients)
</user_constraints>

## Domain 1: Protocol Version Negotiation

### TypeScript SDK (v2.0.0-alpha.0) - Current State
**Confidence: HIGH** (read directly from source)

In `packages/core/src/types/types.ts`:
```typescript
export const LATEST_PROTOCOL_VERSION = '2025-11-25';
export const DEFAULT_NEGOTIATED_PROTOCOL_VERSION = '2025-03-26';
export const SUPPORTED_PROTOCOL_VERSIONS = [
    LATEST_PROTOCOL_VERSION, '2025-06-18', '2025-03-26', '2024-11-05', '2024-10-07'
];
```

Additionally, `spec.types.ts` (auto-generated from the MCP spec) defines:
```typescript
export const LATEST_PROTOCOL_VERSION = 'DRAFT-2026-v1';
```

**Negotiation logic** (in `server/server.ts` `_oninitialize`):
```typescript
const protocolVersion = this._supportedProtocolVersions.includes(requestedVersion)
    ? requestedVersion
    : (this._supportedProtocolVersions[0] ?? LATEST_PROTOCOL_VERSION);
```

Key design: The `Protocol` class accepts `supportedProtocolVersions` as an option (defaults to `SUPPORTED_PROTOCOL_VERSIONS`). Server echoes client's version if supported, otherwise falls back to first supported version (most preferred). Client sends `LATEST_PROTOCOL_VERSION` and validates the server's response against its own supported list.

### Rust SDK (v1.20.0) - Current State
**Confidence: HIGH** (read directly from source)

In `src/lib.rs`:
```rust
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];

pub fn negotiate_protocol_version(client_version: &str) -> String {
    if SUPPORTED_PROTOCOL_VERSIONS.contains(&client_version) {
        client_version.to_string()
    } else {
        DEFAULT_PROTOCOL_VERSION.to_string()
    }
}
```

### Gap Analysis

| Aspect | TypeScript | Rust | Gap |
|--------|-----------|------|-----|
| LATEST_PROTOCOL_VERSION | `2025-11-25` | `2025-06-18` | **Rust behind by one version** |
| Supported count | 5 (incl. 2024 versions) | 4 (incl. 2024 versions) | Rust missing `2025-11-25` |
| Fallback behavior | First in supported list | DEFAULT_PROTOCOL_VERSION | Semantically equivalent |
| Configurable per-server | Yes (via `ProtocolOptions`) | No (constants only) | Minor gap |
| Draft spec tracking | `DRAFT-2026-v1` in spec.types.ts | None | TypeScript auto-syncs from spec repo |

### Recommendations

1. **Add `2025-11-25` to SUPPORTED_PROTOCOL_VERSIONS** and set as LATEST. This version introduces task augmentation and execution metadata.
2. **Drop 2024 versions** per user decision (support only `2025-11-25`, `2025-06-18`, `2025-03-26`).
3. **No need for configurable per-server versions** -- the constant-based approach is simpler and the Rust SDK is opinionated by design.

---

## Domain 2: Conformance Test Suite

### TypeScript SDK Structure
**Confidence: HIGH** (read directly from source)

The TypeScript SDK uses the external `@modelcontextprotocol/conformance` CLI tool:

```
test/conformance/
  src/
    everythingServer.ts   -- Server implementing ALL MCP features
    everythingClient.ts   -- Client handling ALL conformance scenarios
    helpers/
      conformanceOAuthProvider.ts
      logger.ts
      withOAuthRetry.ts
  expected-failures.yaml  -- Known failures (currently: client: [])
  scripts/
    run-server-conformance.sh  -- Launches server + runs conformance CLI
```

**Server conformance tests** (`everythingServer.ts`):
- Implements a complete MCP server with ALL features: tools (text, image, audio, embedded resource, multiple content types, logging, progress, error handling, reconnection, sampling, elicitation), resources (static text, static binary, template, watched/subscribable), prompts (simple, with args, with embedded resource, with image), logging, completions
- Uses Express + NodeStreamableHTTPServerTransport with session management
- Supports SSE resumability (SEP-1699) via EventStore
- Tests: `npx @modelcontextprotocol/conformance server --url http://localhost:3000/mcp`

**Client conformance tests** (`everythingClient.ts`):
- Scenario-based: each scenario maps to a handler function
- Scenarios include: `initialize`, `tools_call`, auth variants (basic-cimd, metadata, scopes, token-endpoint-auth), client-credentials (JWT, basic), cross-app-access, elicitation, SSE retry
- Pattern: `MCP_CONFORMANCE_SCENARIO` env var routes to handler

**Key insight:** The conformance CLI (`@modelcontextprotocol/conformance`) is a **separate package** that runs scenarios against servers/clients. It's not part of the SDK itself -- it's an external test runner.

### Rust SDK - Current State

The Rust SDK has no formal conformance testing. `mcp-tester` provides:
- `check` - basic health/connectivity checks
- `run` - interactive tool execution
- `apps` - MCP Apps validation (tool metadata, widget cross-references)
- `compliance` command stub mentioned in CONTEXT.md

### Gap Analysis

| Aspect | TypeScript | Rust (mcp-tester) | Gap |
|--------|-----------|-------------------|-----|
| Spec compliance testing | `@modelcontextprotocol/conformance` CLI | None | **Major gap** |
| Everything server | Full implementation | None | Needed for testing |
| Auth scenarios | 15+ OAuth/CIMD scenarios | None | Large gap |
| Transport testing | SSE resumability, reconnection | Basic connectivity | Significant gap |
| Content type coverage | text, image, audio, resource, mixed | Via tool calls only | Moderate gap |

### Conformance Test Categories to Port (Priority Order)

1. **Core protocol**: `initialize` (version negotiation, capability exchange)
2. **Tools**: `tools_call` (list, call, content types, error handling)
3. **Resources**: list, read, template, subscribe/unsubscribe
4. **Prompts**: list, get, with args, with embedded content
5. **Transport**: SSE reconnection (SEP-1699), session management
6. **Progress & Logging**: progress notifications, log level management
7. **Completions**: completion/complete
8. **Elicitation**: form-based, URL-based (if Rust supports it)
9. **Sampling**: server-initiated LLM requests
10. **Auth**: OAuth scenarios (lower priority -- Rust has its own auth)

### Recommendations

1. **Add `mcp-tester conformance <url>` command** that runs a suite of test scenarios against any MCP server
2. **Model after the "everything server" pattern** -- create test scenarios that exercise all MCP features
3. **Start with core protocol + tools + resources** -- highest value, lowest complexity
4. **Use the TypeScript `expected-failures.yaml` pattern** to track known gaps during incremental rollout
5. **Output format**: JSON results with pass/fail/skip per scenario (aligns with mcp-tester existing patterns)

---

## Domain 3: MCP Apps / UI Extensions

### TypeScript SDK - Current State
**Confidence: MEDIUM** (TypeScript SDK has minimal MCP Apps support)

The TypeScript v2 SDK has:
- `outputSchema` on ToolSchema (top-level field)
- `structuredContent` on CallToolResultSchema (via Zod schema)
- `_meta` on all major types (Resource, Tool, Prompt, etc.)
- `ToolExecution` schema with `taskSupport` enum
- **No UI adapters, no widget runtime, no ext-apps integration, no host-specific metadata enrichment**

The TypeScript SDK takes a minimalist approach to MCP Apps -- it provides the wire types but no higher-level UI abstractions.

### Rust SDK - Current State
**Confidence: HIGH** (read directly from source)

The Rust SDK has extensive MCP Apps support in `src/server/mcp_apps/`:
- **UIAdapter trait** with implementations: `McpAppsAdapter`, `ChatGptAdapter`, `McpUiAdapter`
- **MultiPlatformResource** for cross-host widget delivery
- **WidgetDir/WidgetEntry** for filesystem-based widget serving
- **Widget runtime** (`widget-runtime.mjs`) with AppBridge class
- **Host-specific metadata enrichment**: `build_meta_map()` emits standard `ui.resourceUri` + ChatGPT `openai/outputTemplate`
- **URI-to-tool-meta index** on ServerCore for automatic resource enrichment
- **Deep merge** for `_meta` collision prevention
- `outputSchema` top-level on ToolInfo (MCP spec 2025-06-18)
- `structuredContent` on CallToolResult with builder pattern
- **mcp-preview** crate for local development with ChatGPT/Claude Desktop emulation
- **mcp-tester apps** command for validating MCP Apps metadata

### Gap Analysis

| Aspect | TypeScript | Rust | Assessment |
|--------|-----------|------|------------|
| outputSchema | Yes (Zod) | Yes (Value) | **Parity** |
| structuredContent | Yes (Zod) | Yes (builder) | **Parity** |
| _meta support | Yes (all types) | Yes (all types) | **Parity** |
| UI adapters | None | 3 adapters | **Rust ahead** |
| Widget runtime | None | Full runtime | **Rust ahead** |
| Host metadata enrichment | None | Standard + ChatGPT + Claude | **Rust ahead** |
| Preview server | None | mcp-preview crate | **Rust ahead** |
| Apps validation | None | mcp-tester apps cmd | **Rust ahead** |
| DX (with_ui, TypedTool) | None | Full builder API | **Rust ahead** |
| ext-apps SDK alignment | Wire types only | Full v1.2.2 protocol | **Rust ahead** |

### Recommendations

1. **Rust is significantly ahead** in MCP Apps. No gaps to fill from TypeScript.
2. **Monitor TypeScript for UI adapter patterns** -- if Anthropic adds official ones, align.
3. **Potential contribution**: the Rust SDK's adapter pattern could inform TypeScript SDK design.
4. **Spec alignment check**: verify Rust's ext-apps implementation against latest spec (v1.2.2) -- already done in Phase 46.

---

## Domain 4: Tasks

### TypeScript SDK - Current State
**Confidence: HIGH** (read directly from source)

The TypeScript SDK has experimental task support across multiple packages:

**Core types** (`packages/core/src/types/types.ts`):
- `TaskSchema`: `{ taskId, status, ttl, createdAt, lastUpdatedAt, pollInterval?, statusMessage? }`
- `TaskStatusSchema`: `'working' | 'input_required' | 'completed' | 'failed' | 'cancelled'`
- `TaskCreationParams`: `{ ttl?, pollInterval? }`
- `TaskAugmentedRequestParams`: extends base params with `task: TaskMetadata`
- Methods: `tasks/get`, `tasks/result`, `tasks/list`, `tasks/cancel`
- Notifications: `notifications/tasks/status`
- Capability schemas: `ServerTasksCapability` and `ClientTasksCapability` with `requests` sub-capabilities

**Task interfaces** (`packages/core/src/experimental/tasks/interfaces.ts`):
- `TaskStore`: createTask, getTask, storeTaskResult, getTaskResult, updateTaskStatus, listTasks (all with optional sessionId)
- `TaskMessageQueue`: enqueue, dequeue, dequeueAll (for side-channel message delivery)
- `CreateTaskOptions`: `{ ttl?, pollInterval?, context? }`
- `isTerminal()` helper for status checking

**In-memory implementations** (`packages/core/src/experimental/tasks/stores/inMemory.ts`):
- `InMemoryTaskStore` with TTL-based cleanup via setTimeout
- `InMemoryTaskMessageQueue` with per-task FIFO queues
- Session isolation enforcement

**Server integration** (`packages/server/src/experimental/tasks/`):
- `ExperimentalServerTasks`: `requestStream`, `createMessageStream`, `elicitInputStream`, `getTask`, `getTaskResult`, `listTasks`, `cancelTask`
- `ExperimentalMcpServerTasks` (McpServer level): `registerToolTask` method for task-based tools
- `ToolTaskHandler` interface: `createTask`, `getTask`, `getTaskResult`
- `ToolExecution.taskSupport`: `'required' | 'optional' | 'forbidden'`

**Response streaming** (`packages/core/src/shared/responseMessage.ts`):
- `ResponseMessage<T>`: union of `taskCreated | taskStatus | result | error`
- AsyncGenerator-based streaming API for task-augmented requests

### Rust SDK - Current State
**Confidence: HIGH** (read directly from source)

The Rust SDK has `pmcp-tasks` crate (v0.1.0) with more extensive backend support:

**Types**: TaskRecord, TaskWithVariables, task status state machine validation
**Store trait**: with pluggable backends
**Backends**: InMemoryBackend, DynamoDbBackend (optional), RedisBackend (optional)
**Security**: owner binding, owner resolution from auth context
**Context**: TaskContext for tool handlers
**Router**: TaskRouterImpl bridging pmcp's TaskRouter trait to TaskStore
**PMCP extension**: Task variables (shared scratchpad between client and server)

### Gap Analysis

| Aspect | TypeScript | Rust | Assessment |
|--------|-----------|------|------------|
| Wire types (Task, TaskStatus) | Complete (Zod) | Complete (serde) | **Parity** |
| Task state machine | 5 states, isTerminal | 5 states + validation | **Parity** |
| TaskStore interface | Create, get, store result, update, list | Create, get, store result, update, list | **Parity** |
| In-memory store | Yes (with TTL cleanup) | Yes (InMemoryBackend) | **Parity** |
| DynamoDB store | No | Yes | **Rust ahead** |
| Redis store | No | Yes | **Rust ahead** |
| Task variables (PMCP extension) | No | Yes (TaskWithVariables) | **Rust ahead** |
| Owner binding / security | Session isolation only | OAuth sub/client ID binding | **Rust ahead** |
| TaskMessageQueue | Yes (side-channel delivery) | Not explicit | **TypeScript ahead** |
| registerToolTask DX | Yes (experimental) | Via TaskContext + Router | **Different approach** |
| AsyncGenerator streaming | requestStream, createMessageStream | Not implemented | **TypeScript ahead** |
| ToolExecution.taskSupport | required/optional/forbidden | Value field on ToolInfo | **Parity** (wire-level) |
| Capability negotiation | tasks.requests.tools.call | Not in capabilities | **TypeScript ahead** |
| Task status notifications | notifications/tasks/status | Not implemented | **TypeScript ahead** |

### Recommendations

1. **Add task capability negotiation** to ServerCapabilities (tasks field with requests sub-capabilities)
2. **Add `notifications/tasks/status`** notification support
3. **Consider TaskMessageQueue pattern** for side-channel message delivery during long-running tasks
4. **The PMCP task variables extension is a differentiator** -- keep it
5. **AsyncGenerator streaming** is TypeScript-specific; Rust equivalent would be `Stream<Item=ResponseMessage>`
6. **The `ToolExecution.taskSupport` enum** should be explicitly typed (currently `Value`)

---

## Domain 5: Framework Adapters

### TypeScript SDK - Current State
**Confidence: HIGH** (read directly from source)

The TypeScript SDK has three middleware packages:

**`@modelcontextprotocol/express`** (`packages/middleware/express/`):
- `createMcpExpressApp(options)`: Creates pre-configured Express app
- DNS rebinding protection middleware (automatic for localhost)
- Host header validation middleware
- Options: `host`, `allowedHosts`, `jsonLimit`
- Very thin -- just Express app creation + security middleware

**`@modelcontextprotocol/hono`** (`packages/middleware/hono/`):
- `createMcpHonoApp(options)`: Creates pre-configured Hono app
- Same DNS rebinding / host validation as Express
- JSON body parsing middleware (sets `parsedBody` on context)
- Web Standards compatible (works on Cloudflare Workers, Deno, Bun)

**`@modelcontextprotocol/node`** (`packages/middleware/node/`):
- `NodeStreamableHTTPServerTransport`: Wraps `WebStandardStreamableHTTPServerTransport`
- Converts Node.js `IncomingMessage`/`ServerResponse` to Web Standard `Request`/`Response`
- Uses `@hono/node-server` for conversion
- Supports session management, SSE streaming, pre-parsed request bodies

**Architecture pattern**: The TypeScript SDK separates transport (how messages flow) from framework integration (how HTTP requests are routed). The `WebStandardStreamableHTTPServerTransport` in `@modelcontextprotocol/server` is the core; middleware packages adapt it to specific frameworks.

### Rust SDK - Current State

The Rust SDK uses Axum internally for streamable HTTP but does not expose a clean adapter layer:
- `src/server/transport/` contains WebSocket transports only
- Streamable HTTP is in `src/shared/streamable_http.rs` (client-side) and server-side Axum routes are inline
- No Tower middleware layer
- No framework-agnostic adapter

### Gap Analysis

| Aspect | TypeScript | Rust | Gap |
|--------|-----------|------|-----|
| Framework-specific middleware | Express, Hono | None | **Major gap** |
| DNS rebinding protection | Built-in (auto for localhost) | None | **Security gap** |
| Host header validation | Configurable allowedHosts | None | **Security gap** |
| Transport abstraction | WebStandard + Node adapter | Axum-specific inline | **Architecture gap** |
| Body parsing | Middleware-handled | Framework-specific | Minor |

### Recommendations

**Framework adapter design** (Claude's Discretion recommendation):

1. **Tower middleware layer** (framework-agnostic):
   - `McpLayer` implementing `tower::Layer` that wraps MCP protocol handling
   - DNS rebinding protection as a `tower::Layer`
   - Host header validation as a `tower::Layer`
   - Session management middleware
   - Works with any Tower-compatible server (Axum, Hyper, Tonic)

2. **Axum convenience adapter**:
   - `McpRouter` that creates an `axum::Router` with `/mcp` routes (POST, GET for SSE, DELETE for session termination)
   - Pre-configured with Tower middleware stack
   - Handles request conversion to MCP transport
   - Pattern: `let router = McpRouter::new(server).with_dns_protection().build();`

3. **Priority**: Tower middleware first (greatest flexibility), Axum adapter second (convenience)

4. **Skip Actix-web** initially -- Tower middleware works with Hyper which can be used as a standalone server

---

## Domain 6: Package Structure

### TypeScript SDK - Current State
**Confidence: HIGH** (read directly from source)

```
packages/
  core/         -- @modelcontextprotocol/core (v2.0.0-alpha.0)
                   Protocol types, schemas (Zod v4), shared protocol logic,
                   auth errors, transport interface, experimental tasks interfaces
  server/       -- @modelcontextprotocol/server (v2.0.0-alpha.0)
                   Server class, McpServer high-level API, streamable HTTP transport,
                   stdio transport, experimental tasks server
  client/       -- @modelcontextprotocol/client (v2.0.0-alpha.0)
                   Client class, auth providers, transport implementations,
                   experimental tasks client
  middleware/
    express/    -- @modelcontextprotocol/express
    hono/       -- @modelcontextprotocol/hono
    node/       -- @modelcontextprotocol/node (NodeStreamableHTTPServerTransport)
```

**Why they split**: ESM/CJS module resolution, tree-shaking for bundlers, separate versioning for middleware, framework-specific peer dependencies. The `core` package has zero framework dependencies; `server` and `client` re-export everything from `core`.

### Rust SDK - Current State

```
pmcp (v1.20.0)          -- Main crate, feature-gated
  features: full, http-client, http-server, websocket,
            mcp-apps, auth, logging, etc.
crates/
  pmcp-macros/          -- Derive macros
  mcp-tester/           -- Testing CLI tool
  mcp-preview/          -- Local preview server
  pmcp-tasks/           -- Experimental tasks (v0.1.0)
  pmcp-widget-utils/    -- Widget utility functions
  pmcp-server/          -- PMCP MCP Server
```

### Gap Analysis

| Aspect | TypeScript | Rust | Assessment |
|--------|-----------|------|------------|
| Core/Server/Client split | 3 packages | 1 crate + features | **Different, both valid** |
| Tree-shaking equivalent | Package boundaries | Feature flags | **Rust has equivalent** (proven in Phase 52: 249 -> 134 deps) |
| Middleware isolation | Separate packages | N/A (no middleware yet) | Gap but correct to not split |
| Version independence | Per-package semver | Workspace versioning | TypeScript more granular |
| Dependency isolation | Package boundaries | Feature flags | **Equivalent** |

### Recommendations

1. **Keep feature flags approach** per user decision -- this is correct for Rust
2. **When framework adapters are added**, they should be separate crates (like `pmcp-tower` or `pmcp-axum`) since they introduce framework-specific dependencies
3. **No need to split pmcp into core/server/client** -- the feature flag approach already provides compile-time tree-shaking

---

## Cross-Cutting Findings

### TypeScript SDK v2 Breaking Changes (Relevant to Rust)

1. **Auth removed from server**: TypeScript v2 removed server-side auth, recommending dedicated auth libraries. Rust SDK keeps JWT + Cognito/OIDC -- this is a **strength**, not a gap.
2. **Zod v4**: All schema validation uses Zod v4 with `.looseObject()` for extensibility. Rust uses serde with `#[serde(flatten)]` for the same effect.
3. **Web Standard APIs**: TypeScript targets Cloudflare Workers, Deno, Bun via `WebStandardStreamableHTTPServerTransport`. Not relevant for Rust.
4. **Experimental namespace**: TypeScript uses `server.experimental.tasks` access pattern. Rust uses separate crate (`pmcp-tasks`) -- cleaner isolation.

### Protocol Version `2025-11-25` Changes

This version adds:
- Task augmentation (`task` field in request params)
- `ToolExecution` with `taskSupport` enum on ToolInfo
- `tasks/*` methods (get, result, list, cancel)
- `notifications/tasks/status`
- Tasks capability in both client and server capabilities
- `io.modelcontextprotocol/related-task` meta key
- Elicitation improvements (form vs URL mode)
- Icon schema for tools, prompts, resources, and implementations

### Priority Scoring for Recommendations

| Area | Priority | Effort | Value | Rationale |
|------|----------|--------|-------|-----------|
| Protocol version update | P0 | Low | High | Prerequisite for Tasks interop |
| Conformance testing | P1 | Medium | High | Spec compliance, quality signal |
| Tasks capability negotiation | P1 | Low | Medium | Interop with TypeScript clients |
| Tower middleware | P2 | High | High | Framework adapter story |
| MCP Apps gaps | P3 | Low | Low | Rust already ahead |
| Package structure | N/A | None | None | No changes needed |

---

## Architecture Patterns

### Conformance Test Architecture (for mcp-tester)

```
crates/mcp-tester/src/
  conformance/
    mod.rs              -- ConformanceRunner, scenario registry
    scenarios/
      mod.rs            -- Scenario trait, ScenarioResult
      initialize.rs     -- Protocol init + version negotiation
      tools.rs          -- Tool listing, calling, content types
      resources.rs      -- Resource list, read, template, subscribe
      prompts.rs        -- Prompt list, get, with args
      progress.rs       -- Progress notifications
      logging.rs        -- Log level management
      completions.rs    -- Completion/complete
      transport.rs      -- SSE reconnection, session management
    expected_failures.rs -- Known failures config (YAML/TOML)
    report.rs           -- JSON/text output formatting
```

**Pattern**: Each scenario is a struct implementing a `ConformanceScenario` trait:
```rust
#[async_trait]
pub trait ConformanceScenario: Send + Sync {
    fn name(&self) -> &str;
    fn category(&self) -> &str;
    async fn run(&self, client: &McpClient) -> ScenarioResult;
}
```

### Tower Middleware Architecture

```rust
// Tower Layer for MCP
pub struct McpLayer {
    config: McpLayerConfig,
}

impl<S> Layer<S> for McpLayer {
    type Service = McpService<S>;
    fn layer(&self, inner: S) -> Self::Service { ... }
}

// Axum convenience
pub fn mcp_router(server: ServerCore) -> Router {
    Router::new()
        .route("/mcp", post(handle_post).get(handle_sse).delete(handle_delete))
        .layer(DnsRebindingProtection::localhost())
        .with_state(server)
}
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| DNS rebinding protection | Custom header checks | Tower middleware layer | Security-critical, well-defined pattern |
| JSON-RPC message parsing | Manual parsing | Existing serde types | Already comprehensive |
| Task state machine | Ad-hoc status tracking | pmcp-tasks crate | Already implements full state machine with validation |
| SSE event store | Custom storage | Existing EventStore pattern | TypeScript's pattern is proven |

---

## Common Pitfalls

### Pitfall 1: Protocol Version Mismatch
**What goes wrong:** Server claims version support but doesn't implement features from that version
**Why it happens:** Adding version string without implementing the features it requires
**How to avoid:** Each supported version must have its feature set validated by conformance tests
**Warning signs:** Conformance tests passing for old features but failing for version-specific ones

### Pitfall 2: Conformance Test Fragility
**What goes wrong:** Tests break when server response includes extra fields
**Why it happens:** Over-constraining expected responses
**How to avoid:** Use structural matching (required fields present) not exact matching
**Warning signs:** Tests failing on servers that add extension fields to responses

### Pitfall 3: Task Capability Negotiation Mismatch
**What goes wrong:** Server claims task support but client doesn't understand task responses
**Why it happens:** Missing capability check before sending task-augmented responses
**How to avoid:** Always check `capabilities.tasks.requests.tools.call` before returning CreateTaskResult
**Warning signs:** TypeScript SDK clients getting unexpected response shapes

### Pitfall 4: Framework Adapter Abstraction Leak
**What goes wrong:** Tower middleware assumes Axum-specific context types
**Why it happens:** Building adapter around Axum first, then trying to generalize
**How to avoid:** Start with pure Tower Layer/Service types, add Axum convenience on top
**Warning signs:** `axum::extract` types appearing in the middleware layer

---

## Code Examples

### Protocol Version Update Pattern
```rust
// src/lib.rs
pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
];
```

### Task Capabilities Addition Pattern
```rust
// src/types/capabilities.rs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TasksCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<TaskRequestsCapability>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequestsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsTaskCapability>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsTaskCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<HashMap<String, Value>>,
}
```

### Conformance Scenario Pattern
```rust
// crates/mcp-tester/src/conformance/scenarios/initialize.rs
pub struct InitializeScenario;

#[async_trait]
impl ConformanceScenario for InitializeScenario {
    fn name(&self) -> &str { "initialize" }
    fn category(&self) -> &str { "core" }

    async fn run(&self, client: &McpClient) -> ScenarioResult {
        let result = client.initialize().await?;

        // Verify protocol version is in supported list
        assert!(SUPPORTED_PROTOCOL_VERSIONS.contains(&result.protocol_version.as_str()),
            "Server returned unsupported protocol version: {}", result.protocol_version);

        // Verify required capability fields
        assert!(result.server_info.name.len() > 0, "Server name must not be empty");
        assert!(result.server_info.version.len() > 0, "Server version must not be empty");

        ScenarioResult::pass()
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single protocol version | Multi-version negotiation | 2025-06-18 spec | Backward compat with older clients |
| Tools always synchronous | ToolExecution.taskSupport | 2025-11-25 spec | Long-running tools via task polling |
| Monolithic SDK package | Core/Server/Client split (TS) | v2.0.0-alpha.0 | Better tree-shaking (TS-specific) |
| Server-managed auth | External auth libs (TS) | v2.0.0-alpha.0 | Simpler SDK (but Rust approach is fine) |
| Manual conformance | @modelcontextprotocol/conformance CLI | v2 era | Standardized compliance testing |

---

## Open Questions

1. **Should the gap analysis report be a separate deliverable file?**
   - What we know: CONTEXT.md says "produce gap analysis with prioritized recommendations"
   - What's unclear: Is the report this RESEARCH.md or a separate GAP-ANALYSIS.md?
   - Recommendation: The plan should produce a standalone `GAP-ANALYSIS.md` suitable for external review

2. **Which `@modelcontextprotocol/conformance` scenarios exist?**
   - What we know: The CLI exists as a separate package, TypeScript SDK references it
   - What's unclear: Full list of scenarios (we see server/client but not the scenario catalog)
   - Recommendation: The planner should include a task to enumerate conformance scenarios from the CLI itself

3. **Should Rust add `icons` field to Implementation/ToolInfo?**
   - What we know: TypeScript has `IconsSchema` on Implementation, Tool has _meta for icons
   - What's unclear: Whether any clients actually render icons yet
   - Recommendation: Low priority -- add to gap analysis but don't implement yet

---

## Sources

### Primary (HIGH confidence)
- TypeScript SDK source code at `~/Development/mcp/sdk/typescript-sdk/` (v2.0.0-alpha.0)
  - `packages/core/src/types/types.ts` -- Protocol types, version constants, task schemas
  - `packages/core/src/types/spec.types.ts` -- Auto-generated draft spec types
  - `packages/core/src/shared/protocol.ts` -- Protocol class, TaskStore, TaskContext
  - `packages/core/src/experimental/tasks/` -- Task interfaces, InMemory stores
  - `packages/server/src/server/server.ts` -- Server class, version negotiation
  - `packages/server/src/experimental/tasks/` -- Server task integration
  - `packages/middleware/express/src/express.ts` -- Express adapter
  - `packages/middleware/hono/src/hono.ts` -- Hono adapter
  - `packages/middleware/node/src/streamableHttp.ts` -- Node transport wrapper
  - `test/conformance/` -- Conformance test infrastructure

- Rust SDK source code at current working directory (v1.20.0)
  - `src/lib.rs` -- Version constants, negotiation function
  - `src/types/protocol.rs` -- ToolInfo, CallToolResult, ToolAnnotations
  - `src/types/capabilities.rs` -- Client/Server capabilities
  - `src/server/mcp_apps/` -- MCP Apps adapters and builders
  - `crates/pmcp-tasks/` -- Tasks crate with backends
  - `docs/design/tasks-feature-design.md` -- Tasks design document

### Secondary (MEDIUM confidence)
- MCP specification v2025-11-25 (inferred from TypeScript SDK type definitions)
- ext-apps SDK v1.2.2 (referenced in CONTEXT.md canonical refs)

---

## Metadata

**Confidence breakdown:**
- Protocol version negotiation: HIGH -- read both SDK source files directly
- Conformance testing: HIGH -- read TypeScript conformance infrastructure completely
- MCP Apps: HIGH -- both SDKs read directly, Rust extensively documented from prior phases
- Tasks: HIGH -- both implementations read in detail
- Framework adapters: HIGH -- TypeScript middleware read completely, Rust transport understood
- Package structure: HIGH -- both package structures enumerated

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (30 days -- both SDKs are stable in their respective areas)
