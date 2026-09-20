# Phase 53 Verification Notes

**Verified:** 2026-03-20
**Sources:** TypeScript SDK v2.0.0-alpha.0 at `~/Development/mcp/sdk/typescript-sdk/`, Rust SDK v1.20.0 at working directory.
**Method:** Direct source code reading with file:line references for every comparison point.

---

## Domain 1: Protocol Versions

### TypeScript State

**File:** `packages/core/src/types/types.ts:3-5`
```typescript
export const LATEST_PROTOCOL_VERSION = '2025-11-25';
export const DEFAULT_NEGOTIATED_PROTOCOL_VERSION = '2025-03-26';
export const SUPPORTED_PROTOCOL_VERSIONS = [LATEST_PROTOCOL_VERSION, '2025-06-18', '2025-03-26', '2024-11-05', '2024-10-07'];
```

**File:** `packages/core/src/types/spec.types.ts:37-38`
```typescript
export const LATEST_PROTOCOL_VERSION = 'DRAFT-2026-v1';
```
The `spec.types.ts` is auto-generated from the MCP specification repo (commit `838d6f69`) and tracks the draft spec. It is 3,247 lines and contains pure TypeScript interface types (no Zod schemas). The `types.ts` file uses Zod schemas and is the runtime version.

**Negotiation logic -- File:** `packages/server/src/server/server.ts:431-447`
```typescript
private async _oninitialize(request: InitializeRequest): Promise<InitializeResult> {
    const requestedVersion = request.params.protocolVersion;
    this._clientCapabilities = request.params.capabilities;
    this._clientVersion = request.params.clientInfo;

    const protocolVersion = this._supportedProtocolVersions.includes(requestedVersion)
        ? requestedVersion
        : (this._supportedProtocolVersions[0] ?? LATEST_PROTOCOL_VERSION);

    return {
        protocolVersion,
        capabilities: this.getCapabilities(),
        serverInfo: this._serverInfo,
        ...(this._instructions && { instructions: this._instructions })
    };
}
```

Key: `_supportedProtocolVersions` comes from `ProtocolOptions` and defaults to the `SUPPORTED_PROTOCOL_VERSIONS` array. The fallback when client version is not in the list is the **first element** of the supported array (i.e., the most-preferred version), not a separate default constant. The `DEFAULT_NEGOTIATED_PROTOCOL_VERSION` constant exists but is not used in the server negotiation -- it is used by the client as the version to send when the user doesn't specify one.

**New in 2025-11-25 (verified from types.ts):**
- `TaskSchema` (types.ts:743-764): `{ taskId, status, ttl, createdAt, lastUpdatedAt, pollInterval?, statusMessage? }`
- `TaskStatusSchema` (types.ts:737): `'working' | 'input_required' | 'completed' | 'failed' | 'cancelled'`
- `TaskCreationParamsSchema` (types.ts:77-88): `{ ttl?, pollInterval? }`
- `TaskMetadataSchema` (types.ts:90-92): `{ ttl? }`
- `TaskAugmentedRequestParamsSchema` (types.ts:126-136): extends BaseRequestParams with `task: TaskMetadata`
- `TaskStatusNotificationSchema` (types.ts:781-784): `notifications/tasks/status`
- `GetTaskRequestSchema` (types.ts:789-794): `tasks/get`
- `GetTaskPayloadRequestSchema` (types.ts:804-809): `tasks/result`
- `ListTasksRequestSchema` (types.ts:822-824): `tasks/list`
- `CancelTaskRequestSchema` (types.ts:836-841): `tasks/cancel`
- `CreateTaskResultSchema` (types.ts:769-771): wraps TaskSchema in `{ task: ... }`
- `ToolExecutionSchema` (types.ts:1396-1406): `{ taskSupport?: 'required' | 'optional' | 'forbidden' }`
- `IconSchema` (types.ts:344-368): `{ src, mimeType?, sizes?, theme? }` where theme is `'light' | 'dark'`
- `IconsSchema` (types.ts:374-387): `{ icons?: Icon[] }` -- added to Implementation, Tool, Prompt, Resource, ResourceTemplate
- `BaseMetadataSchema` (types.ts:392-404): `{ name, title? }` -- shared by Tool, Prompt, Resource, Implementation
- `ImplementationSchema` (types.ts:410-427): extends BaseMetadata with `icons`, `version`, `websiteUrl?`, `description?`
- `RELATED_TASK_META_KEY` (types.ts:7): `'io.modelcontextprotocol/related-task'`
- `RelatedTaskMetadataSchema` (types.ts:98-100): `{ taskId: string }`
- `ServerTasksCapabilitySchema` (types.ts:492-516): `{ list?, cancel?, requests?: { tools?: { call? } } }`
- `ClientTasksCapabilitySchema` (types.ts:455-487): `{ list?, cancel?, requests?: { sampling?: { createMessage? }, elicitation?: { create? } } }`
- `ResourceLinkSchema` (types.ts:1296-1298): extends Resource with `type: 'resource_link'`
- `AudioContentSchema` (types.ts:1223-1244): `{ type: 'audio', data, mimeType, annotations?, _meta? }`
- `ToolUseContentSchema` (types.ts:1250-1272): `{ type: 'tool_use', name, id, input, _meta? }`
- Elicitation capability expanded: `{ form?: { applyDefaults? }, url? }` (types.ts:429-450)
- Sampling capability expanded: `{ context?, tools? }` (types.ts:529-541)

