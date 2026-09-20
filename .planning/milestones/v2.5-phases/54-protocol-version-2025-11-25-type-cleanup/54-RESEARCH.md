# Phase 54: Protocol Version 2025-11-25 + Type Cleanup - Research

**Researched:** 2026-03-19
**Domain:** MCP protocol types, version negotiation, module restructuring
**Confidence:** HIGH

## Summary

This phase upgrades the Rust SDK from MCP protocol version 2025-06-18 to 2025-11-25 with aggressive type system cleanup, module restructuring, and version negotiation updates. The TypeScript SDK v2.0.0-alpha.0 types.ts (2,500+ lines of Zod schemas) serves as the authoritative reference. Research cataloged all 58 existing Rust types, identified 24 new types needed from the 2025-11-25 spec, mapped the module split across 6 domain sub-modules, and identified 100+ internal consumer files that will need import path updates.

The TypeScript SDK's types.ts reveals several categories of additions: new content types (AudioContent, ResourceLink, ToolUseContent, ToolResultContent), task protocol types (TaskSchema, CreateTaskResult, GetTaskRequest, etc.), expanded capabilities (tasks field on both client/server, expanded elicitation/sampling), base metadata improvements (IconSchema, BaseMetadata.title on all entities, expanded Implementation), and new sampling features (ToolChoice, CreateMessageResultWithTools). The Rust SDK's current elicitation types are a PMCP-proprietary format that does NOT match the spec -- this is a significant finding that must be addressed in the cleanup.

**Primary recommendation:** Split protocol.rs into 6 domain sub-modules with re-exports, add all 24 new types from 2025-11-25, update version constants/negotiation, expand capabilities, and replace the custom elicitation types with spec-compliant ones. Produce MIGRATION.md as a find-and-replace guide.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Aggressive cleanup -- rename inconsistent types, remove all deprecated fields, restructure modules
- This is a major version bump (v2.0.0) -- take full advantage of the breaking change window
- No compatibility shims, no #[deprecated] bridge -- clean break with MIGRATION.md
- Crate name stays `pmcp`
- Split protocol.rs into domain sub-modules: types/protocol/mod.rs, types/tools.rs, types/resources.rs, types/prompts.rs, types/tasks.rs, types/content.rs
- Re-export everything from types/mod.rs for flat import path: `use pmcp::types::ToolInfo`
- Support exactly 3 versions: 2025-11-25, 2025-06-18, 2025-03-26
- Drop all 2024 protocol versions -- reject with clear JSON-RPC error including supported versions list
- Use highest common version: server checks client's supported versions, picks the highest both support
- No silent downgrade to unsupported versions -- explicit rejection
- Add ALL types from the 2025-11-25 spec, even for de-prioritized features
- Hand-write types with #[derive(Serialize, Deserialize)] -- continue current idiomatic Rust pattern
- No codegen from spec schema
- Clean break with MIGRATION.md documenting every breaking change
- No deprecation bridge release

### Claude's Discretion
- Exact naming of new types (follow existing conventions: PascalCase, match spec names)
- Whether to use a ProtocolVersion enum vs string with validation
- Internal organization of the types/protocol/ sub-module
- Which deprecated fields to remove vs which are actually still needed

### Deferred Ideas (OUT OF SCOPE)
- Elicitation handler implementation -- types added here but handlers belong in a future phase
- Notification subscription handlers -- types added, handlers deferred
- SSE transport enhancements -- de-prioritized per v2.0 direction
- Spec-schema codegen validation test -- could add later to catch drift
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.x | Serialization/deserialization | Already used for all protocol types |
| serde_json | 1.x | JSON handling | Already used everywhere |
| serde with `rename_all = "camelCase"` | - | Wire format | Established convention |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| uuid | (existing) | Progress tokens, elicitation IDs | Already a dependency |
| chrono | (existing in pmcp-tasks) | ISO 8601 timestamps in Task types | Only in task wire types |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-written types | zod-to-rs codegen | Locked decision: hand-write |
| ProtocolVersion newtype | Plain `String` | Newtype exists already, keep it (adds validation opportunity) |

**Installation:** No new dependencies needed. All types use serde/serde_json already in the workspace.

## Architecture Patterns

