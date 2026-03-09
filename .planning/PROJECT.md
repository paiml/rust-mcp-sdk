# PMCP SDK Extensions

## What This Is

Extensions for the PMCP SDK: a `pmcp-tasks` crate implementing MCP Tasks (experimental spec 2025-11-25) with pluggable storage backends, and a complete MCP Apps developer experience — from `cargo pmcp app new` scaffolding through live preview with dual bridge modes to ChatGPT manifest generation and demo landing pages — enabling rich UI widgets served from MCP servers across ChatGPT, Claude, and other MCP clients.

## Core Value

Tool handlers can manage long-running operations through a durable task lifecycle (create, poll, complete) with shared variable state that persists across tool calls — giving servers memory without an LLM.

## Requirements

### Validated

- ✓ Core protocol types (Task, TaskStatus, CreateTaskResult, etc.) matching MCP spec 2025-11-25 — v1.0
- ✓ Task status state machine with validated transitions (5 states, 46 transition tests) — v1.0
- ✓ TaskStore trait with pluggable storage backends (11 async methods) — v1.0
- ✓ In-memory storage backend for dev/testing (DashMap, atomic transitions) — v1.0
- ✓ TaskContext for ergonomic handler integration (typed accessors, status transitions) — v1.0
- ✓ PMCP extension: task variables as shared client/server scratchpad via `_meta` — v1.0
- ✓ Server/client task capability types and negotiation via `experimental.tasks` — v1.0
- ✓ Tool-level task support declaration (forbidden/optional/required) — v1.0
- ✓ TaskRouter for routing tasks/get, tasks/result, tasks/list, tasks/cancel — v1.0
- ✓ Task interception for task-augmented tools/call requests — v1.0
- ✓ Owner binding security (OAuth sub, client ID, session ID fallback) — v1.0
- ✓ TaskSecurityConfig with configurable limits (max tasks, TTL, variable size) — v1.0
- ✓ Comprehensive test suite: unit (200+), property (13), integration (11), security (19) — v1.0
- ✓ Basic tasks example (60_tasks_basic.rs) — v1.0
- ✓ Task-aware workflow prompts that create tasks and bind step progress — v1.1
- ✓ Partial server-side execution with automatic pause on unresolvable steps — v1.1
- ✓ Structured prompt reply conveying completed steps, remaining steps, and task ID — v1.1
- ✓ Step state tracking in task variables (standard schema: goal, steps, completed, remaining) — v1.1
- ✓ Client continuation pattern via direct tool calls guided by prompt reply — v1.1
- ✓ Working example demonstrating task-prompt bridge with multi-step workflow — v1.1
- ✓ Lower-level KV storage backend trait for pluggable persistence — v1.2
- ✓ GenericTaskStore that delegates to any StorageBackend implementation — v1.2
- ✓ InMemoryBackend refactored from existing InMemoryTaskStore — v1.2
- ✓ DynamoDB backend behind `dynamodb` feature flag (cloud-only tests) — v1.2
- ✓ Redis backend behind `redis` feature flag (proving the trait) — v1.2
- ✓ Automated feature-flag verification across all backend combinations — v1.2
- ✓ mcp-preview widget iframe rendering with working MCP bridge proxy — v1.3
- ✓ WASM in-browser MCP client with proxy/WASM toggle and standalone polyfill — v1.3
- ✓ Shared bridge library (App, PostMessageTransport, AppBridge) eliminating inline JS — v1.3
- ✓ File-based widget authoring with WidgetDir hot-reload and bridge auto-injection — v1.3
- ✓ `cargo pmcp app new` CLI scaffolding with documented bridge API and CSP helpers — v1.3
- ✓ ChatGPT-compatible ai-plugin.json manifest generation — v1.3
- ✓ Standalone demo landing pages with mock bridge — v1.3
- ✓ Chess, map, and dataviz MCP App examples shipping — v1.3
- ✓ 20 chromiumoxide CDP E2E browser tests across 3 widget suites — v1.3
- ✓ Book Ch 14 (Performance & Load Testing) — 961-line comprehensive chapter with CLI, config, metrics, CI/CD — v1.4
- ✓ Book Ch 15 Load Testing cross-reference section — v1.4
- ✓ Book Ch 12.5 (MCP Apps) rewritten with WidgetDir, cargo pmcp app, adapter pattern — v1.4
- ✓ Course Ch 18-03 hands-on load testing tutorial (952 lines) — v1.4
- ✓ Course Ch 12 Load Testing cross-reference section — v1.4
- ✓ Course Ch 20 sub-chapters rewritten with WidgetDir/mcpBridge/adapter paradigm — v1.4
- ✓ Course quizzes and exercises for load testing and MCP Apps content — v1.4