### Rust State

**File:** `src/lib.rs:263`
```rust
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";
```

**File:** `src/lib.rs:279`
```rust
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";
```

**File:** `src/lib.rs:306-311`
```rust
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];
```

**File:** `src/lib.rs:317-323`
```rust
pub fn negotiate_protocol_version(client_version: &str) -> String {
    if SUPPORTED_PROTOCOL_VERSIONS.contains(&client_version) {
        client_version.to_string()
    } else {
        DEFAULT_PROTOCOL_VERSION.to_string()
    }
}
```

### Delta (What Rust Needs)

| Item | Status | Detail |
|------|--------|--------|
| `LATEST_PROTOCOL_VERSION = "2025-11-25"` | MISSING | Currently `"2025-06-18"` |
| `"2025-11-25"` in SUPPORTED list | MISSING | Need to add |
| Drop `"2024-11-05"` and `"2024-10-07"` | Per user decision | Keep only latest 3 |
| Fallback algorithm difference | MINOR | Rust uses `DEFAULT_PROTOCOL_VERSION` (fixed constant); TypeScript uses `supportedVersions[0]` (first preferred). Semantically equivalent when constants are correct. |
| `IconSchema` / `IconsSchema` | MISSING | Not on Implementation, ToolInfo, ResourceInfo, or PromptInfo |
| `BaseMetadataSchema` (`title` field) | PARTIAL | ToolAnnotations has `title`, but Resource/Prompt/Implementation do not have top-level `title` |
| `Implementation.websiteUrl` | MISSING | Not in Rust Implementation struct |
| `Implementation.description` | MISSING | Not in Rust Implementation struct |
| `Implementation.icons` | MISSING | Not in Rust Implementation struct |
| `ResourceLinkSchema` content type | MISSING | Rust has `Content::Resource` but not `Content::ResourceLink` |
| `AudioContentSchema` content type | MISSING | Rust has Text, Image, Resource but not Audio |
| `ToolUseContentSchema` content type | MISSING | Not in Rust (used for sampling with tools) |
| `RELATED_TASK_META_KEY` constant | MISSING | Not defined in Rust |
| `TaskAugmentedRequestParams` | MISSING | No `task` field on request params |
| Elicitation capability expansion (form/url) | MISSING | Rust `ElicitationCapabilities` is empty struct |
| Sampling capability expansion (context/tools) | MISSING | Rust `SamplingCapabilities` only has `models` |

---

## Domain 2: Conformance Testing

### Architecture Pattern

**Conformance CLI invocation:**
- External package: `@modelcontextprotocol/conformance`
- Server testing: `npx @modelcontextprotocol/conformance server --url http://localhost:3000/mcp`
- Client testing: Sets `MCP_CONFORMANCE_SCENARIO` env var, launches client with server URL
- Infrastructure at: `test/conformance/` with `everythingServer.ts`, `everythingClient.ts`, helpers, scripts

**File:** `test/conformance/scripts/run-server-conformance.sh` -- launches the server, then runs the conformance CLI against it.

**Expected failures:** `test/conformance/expected-failures.yaml` -- Currently `client: []` (no expected client failures).

### Scenario Catalog: Server-Side (everythingServer.ts -- 1,014 lines)

**Tools registered (12 total):**

