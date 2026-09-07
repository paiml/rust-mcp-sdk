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
- ✓ Examples cleanup: 17 orphans registered, 63 files role-prefixed (s/c/t/m), accurate PMCP README index, protocol badge 2025-11-25 — v2.1 (Phase 65)
- ✓ Macros documentation rewrite: deleted deprecated `#[tool]`/`#[tool_router]`/stub `#[prompt]`/`#[resource]` from pmcp-macros (898 LOC, 46% of crate); rewrote pmcp-macros/README.md from scratch (355 lines, 5 compiling `rust,no_run` doctests for all four `mcp_*` macros) wired via `#![doc = include_str!("../README.md")]`; published pmcp-macros 0.5.0 and pmcp 2.3.0 with full MACR-02 migration guide — v2.1 (Phase 66)
- ✓ Client host surface: server→client `sampling/createMessage` (incl. tools/tool_choice), `elicitation/create`, `roots/list` via builder-registered handler registry; preflight sampling approval; registry-authoritative capability derivation — v2.4 (Phase 106, pmcp 2.16.0)
- ✓ `pmcp-package` adopted + wire-frozen (pinned-digest golden fixtures for all four kinds); team-server tool contracts as provable-contracts YAML + conformance fixtures — v2.4 (Phase 107)
- ✓ `pmcp-agent` 0.x loop crate: pure loop between effect seams (`CompletionSource`/`ToolInvoker`/`ConversationStore`), `SamplingSource` + feature-gated `OpenAiCompatSource`/`AnthropicSource`, agent-as-server adapter, tasks-aware `poll_decision` — v2.4 (Phase 108)
- ✓ `pmcp-team-servers` reference crate: team-fs, mem-mcp (BM25), approval-mcp, team-mcp (members as agent-as-server tools), additive pmcp-core namespaced `_meta`, in-process `TeamRuntime`, exportable wire-level conformance harness — v2.4 (Phase 109)
- ✓ cargo-pmcp 0.18.0 agent/team verbs: `agent new`/`agent dev`, `team dev`, `package inspect|capture` with version-pin tripwires — v2.4 (Phase 110)
- ✓ All 7 v2.4 crates published to crates.io as pmcp v2.17.0 (2026-07-19, PR #302/tag v2.17.0)
- ✓ docs.rs pipeline and feature flags: `Cargo.toml` `[package.metadata.docs.rs]` replaced `all-features = true` with explicit 15-feature list + dual targets (`x86_64-unknown-linux-gnu` + `aarch64-unknown-linux-gnu` for first-class ARM64/Graviton coverage); created crate-focused `CRATE-README.md` at repo root (171 lines, 18-row Cargo Features table) wired into `src/lib.rs` via `#![doc = include_str!("../CRATE-README.md")]` (matches Phase 66 pmcp-macros pattern, pulls DOCD-02 from Future Requirements into scope); fixed all 29 rustdoc warnings across 16 source files (+8 residual links orchestrator-applied) via the "demote to backticks" pattern; adopted `feature(doc_cfg)` (post-RFC 3631 now provides auto-cfg badging by default — original D-01 `doc_auto_cfg` flip was invalidated by Rust 1.92.0 upstream removal, amended mid-phase); added new `make doc-check` target (stable toolchain, D-16 feature list, TAB-indentation guarded) and CI `Check rustdoc zero-warnings` step inside the existing `quality-gate` job (deliberately NOT chained into local `make quality-gate` per D-27 to protect developer iteration speed); no pmcp version bump (D-28 — stays at 2.3.0, docs.rs re-renders on next unrelated release); human-verify nightly badge checkpoint APPROVED — v2.1 (Phase 67)

- ✓ Version plumbing & negotiation (VERS-01..09): per-request `_meta` clientInfo, `server/discover`, required `Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version` headers, error-code rename `-32002`→`-32602` — v2.5 (Phase 112)
- ✓ Stateless streamable-HTTP + multi-round-trip elicitation (HTTP-01..08): `initialize`/`initialized` and `Mcp-Session-Id` removal path, `InputRequiredResult`/`requestState`; era resolved from the RAW request body so every method — not just the three `_meta`-bearing ones — can be a v2 request — v2.5 (Phase 113)
- ✓ Tasks extension migration (TASK-01..06): `tasks/list` removed, `tasks/update` added, server-directed creation; the v1.x DynamoDB/Redis task-store investment survived as an API reshape only — v2.5 (Phase 114)
- ✓ JSON Schema 2020-12 + caching hints (SCHM-*): `structuredContent` as any JSON value, `ttlMs`/`cacheScope`, position-aware `$schema` traversal derived from the pinned meta-schemas rather than hand-kept keyword lists — v2.5 (Phase 115)
- ✓ Auth-hardening SEPs (AUTH-*): RFC 9207 `iss` validation, DCR `application_type`, the six SEPs — v2.5 (Phase 116)
- ✓ v1 severability (SMPL-01/02, CLNT-03/04): default-on `v1-compat` plus a `full-v2` feature set severing v1 machinery at compile time via signature-identical paired modules, so call sites carry no `#[cfg]`; CI severance gate is a real merge blocker — v2.5 (Phase 117)
- ✓ Client & agents on v2 (CLNT-01/02/05): era-aware client, `mcp-tester --dual-run` diffing both eras against an `era-deltas.yaml` baseline — v2.5 (Phases 117, 118.2)
- ✓ Conformance against the official suite (CONF-*): nine gaps G-1..G-9 found and closed; `Content::Resource` emits the spec shape on both eras via a tolerant `#[serde(try_from)]` reader — v2.5 (Phases 118, 118.1, 118.2)
- ✓ Docs in three shapes (DOCS-*, carried from v2.4 Phase 111): pmcp-book v2 migration chapter, README/CHANGELOG protocol-era rewrite, course alignment — v2.5 (Phase 119)
- ✓ Published as pmcp v2.19.0 (2026-08-20, PR #337/tag v2.19.0)

### Active

<!-- Milestone v2.6 AI-Package Portability. Full text with traceability in .planning/REQUIREMENTS.md. -->