### Active

## Current Milestone: v1.6 CLI DX Overhaul

**Goal:** Normalize the cargo pmcp CLI for consistency and developer experience ahead of course recording — fix flag inconsistencies, propagate auth to all server-facing commands, surface mcp-tester via cargo pmcp test, and add doctor/completions commands.

**Target features:**
- Flag consistency: positional URL, unified --server, --verbose/-v, --yes, -o, --format
- Auth propagation: OAuth + API key flags on test check/run/generate, preview, schema export
- Tester integration: surface mcp-tester commands through cargo pmcp test with aligned flags
- New commands: cargo pmcp doctor (workspace health), cargo pmcp completions (shell completions)
- Help polish: consistent help text, usage examples, global --no-color/--quiet

### Future

- [ ] CloudFormation template integrating with cargo-pmcp deployment plugin system
- [ ] Integration with cargo-pmcp deployment plugin system (DynamoDB table via CFN stack)
- [ ] Cross-server task sharing on pmcp.run — shared TaskStore backend + OAuth sub owner binding enables multi-server workflow continuity
- [ ] DataSource::TaskVariable for steps to read values from task variable store
- [ ] Workflow resume from task state (re-invoke prompt with task ID to continue from last step)
- [ ] StepExecution user API for runtime step mode customization
- [ ] Examples: code mode, DynamoDB backend
- [ ] Loadtest provider trait abstraction (when second provider appears)
- [ ] Remote execution trigger from CLI (`cargo pmcp loadtest run --remote`)
- [ ] Result download/polling from CLI

## Current State

v1.6 in progress. All prior milestones (v1.0-v1.5) shipped.

**Shipped milestones:**
- v1.0: MCP Tasks Foundation (types, store, server integration)
- v1.1: Task-Prompt Bridge (workflow execution, handoff, continuation)
- v1.2: Pluggable Storage Backends (DynamoDB, Redis, feature flags)
- v1.3: MCP Apps Developer Experience (preview, WASM, authoring, publishing, examples, E2E)
- v1.4: Book & Course Update (load testing docs, MCP Apps chapter refresh, quizzes, exercises)
- v1.5: Cloud Load Testing Upload (loadtest config upload, OAuth for load testing)

### Out of Scope

- Task status notifications — skip for now, rely on polling only (validated by v1.0: polling works well)
- Bounded blocking on tasks/result — polling-only behavior
- Redis Cluster support — single-node sufficient (validated by v1.2: single-node Redis backend shipped)
- Task progress streaming via SSE — future phase
- Moving types into core pmcp crate — wait for spec stabilization
- Namespaced variable keys — flat keys with convention recommendation in docs (validated by v1.0: flat keys sufficient)
- Variable size enforcement per-backend — trait-level configurable limit works (validated by v1.0)
- Automatic client execution — MCP clients decide when/how to call tools; server cannot drive client
- Per-step task statuses — single task status with variable-level step tracking suffices (validated by v1.1)
- Workflow branching/conditionals — sequential-only; branching is a different workflow engine
- DynamoDB Local / docker-based testing — cloud-only DynamoDB in CI