| # | Tool Name | Category | Purpose | File Line |
|---|-----------|----------|---------|-----------|
| 1 | `test_simple_text` | Content | Simple text response | :115-125 |
| 2 | `test_image_content` | Content | Image content (base64 PNG) | :128-138 |
| 3 | `test_audio_content` | Content | Audio content (base64 WAV) | :141-151 |
| 4 | `test_embedded_resource` | Content | Embedded resource content | :154-173 |
| 5 | `test_multiple_content_types` | Content | Mixed text + image + resource | :176-197 |
| 6 | `test_tool_with_logging` | Logging | Emits log notifications during execution | :200-236 |
| 7 | `test_tool_with_progress` | Progress | Reports progress notifications (0/50/100) | :239-284 |
| 8 | `test_error_handling` | Error | Intentionally throws error | :287-295 |
| 9 | `test_reconnection` | Transport | SEP-1699 SSE stream closure + reconnection | :298-332 |
| 10 | `test_sampling` | Sampling | Server-initiated LLM request via sampling/createMessage | :335-383 |
| 11 | `test_elicitation` | Elicitation | Server-initiated user input via elicitation/create | :386-434 |
| 12 | `test_elicitation_sep1034_defaults` | Elicitation | Elicitation with default values (SEP-1034) | :437-505 |
| 13 | `test_elicitation_sep1330_enums` | Elicitation | Elicitation enum schema variants (SEP-1330) | :508-598 |
| 14 | `json_schema_2020_12_tool` | Schema | JSON Schema 2020-12 conformance (SEP-1613) | :601-625 |

**Resources registered (4 total):**

| # | Resource Name | URI | Type | File Line |
|---|---------------|-----|------|-----------|
| 1 | `static-text` | `test://static-text` | Static text | :630-649 |
| 2 | `static-binary` | `test://static-binary` | Static binary (base64 PNG) | :652-671 |
| 3 | `template` | `test://template/{id}/data` | URI template | :674-698 |
| 4 | `watched-resource` | `test://watched-resource` | Subscribable/watched | :701-720 |

**Prompts registered (4 total):**

| # | Prompt Name | Category | File Line |
|---|-------------|----------|-----------|
| 1 | `test_simple_prompt` | Basic | :740-759 |
| 2 | `test_prompt_with_arguments` | With args (arg1, arg2) | :762-785 |
| 3 | `test_prompt_with_embedded_resource` | With embedded resource | :788-821 |
| 4 | `test_prompt_with_image` | With image content | :823-848 |

**Other handlers:**
- Subscribe/Unsubscribe (resources/subscribe, resources/unsubscribe) :723-735
- Logging (logging/setLevel) :852-856
- Completion (completion/complete -- returns empty values) :860-870

**Server capabilities declared:** `{ tools: { listChanged: true }, resources: { subscribe: true, listChanged: true }, prompts: { listChanged: true }, logging: {}, completions: {} }` (:75-89)

**Transport:** Express + `NodeStreamableHTTPServerTransport` with:
- Session management (per-session transport + server instances) :893-957
- EventStore for SSE resumability (SEP-1699) :30-59
- CORS with `Mcp-Session-Id` exposed :884-890
- DNS rebinding protection via `localhostHostValidation()` :881
- POST /mcp (tool calls, requests) :893-958
- GET /mcp (SSE streams) :961-985
- DELETE /mcp (session termination) :988+

### Scenario Catalog: Client-Side (everythingClient.ts -- 495 lines)

**Scenarios registered (21 total):**

| # | Scenario Name | Category | Handler Function | File Line |
|---|---------------|----------|------------------|-----------|
| 1 | `initialize` | Core | `runBasicClient` | :144 |
| 2 | `tools_call` | Tools | `runToolsCallClient` | :145 |
| 3 | `auth/basic-cimd` | Auth | `runAuthClient` | :176-194 |
| 4 | `auth/metadata-default` | Auth | `runAuthClient` | :176-194 |
| 5 | `auth/metadata-var1` | Auth | `runAuthClient` | :176-194 |
| 6 | `auth/metadata-var2` | Auth | `runAuthClient` | :176-194 |
| 7 | `auth/metadata-var3` | Auth | `runAuthClient` | :176-194 |
| 8 | `auth/2025-03-26-oauth-metadata-backcompat` | Auth | `runAuthClient` | :176-194 |
| 9 | `auth/2025-03-26-oauth-endpoint-fallback` | Auth | `runAuthClient` | :176-194 |
| 10 | `auth/scope-from-www-authenticate` | Auth | `runAuthClient` | :176-194 |
| 11 | `auth/scope-from-scopes-supported` | Auth | `runAuthClient` | :176-194 |
| 12 | `auth/scope-omitted-when-undefined` | Auth | `runAuthClient` | :176-194 |
| 13 | `auth/scope-step-up` | Auth | `runAuthClient` | :176-194 |
| 14 | `auth/scope-retry-limit` | Auth | `runAuthClient` | :176-194 |
| 15 | `auth/token-endpoint-auth-basic` | Auth | `runAuthClient` | :176-194 |
| 16 | `auth/token-endpoint-auth-post` | Auth | `runAuthClient` | :176-194 |
| 17 | `auth/token-endpoint-auth-none` | Auth | `runAuthClient` | :176-194 |
| 18 | `auth/client-credentials-jwt` | Auth | `runClientCredentialsJwt` | :231 |
| 19 | `auth/client-credentials-basic` | Auth | `runClientCredentialsBasic` | :263 |
| 20 | `auth/cross-app-access-complete-flow` | Auth | `runCrossAppAccessCompleteFlow` | :311 |
| 21 | `auth/pre-registration` | Auth | `runPreRegistrationClient` | :348 |
| 22 | `elicitation-sep1034-client-defaults` | Elicitation | `runElicitationDefaultsClient` | :411 |
| 23 | `sse-retry` | Transport | `runSSERetryClient` | :450 |

