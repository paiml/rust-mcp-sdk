# Gap Analysis: TypeScript MCP SDK v2 vs Rust PMCP SDK v1.20.0

**Date:** 2026-03-19
**Analyzed by:** Claude (automated analysis)
**TypeScript SDK:** v2.0.0-alpha.0 (@modelcontextprotocol/core, server, client, middleware)
**Rust SDK:** v1.20.0 (pmcp crate with feature flags)

## Executive Summary

This report compares the TypeScript MCP SDK v2.0.0-alpha.0 against the Rust PMCP SDK v1.20.0 across six domains: protocol version negotiation, conformance testing, MCP Apps, Tasks, framework adapters, and package structure. The Rust SDK leads significantly in MCP Apps (full adapter pipeline, preview server, validation tooling) and Tasks backends (DynamoDB, Redis, task variables), while the TypeScript SDK leads in protocol currency (supports 2025-11-25), conformance testing (dedicated CLI with 37+ test scenarios), and framework middleware (Express, Hono, Node adapters with DNS rebinding protection). The primary recommendation is to update protocol version support to 2025-11-25, build conformance test infrastructure in mcp-tester, and add Tower middleware for framework adapter support -- three focused phases that close the most impactful gaps.

## Methodology

- Source code comparison of both SDKs from local checkouts (TypeScript at `~/Development/mcp/sdk/typescript-sdk/`, Rust at working directory)
- 6 analysis domains derived from CONTEXT.md user decisions
- Every comparison point verified with file:line references (documented in 53-01-VERIFICATION-NOTES.md)
- Priority scoring: P0 (prerequisite for other work), P1 (high value, should implement next), P2 (medium value, plan for later), P3 (low priority or monitoring only)
- Effort: Low (< 1 phase, ~1-2 plans), Medium (1-2 phases), High (3+ phases)
- Value: Low (nice-to-have), Medium (improves interop or DX), High (required for spec compliance or correctness)

## Gap Summary Table

| # | Domain | Gap | Priority | Effort | Value | Status |
|---|--------|-----|----------|--------|-------|--------|
| 1 | Protocol | Missing `2025-11-25` protocol version | P0 | Low | High | Rust behind |
| 2 | Protocol | `2024-11-05` and `2024-10-07` still in supported list | P0 | Low | Medium | Cleanup needed |
| 3 | Protocol | Missing `IconSchema` / `IconsSchema` on Implementation, Tool, Resource, Prompt | P1 | Low | Medium | Rust behind |
| 4 | Protocol | Missing `BaseMetadata.title` on Resource, Prompt, Implementation | P1 | Low | Medium | Rust behind |
| 5 | Protocol | Missing `Implementation.websiteUrl` and `Implementation.description` | P1 | Low | Low | Rust behind |
| 6 | Protocol | Missing `AudioContent` content type | P1 | Low | Medium | Rust behind |
| 7 | Protocol | Missing `ResourceLink` content type | P1 | Low | Medium | Rust behind |
| 8 | Protocol | Missing `ToolUseContent` content type | P2 | Low | Low | Rust behind |
| 9 | Protocol | Missing expanded `ElicitationCapabilities` (form/url) | P1 | Low | Medium | Rust behind |
| 10 | Protocol | Missing expanded `SamplingCapabilities` (context/tools) | P1 | Low | Medium | Rust behind |
| 11 | Conformance | No conformance test command in mcp-tester | P1 | Medium | High | Rust behind |
| 12 | Conformance | No "everything server" reference implementation | P1 | Medium | Medium | Rust behind |
| 13 | Conformance | No expected-failures tracking | P2 | Low | Low | Rust behind |
| 14 | MCP Apps | Full UI adapter pipeline | P3 | N/A | N/A | Rust ahead |
| 15 | MCP Apps | Preview server with multi-host emulation | P3 | N/A | N/A | Rust ahead |
| 16 | MCP Apps | Apps validation in mcp-tester | P3 | N/A | N/A | Rust ahead |
| 17 | MCP Apps | Widget runtime with AppBridge | P3 | N/A | N/A | Rust ahead |
| 18 | MCP Apps | Missing `_meta` on AudioContent/ToolUseContent (when added) | P2 | Low | Low | Future gap |
| 19 | MCP Apps | Missing `icons` field on Tool, Resource, Prompt | P1 | Low | Medium | Rust behind |
| 20 | Tasks | Missing `ServerCapabilities.tasks` field | P0 | Low | High | Rust behind |
| 21 | Tasks | Missing `ClientCapabilities.tasks` field | P0 | Low | High | Rust behind |
| 22 | Tasks | Missing `Task.pollInterval` field | P1 | Low | Medium | Rust behind |
| 23 | Tasks | Missing `Task.statusMessage` field | P1 | Low | Medium | Rust behind |
| 24 | Tasks | Missing `notifications/tasks/status` notification | P1 | Low | Medium | Rust behind |
| 25 | Tasks | Missing `TaskMessageQueue` for async polling | P2 | Medium | Medium | Rust behind |
| 26 | Tasks | Missing `RELATED_TASK_META_KEY` constant | P1 | Low | Low | Rust behind |
| 27 | Tasks | Missing `TaskAugmentedRequestParams` | P1 | Low | Medium | Rust behind |
| 28 | Tasks | Missing `CreateTaskResult` wrapper | P1 | Low | Low | Rust behind |
| 29 | Tasks | DynamoDB/Redis backends | P3 | N/A | N/A | Rust ahead |
| 30 | Tasks | Task variables (PMCP extension) | P3 | N/A | N/A | Rust ahead |
| 31 | Tasks | Owner binding (OAuth sub/client ID) | P3 | N/A | N/A | Rust ahead |
| 32 | Framework | No Tower middleware layer | P2 | High | High | Rust behind |
| 33 | Framework | No DNS rebinding protection | P2 | Medium | High | Rust behind |
| 34 | Framework | No host header validation | P2 | Low | Medium | Rust behind |
| 35 | Package | Feature flags vs crate split | N/A | None | N/A | No change needed |