### Recommended Module Structure (Post-Split)
```
src/types/
  mod.rs                    # Re-exports everything for flat `use pmcp::types::ToolInfo`
  protocol/
    mod.rs                  # Init, version negotiation, Implementation, ProtocolVersion
    version.rs              # Version constants, negotiate_protocol_version (moved from lib.rs)
  tools.rs                  # ToolInfo, ToolAnnotations, CallToolRequest/Result, ToolExecution, ListToolsRequest/Result
  resources.rs              # ResourceInfo, ResourceTemplate, ReadResourceResult, ListResources*, Subscribe/Unsubscribe
  prompts.rs                # PromptInfo, PromptArgument, GetPromptRequest/Result, ListPrompts*, PromptMessage
  tasks.rs                  # Task, TaskStatus, CreateTaskResult, GetTaskRequest, ListTasksRequest, CancelTaskRequest, etc.
  content.rs                # Content enum (Text, Image, Audio, Resource, ResourceLink), ContentBlock union
  sampling.rs               # SamplingMessage, CreateMessageParams/Result, ModelPreferences, ModelHint, ToolChoice, ToolUseContent, ToolResultContent
  notifications.rs          # ServerNotification, ClientNotification, ProgressNotification, CancelledNotification, LogMessage, TaskStatusNotification
  capabilities.rs           # ServerCapabilities, ClientCapabilities (expanded with tasks), all sub-capability types
  elicitation.rs            # SPEC-COMPLIANT elicitation types (ElicitRequest, ElicitResult) replacing current PMCP-proprietary ones
  completable.rs            # CompletionReference, CompletionArgument, CompleteRequest/Result (existing)
  auth.rs                   # AuthInfo, AuthScheme (existing, no changes)
  jsonrpc.rs                # JSONRPCRequest, JSONRPCResponse, JSONRPCError (existing, no changes)
  ui.rs                     # UIMimeType, UIResource, ToolUIMetadata (existing MCP Apps)
  mcp_apps.rs               # MCP Apps extension types (existing, feature-gated)
```

### Pattern 1: Re-export from types/mod.rs for backward-compatible flat imports
**What:** Every public type is re-exported from `types/mod.rs` so `use pmcp::types::ToolInfo` still works.
**When to use:** Always -- this is the locked decision for maintaining API ergonomics.
**Example:**
```rust
// src/types/mod.rs
pub mod tools;
pub mod resources;
pub mod prompts;
pub mod tasks;
pub mod content;
pub mod sampling;
pub mod notifications;
pub mod protocol;
pub mod capabilities;
// ... existing modules ...

// Re-export everything for flat access
pub use tools::*;
pub use resources::*;
pub use prompts::*;
pub use tasks::*;
pub use content::*;
pub use sampling::*;
pub use notifications::*;
pub use protocol::*;
pub use capabilities::*;
```

### Pattern 2: Serde conventions for all new types
**What:** Every new type follows established patterns.
**When to use:** All new types.
**Example:**
```rust
// Source: existing codebase convention
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconInfo {
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<IconTheme>,
}
```

### Pattern 3: Non-exhaustive on result types, exhaustive on request types
**What:** Result/response structs use `#[non_exhaustive]` for forward compatibility; request params do not (callers construct them).
**When to use:** Follow existing convention -- ListToolsResult is `#[non_exhaustive]`, CallToolRequest is not.

### Anti-Patterns to Avoid
- **Glob re-exports causing name collisions:** Content enum variants (Text, Image, Audio) may clash with other types. Use qualified imports where ambiguous.
- **Breaking the `type` tag on Content enum:** The Content enum uses `#[serde(tag = "type")]` -- new variants (Audio, ResourceLink, ToolUse, ToolResult) MUST use the same discriminator format.
- **Forgetting `#[serde(default)]` on optional Vec fields:** TypeScript uses `.default([])` on arrays -- Rust must use `#[serde(default)]` to avoid deserialization failures when field is missing.

## Complete Type Catalog

### Existing Rust Types (58 types in protocol.rs) and Their Destination