**Total: 23 client scenarios** (2 core + 18 auth + 1 elicitation + 1 transport + 1 pre-registration)

**Conformance context schema** (everythingClient.ts:42-68): Discriminated union on `name` field supporting:
- `auth/client-credentials-jwt`: `{ client_id, private_key_pem, signing_algorithm? }`
- `auth/client-credentials-basic`: `{ client_id, client_secret }`
- `auth/pre-registration`: `{ client_id, client_secret }`
- `auth/cross-app-access-complete-flow`: `{ client_id, client_secret, idp_client_id, idp_id_token, idp_issuer, idp_token_endpoint }`

### Rust Gap

- **No conformance command in mcp-tester** -- only `check`, `run`, `apps`, `compliance` mentioned
- **No "everything server"** reference implementation
- **No expected-failures tracking**
- **Priority for porting:** Core (initialize, tools_call) > Content types > Resources > Prompts > Transport > Auth scenarios

---

## Domain 3: MCP Apps

### TypeScript State

**Wire types present (verified in types.ts):**
- `outputSchema` on ToolSchema (:1434-1441) -- top-level field, `type: 'object'` root required
- `structuredContent` on CallToolResultSchema (:1489) -- `Record<string, unknown>`
- `_meta` on all major types: Resource (:958), ResourceTemplate (:993), Prompt (:1134), Tool (:1455), Content types (Text :1191, Image :1217, Audio :1243, EmbeddedResource :1288), ResourceContents (:865), Result (:170)

**No UI adapter code in TypeScript SDK.** Searched `packages/server/src/` for "ui", "widget", "app", "adapter" -- no UI/widget rendering logic exists. The TypeScript SDK is wire-types only for MCP Apps.

### Rust State

**Extensive MCP Apps support (verified):**
- `src/server/mcp_apps/` module with UIAdapter trait and implementations
- `ToolInfo._meta` field (protocol.rs:237-239) with `build_meta_map()` for standard `ui.resourceUri` + legacy `ui/resourceUri`
- `ToolInfo::with_ui()` constructor (protocol.rs:307-328) for widget-linked tools
- `ToolInfo::with_widget_meta()` (protocol.rs:366-373) for deep-merged widget metadata
- `ToolInfo::with_meta_entry()` (protocol.rs:394-399) for composable _meta entries
- `ToolInfo.output_schema` (protocol.rs:231-232) -- top-level field (MCP spec 2025-06-18)
- `ToolInfo.execution` (protocol.rs:245-246) -- `Option<Value>` for task support level
- `CallToolResult.structured_content` with builder pattern and `with_widget_enrichment()`
- `mcp-preview` crate for local development with ChatGPT/Claude Desktop emulation
- `mcp-tester apps` command for MCP Apps validation
- Deep merge utility in `src/types/ui.rs`
- `pmcp-widget-utils` crate for bridge script injection
- Host-specific metadata enrichment: standard `ui.resourceUri`, ChatGPT `openai/outputTemplate`, legacy flat keys

### Delta

**Rust is significantly ahead.** TypeScript has wire types only; Rust has full widget runtime, multi-platform adapters, preview server, validation tooling.