## Domain Analysis

### Domain 1: Protocol Version Negotiation

#### Current State

| Aspect | TypeScript SDK v2 | Rust SDK v1.20.0 |
|--------|-------------------|------------------|
| LATEST_PROTOCOL_VERSION | `2025-11-25` | `2025-06-18` |
| DEFAULT_NEGOTIATED_VERSION | `2025-03-26` | `2025-03-26` |
| Supported versions | `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`, `2024-10-07` | `2025-06-18`, `2025-03-26`, `2024-11-05`, `2024-10-07` |
| Fallback behavior | First element of supported list | Fixed `DEFAULT_PROTOCOL_VERSION` constant |
| Per-server configuration | Yes (via `ProtocolOptions`) | No (constants only) |
| Draft spec tracking | `DRAFT-2026-v1` in `spec.types.ts` | None |

**Source refs:** TypeScript `packages/core/src/types/types.ts:3-5`, Rust `src/lib.rs:263-323`

#### Gaps

**Gap 1 (P0): Missing 2025-11-25 protocol version**
Rust's `LATEST_PROTOCOL_VERSION` is `"2025-06-18"` while TypeScript uses `"2025-11-25"`. The 2025-11-25 version introduces 20+ new types and fields including `TaskSchema`, `IconSchema`, `AudioContent`, `ResourceLink`, `ToolUseContent`, expanded capabilities for elicitation and sampling, and task capability negotiation. This is a prerequisite for all Tasks interop and spec compliance work.

**Gap 2 (P0): Stale 2024 versions in supported list**
Per the locked decision to support only the latest 3 versions, `2024-11-05` and `2024-10-07` must be removed from `SUPPORTED_PROTOCOL_VERSIONS` and replaced with `2025-11-25`.

**Gap 3 (P1): Missing IconSchema / IconsSchema**
TypeScript's 2025-11-25 spec adds `icons: Icon[]` to Implementation, Tool, Prompt, Resource, and ResourceTemplate. Each `Icon` has `{ src, mimeType?, sizes?, theme? }` where theme is `'light' | 'dark'`. Rust has none of these fields.

**Gap 4 (P1): Missing BaseMetadata.title**
The 2025-11-25 spec adds a `title` field to a shared `BaseMetadata` type used by Tool, Prompt, Resource, and Implementation. Rust's `ToolAnnotations` has `title`, but `ResourceInfo`, `PromptInfo`, and `Implementation` do not have a top-level `title`. This means TypeScript tools have two title locations (`tool.title` from BaseMetadata and `tool.annotations.title`).

**Gap 5 (P1): Missing Implementation expansion**
TypeScript's `Implementation` struct now has `{ name, version, title?, websiteUrl?, description?, icons? }`. Rust's `Implementation` only has `{ name, version }`.

**Gap 6-8 (P1-P2): Missing content types**
Three new content types in 2025-11-25:
- `AudioContent` (`type: 'audio'`, with `data`, `mimeType`, `annotations?`, `_meta?`) -- P1, needed for multi-modal servers
- `ResourceLink` (`type: 'resource_link'`, extends Resource) -- P1, different from `EmbeddedResource` (`type: 'resource'`), represents a link rather than inline embed
- `ToolUseContent` (`type: 'tool_use'`, with `name`, `id`, `input`) -- P2, used in sampling with tools