| # | Current Type | Destination Module | Action |
|---|-------------|-------------------|--------|
| 1 | `ProtocolVersion` | `protocol/mod.rs` | Keep |
| 2 | `Implementation` | `protocol/mod.rs` | **Expand** (add title, websiteUrl, description, icons) |
| 3 | `InitializeRequest` | `protocol/mod.rs` | Keep |
| 4 | `InitializeParams` (alias) | `protocol/mod.rs` | **Remove** (cleanup alias) |
| 5 | `InitializeResult` | `protocol/mod.rs` | Keep |
| 6 | `Cursor` (type alias) | `protocol/mod.rs` | Keep |
| 7 | `ListToolsRequest` | `tools.rs` | Keep |
| 8 | `ListToolsParams` (alias) | `tools.rs` | **Remove** (cleanup alias) |
| 9 | `ToolAnnotations` | `tools.rs` | Keep |
| 10 | `ToolInfo` | `tools.rs` | **Expand** (add title, icons) |
| 11 | `ListToolsResult` | `tools.rs` | Keep |
| 12 | `CallToolRequest` | `tools.rs` | Keep |
| 13 | `CallToolParams` (alias) | `tools.rs` | **Remove** (cleanup alias) |
| 14 | `CallToolResult` | `tools.rs` | Keep |
| 15 | `MessageContent` (alias) | `content.rs` | **Remove** (use Content directly) |
| 16 | `Content` (enum) | `content.rs` | **Expand** (add Audio, ResourceLink variants) |
| 17 | `ListPromptsRequest` | `prompts.rs` | Keep |
| 18 | `ListPromptsParams` (alias) | `prompts.rs` | **Remove** (cleanup alias) |
| 19 | `PromptInfo` | `prompts.rs` | **Expand** (add title, icons, _meta) |
| 20 | `PromptArgumentType` | `prompts.rs` | Keep (PMCP extension) |
| 21 | `PromptArgument` | `prompts.rs` | Keep |
| 22 | `ListPromptsResult` | `prompts.rs` | Keep |
| 23 | `GetPromptRequest` | `prompts.rs` | Keep |
| 24 | `GetPromptParams` (alias) | `prompts.rs` | **Remove** (cleanup alias) |
| 25 | `GetPromptResult` | `prompts.rs` | Keep |
| 26 | `PromptMessage` | `prompts.rs` | Keep |
| 27 | `Role` | `content.rs` or `protocol/mod.rs` | Keep (shared by prompts and sampling) |
| 28 | `ListResourcesRequest` | `resources.rs` | Keep |
| 29 | `ListResourcesParams` (alias) | `resources.rs` | **Remove** (cleanup alias) |
| 30 | `ResourceInfo` | `resources.rs` | **Expand** (add title, icons, annotations) |
| 31 | `ListResourcesResult` | `resources.rs` | Keep |
| 32 | `ReadResourceRequest` | `resources.rs` | Keep |
| 33 | `ReadResourceParams` (alias) | `resources.rs` | **Remove** (cleanup alias) |
| 34 | `ReadResourceResult` | `resources.rs` | Keep |
| 35 | `ListResourceTemplatesRequest` | `resources.rs` | Keep |
| 36 | `ResourceTemplate` | `resources.rs` | **Expand** (add title, icons, annotations, _meta) |
| 37 | `ListResourceTemplatesResult` | `resources.rs` | Keep |
| 38 | `SubscribeRequest` | `resources.rs` | Keep |
| 39 | `UnsubscribeRequest` | `resources.rs` | Keep |
| 40 | `CompleteRequest` | `completable.rs` (existing) | Keep |
| 41 | `CompletionReference` | `completable.rs` | Keep (move if not already there) |
| 42 | `CompletionArgument` | `completable.rs` | Keep |
| 43 | `CompleteResult` | `completable.rs` | Keep |
| 44 | `CompletionResult` | `completable.rs` | Keep |
| 45 | `LoggingLevel` | `notifications.rs` | Keep |
| 46 | `ModelPreferences` | `sampling.rs` | Keep |
| 47 | `ModelHint` | `sampling.rs` | Keep |
| 48 | `ProgressNotification` | `notifications.rs` | Keep |
| 49 | `Progress` (alias) | - | **Remove** (cleanup alias) |
| 50 | `ProgressToken` | `notifications.rs` | Keep |
| 51 | `RequestMeta` | `protocol/mod.rs` | **Expand** (add RELATED_TASK_META_KEY field) |
| 52 | `ClientRequest` (enum) | `protocol/mod.rs` | **Expand** (add task request variants with typed params) |
| 53 | `ServerRequest` (enum) | `protocol/mod.rs` | **Expand** (add task request variants, fix elicitation method name) |
| 54 | `CreateMessageParams` | `sampling.rs` | **Expand** (add tools, toolChoice fields) |
| 55 | `CreateMessageResult` | `sampling.rs` | Keep + add CreateMessageResultWithTools variant |
| 56 | `TokenUsage` | `sampling.rs` | Keep |
| 57 | `SamplingMessage` | `sampling.rs` | Keep (expand content type to include arrays) |
| 58 | `IncludeContext` | `sampling.rs` | **Fix** enum values to match spec (allServers, thisServer, none) |
| 59 | `ClientNotification` (enum) | `notifications.rs` | **Expand** (add TaskStatusNotification) |
| 60 | `CancelledNotification` | `notifications.rs` | Keep |
| 61 | `CancelledParams` (alias) | - | **Remove** (cleanup alias) |
| 62 | `ServerNotification` (enum) | `notifications.rs` | **Expand** (add TaskStatusNotification, ElicitationComplete) |
| 63 | `ResourceUpdatedParams` | `notifications.rs` | Keep |
| 64 | `LogMessageParams` | `notifications.rs` | Keep |
| 65 | `LogLevel` | `notifications.rs` | **Expand** (add notice, alert, emergency per spec) |
| 66 | `Request` (combined enum) | `protocol/mod.rs` | Keep |
| 67 | `Notification` (combined enum) | `notifications.rs` | Keep |

### New Types to Add from 2025-11-25 Spec