## Context

Shipped v1.4 with ~41,000+ Rust LOC across the workspace (v1.0: ~11,500 + v1.1: +10,697 + v1.2: +9,802 + v1.3: +9,197) plus 8,140 lines of documentation content in v1.4.
Tech stack: `pmcp-tasks` (serde, async-trait, dashmap, uuid, chrono, tokio, parking_lot; optional: aws-sdk-dynamodb, redis) + `pmcp` core (protocol types, ServerCore routing, workflow system, MCP Apps) + `cargo-pmcp` (CLI tooling) + `mcp-preview` (browser preview) + `mcp-e2e-tests` (chromiumoxide CDP) + `packages/widget-runtime` (TypeScript bridge library).

- The MCP Tasks spec is experimental (2025-11-25). Most MCP clients don't support it yet, so the feature is optional and isolated in `pmcp-tasks`.
- PMCP extends the minimal spec with task variables — a shared scratchpad visible to both client and server via `_meta`. This is the key innovation for servers without LLM capabilities.
- v1.1 bridges the `SequentialWorkflow` system with tasks: workflows pause mid-execution and the client continues via structured handoff guidance.
- v1.2 introduced pluggable storage backends: `StorageBackend` KV trait with `GenericTaskStore<B>` centralizing all domain logic. Three backends ship: `InMemoryBackend` (default), `DynamoDbBackend` (feature-flagged), `RedisBackend` (feature-flagged).
- v1.3 shipped the complete MCP Apps developer experience: `mcp-preview` with dual proxy/WASM bridge modes, `WidgetDir` file-based widget authoring with hot-reload, `cargo pmcp app new` scaffolding, `cargo pmcp app build` for manifest+landing generation, and 3 example apps (chess, map, dataviz) with 20 E2E browser tests.
- MCP Apps is an OpenAI extension (ChatGPT Apps / SEP-1865) adding rich HTML UI widgets to MCP servers. PMCP SDK supports multiple MIME types: `text/html+skybridge` (ChatGPT), `text/html+mcp` (standard MCP Apps), `text/html` (MCP-UI). Core types and adapters are in `src/types/mcp_apps.rs` behind `mcp-apps` feature flag.
- The shared bridge library (`packages/widget-runtime/`) provides App, PostMessageTransport, and AppBridge classes with TypeScript type definitions, compiled to ESM/CJS.
- Detailed design document: `docs/design/tasks-feature-design.md`

## Constraints