**Gap 9-10 (P1): Expanded capability schemas**
- `ElicitationCapabilities` expanded to `{ form?: { applyDefaults? }, url? }` -- Rust has empty struct
- `SamplingCapabilities` expanded with `context?` and `tools?` sub-capabilities -- Rust only has `models`

#### Recommendations

1. **Add `2025-11-25` as LATEST_PROTOCOL_VERSION** and update the supported list to `["2025-11-25", "2025-06-18", "2025-03-26"]`, dropping 2024 versions
2. **Add all 2025-11-25 types** in a single phase: IconSchema, AudioContent, ResourceLink, ToolUseContent, expanded Implementation, BaseMetadata.title, expanded ElicitationCapabilities, expanded SamplingCapabilities
3. **Keep the constant-based negotiation approach** -- per-server configuration adds complexity without clear benefit for the Rust SDK's opinionated design

### Domain 2: Conformance Test Suite

#### Current State

| Aspect | TypeScript SDK v2 | Rust SDK v1.20.0 |
|--------|-------------------|------------------|
| Conformance CLI | `@modelcontextprotocol/conformance` (external package) | None |
| Reference server | `everythingServer.ts` (1,014 lines, all MCP features) | None |
| Reference client | `everythingClient.ts` (495 lines, 23 scenarios) | None |
| Expected failures tracking | `expected-failures.yaml` | None |
| Server test tools | 14 tools across 8 categories | N/A |
| Server test resources | 4 resources (static, binary, template, watched) | N/A |
| Server test prompts | 4 prompts (simple, args, embedded resource, image) | N/A |
| Client test scenarios | 23 (2 core + 18 auth + 1 elicitation + 1 transport + 1 pre-reg) | N/A |
| Transport | Express + StreamableHTTP + SSE resumability | Basic connectivity check |

#### Conformance Scenario Catalog

The TypeScript conformance infrastructure provides a comprehensive test surface. These are the scenarios that `mcp-tester conformance` should target:

**Server-Side Tools (14 total):**

| # | Tool | Category | Tests |
|---|------|----------|-------|
| 1 | `test_simple_text` | Content | Simple text response |
| 2 | `test_image_content` | Content | Base64 PNG image |
| 3 | `test_audio_content` | Content | Base64 WAV audio |
| 4 | `test_embedded_resource` | Content | Embedded resource content |
| 5 | `test_multiple_content_types` | Content | Mixed text + image + resource |
| 6 | `test_tool_with_logging` | Logging | Log notifications during execution |
| 7 | `test_tool_with_progress` | Progress | Progress notifications (0/50/100) |
| 8 | `test_error_handling` | Error | Intentional error throwing |
| 9 | `test_reconnection` | Transport | SSE stream closure + reconnection (SEP-1699) |
| 10 | `test_sampling` | Sampling | Server-initiated createMessage |
| 11 | `test_elicitation` | Elicitation | Server-initiated user input |
| 12 | `test_elicitation_sep1034_defaults` | Elicitation | Default values (SEP-1034) |
| 13 | `test_elicitation_sep1330_enums` | Elicitation | Enum schema variants (SEP-1330) |
| 14 | `json_schema_2020_12_tool` | Schema | JSON Schema 2020-12 (SEP-1613) |

**Server-Side Resources (4 total):**

| # | Resource | URI | Tests |
|---|----------|-----|-------|
| 1 | `static-text` | `test://static-text` | Static text content |
| 2 | `static-binary` | `test://static-binary` | Binary content (base64 PNG) |
| 3 | `template` | `test://template/{id}/data` | URI template expansion |
| 4 | `watched-resource` | `test://watched-resource` | Subscribe/notification |

**Server-Side Prompts (4 total):**

| # | Prompt | Tests |
|---|--------|-------|
| 1 | `test_simple_prompt` | Basic prompt listing and retrieval |
| 2 | `test_prompt_with_arguments` | Prompt with arg1, arg2 |
| 3 | `test_prompt_with_embedded_resource` | Prompt returning embedded resource |
| 4 | `test_prompt_with_image` | Prompt returning image content |

**Client-Side Scenarios (23 total):**

| Category | Count | Scenarios |
|----------|-------|-----------|
| Core | 2 | `initialize`, `tools_call` |
| Auth | 18 | `auth/basic-cimd`, `auth/metadata-default`, `auth/metadata-var1-3`, `auth/2025-03-26-oauth-*`, `auth/scope-*`, `auth/token-endpoint-auth-*`, `auth/client-credentials-*`, `auth/cross-app-access`, `auth/pre-registration` |
| Elicitation | 1 | `elicitation-sep1034-client-defaults` |
| Transport | 1 | `sse-retry` |
| Pre-registration | 1 | `auth/pre-registration` |

#### Gaps