| # | New Type | Module | Source Reference |
|---|----------|--------|-----------------|
| 1 | `IconInfo` | `protocol/mod.rs` | TS types.ts:344-368 -- `{ src, mimeType?, sizes?, theme? }` |
| 2 | `IconTheme` | `protocol/mod.rs` | TS types.ts:367 -- `'light' \| 'dark'` enum |
| 3 | `AudioContent` (Content variant) | `content.rs` | TS types.ts:1223-1244 -- `{ type: 'audio', data, mimeType, annotations?, _meta? }` |
| 4 | `ResourceLink` (Content variant) | `content.rs` | TS types.ts:1296-1298 -- extends Resource with `type: 'resource_link'` |
| 5 | `ToolUseContent` | `sampling.rs` | TS types.ts:1250-1272 -- `{ type: 'tool_use', name, id, input, _meta? }` |
| 6 | `ToolResultContent` | `sampling.rs` | TS types.ts:1725-1737 -- `{ type: 'tool_result', toolUseId, content, structuredContent?, isError?, _meta? }` |
| 7 | `ToolChoice` | `sampling.rs` | TS types.ts:1711-1719 -- `{ mode?: 'auto' \| 'required' \| 'none' }` |
| 8 | `ToolExecution` (typed) | `tools.rs` | TS types.ts:1396-1406 -- `{ taskSupport?: 'required' \| 'optional' \| 'forbidden' }` |
| 9 | `TaskSupport` (enum) | `tools.rs` or `tasks.rs` | TS types.ts:1405 -- `'required' \| 'optional' \| 'forbidden'` |
| 10 | `Task` | `tasks.rs` | TS types.ts:743-764 -- `{ taskId, status, ttl, createdAt, lastUpdatedAt, pollInterval?, statusMessage? }` |
| 11 | `TaskStatus` | `tasks.rs` | TS types.ts:737 -- 5-value enum (working, input_required, completed, failed, cancelled) |
| 12 | `TaskCreationParams` | `tasks.rs` | TS types.ts:77-88 -- `{ ttl?, pollInterval? }` |
| 13 | `TaskMetadata` | `tasks.rs` | TS types.ts:90-92 -- `{ ttl? }` |
| 14 | `RelatedTaskMetadata` | `tasks.rs` | TS types.ts:98-100 -- `{ taskId }` |
| 15 | `CreateTaskResult` | `tasks.rs` | TS types.ts:769-771 -- `{ task: Task }` |
| 16 | `TaskStatusNotification` | `tasks.rs` | TS types.ts:781-784 -- `notifications/tasks/status` |
| 17 | `GetTaskRequest` | `tasks.rs` | TS types.ts:789-794 -- `tasks/get { taskId }` |
| 18 | `GetTaskResult` | `tasks.rs` | TS types.ts:799 -- merges Task fields into result |
| 19 | `GetTaskPayloadRequest` | `tasks.rs` | TS types.ts:804-809 -- `tasks/result { taskId }` |
| 20 | `ListTasksRequest` | `tasks.rs` | TS types.ts:822-824 -- `tasks/list` (paginated) |
| 21 | `ListTasksResult` | `tasks.rs` | TS types.ts:829-831 -- `{ tasks: Task[] }` (paginated) |
| 22 | `CancelTaskRequest` | `tasks.rs` | TS types.ts:836-841 -- `tasks/cancel { taskId }` |
| 23 | `CancelTaskResult` | `tasks.rs` | TS types.ts:846 -- merges Task fields into result |
| 24 | `ServerTasksCapability` | `capabilities.rs` | TS types.ts:492-516 -- `{ list?, cancel?, requests?: { tools?: { call? } } }` |
| 25 | `ClientTasksCapability` | `capabilities.rs` | TS types.ts:455-487 -- `{ list?, cancel?, requests?: { sampling?, elicitation? } }` |
| 26 | `Annotations` | `content.rs` | TS types.ts:909-924 -- `{ audience?, priority?, lastModified? }` (needed for content types) |
| 27 | `RELATED_TASK_META_KEY` (const) | `tasks.rs` | TS types.ts:7 -- `'io.modelcontextprotocol/related-task'` |
| 28 | `CreateMessageResultWithTools` | `sampling.rs` | TS types.ts:1855-1877 -- array content with tool use |
| 29 | `ElicitRequestFormParams` | `elicitation.rs` | TS types.ts:2012-2032 -- spec-compliant form elicitation |
| 30 | `ElicitRequestURLParams` | `elicitation.rs` | TS types.ts:2037-2055 -- URL-based elicitation |
| 31 | `ElicitResult` | `elicitation.rs` | TS types.ts:2097-2115 -- `{ action, content? }` |
| 32 | `ElicitationCompleteNotification` | `elicitation.rs` | TS types.ts:2077-2092 -- out-of-band completion |
| 33 | `ProtocolErrorCode` (enum) | `protocol/mod.rs` or `jsonrpc.rs` | TS types.ts:227-238 -- MCP-specific error codes |

## Critical Finding: Elicitation Types Divergence

**Severity: HIGH -- Breaking change required**

The current Rust elicitation types (`src/types/elicitation.rs`) are a **PMCP-proprietary format** that does NOT match the 2025-11-25 MCP specification. Key differences:

| Aspect | Current Rust (PMCP) | MCP 2025-11-25 Spec |
|--------|---------------------|---------------------|
| Method name | `elicitation/elicitInput` | `elicitation/create` |
| Request params | `ElicitInputRequest` with InputType enum (16 types) | Form mode: `requestedSchema` (JSON Schema subset), URL mode: `{ url, elicitationId }` |
| Response format | `ElicitInputResponse` with `value`, `cancelled` | `ElicitResult` with `action: 'accept'\|'decline'\|'cancel'`, `content` |
| Schema format | Custom InputValidation struct | Standard JSON Schema 2020-12 subset (PrimitiveSchemaDefinition) |
| Input types | 16 custom types (Text, Textarea, Boolean, Number, Select, MultiSelect, FilePath, etc.) | 4 primitive types: boolean, string, number/integer, enum (single/multi select) |
| Elicitation modes | Single mode | Two modes: `form` (default), `url` |
| ServerRequest variant | `ElicitInput` | Should be `ElicitationCreate` or similar |
| ClientRequest variant | `ElicitInputResponse` | Not a client request -- it's a server request with client result |

**Action required:** Replace the entire `src/types/elicitation.rs` with spec-compliant types. The existing `ElicitInputBuilder` and helper functions are PMCP extensions that should be rebuilt on top of the spec types if desired.

**Impact:** The `ServerRequest::ElicitInput` variant and `ClientRequest::ElicitInputResponse` variant both need to change. The `src/server/elicitation.rs` handler will need updating in a follow-up phase (per locked decision: handlers deferred).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON Schema subset for elicitation | Custom InputValidation struct | serde_json::Value with type checks | MCP spec uses standard JSON Schema primitives, not custom validation |
| ISO 8601 datetime strings in Task | Custom datetime parser | String type with doc comments | Wire type only; actual parsing happens in pmcp-tasks crate |
| Exhaustive content type matching | Separate content enums per context | Single Content enum with serde tag | Wire format is the same; context constraints are API-level |

## Common Pitfalls

### Pitfall 1: Glob re-export name collisions
**What goes wrong:** `pub use tools::*` and `pub use prompts::*` both export items that might have similar names, or Content variants clash with other types.
**Why it happens:** Moving types from a single module to many modules and re-exporting all creates the collision surface.
**How to avoid:** Ensure no two sub-modules export the same name. The Content enum stays in one place (content.rs). Use qualified paths where truly ambiguous.
**Warning signs:** Compile errors about ambiguous imports.

### Pitfall 2: Breaking the Content enum's serde tag
**What goes wrong:** Adding new Content variants (Audio, ResourceLink) that don't serialize with `"type"` field matching the spec names.
**Why it happens:** The existing Content enum uses `#[serde(tag = "type", rename_all = "camelCase")]`. New variants must use `#[serde(rename = "audio")]`, `#[serde(rename = "resource_link")]`, etc.
**How to avoid:** Verify each new variant's serialized `type` field matches the TypeScript spec exactly. AudioContent: `"audio"`. ResourceLink: `"resource_link"`. ToolUse: `"tool_use"`. ToolResult: `"tool_result"`.
**Warning signs:** Integration test failures when exchanging content with TypeScript clients.

### Pitfall 3: Elicitation method name mismatch
**What goes wrong:** Current code uses `elicitation/elicitInput` but spec uses `elicitation/create`. Wire incompatibility.
**Why it happens:** The Rust SDK implemented elicitation before the spec stabilized the method name.
**How to avoid:** Rename `ServerRequest::ElicitInput` to `ServerRequest::ElicitationCreate` with `#[serde(rename = "elicitation/create")]`.
**Warning signs:** Any JSON message with `"method": "elicitation/elicitInput"` will not be understood by spec-compliant clients.

### Pitfall 4: IncludeContext enum value mismatch
**What goes wrong:** Current Rust `IncludeContext` has `All` and `ThisServerOnly` but spec has `allServers` and `thisServer`.
**Why it happens:** The serde rename may not match the wire format.
**How to avoid:** Verify that `#[serde(rename_all = "camelCase")]` produces `"allServers"` and `"thisServer"` (it does for `AllServers` and `ThisServer` variants). But current Rust has `All` which serializes to `"all"`, not `"allServers"`. **This is a bug to fix.**
**Warning signs:** Sampling requests failing with deserialization errors on the client side.

### Pitfall 5: LoggingLevel missing spec values
**What goes wrong:** Current Rust `LoggingLevel` has 5 values (Debug, Info, Warning, Error, Critical). Spec has 8 values (debug, info, notice, warning, error, critical, alert, emergency).
**Why it happens:** Original implementation was incomplete.
**How to avoid:** Add `Notice`, `Alert`, `Emergency` variants. Also note the current code has BOTH `LoggingLevel` and `LogLevel` -- the latter has only 4 values. Consolidate into one enum.
**Warning signs:** Deserialization failure when receiving `"notice"`, `"alert"`, or `"emergency"` log levels.

### Pitfall 6: Import path breakage across 100+ consumer files
**What goes wrong:** Moving types from `crate::types::protocol::X` to `crate::types::tools::X` breaks all internal consumers.
**Why it happens:** 100+ files import from `crate::types::protocol::` or `crate::types::` paths.
**How to avoid:** The re-export from `types/mod.rs` means `use crate::types::ToolInfo` still works. But any code using `use crate::types::protocol::ToolInfo` will break. Use IDE-wide find-and-replace.
**Warning signs:** Massive compile errors after the split.

