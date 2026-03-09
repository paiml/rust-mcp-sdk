# Milestones

## v1.0 MCP Tasks Foundation (Shipped: 2026-02-22)

**Phases completed:** 3 phases, 9 plans
**Lines of code:** ~11,500 Rust LOC (7,621 source + 3,888 tests/examples)
**Timeline:** 2026-02-21 → 2026-02-22

**Delivered:** Complete MCP Tasks support for the PMCP SDK — from spec-compliant protocol types through in-memory storage with security enforcement to full server integration with task-augmented tool calls, lifecycle polling, and working examples.

**Key accomplishments:**
1. Complete MCP 2025-11-25 Tasks wire types with spec-compliant serialization (10 protocol types, state machine with validated transitions)
2. In-memory task store with DashMap concurrency, owner isolation, and configurable security limits (max tasks, TTL, anonymous access)
3. TaskContext ergonomic wrapper with typed variable accessors and atomic completion
4. Server integration — task-augmented tool calls intercepted and routed through TaskRouter trait, avoiding circular crate dependencies
5. Full lifecycle integration tests (11 tests) proving create-poll-complete-result flow end-to-end through real ServerCore
6. Working example (`60_tasks_basic.rs`) demonstrating the complete task lifecycle with background execution simulation

**Requirements:** 51/51 satisfied (TYPE-01..10, STOR-01..07, HNDL-01..06, SEC-01..08, INTG-01..12, TEST-01..04/06..08, EXMP-01)

---


## v1.1 Task-Prompt Bridge (Shipped: 2026-02-23)

**Phases completed:** 5 phases, 10 plans
**Code changes:** +10,697 / -553 across 77 files
**Timeline:** 2026-02-22 → 2026-02-23

**Delivered:** Task-prompt bridge for the PMCP SDK — workflow prompts create tasks, execute server-resolvable steps, return structured handoff with remaining step guidance, and support client continuation via `_task_id` binding.

**Key accomplishments:**
1. Task-aware workflow composition via `TaskWorkflowPromptHandler` that wraps `WorkflowPromptHandler` with zero modification to existing behavior
2. Active execution engine that creates tasks, runs server-resolvable steps sequentially, and pauses at client-deferred steps with typed `PauseReason` diagnostics
3. Hybrid handoff format with `_meta` JSON for machine parsing plus natural language narrative, including resolved arguments and remaining step guidance
4. Client continuation via `_task_id` in `_meta` with fire-and-forget step recording and cancel-with-result completion
5. End-to-end integration validation through `ServerCore::handle_request` plus lifecycle example (`62_task_workflow_lifecycle.rs`)
6. Quality polish closing all audit findings: accurate `SchemaMismatch` diagnostics, complete `PauseReason` coverage, zero clippy warnings, safe TTL overflow handling

**Requirements:** 19/19 satisfied (FNDX-01..05, EXEC-01..04, HAND-01..03, CONT-01..03, INTG-01..04)

---


## v1.2 Pluggable Storage Backends (Shipped: 2026-02-24)

**Phases completed:** 5 phases, 9 plans, 15 tasks
**Code changes:** +9,802 / -544 across 47 files
**Timeline:** 2026-02-23 → 2026-02-24

**Delivered:** Pluggable KV storage backend layer for MCP Tasks — StorageBackend trait with GenericTaskStore centralizing all domain logic, InMemoryBackend refactored from existing store, plus production-ready DynamoDB and Redis backends behind feature flags with automated feature-flag verification in CI.

**Key accomplishments:**
1. StorageBackend async trait with 6 KV methods and GenericTaskStore<B> implementing all 11 domain operations once, backend-agnostically
2. InMemoryBackend refactor replacing InMemoryTaskStore internals with GenericTaskStore<InMemoryBackend> — zero behavioral changes, all 500+ tests pass unchanged
3. DynamoDbBackend with single-table design (composite keys), CAS via ConditionExpression, native TTL, behind `dynamodb` feature flag with 18 cloud integration tests
4. RedisBackend with Lua atomic scripts, per-owner sorted set indexing, EXPIRE TTL with application-level enforcement, behind `redis` feature flag with 19 integration tests
5. Automated feature-flag verification: `make test-feature-flags` target and CI job testing all 4 feature combinations (none, dynamodb, redis, both) with zero doc-link warnings