**Gap 11 (P1): No conformance test command**
mcp-tester has `check`, `run`, `apps`, and `compliance` but no dedicated conformance scenario runner. The locked decision calls for `mcp-tester conformance <url>` that exercises all MCP features against any server.

**Gap 12 (P1): No reference implementation**
TypeScript's `everythingServer.ts` implements all MCP features in a single test server. This pattern is valuable for both conformance testing and as developer documentation.

**Gap 13 (P2): No expected-failures tracking**
TypeScript uses `expected-failures.yaml` to track known failures during incremental rollout. This pattern allows shipping partial conformance without blocking on edge cases.

#### Recommendations

1. **Add `mcp-tester conformance <url>` command** with a `ConformanceScenario` trait and scenario registry. Start with core protocol (initialize, version negotiation, capability exchange) and tools (list, call, content types, error handling).
2. **Port scenarios in priority order:** Core (initialize, tools_call) > Content types (text, image, resource) > Resources (list, read, template, subscribe) > Prompts > Progress/Logging > Transport (SSE reconnection) > Elicitation > Sampling. Auth scenarios are lower priority since Rust has its own auth implementation.
3. **Implement expected-failures tracking** using a TOML file pattern (aligns with Rust ecosystem conventions) to allow incremental rollout.
4. **Use structural matching** (required fields present) not exact matching to avoid fragility with servers that include extension fields.
5. **Output JSON results** with pass/fail/skip per scenario, aligning with mcp-tester's existing output patterns.

### Domain 3: MCP Apps / UI Extensions

#### Current State

| Aspect | TypeScript SDK v2 | Rust SDK v1.20.0 |
|--------|-------------------|------------------|
| `outputSchema` on Tool | Yes (Zod schema) | Yes (serde_json::Value) |
| `structuredContent` on CallToolResult | Yes (Zod schema) | Yes (builder pattern) |
| `_meta` on all major types | Yes | Yes |
| UI adapters | None | 3 adapters (Standard, ChatGPT, ClaudeDesktop) |
| Widget runtime | None | Full runtime with AppBridge |
| Host metadata enrichment | None | Standard `ui.resourceUri` + ChatGPT `openai/outputTemplate` + legacy flat keys |
| Preview server | None | mcp-preview crate with multi-host emulation |
| Apps validation | None | mcp-tester `apps` command |
| Builder DX | Wire types only | `with_ui()`, `TypedTool`, `TypedToolWithOutput`, `TypedSyncTool`, `WasmTypedTool` |
| ext-apps SDK alignment | Wire types only | Full v1.2.2 protocol alignment |
| Deep merge for _meta | N/A (no adapters) | Yes, in `ui.rs` |
| URI-to-tool-meta index | N/A | Yes, on ServerCore |

#### Assessment: Rust Ahead

The Rust SDK is substantially ahead in MCP Apps. TypeScript provides only the wire-level types (`outputSchema`, `structuredContent`, `_meta` fields) while Rust provides the complete developer experience stack: UI adapters, widget runtime with AppBridge, preview server with ChatGPT/Claude Desktop emulation, validation tooling, builder APIs, deep merge for metadata collision prevention, and automatic URI-to-tool-meta indexing.