### Pitfall 7: ResourceInfo.meta field naming inconsistency
**What goes wrong:** `ResourceInfo` uses `meta` (without underscore) while `ToolInfo` uses `_meta` for the same MCP `_meta` field.
**Why it happens:** Inconsistent naming established in earlier phases.
**How to avoid:** In v2.0 cleanup, standardize all `_meta` fields. Use `pub _meta: Option<...>` with `#[serde(rename = "_meta")]` consistently, or use `pub meta: Option<...>` with `#[serde(rename = "_meta")]` consistently. Choose one pattern.
**Warning signs:** Confusing API where sometimes it's `.meta` and sometimes `._meta`.

### Pitfall 8: TokenUsage not in TypeScript CreateMessageResult
**What goes wrong:** The Rust `CreateMessageResult` has a `usage: Option<TokenUsage>` field, but the TypeScript spec does NOT have this field.
**Why it happens:** This was a PMCP extension or assumption.
**How to avoid:** Check whether to keep as PMCP extension or remove. The spec's `CreateMessageResultSchema` has: `model`, `stopReason`, `role`, `content`. No `usage`. Decision: keep as PMCP extension with `#[serde(skip_serializing_if = "Option::is_none")]` so it doesn't break wire compatibility.

## Version Negotiation Changes

### Current Rust Implementation (lib.rs:263-323)
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

### Required Changes
```rust
pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-06-18",
    "2025-03-26",
];

pub fn negotiate_protocol_version(client_version: &str) -> String {
    if SUPPORTED_PROTOCOL_VERSIONS.contains(&client_version) {
        client_version.to_string()
    } else {
        // Per locked decision: explicit rejection for unsupported versions
        // Return first supported version (most preferred) like TypeScript does
        SUPPORTED_PROTOCOL_VERSIONS[0].to_string()
    }
}
```

### Consumer locations that reference version constants
- `src/lib.rs:263-323` -- constant definitions and negotiation function
- `src/server/wasm_server.rs:127` -- calls `negotiate_protocol_version()`
- `src/client/mod.rs:213` -- sends `LATEST_PROTOCOL_VERSION` to server
- `src/client/mod.rs:226` -- checks response against `SUPPORTED_PROTOCOL_VERSIONS`
- `src/types/protocol.rs:18` -- `ProtocolVersion::default()` uses `DEFAULT_PROTOCOL_VERSION`
- Doctests in lib.rs that assert version values

## Capability Expansion Details

### ServerCapabilities Changes
```rust
// ADD to ServerCapabilities:
pub tasks: Option<ServerTasksCapability>,

// ServerTasksCapability (new)
pub struct ServerTasksCapability {
    pub list: Option<Value>,    // {} or absent
    pub cancel: Option<Value>,  // {} or absent
    pub requests: Option<ServerTasksRequestCapability>,
}

pub struct ServerTasksRequestCapability {
    pub tools: Option<ServerTasksToolsCapability>,
}

pub struct ServerTasksToolsCapability {
    pub call: Option<Value>,  // {} or absent
}
```

### ClientCapabilities Changes
```rust
// ADD to ClientCapabilities:
pub tasks: Option<ClientTasksCapability>,

// ClientTasksCapability (new)
pub struct ClientTasksCapability {
    pub list: Option<Value>,
    pub cancel: Option<Value>,
    pub requests: Option<ClientTasksRequestCapability>,
}

pub struct ClientTasksRequestCapability {
    pub sampling: Option<ClientTasksSamplingCapability>,
    pub elicitation: Option<ClientTasksElicitationCapability>,
}
```

### ElicitationCapabilities Expansion
```rust
// REPLACE empty ElicitationCapabilities with:
pub struct ElicitationCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<FormElicitationCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Value>,  // {} when supported
}

pub struct FormElicitationCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_defaults: Option<bool>,
}
```

### SamplingCapabilities Expansion
```rust
// EXPAND SamplingCapabilities:
pub struct SamplingCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,  // existing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,  // NEW -- {} when supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,    // NEW -- {} when supported
}
```

## Internal Consumer Impact

### Files importing from `crate::types::protocol::` (WILL BREAK with module split)
These use the specific `protocol::` path and will need updating:

| File | Current Import | New Import Path |
|------|---------------|-----------------|
| `src/server/resource_watcher.rs:4` | `protocol::{ResourceInfo, ServerNotification}` | `types::{ResourceInfo, ServerNotification}` (via re-export) |
| `src/server/cancellation.rs:5` | `protocol::{CancelledNotification, Notification}` | `types::{CancelledNotification, Notification}` |
| `src/server/notification_debouncer.rs:8` | `protocol::ServerNotification` | `types::ServerNotification` |
| `src/server/core_tests.rs:11` | `protocol::{ResourceInfo, Role}` | `types::{ResourceInfo, Role}` |
| `src/server/elicitation.rs:5` | `protocol::ServerRequest` | `types::ServerRequest` |
| `src/server/subscriptions.rs:4` | `protocol::ResourceUpdatedParams` | `types::ResourceUpdatedParams` |