- **Isolation**: Must be a separate crate (`pmcp-tasks`) — experimental feature cannot destabilize core SDK
- **Spec compliance**: Protocol types must match MCP 2025-11-25 schema exactly
- **Feature gating**: DynamoDB backend behind `dynamodb` feature flag
- **Compatibility**: No breaking changes to existing `pmcp` crate API (validated: v1.0 and v1.1 only additive changes)
- **Testing**: Real DynamoDB in CI (cloud test table), no local docker dependency
- **Variable limits**: Trait-level configurable size limit enforced across all backends

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Separate crate (`pmcp-tasks`) | Experimental spec isolation, independent versioning | ✓ Good — clean separation, pmcp core unchanged for non-task users |
| Polling-only for tasks/result | Simpler implementation, Lambda-compatible, spec allows it | ✓ Good — 11 integration tests validate polling flow |
| Trait-level variable size limits | Consistent enforcement across backends, not just DynamoDB's 400KB | ✓ Good — `StoreConfig.max_variable_size_bytes` enforced in InMemoryTaskStore |
| Skip notifications for now | Simplifies initial implementation, polling sufficient | ✓ Good — TaskStatusNotification type defined but not wired; ready for v2 |
| Flat variable keys | Simplicity over structure, convention in docs | ✓ Good — top-level injection into `_meta` works cleanly |
| Capabilities via experimental field | Spec-compliant for experimental features, migrate when stabilized | ✓ Good — `experimental.tasks` auto-configured by `with_task_store()` |
| serde_json::Value for TaskRouter | Avoid circular crate dependency (pmcp-tasks depends on pmcp) | ✓ Good — clean trait boundary, pmcp has zero knowledge of pmcp-tasks types |
| DashMap for InMemoryTaskStore | Matches SessionManager pattern in existing codebase | ✓ Good — concurrent access tested with 10-thread proptest |
| Owner ID as structural key | NotFound on mismatch (never OwnerMismatch) — no info leakage | ✓ Good — 19 security tests verify isolation |
| TaskRouter in pmcp, impl in pmcp-tasks | One-directional dependency, builder accepts Arc\<dyn TaskRouter\> | ✓ Good — example only needs pmcp-tasks imports |
| Composition over modification (v1.1) | TaskWorkflowPromptHandler wraps WorkflowPromptHandler without changing it | ✓ Good — zero backward-compatibility issues, all existing tests pass unchanged |
| Hybrid handoff format (v1.1) | `_meta` JSON for machine parsing + natural language for LLM clients | ✓ Good — works with any MCP client regardless of structured output support |
| Fire-and-forget continuation (v1.1) | Continuation recording never fails the tool call | ✓ Good — tool results always returned to client; recording is best-effort |
| Cancel-as-completion (v1.1) | `tasks/cancel` with result transitions to Completed, not Cancelled | ✓ Good — enables clean workflow completion after all steps done client-side |
| Local mirror types (v1.1) | PauseReason/StepStatus mirrored in pmcp to avoid circular dependency | ✓ Good — same approach as TaskRouter; clean trait boundary preserved |
| Runtime best-effort execution (v1.1) | Dropped StepExecution enum; steps execute what they can at runtime | ✓ Good — simpler than static classification; PauseReason captures why stops |
| KV StorageBackend with GenericTaskStore (v1.2) | Domain logic once, backends are dumb KV stores | ✓ Good — 3 backends share identical domain logic; zero divergence |
| CAS in trait from day one (v1.2) | Retrofitting after backends exist would require rewriting every backend | ✓ Good — all 3 backends implement put_if_version atomically |
| Canonical JSON serialization (v1.2) | Prevents format divergence across backends | ✓ Good — identical round-trip behavior regardless of backend |
| Composite string keys (v1.2) | `{owner_id}:{task_id}` for universal backend support | ✓ Good — maps naturally to DynamoDB partition keys and Redis key prefixes |
| Feature-flagged backends (v1.2) | DynamoDB/Redis behind feature flags, InMemory always available | ✓ Good — zero-cost default, opt-in for production backends |
| Lua scripts for Redis CAS (v1.2) | Atomic check-and-set without WATCH/MULTI race conditions | ✓ Good — 19 integration tests verify atomicity |
| Session-once RwLock for MCP proxy (v1.3) | Resettable session support for reconnect button; OnceCell cannot reset | ✓ Good — session persists across requests, reconnect works |
| Bridge-first approach (v1.3) | Preview bridge is the load-bearing dependency for all downstream phases | ✓ Good — phase ordering validated as correct |
| Extract shared library after proving (v1.3) | Build 2 bridge implementations before extracting widget-runtime.js | ✓ Good — abstraction covers both proxy and WASM cases |
| App class uses document.referrer for origin (v1.3) | Prevents CVE-class wildcard postMessage vulnerability | ✓ Good — security fix for the blocked concern |
| WidgetDir disk reads on every call (v1.3) | Zero-config hot-reload without file watchers | ✓ Good — simplest approach, no caching bugs |
| chromiumoxide over Playwright (v1.3) | Pure Rust E2E tests, no Node.js dependency | ✓ Good — 20 tests pass, auto-downloads Chromium |
| Standalone examples (workspace exclude) (v1.3) | Avoids feature flag unification conflicts | ✓ Good — each example builds independently |

---
*Last updated: 2026-03-03 after v1.6 milestone start*