**Requirements:** 22/22 satisfied (ABST-01..04, IMEM-01..03, DYNA-01..06, RDIS-01..05, TEST-01..04)

---


## v1.3 MCP Apps Developer Experience (Shipped: 2026-02-26)

**Phases completed:** 6 phases, 12 plans, 23 tasks
**Code changes:** +9,197 / -423 across 47 files
**Timeline:** 2026-02-24 → 2026-02-26

**Delivered:** Production-ready MCP Apps developer experience for the PMCP SDK — from `cargo pmcp app new` scaffolding through `cargo pmcp preview` with dual bridge modes to `cargo pmcp app build` for ChatGPT manifest and demo landing page generation, with 20 E2E browser tests proving the full widget pipeline.

**Key accomplishments:**
1. Session-persistent MCP proxy with resource picker, bridge call logging in DevTools, and connection status lifecycle in preview UI
2. WASM in-browser MCP client with proxy/WASM toggle, CallToolResult response normalization, and standalone widget-runtime.js polyfill
3. MCP Apps-aligned TypeScript bridge library (App, PostMessageTransport, AppBridge) eliminating ~250 lines of duplicated inline JavaScript
4. File-based widget authoring via WidgetDir with hot-reload disk reads, bridge auto-injection, and `cargo pmcp app new` CLI scaffolding
5. ChatGPT-compatible ai-plugin.json manifest generation and standalone demo landing pages with mock bridge
6. Chess, map, and dataviz MCP App examples with 20 chromiumoxide CDP browser tests across 3 widget suites

**Requirements:** 26/26 satisfied (PREV-01..07, WASM-01..05, DEVX-01..07, PUBL-01..02, SHIP-01..05)

### Known Tech Debt
- Dual `inject_bridge_script` implementations (mcp-preview vs pmcp core) — architectural decision, not a bug
- E2E tests use mock bridge injection (CDP), not the real postMessage bridge chain
- Unused API endpoints in preview server (GET /api/status, GET /ws)

---


## v1.4 Book & Course Update (Shipped: 2026-02-28)

**Phases completed:** 5 phases, 10 plans
**Content changes:** +8,140 / -2,066 across 40 files
**Timeline:** 2026-02-27 → 2026-02-28

**Delivered:** Complete documentation update for pmcp-book and pmcp-course — load testing chapters, MCP Apps chapter refreshes, quizzes, exercises, and cross-references wiring book and course content together.

**Key accomplishments:**
1. Book Ch 14 (Performance & Load Testing): 961-line comprehensive chapter covering `cargo pmcp loadtest` CLI, TOML config, flat/staged execution, HdrHistogram metrics, breaking point detection, coordinated omission, and CI/CD integration
2. Book Ch 12.5 (MCP Apps): 1294-line complete rewrite with WidgetDir file-based authoring, `cargo pmcp app` workflow, multi-platform adapter pattern, and chess/map/dataviz example walkthroughs
3. Course Ch 18-03: 952-line hands-on load testing tutorial with progressive difficulty from first test through capacity planning
4. Course Ch 20: 4 sub-chapters (1,646 lines total) rewritten with WidgetDir/mcpBridge paradigm, bridge communication, adapter pattern, and example walkthroughs
5. Course quizzes and exercises: ch18 quiz (10 questions), ch18 AI-guided exercise (6 phases), ch20 quiz refreshed (12→14 questions), SUMMARY.md updated

**Requirements:** 19/19 satisfied (BKLT-01..04, BKAP-01..04, CRLT-01..04, CRAP-01..03, CRQE-01..04)

### Known Tech Debt
- ch18-operations.toml quiz not embedded via `{{#quiz}}` in any course page
- loadtest.ai.toml exercise not embedded via `{{#exercise}}` in ch18-exercises.md
- ch19-exercises.md links to old ch20-applications.md instead of ch20-mcp-apps.md
- Orphaned ch20-applications.md still exists with stale sub-chapter links
- ch18-operations.md is a 1-line stub

---