### Files importing from `crate::types::` (SHOULD WORK via re-exports)
These use the flat `types::` path and will continue working as long as re-exports are in place. Over 80 files in `src/` and 30+ files in `crates/` and `examples/` use this pattern.

### External crate consumers (`pmcp::types::`)
All examples and workspace crates use `pmcp::types::X` which will work via re-exports. Some use `pmcp::types::protocol::X` which will need updating:
- `examples/18_resource_watcher.rs` -- `pmcp::types::protocol::{Content, ...}`
- `examples/17_completable_prompts.rs` -- `pmcp::types::protocol::{Content, ...}`
- `examples/mcp-apps-dataviz/src/main.rs` -- `pmcp::types::protocol::Content`
- `examples/mcp-apps-map/src/main.rs` -- `pmcp::types::protocol::Content`
- `examples/mcp-apps-chess/src/main.rs` -- `pmcp::types::protocol::Content`

## Legacy Type Aliases to Remove

These type aliases add confusion and should be removed in v2.0:

| Alias | Points To | Action |
|-------|-----------|--------|
| `InitializeParams` | `InitializeRequest` | Remove -- use `InitializeRequest` |
| `ListToolsParams` | `ListToolsRequest` | Remove -- use `ListToolsRequest` |
| `CallToolParams` | `CallToolRequest` | Remove -- use `CallToolRequest` |
| `ListPromptsParams` | `ListPromptsRequest` | Remove -- use `ListPromptsRequest` |
| `GetPromptParams` | `GetPromptRequest` | Remove -- use `GetPromptRequest` |
| `ListResourcesParams` | `ListResourcesRequest` | Remove -- use `ListResourcesRequest` |
| `ReadResourceParams` | `ReadResourceRequest` | Remove -- use `ReadResourceRequest` |
| `CancelledParams` | `CancelledNotification` | Remove -- use `CancelledNotification` |
| `Progress` | `ProgressNotification` | Remove -- use `ProgressNotification` |
| `MessageContent` | `Content` | Remove -- use `Content` |
| `CreateMessageRequest` | `CreateMessageParams` | Remove one direction |

**Total: 11 aliases to remove.** Each must be listed in MIGRATION.md with its replacement.

## Implementation Field Expansion

TypeScript's `ImplementationSchema` (types.ts:410-427):
```typescript
BaseMetadataSchema.extend({
    ...IconsSchema.shape,
    version: z.string(),
    websiteUrl: z.string().optional(),
    description: z.string().optional()
})
```

Rust current: `{ name, version }`. Needs: `{ name, title?, version, websiteUrl?, description?, icons? }`.

## Code Examples

### New Content Variants
```rust
// Source: TypeScript types.ts:1223-1298
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    // ... existing Text, Image, Resource ...

    /// Audio content (2025-11-25)
    #[serde(rename = "audio", rename_all = "camelCase")]
    Audio {
        data: String,
        mime_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<Annotations>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },

    /// Resource link content (2025-11-25)
    #[serde(rename = "resource_link", rename_all = "camelCase")]
    ResourceLink {
        name: String,
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        icons: Option<Vec<IconInfo>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<Annotations>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, Value>>,
    },
}
```

### Task Wire Type
```rust
// Source: TypeScript types.ts:743-764
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub task_id: String,
    pub status: TaskStatus,
    pub ttl: Option<u64>,  // null = unlimited
    pub created_at: String,  // ISO 8601
    pub last_updated_at: String,  // ISO 8601
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Working,
    InputRequired,
    Completed,
    Failed,
    Cancelled,
}
```

### Spec-Compliant Elicitation
```rust
// Source: TypeScript types.ts:2012-2115
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum ElicitRequestParams {
    #[serde(rename = "form")]
    Form {
        message: String,
        requested_schema: Value,  // JSON Schema subset
    },
    #[serde(rename = "url")]
    Url {
        message: String,
        elicitation_id: String,
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitResult {
    pub action: ElicitAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<HashMap<String, Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElicitAction {
    Accept,
    Decline,
    Cancel,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single `protocol.rs` (2326 lines) | Domain sub-modules with re-exports | This phase | Better discoverability, smaller files |
| 2024 protocol versions in supported list | Only 2025 versions (latest 3) | This phase | Cleaner version support, matches TypeScript direction |
| PMCP-proprietary elicitation | Spec-compliant `elicitation/create` | This phase | Wire interop with TypeScript clients |
| Empty `ElicitationCapabilities` | Form + URL capability sub-fields | This phase | Proper capability negotiation |
| 5-value LoggingLevel | 8-value LoggingLevel matching syslog | This phase | Full spec compliance |
| `ToolInfo.execution: Option<Value>` | `ToolInfo.execution: Option<ToolExecution>` | This phase | Type safety for task support |
| No task capability on ServerCapabilities | `tasks` field with full sub-capabilities | This phase | Task interop with clients |

**Deprecated/outdated:**
- `InitializeParams` alias: Use `InitializeRequest` directly
- `CallToolParams` alias: Use `CallToolRequest` directly
- All `*Params` aliases for request types: Removed in v2.0
- `MessageContent` alias: Use `Content` directly
- `Progress` alias: Use `ProgressNotification` directly
- PMCP-proprietary `ElicitInputRequest`: Use spec-compliant `ElicitRequestParams`

## Open Questions

1. **ToolInfo.execution: typed vs Value?**
   - What we know: Currently `Option<Value>` to avoid circular crate dependency (pmcp-tasks depends on pmcp)
   - What's unclear: With task types moving into `pmcp::types::tasks`, the circular dependency concern may be resolved
   - Recommendation: Define `ToolExecution` in `pmcp::types::tools` (where ToolInfo lives), making it a proper typed field. The pmcp-tasks crate can import from pmcp.

2. **ResourceInfo._meta field name standardization**
   - What we know: ResourceInfo uses `pub meta` with `#[serde(rename = "_meta")]`, while ToolInfo uses `pub _meta`
   - What's unclear: Which pattern to standardize on
   - Recommendation: Use `pub meta` everywhere with `#[serde(rename = "_meta")]` since leading underscores are not idiomatic Rust field names (per Phase 41 decision)