**Items to monitor:**
- When TypeScript adds UI adapter patterns, align the API surface where it makes sense
- TypeScript's `_meta` on `AudioContent` and `ToolUseContent` -- when those content types are added to Rust, include `_meta` fields
- The `icons` field on Tool, Resource, Prompt (TypeScript's `IconsSchema`) should be added to Rust types as part of the 2025-11-25 protocol update (Gap 19)

#### Recommendations

1. **No action needed** to catch up with TypeScript -- Rust is ahead
2. **Add `icons` field** to `ToolInfo`, `ResourceInfo`, `PromptInfo`, and `Implementation` as part of the protocol version update (shared with Domain 1 recommendations)
3. **Monitor TypeScript SDK** for official UI adapter patterns; adapt if Anthropic standardizes a host adapter API
4. **Ensure `_meta` is included** on any new content types (AudioContent, ResourceLink, ToolUseContent) when they are implemented

### Domain 4: Tasks

#### Current State

**Field-by-Field Task Type Comparison:**

| Field | TypeScript `TaskSchema` | Rust `TaskRecord` | Match? |
|-------|------------------------|-------------------|--------|
| `taskId` / `id` | `z.string()` | `id: String` | Yes (different field name) |
| `status` | 5 values: working, input_required, completed, failed, cancelled | 5 identical values | Yes |
| `ttl` | `z.union([z.number(), z.null()])` | `ttl: Option<u64>` | Yes |
| `createdAt` | ISO 8601 string | `DateTime<Utc>` | Yes |
| `lastUpdatedAt` | ISO 8601 string | `DateTime<Utc>` | Yes |
| `pollInterval` | `z.optional(z.number())` | Not present | **Missing** |
| `statusMessage` | `z.optional(z.string())` | Not present | **Missing** |

**Capability Negotiation Comparison:**

| Capability | TypeScript | Rust | Match? |
|------------|-----------|------|--------|
| `ServerCapabilities.tasks` | `{ list?, cancel?, requests?: { tools?: { call? } } }` | Field does not exist | **Missing** |
| `ClientCapabilities.tasks` | `{ list?, cancel?, requests?: { sampling?: { createMessage? }, elicitation?: { create? } } }` | Field does not exist | **Missing** |

**Store / Backend Comparison:**

| Aspect | TypeScript | Rust | Assessment |
|--------|-----------|------|------------|
| In-memory store | Yes (with TTL cleanup) | Yes (InMemoryBackend) | Parity |
| DynamoDB store | No | Yes | **Rust ahead** |
| Redis store | No | Yes | **Rust ahead** |
| Task variables | No | Yes (TaskWithVariables) | **Rust ahead** |
| Owner binding | Session isolation only | OAuth sub/client ID | **Rust ahead** |
| TaskMessageQueue | Yes (side-channel delivery) | No | **TypeScript ahead** |
| AsyncGenerator streaming | Yes (requestStream, createMessageStream) | No | **TypeScript ahead** |

#### Gaps

**Gap 20-21 (P0): Missing task capability negotiation**
`ServerCapabilities` and `ClientCapabilities` both need a `tasks` field for proper capability negotiation. Without these, a client cannot discover whether a server supports task operations, and a server cannot determine if a client can handle task-augmented responses. This is a prerequisite for any Tasks interop.

**Gap 22-23 (P1): Missing pollInterval and statusMessage fields**
`pollInterval` tells clients how frequently to poll for task updates. `statusMessage` provides human-readable status text. Both are needed for interop with TypeScript clients.

**Gap 24 (P1): Missing notifications/tasks/status**
TypeScript defines `TaskStatusNotificationSchema` for `notifications/tasks/status`. This enables real-time task status updates without polling.

**Gap 25 (P2): Missing TaskMessageQueue**
TypeScript's `TaskMessageQueue` enables side-channel message delivery during long-running tasks (e.g., server can request sampling or elicitation while a task is in progress). Rust's `RequestHandlerExtra` provides real-time access during synchronous execution, so the queue is only needed for truly async (polled) tasks.

**Gap 26-28 (P1): Missing wire-level types**
`RELATED_TASK_META_KEY` (`io.modelcontextprotocol/related-task`), `TaskAugmentedRequestParams` (task field on request params), and `CreateTaskResult` (response wrapper) are needed for wire-level interop.

#### Recommendations

1. **Add `tasks` field to `ServerCapabilities` and `ClientCapabilities`** with the full sub-capability structure (list, cancel, requests). This is P0 and must ship with the protocol version update.
2. **Add `pollInterval` and `statusMessage` to TaskRecord** and update all store implementations. These are straightforward additions to existing types.
3. **Add `notifications/tasks/status` notification type** and wire it through the notification dispatch system.
4. **Add `RELATED_TASK_META_KEY`, `TaskAugmentedRequestParams`, and `CreateTaskResult`** as protocol types for wire-level interop.
5. **Defer `TaskMessageQueue`** to a later phase -- it requires architectural design work and is only needed for async polled tasks, which are not yet a common pattern in Rust servers.
6. **Keep task variables and DynamoDB/Redis backends** as PMCP differentiators. These are practical features that TypeScript does not have.

### Domain 5: Framework Adapters

#### Current State

| Aspect | TypeScript SDK v2 | Rust SDK v1.20.0 |
|--------|-------------------|------------------|
| Express adapter | `createMcpExpressApp(options)` (88 lines) | N/A |
| Hono adapter | `createMcpHonoApp(options)` (91 lines) | N/A |
| Node transport wrapper | `NodeStreamableHTTPServerTransport` (~200 lines) | N/A |
| DNS rebinding protection | Auto-enabled for localhost binds | None |
| Host header validation | `validateHostHeader()` with configurable allowlist | None |
| Transport abstraction | `WebStandardStreamableHTTPServerTransport` | Axum-specific inline routes |
| Body parsing | Middleware-handled | Framework-specific |

**TypeScript DNS Rebinding Protection Algorithm:**
1. If `hostHeader` is missing: reject with 403 (JSON-RPC error)
2. Parse hostname via `new URL(http://${hostHeader}).hostname`
3. Check `allowedHostnames.includes(hostname)` -- simple string match
4. Default localhost allowlist: `['localhost', '127.0.0.1', '[::1]']`
5. When binding to `0.0.0.0` or `::`: warn on console, no protection unless `allowedHosts` provided

**TypeScript Adapter Pattern:**
The adapters are thin convenience layers. The core logic is in `WebStandardStreamableHTTPServerTransport` (in `@modelcontextprotocol/server`). Express and Hono adapters create pre-configured framework apps with security middleware applied. The Node adapter converts Node.js IncomingMessage/ServerResponse to Web Standard Request/Response using `@hono/node-server`.

#### Gaps

**Gap 32 (P2): No Tower middleware layer**
Rust has no framework-agnostic middleware layer for MCP server hosting. The streamable HTTP implementation is inline Axum code. A Tower Layer/Service abstraction would enable MCP servers to be hosted in any Tower-compatible framework (Axum, Hyper, Tonic, Warp).

**Gap 33 (P2): No DNS rebinding protection**
MCP servers binding to localhost are vulnerable to DNS rebinding attacks without host header validation. This is a security-relevant gap.

**Gap 34 (P2): No host header validation**
No configurable host header validation middleware exists for production deployments.

#### Recommendations

1. **Design a Tower middleware stack** with three layers:
   - `DnsRebindingProtection` layer: validates Host header against allowlist, auto-enabled for localhost binds
   - `HostHeaderValidation` layer: configurable allowlist for production deployments
   - `McpSessionManagement` layer: session lifecycle management (already partially implemented in Axum routes)

2. **Build an Axum convenience function** on top of the Tower stack:
   ```rust
   pub fn mcp_router(server: ServerCore) -> Router {
       Router::new()
           .route("/mcp", post(handle_post).get(handle_sse).delete(handle_delete))
           .layer(DnsRebindingProtection::localhost())
           .with_state(server)
   }
   ```

3. **Ship Tower middleware as a separate crate** (`pmcp-tower` or `pmcp-middleware`) to avoid adding tower/axum as required dependencies for users who use other transports.

4. **Skip Actix-web adapter** initially -- Tower middleware works with Hyper which covers the majority of Rust HTTP server use cases.

5. **Port the DNS rebinding protection algorithm** from TypeScript's `validateHostHeader` function -- the logic is well-defined and security-critical.

### Domain 6: Package Structure

#### Current State

| Aspect | TypeScript SDK v2 | Rust SDK v1.20.0 |
|--------|-------------------|------------------|
| Architecture | 6 packages: core, server, client, express, hono, node | 1 main crate + 5 workspace crates |
| Modularity mechanism | npm package boundaries | Cargo feature flags |
| Tree-shaking equivalent | Package boundaries + bundler tree-shaking | Feature flags (compile-time) |
| Dependency isolation | Per-package `package.json` | Feature-gated `dep:` syntax |
| Version independence | Per-package semver | Workspace-level versioning |
| Split rationale | ESM/CJS resolution, bundler tree-shaking | N/A for Rust |

**Rust Workspace Structure:**
```
pmcp (v1.20.0)              -- Main crate, 15 feature flags
crates/
  pmcp-macros/              -- Derive macros
  mcp-tester/               -- Testing CLI tool
  mcp-preview/              -- Local preview server
  pmcp-tasks/               -- Experimental tasks (v0.1.0)
  pmcp-widget-utils/        -- Widget utility functions
  pmcp-server/              -- PMCP MCP Server
```

#### Assessment: No Changes Needed

The feature flags approach is correct for Rust. The TypeScript split was driven by ESM/CJS module resolution and bundler tree-shaking concerns that do not apply to Rust. Phase 52 already proved the feature flag approach works well, reducing transitive dependencies from 249 to 134 without any crate splitting.

The only structural addition needed is a new crate for framework middleware (`pmcp-tower` or `pmcp-middleware`), which follows the existing pattern of separate workspace crates for separate concerns.

#### Recommendations

1. **No package structure changes** -- the feature flags approach is validated and correct
2. **Add a `pmcp-tower` or `pmcp-middleware` crate** when implementing framework adapters (Domain 5) to isolate Tower/Axum dependencies
3. **Do not split `pmcp` into core/server/client** -- the feature flag approach already provides compile-time dependency selection

## Areas Where Rust Leads

The Rust SDK has significant advantages over the TypeScript SDK in several areas that are worth highlighting for positioning:

| Area | Rust Capability | TypeScript Equivalent |
|------|----------------|----------------------|
| MCP Apps UI Adapters | 3 adapters (Standard, ChatGPT, ClaudeDesktop) with automatic metadata enrichment | Wire types only (`outputSchema`, `structuredContent`, `_meta`) |
| Widget Runtime | Full AppBridge with postMessage protocol, method normalization, readiness signals | None |
| Preview Server | mcp-preview with multi-host emulation (standard, ChatGPT modes) and DevTools | None |
| Apps Validation | `mcp-tester apps` command with strict/permissive modes and host-specific checks | None |
| Builder DX | `with_ui()`, `TypedTool`, `TypedToolWithOutput`, `TypedSyncTool`, `WasmTypedTool` | Basic `registerTool()` only |
| Task Storage Backends | DynamoDB, Redis, In-Memory with pluggable store trait | In-Memory only |
| Task Variables | PMCP extension for shared client/server state during task execution | None |
| Task Security | Owner binding via OAuth subject/client ID with owner resolution from auth context | Session isolation only |
| Server-Side Auth | JWT validation, Cognito/OIDC providers, API key middleware | Removed in v2 (recommends external libs) |
| Deep Merge for _meta | Collision-safe deep merge in `ui.rs` preventing metadata loss | N/A (no metadata pipeline) |
| URI-to-Tool-Meta Index | Automatic resource enrichment from tool metadata at build time | N/A |
| Dependency Optimization | Feature flags reducing deps from 249 to 134 (Phase 52) | Package boundaries |
| ToolInfo Caching | Cached at registration, avoiding per-request cloning | Per-request metadata calls |
| Tool Execution Wire Type | `ToolInfo.execution` field supporting `taskSupport` enum | Equivalent (`ToolExecutionSchema`) |

**Server-side auth is a deliberate divergence.** TypeScript v2 removed server-side auth, recommending external auth libraries. Rust keeps JWT + Cognito/OIDC providers as a practical feature for enterprise deployments. This is a strength, not a gap.

## Proposed Implementation Phases

Based on priority scoring, the following phases close the most impactful gaps. Phase numbering continues from the current ROADMAP (Phase 53 is the last existing phase).

### Phase 54: Protocol Version 2025-11-25 Support (P0, Low effort)

**Goal:** Update the Rust SDK to support the 2025-11-25 MCP protocol version, adding all new types, content variants, expanded capabilities, and task capability negotiation fields.

**Scope:**
- Update `LATEST_PROTOCOL_VERSION` to `"2025-11-25"` and `SUPPORTED_PROTOCOL_VERSIONS` to `["2025-11-25", "2025-06-18", "2025-03-26"]`
- Add `IconSchema`, `AudioContent`, `ResourceLink`, `ToolUseContent` types
- Add `title` to `ResourceInfo`, `PromptInfo`, `Implementation`
- Add `websiteUrl`, `description`, `icons` to `Implementation`
- Add `icons` to `ToolInfo`, `ResourceInfo`, `PromptInfo`, `ResourceTemplateInfo`
- Expand `ElicitationCapabilities` with `form` and `url` fields
- Expand `SamplingCapabilities` with `context` and `tools` fields
- Add `tasks` field to `ServerCapabilities` and `ClientCapabilities`
- Add `pollInterval` and `statusMessage` to task types
- Add `notifications/tasks/status`, `RELATED_TASK_META_KEY`, `TaskAugmentedRequestParams`, `CreateTaskResult`
- Files affected: `src/lib.rs`, `src/types/protocol.rs`, `src/types/capabilities.rs`, `src/types/content.rs` (or equivalent), task-related type files
- Estimated: 2-3 plans

**Depends on:** Phase 53

### Phase 55: Conformance Test Infrastructure (P1, Medium effort)

**Goal:** Add a `mcp-tester conformance <url>` command that runs a suite of MCP spec compliance test scenarios against any MCP server, starting with core protocol and tools.

**Scope:**
- Add `ConformanceScenario` trait and scenario registry to mcp-tester
- Implement core scenarios: `initialize` (version negotiation, capability exchange), `tools_call` (list, call, content types, error handling)
- Implement resource scenarios: `resources_list`, `resources_read`, `resources_template`, `resources_subscribe`
- Implement prompt scenarios: `prompts_list`, `prompts_get`, `prompts_with_args`
- Add expected-failures TOML tracking
- Add JSON output format for CI integration
- Files affected: `crates/mcp-tester/src/conformance/`, `crates/mcp-tester/src/main.rs`
- Estimated: 2-3 plans

**Depends on:** Phase 54 (needs 2025-11-25 types for version negotiation testing)

### Phase 56: Tower Middleware and DNS Rebinding Protection (P2, Medium effort)

**Goal:** Build a Tower middleware stack for MCP server hosting with DNS rebinding protection, host header validation, and an Axum convenience adapter.

**Scope:**
- Create `crates/pmcp-middleware/` (or `pmcp-tower/`) workspace crate
- Implement `DnsRebindingProtection` Tower Layer with localhost auto-detection
- Implement `HostHeaderValidation` Tower Layer with configurable allowlist
- Build `mcp_router()` Axum convenience function
- Refactor existing inline Axum routes to use the middleware stack
- Files affected: new crate + `src/shared/streamable_http.rs` refactor
- Estimated: 2 plans

**Depends on:** Phase 54 (should use updated protocol types)

### Phase 57: Conformance Test Extension -- Advanced Scenarios (P2, Medium effort)

**Goal:** Extend the conformance test suite with progress/logging, transport, elicitation, and sampling scenarios.

**Scope:**
- Progress notification scenarios
- Logging level management scenarios
- SSE reconnection / resumability scenarios (SEP-1699)
- Elicitation scenarios (form-based, defaults per SEP-1034)
- Sampling scenarios (server-initiated createMessage)
- Completion scenarios
- Files affected: `crates/mcp-tester/src/conformance/scenarios/`
- Estimated: 2 plans

**Depends on:** Phase 55

## Deferred (Out of Scope)

The following items were identified during analysis but are explicitly out of scope per CONTEXT.md deferred decisions:

- **WebSocket transport (full bidirectional)** -- TypeScript has client-side WebSocket. Rust has server-side WebSocket (`websocket` feature) but no full bidirectional WebSocket transport for streamable HTTP. Deferred per CONTEXT.md.
- **Cross-runtime WASM support** -- TypeScript targets Cloudflare Workers, Deno, and Bun via Web Standard APIs. Rust has a `wasm` feature flag for browser clients but no cross-runtime target. Deferred per CONTEXT.md.
- **Auth conformance scenarios** -- TypeScript has 18 auth scenarios in its conformance suite (OAuth, CIMD, client credentials, cross-app access). Since Rust has its own auth implementation (JWT + Cognito/OIDC) that diverges from TypeScript's approach, porting these scenarios is low priority. Auth testing should focus on Rust's own auth middleware.
- **TaskMessageQueue** -- Side-channel message delivery for async polled tasks. Architectural design needed; defer until async task polling is a proven use case in the Rust ecosystem.
- **Per-server protocol version configuration** -- TypeScript allows overriding `supportedProtocolVersions` per server instance. The constant-based approach is simpler and adequate for the Rust SDK.
- **Draft spec tracking** -- TypeScript auto-generates types from `DRAFT-2026-v1` spec. Track manually when the draft stabilizes.

## Appendix: Source References

### TypeScript SDK (v2.0.0-alpha.0)

| File | Content |
|------|---------|
| `packages/core/src/types/types.ts` | Protocol types, version constants (lines 3-5), TaskSchema (743-764), IconSchema (344-387), AudioContent (1223-1244), ResourceLink (1296-1298), ToolUseContent (1250-1272), capabilities (429-560) |
| `packages/core/src/types/spec.types.ts` | Auto-generated draft spec types (3,247 lines) |
| `packages/core/src/experimental/tasks/interfaces.ts` | TaskStore (164-230), TaskMessageQueue (103-131), QueuedMessage union (54-85) |
| `packages/server/src/server/server.ts` | Server class, `_oninitialize` negotiation (431-447) |
| `packages/server/src/experimental/tasks/` | ExperimentalServerTasks, registerToolTask |
| `packages/middleware/express/src/express.ts` | Express adapter (88 lines) |
| `packages/middleware/hono/src/hono.ts` | Hono adapter (91 lines) |
| `packages/middleware/node/src/streamableHttp.ts` | Node transport wrapper (~200 lines) |
| `packages/server/src/server/middleware/hostHeaderValidation.ts` | DNS rebinding protection logic |
| `test/conformance/src/everythingServer.ts` | Reference server (1,014 lines, 14 tools, 4 resources, 4 prompts) |
| `test/conformance/src/everythingClient.ts` | Reference client (495 lines, 23 scenarios) |
| `test/conformance/expected-failures.yaml` | Known failures tracking |

### Rust SDK (v1.20.0)

| File | Content |
|------|---------|
| `src/lib.rs` | Version constants (263-323), `negotiate_protocol_version()` |
| `src/types/protocol.rs` | ToolInfo (with `output_schema`, `execution`, `_meta`), CallToolResult, Implementation |
| `src/types/capabilities.rs` | ServerCapabilities, ClientCapabilities (no `tasks` field) |
| `src/server/mcp_apps/` | UIAdapter trait, adapters, deep merge, metadata pipeline |
| `src/types/ui.rs` | deep_merge utility |
| `crates/pmcp-tasks/` | TaskRecord, TaskStore trait, backends (InMemory, DynamoDB, Redis) |
| `crates/mcp-tester/src/` | check, run, apps commands |
| `crates/mcp-preview/` | Preview server with multi-host emulation |
| `crates/pmcp-widget-utils/` | Widget utility functions, bridge script injection |
| `docs/design/tasks-feature-design.md` | Tasks feature design document |