**Minor items to monitor:**
- TypeScript's `_meta` on AudioContent and ToolUseContent -- Rust doesn't have those content types yet, but when added they should include `_meta`
- The `icons` field on Resource, Prompt, Tool (TypeScript's IconsSchema) -- Rust doesn't have this yet

---

## Domain 4: Tasks

### Field-by-Field Comparison: Task Type

| Field | TypeScript `TaskSchema` (types.ts:743-764) | Rust `TaskRecord` (pmcp-tasks) | Match? |
|-------|---------------------------------------------|-------------------------------|--------|
| `taskId` | `z.string()` | `id: String` | Yes (different field name) |
| `status` | `TaskStatusSchema` (5 values) | `status: TaskStatus` (5 values) | Yes |
| `ttl` | `z.union([z.number(), z.null()])` | `ttl: Option<u64>` | Yes (Rust uses Option instead of null) |
| `createdAt` | `z.string()` (ISO 8601) | `created_at: DateTime<Utc>` | Yes |
| `lastUpdatedAt` | `z.string()` (ISO 8601) | `updated_at: DateTime<Utc>` | Yes |
| `pollInterval` | `z.optional(z.number())` | Not present | **Missing in Rust** |
| `statusMessage` | `z.optional(z.string())` | Not present | **Missing in Rust** |

### Task Status Values Comparison

| Status | TypeScript (types.ts:737) | Rust | Match? |
|--------|--------------------------|------|--------|
| `working` | Yes | Yes | Yes |
| `input_required` | Yes | Yes | Yes |
| `completed` | Yes | Yes | Yes |
| `failed` | Yes | Yes | Yes |
| `cancelled` | Yes | Yes | Yes |

### TaskStore Interface Comparison

| Method | TypeScript `TaskStore` (interfaces.ts:164-230) | Rust `TaskStore` trait | Match? |
|--------|------------------------------------------------|----------------------|--------|
| `createTask(params, requestId, request, sessionId?)` | Yes | `create_task(...)` | Yes (params differ slightly) |
| `getTask(taskId, sessionId?)` | Yes | `get_task(id)` | Yes (Rust lacks sessionId param) |
| `storeTaskResult(taskId, status, result, sessionId?)` | Yes | `store_result(id, result)` | Partial (TypeScript passes status) |
| `getTaskResult(taskId, sessionId?)` | Yes | `get_result(id)` | Yes |
| `updateTaskStatus(taskId, status, statusMessage?, sessionId?)` | Yes | `update_status(id, status)` | Partial (TS has statusMessage) |
| `listTasks(cursor?, sessionId?)` | Yes | `list_tasks(...)` | Yes |

### TaskMessageQueue (TypeScript-only)

**File:** `packages/core/src/experimental/tasks/interfaces.ts:103-131`

```typescript
export interface TaskMessageQueue {
    enqueue(taskId: string, message: QueuedMessage, sessionId?: string, maxSize?: number): Promise<void>;
    dequeue(taskId: string, sessionId?: string): Promise<QueuedMessage | undefined>;
    dequeueAll(taskId: string, sessionId?: string): Promise<QueuedMessage[]>;
}
```

`QueuedMessage` is a discriminated union of: `QueuedRequest`, `QueuedNotification`, `QueuedResponse`, `QueuedError` (interfaces.ts:54-85). Each has `{ type, timestamp, message }`.

**Purpose:** Side-channel message delivery during long-running tasks. When a task is running asynchronously, the server can queue messages (requests to the client, notifications, etc.) that the client retrieves when polling for task status. This enables the server to request sampling or elicitation while a task is in progress.

**Rust equivalent needed?** Yes, if Rust wants to support server-to-client requests (sampling, elicitation) during async task execution. The current Rust task implementation doesn't have this pattern. However, Rust's `RequestHandlerExtra` provides real-time access to sampling/elicitation during synchronous tool execution, so the queue is only needed for truly async (polled) tasks.

### ToolExecution.taskSupport Comparison

**TypeScript** (types.ts:1396-1406):
```typescript
export const ToolExecutionSchema = z.object({
    taskSupport: z.enum(['required', 'optional', 'forbidden']).optional()
});
```
Placed on `ToolSchema` as `execution: ToolExecutionSchema.optional()` (types.ts:1449).

**Rust** (protocol.rs:240-246):
```rust
pub execution: Option<Value>,
```
Uses `serde_json::Value` to avoid circular dependency with `pmcp-tasks`. Tests verify `json!({"taskSupport": "required"})` serialization (protocol.rs:1972-1977).

**Assessment:** Wire-level parity. Rust stores as opaque JSON which is correct for forward compatibility. TypeScript uses typed Zod schema for validation.

### Capability Schema Comparison

**TypeScript** `ServerTasksCapabilitySchema` (types.ts:492-516):
```
{ list?, cancel?, requests?: { tools?: { call? } } }
```

**TypeScript** `ClientTasksCapabilitySchema` (types.ts:455-487):
```
{ list?, cancel?, requests?: { sampling?: { createMessage? }, elicitation?: { create? } } }
```

Both are on the respective `ServerCapabilitiesSchema.tasks` (types.ts:638) and `ClientCapabilitiesSchema.tasks` (types.ts:560) fields.

**Rust** `ServerCapabilities` (capabilities.rs:44-74): **No `tasks` field.** The struct has `tools`, `prompts`, `resources`, `logging`, `completions`, `sampling`, `experimental` but no `tasks`.

**Rust** `ClientCapabilities` (capabilities.rs:23-41): **No `tasks` field.** Has `sampling`, `elicitation`, `roots`, `experimental`.

**Gap:** Both ServerCapabilities and ClientCapabilities need a `tasks` field for proper capability negotiation.

### Task Notifications

**TypeScript** `TaskStatusNotificationSchema` (types.ts:781-784):
```typescript
export const TaskStatusNotificationSchema = NotificationSchema.extend({
    method: z.literal('notifications/tasks/status'),
    params: TaskStatusNotificationParamsSchema  // merges NotificationsParams + TaskSchema
});
```

**Rust:** No `notifications/tasks/status` notification type defined.

### Delta Summary

| Item | TypeScript | Rust | Priority |
|------|-----------|------|----------|
| Task `pollInterval` field | Yes | No | P1 -- needed for interop |
| Task `statusMessage` field | Yes | No | P1 -- needed for diagnostics |
| `ServerCapabilities.tasks` | Yes | No | P0 -- required for capability negotiation |
| `ClientCapabilities.tasks` | Yes | No | P0 -- required for capability negotiation |
| `notifications/tasks/status` | Yes | No | P1 -- needed for real-time status |
| TaskMessageQueue | Yes | No | P2 -- only for async polling tasks |
| Session isolation on TaskStore | Yes (sessionId params) | Partial (owner binding) | P2 |
| `RELATED_TASK_META_KEY` | Yes | No | P1 -- interop metadata |
| `TaskAugmentedRequestParams` | Yes | No | P1 -- wire-level interop |
| `CreateTaskResult` wrapper | Yes | No | P1 -- response format |

---

## Domain 5: Framework Adapters

### TypeScript Express Adapter

**File:** `packages/middleware/express/src/express.ts`
**Public API:**
```typescript
export function createMcpExpressApp(options?: CreateMcpExpressAppOptions): Express;

export interface CreateMcpExpressAppOptions {
    host?: string;           // Default: '127.0.0.1'
    allowedHosts?: string[]; // Custom allowed hostnames
    jsonLimit?: string;      // Body size limit (e.g., '1mb')
}
```

**Exports also include** (from index/middleware):
- `hostHeaderValidation(allowedHostnames: string[]): RequestHandler`
- `localhostHostValidation(): RequestHandler`

**Behavior:**
- When `host` is `127.0.0.1`, `localhost`, or `::1` (default): auto-applies DNS rebinding protection
- When `host` is `0.0.0.0` or `::`: warns on console, no protection unless `allowedHosts` provided
- When `allowedHosts` provided: uses custom host validation regardless of host
- Always applies `express.json()` middleware with optional `limit`

### TypeScript Hono Adapter

**File:** `packages/middleware/hono/src/hono.ts`
**Public API:**
```typescript
export function createMcpHonoApp(options?: CreateMcpHonoAppOptions): Hono;

export interface CreateMcpHonoAppOptions {
    host?: string;           // Default: '127.0.0.1'
    allowedHosts?: string[]; // Custom allowed hostnames
}
```

**No `jsonLimit` option** (Hono handles body parsing differently).
**Additional behavior:** Custom JSON body parsing middleware that stashes parsed body into `c.set('parsedBody', ...)` (hono.ts:47-68).

### TypeScript Node Transport Wrapper

**File:** `packages/middleware/node/src/streamableHttp.ts`
**Public API:**
```typescript
export class NodeStreamableHTTPServerTransport implements Transport {
    constructor(options?: StreamableHTTPServerTransportOptions);
    get sessionId(): string | undefined;
    handleRequest(req: IncomingMessage, res: ServerResponse, parsedBody?: unknown): Promise<void>;
    // ... Transport interface methods (send, close, onmessage, etc.)
}
```

**Key design:** Wraps `WebStandardStreamableHTTPServerTransport` (from `@modelcontextprotocol/server`). Uses `@hono/node-server`'s `getRequestListener` to convert Node.js HTTP to Web Standard `Request`/`Response`. Supports `parsedBody` passthrough for Express integration.

### DNS Rebinding Protection Logic

**Core validation function** -- `packages/server/src/server/middleware/hostHeaderValidation.ts`:

```typescript
export function validateHostHeader(
    hostHeader: string | null | undefined,
    allowedHostnames: string[]
): HostHeaderValidationResult;
```

**Algorithm:**
1. If `hostHeader` is missing: return `{ ok: false, errorCode: 'missing_host' }`
2. Parse hostname using `new URL(`http://${hostHeader}`).hostname` (handles IPv4, IPv6 with brackets, port stripping)
3. Check `allowedHostnames.includes(hostname)` -- simple string match
4. If not in list: return `{ ok: false, errorCode: 'invalid_host' }`
5. Otherwise: return `{ ok: true, hostname }`

**Default localhost allowlist:** `['localhost', '127.0.0.1', '[::1]']`

**Error response format** (JSON-RPC compliant):
```json
{ "jsonrpc": "2.0", "error": { "code": -32000, "message": "..." }, "id": null }
```

Returns HTTP 403 status.

### Rust Equivalent Requirements

**Tower middleware stack needed:**

| Layer | TypeScript Equivalent | Rust Crate |
|-------|----------------------|------------|
| DNS rebinding protection | `hostHeaderValidation` | Custom `tower::Layer` |
| Host header validation | `validateHostHeader` | Same layer, configurable |
| JSON body parsing | `express.json()` / Hono middleware | Axum built-in |
| Session management | `NodeStreamableHTTPServerTransport` | Already in Rust (inline Axum) |

**Recommended Rust crate dependencies:**
- `tower` 0.5 / `tower-service` 0.3 / `tower-layer` 0.3 -- for Layer/Service abstractions
- `axum` 0.8 -- already a dependency for `streamable-http` feature
- No need for separate Actix-web adapter initially

**Key pattern to replicate:** The TypeScript middleware packages are very thin (Express: 88 lines, Hono: 91 lines, Node: ~200 lines). The core logic is in the shared `validateHostHeader` function. Rust equivalent would be a single Tower Layer (~100 lines) with an Axum convenience function (~30 lines).

---

## Domain 6: Package Structure

### Rust Feature Flags (Cargo.toml:134-155)

```
default = ["logging"]
full = ["websocket", "http", "streamable-http", "sse", "validation",
        "resource-watcher", "rayon", "schema-generation", "jwt-auth",
        "composition", "mcp-apps", "http-client", "logging"]
composition = ["streamable-http"]
mcp-apps = []
jwt-auth = ["http-client", "dep:jsonwebtoken"]
http-client = ["dep:reqwest"]
logging = ["dep:tracing-subscriber"]
oauth = ["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]
sse = ["http-client", "dep:bytes"]
websocket = ["dep:tokio-tungstenite"]
http = ["dep:hyper", "dep:hyper-util", "dep:bytes"]
streamable-http = ["dep:hyper", "dep:hyper-util", "dep:hyper-rustls",
                   "dep:rustls", "dep:futures-util", "dep:bytes", "dep:axum"]
validation = ["dep:jsonschema", "dep:garde"]
resource-watcher = ["dep:notify", "dep:glob-match"]
schema-generation = ["dep:schemars"]
wasm = ["websocket-wasm", "uuid/js", "dep:futures-channel", "dep:futures-locks"]
websocket-wasm = []
```

### TypeScript Exports That Would Require New Feature Flags

| TypeScript Export | Rust Equivalent | New Feature Flag? |
|-------------------|----------------|-------------------|
| `@modelcontextprotocol/express` | Tower middleware + Axum adapter | `tower-middleware` or new crate `pmcp-tower` |
| `@modelcontextprotocol/hono` | N/A (Rust doesn't have Hono) | No |
| `@modelcontextprotocol/node` | Already covered by `streamable-http` | No |
| Conformance CLI | `mcp-tester conformance` command | No (separate binary crate) |
| `experimental/tasks` | `pmcp-tasks` crate | No (already separate) |

### Assessment

Feature flags approach is sufficient. The only new Rust artifact would be either:
1. A `tower-middleware` feature flag on `pmcp` (adds `tower`, `tower-layer`, `tower-service` deps), or
2. A separate `pmcp-tower` crate (cleaner isolation for framework-specific deps)

**Recommendation:** Separate `pmcp-tower` crate, following the TypeScript pattern of separate middleware packages. This avoids adding `tower` as a dependency to the core `pmcp` crate for users who don't need it.

---

## Surprises (Findings Not in RESEARCH.md)

### Surprise 1: `add_numbers` tool comes from conformance CLI, not SDK
The `everythingClient.ts` calls `add_numbers` tool in the `tools_call` scenario (:131), but this tool is **not registered** in `everythingServer.ts`. It must be provided by the conformance CLI runner itself when testing servers. This means the conformance CLI has its own set of expected tools/behaviors -- the "everything server" is for testing that a server correctly implements all MCP features, while the conformance CLI tests against its own expectations.

### Surprise 2: TypeScript ToolSchema has `title` on BaseMetadata, not just annotations
The TypeScript `ToolSchema` extends `BaseMetadataSchema` which has `title` as a direct field (types.ts:392-404), separate from `ToolAnnotations.title`. This means TypeScript tools have TWO title locations: `tool.title` (from BaseMetadata) and `tool.annotations.title`. The Rust SDK only has `ToolAnnotations.title`. This is a spec alignment issue -- the 2025-11-25 spec added `title` to the base metadata for all entity types.

### Surprise 3: Elicitation capabilities significantly expanded
RESEARCH.md mentions elicitation but doesn't detail the capability expansion. TypeScript's `ElicitationCapabilitySchema` (types.ts:429-450) now has `{ form?: { applyDefaults? }, url? }` -- supporting both form-based and URL-based elicitation modes. Rust's `ElicitationCapabilities` is an empty struct (capabilities.rs:130-134).

### Surprise 4: Sampling capabilities expanded with `context` and `tools`
The TypeScript `ClientCapabilitiesSchema.sampling` (types.ts:529-541) now has `{ context?, tools? }` sub-capabilities, supporting context inclusion and tool use in sampling requests. Rust's `SamplingCapabilities` only has `models: Option<Vec<String>>` (capabilities.rs:119-123).

### Surprise 5: `ResourceLink` is a new content type
TypeScript has `ResourceLinkSchema` (types.ts:1296-1298) extending `ResourceSchema` with `type: 'resource_link'`. This is different from `EmbeddedResource` (`type: 'resource'`) -- it's a link to a resource rather than an inline embed. Rust doesn't have this content variant.

### Surprise 6: SEP-specific test tools
The conformance server includes tools for specific Specification Enhancement Proposals (SEPs):
- SEP-1034: Elicitation defaults
- SEP-1330: Enum schema improvements (5 enum variants: untitled single, titled single via oneOf, legacy enumNames, untitled multi, titled multi via anyOf)
- SEP-1613: JSON Schema 2020-12 conformance
- SEP-1699: SSE reconnection/resumability

These SEPs represent specific spec evolution that Rust should track.

### Surprise 7: TypeScript conformance server creates per-session server instances
The `everythingServer.ts` creates a new `McpServer` instance per session (:68-90, :904), not a single shared server. Each session gets its own state. This is significant for conformance testing -- it means the server must handle session lifecycle correctly.

### Surprise 8: `ImplementationSchema` now has `websiteUrl` and `description`
TypeScript's `ImplementationSchema` (types.ts:410-427) extends BaseMetadata with `{ version, websiteUrl?, description?, icons? }`. Rust's `Implementation` struct (protocol.rs:37-43) only has `{ name, version }`. Missing: `title`, `websiteUrl`, `description`, `icons`.

### Surprise 9: TypeScript removed server-side auth but SDK has conformance auth scenarios
Despite TypeScript v2 removing server-side auth from the SDK itself, the conformance test infrastructure has 18 auth scenarios including OAuth, CIMD, client credentials (JWT and basic), cross-app access, and pre-registration. These auth patterns are tested at the protocol level, not as SDK features. Rust's server-side auth (JWT, Cognito, OIDC) remains a practical differentiator.

### Surprise 10: `ToolResultContent` for sampling with tools
TypeScript has `ToolResultContentSchema` (types.ts ~1700-1737) for including tool results in sampling responses, and `ToolUseContent` for tool calls within sampling. Combined with the sampling capability expansion (`tools` sub-capability), this enables a complete tool-use loop within sampling. Rust doesn't have these content types or capabilities.