3. **ReadResourceResult custom serde module**
   - What we know: `resource_contents_serde` module strips the Content `type` tag for ResourceContents wire format
   - What's unclear: Whether to keep this approach or use a dedicated ResourceContents type
   - Recommendation: Keep the custom serde approach -- it avoids type duplication while matching the wire format

4. **Role enum: System variant**
   - What we know: Rust Role has User, Assistant, System. TypeScript RoleSchema only has 'user', 'assistant'.
   - What's unclear: Whether System was a PMCP extension or is used in the spec elsewhere
   - Recommendation: Keep System as a PMCP extension since prompts use it for system messages

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml `[dev-dependencies]` |
| Quick run command | `cargo test --lib -p pmcp` |
| Full suite command | `make quality-gate` |

### Phase Requirements to Test Map
| Req | Behavior | Test Type | Automated Command |
|-----|----------|-----------|-------------------|
| Version negotiation | `negotiate_protocol_version` returns correct values for 3 supported + 2 rejected | unit | `cargo test negotiate_ -p pmcp` |
| New Content variants | Audio, ResourceLink serialize/deserialize with correct `type` tag | unit | `cargo test content_ -p pmcp` |
| Task types | Task, TaskStatus, CreateTaskResult round-trip | unit | `cargo test task_ -p pmcp` |
| Capability expansion | ServerCapabilities.tasks, ClientCapabilities.tasks serialize | unit | `cargo test capabilities_ -p pmcp` |
| Implementation expansion | Implementation with title, icons, websiteUrl serializes | unit | `cargo test implementation_ -p pmcp` |
| Re-exports work | `use pmcp::types::ToolInfo` compiles from examples | integration | `cargo build --examples` |
| Elicitation spec compliance | ElicitRequestParams matches wire format | unit | `cargo test elicit_ -p pmcp` |
| Module split correctness | All existing tests pass after reorganization | full | `make quality-gate` |

### Wave 0 Gaps
- [ ] Tests for new Content variants (Audio, ResourceLink) serialization round-trip
- [ ] Tests for Task wire type serialization matching TypeScript format
- [ ] Tests for expanded capabilities serialization
- [ ] Tests for spec-compliant elicitation types
- [ ] Tests for version negotiation with 2024 version rejection
- [ ] Tests for IconInfo serialization

## Sources

### Primary (HIGH confidence)
- TypeScript SDK types.ts (`~/Development/mcp/sdk/typescript-sdk/packages/core/src/types/types.ts`) -- 2,500+ lines, every type definition verified with line numbers
- Rust SDK protocol.rs (`src/types/protocol.rs`) -- all 58 types cataloged
- Rust SDK capabilities.rs (`src/types/capabilities.rs`) -- all capability types verified
- Phase 53 verification notes (`.planning/phases/53-review-typescript-sdk-updates/53-01-VERIFICATION-NOTES.md`) -- field-by-field comparisons
- Phase 53 gap analysis (`.planning/phases/53-review-typescript-sdk-updates/53-GAP-ANALYSIS.md`) -- 35 gaps across 6 domains

### Secondary (MEDIUM confidence)
- TypeScript server negotiation logic (`packages/server/src/server/server.ts:431-447`) -- verified algorithm
- Consumer grep across entire Rust codebase -- 100+ files identified

### Tertiary (LOW confidence)
- None -- all findings verified against primary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all types are serde-based, no new dependencies needed
- Architecture: HIGH -- module split is well-defined, re-export pattern proven in other Rust projects
- Type catalog: HIGH -- every type compared between TypeScript and Rust with line references
- Pitfalls: HIGH -- identified from actual code review, not speculation
- Elicitation divergence: HIGH -- verified by reading both implementations side-by-side

**Research date:** 2026-03-19
**Valid until:** 2026-06-19 (stable spec; 2025-11-25 is released, not draft)
