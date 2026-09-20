# Phase 72: Investigate rmcp as Foundations for pmcp — Research

**Researched:** 2026-04-19
**Domain:** SDK architecture / foundational-crate adoption evaluation (protocol-layer delegation vs first-party)
**Confidence:** HIGH for source inventory and rmcp baseline facts; MEDIUM for effort sizing; LOW for downstream-impact forecasting (no user survey conducted in this phase)
**Nature:** Research / decision phase — no code is produced. Deliverable framing: give the planner what it needs to scope a comparison, strategy matrix, PoC proposal, and decision rubric.

## Summary

Phase 69 established that rmcp 1.5.0 has four High-severity ergonomics gaps versus pmcp (MACRO-02, HANDLER-02, HANDLER-05, CLIENT-02) and seeded three parity phases (70 shipped, 71 shipped, CLIENT-02 pending) to close them inside pmcp. Phase 72 inverts the framing: *what does pmcp re-implement that rmcp already does well enough that pmcp could delegate?* The objective is an actionable go/no-go recommendation on whether pmcp's protocol layer should be refactored to sit on top of rmcp, freeing pmcp to focus on its enterprise-DX differentiators (workflow engine, MCP Apps, typed tools, pmcp-code-mode, auth, Lambda deployment, cargo-pmcp tooling).

The pmcp workspace contains roughly **~21.7k lines in `src/types/` + `src/shared/`** — almost all of which maps 1:1 to rmcp's `model` + `transport` modules — versus roughly **~78k lines in `src/server/` + `src/client/`** that encode the pmcp DX surface (builder, typed wrappers, workflow, middleware, MCP Apps, auth, OAuth, streamable HTTP server, etc.). The 21.7k number is the rough upper bound on "deletable protocol code" under a full-adoption strategy; the 78k figure is the asset being preserved. [VERIFIED: `wc -l` against the current checkout at branch `main`, commit ahead of v2.4.0].

rmcp 1.5.0 is an actively-maintained, Anthropic-affiliated, Apache-2.0 crate with a 2–6-week release cadence, 74 total releases, a formal `modelcontextprotocol/rust-sdk` monorepo, and a `1.x.0` pattern suggesting semver discipline. It is the canonical "track the spec most closely" option in the ecosystem. [VERIFIED: crates.io cargo info + GitHub releases page]. There is **no published GOVERNANCE document, maintainer SLA, or breaking-change backport policy** visible via the repo contribution docs as of 2026-04-19 [CITED: `docs/CONTRIBUTE.MD` and CONTRIBUTING search returned 404s beyond a technical contribution guide] — this is the single biggest risk input to the decision.

**Primary recommendation:** Plan Phase 72 as a **three-deliverable research-only phase** producing (1) an inversion inventory (pmcp modules that duplicate rmcp functionality), (2) a scored strategy matrix covering the five architectural options (Full adopt / Hybrid / Selective borrow / Status quo / Fork), (3) a PoC proposal with ≤500-LOC scope on a single pmcp example. Do **not** plan the migration itself inside Phase 72 — the migration (if any) should be a separately-scoped v3.0 milestone after Phase 72's recommendation.

## Architectural Responsibility Map

Phase 72 is a decision/research phase; "capabilities" here are the research products, not runtime capabilities. Tier mapping applies to the *pmcp ecosystem itself* once the decision is made, so the planner has a consistent lens for both the PoC and the decision framework.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Protocol types (Request, Response, JsonRpc*, capabilities, content) | rmcp `model` (hypothesis) | pmcp types (current) | Spec-track burden — rmcp is the canonical tracker, pmcp currently duplicates |
| Wire framing / JSON-RPC framing | rmcp `service` | pmcp `src/shared/protocol.rs` | Protocol-layer concern — natural rmcp ownership |
| Transports (stdio, websocket, SSE, streamable-HTTP, Unix-socket, child-process) | rmcp `transport` | pmcp `src/shared/{stdio,websocket,sse_parser,streamable_http,…}.rs` | Wire concern — rmcp ships broader transport matrix including Unix-socket |
| Typed tool/prompt wrappers (TypedTool, TypedToolWithOutput, TypedPrompt, WasmTypedTool) | pmcp `src/server/` | — | pmcp enterprise DX — preserve |
| Server builder (`ServerCoreBuilder` fluent API) | pmcp `src/server/builder.rs` | — | pmcp enterprise DX — preserve (rmcp has no named builder, see Phase 69 BUILDER-01) |
| Workflow engine (`src/server/workflow/*`, ~5k LOC) | pmcp `src/server/workflow/` | — | pmcp enterprise exclusive — no rmcp equivalent |
| MCP Apps extension (`src/types/mcp_apps.rs`, widget handling) | pmcp `src/types/mcp_apps.rs` + `pmcp-widget-utils` | — | pmcp enterprise exclusive |
| Auth (bearer, OAuth, JWT, middleware) | pmcp `src/server/auth/*`, `src/client/oauth.rs` | rmcp `auth` feature (OAuth2 only) | pmcp has richer middleware-driven auth; rmcp has basic OAuth2 |
| Storage backends (task_store, in-memory, DynamoDB planned) | pmcp `src/server/task_store.rs`, `crates/pmcp-tasks` | — | pmcp enterprise exclusive |
| Middleware system (tool_middleware, http_middleware, observability) | pmcp `src/shared/middleware.rs` (1608 LOC) | tower ecosystem (shared) | pmcp enterprise DX — preserve |
| pmcp-macros (`#[mcp_tool]`, `#[mcp_server]`, `#[mcp_resource]`, rustdoc fallback from Phase 71) | pmcp-macros crate | rmcp-macros (peer) | pmcp enterprise DX — preserve |
| pmcp-code-mode (validation, policy, HMAC tokens, GraphQL/SQL/JS validators) | `crates/pmcp-code-mode/` | — | pmcp enterprise exclusive |
| mcp-tester (load testing, conformance, cloud upload) | `crates/mcp-tester/` | — | pmcp enterprise exclusive |
| mcp-preview (widget iframe harness) | `crates/mcp-preview/` | — | pmcp enterprise exclusive |
| cargo-pmcp (project scaffolding, init) | `cargo-pmcp/` | — | pmcp enterprise exclusive |

**Implication for the decision framework:** Protocol types + framing + transports (roughly `src/types/` and `src/shared/` minus the middleware and tower_layers sub-tree) is the candidate "delegate to rmcp" tier. Everything else is a pmcp differentiator and must survive any migration unchanged in external behavior.

## Phase Requirements

Phase 72 had **no phase-requirement IDs at research start** (the phase description explicitly says "TBD" and asks research to surface candidates). The following REQ-ID candidates are proposed for planner adoption and landing in REQUIREMENTS.md before execution:

| Proposed ID | Description | Research Support |
|----|-------------|------------------|
| RMCP-EVAL-01 | Produce a source-citation-backed inversion inventory: for every module in `src/types/` and `src/shared/` (and touching `src/server/cancellation.rs`), identify the nearest rmcp 1.5.0 equivalent and assess functional overlap (exact / partial / pmcp-superset / pmcp-exclusive) | See Inversion Inventory section below (seed matrix, 13 module families identified) |
| RMCP-EVAL-02 | Score each of five architectural options (Full adopt / Hybrid wrapper / Selective borrow / Status quo + upstream contribution / Fork rmcp) against the criteria rubric (maintenance reduction, migration cost, breaking-change surface, enterprise feature preservation, upgrade agility) | See Strategy Matrix section |
| RMCP-EVAL-03 | Propose 2–3 candidate PoC slices sized to ≤500 LOC each, each exercising a real pmcp feature (one tool registration end-to-end, one typed tool, one workflow step), and pick the minimum one that would prove or disprove the hypothesis | See PoC Scope Sizing section |
| RMCP-EVAL-04 | Produce a decision rubric with falsifiable thresholds (e.g., "if estimated maintenance hours/release ≥ X, adopt; if rmcp median bug-response time ≥ Y days, do not adopt"); rubric must be runnable without new data-gathering past what Phase 72 produces | See Decision Framework section |
| RMCP-EVAL-05 | Document risks and unknowns the planner cannot resolve without CONTEXT.md input: specifically the rmcp governance/SLA question, the pmcp user-base churn tolerance question, and the workspace downstream-crate ripple question (cargo-pmcp, mcp-tester, mcp-preview, pmcp-code-mode, pmcp-tasks) | See Open Questions section |

All five are research/decision products — none require code changes.

## Standard Stack

This is a research phase — no new runtime stack is introduced. The "stack" being compared is the **existing pmcp protocol layer** versus **rmcp 1.5.0**. For reference:

### Core (pmcp — protocol layer, the replacement candidate)
| Module family | LOC (approx) | Purpose | Re-implementation risk |
|---------------|--------------|---------|-----------------------|
| `src/types/` | ~8,700 | Protocol types (protocol, jsonrpc, capabilities, content, tools, prompts, resources, sampling, tasks, elicitation, auth, completable, mcp_apps, ui, notifications) | HIGH — nearly 1:1 overlap with rmcp `model` |
| `src/shared/` | ~13,000 | Transport + protocol helpers (stdio, websocket, sse, streamable_http, wasm_http, wasm_websocket, middleware, protocol, reconnect, session, connection_pool, event_store, context, logging, runtime, batch, cancellation, uri_template, peer, simd_parsing) | HIGH for transports (~6k LOC of `{stdio,websocket,sse_*,streamable_http,wasm_*,reconnect,connection_pool}.rs`); LOWER for middleware/session/observability (pmcp-specific enterprise layer) |
| `src/server/cancellation.rs` | 694 | `RequestHandlerExtra` — now carrying Extensions + peer handle from Phase 70 | LOW — just-shipped pmcp-specific shape; lives at the handler boundary not the protocol boundary |

### Core (rmcp — the candidate foundation) — rmcp 1.5.0
| Module | Purpose | Coverage notes |
|--------|---------|----------------|
| `rmcp::model` | All MCP spec types (Request, Response, JsonRpc*, capabilities, content, tools, prompts, resources, sampling, tasks, elicitation). [VERIFIED: docs.rs/rmcp/1.5.0/rmcp/model/index.html] | Covers spec surface; **no MCP Apps / UI resource types** (SEP-1865 is pmcp-exclusive as of 1.5.0) |
| `rmcp::service` | JSON-RPC framing, `Service` / `Peer<R>` / `RoleClient` / `RoleServer` / `ServiceExt::serve` | Mature; `Peer` is the linchpin for bidirectional RPCs |
| `rmcp::transport` | stdio, child-process, streamable-HTTP (client + server), Unix-socket, async-read-write, sink/stream adapter | Broader transport matrix than pmcp in some axes (Unix-socket native); narrower in others (**no dedicated WebSocket or SSE transport in 1.5.0** — `sse-stream` is for client-side-sse helper only) [CITED: docs.rs/rmcp/1.5.0/rmcp/transport/index.html] |
| `rmcp::handler` | `ServerHandler` + `ClientHandler` traits | Impl-block-oriented (Phase 69 MACRO-01, HANDLER-01) |
| `rmcp::task_manager` | Server-side task orchestration (requires `server` feature) | Overlaps with pmcp's `src/server/task_store.rs` + `crates/pmcp-tasks/` — comparison needed |

### Alternatives Considered (for the decision itself)
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Adopt rmcp | Status quo + upstream ergonomic PRs | Zero migration cost, full control of DX; keeps 21.7k LOC maintenance burden |
| Adopt rmcp | Fork rmcp into `vendor/` | Escape hatch — preserves control; bifurcates the ecosystem |
| Full adopt | Hybrid wrapper | Moderate savings, moderate churn; pmcp still owns a re-export facade |
| Full adopt | Selective borrow (e.g., transports only) | Lowest churn, smallest savings; complex because transports carry framing |

**Installation (verification):** `cargo info rmcp` confirmed version 1.5.0 is the current stable as of 2026-04-19. [VERIFIED: `cargo info rmcp` executed 2026-04-19]

**Version verification:**
```bash
cargo info rmcp     # 1.5.0 (matches Phase 69 pinned baseline)
cargo info pmcp     # 2.4.0 (local + crates.io)
```

[VERIFIED: both commands executed 2026-04-19; rmcp 1.5.0 published 2026-04-16]

## Inversion Inventory (the anchor research product)

This is the concrete translation of "what does pmcp re-implement that rmcp already does well enough?" Each row is a candidate pmcp module family mapped to its rmcp counterpart with an overlap rating. Planner uses this as the seed for RMCP-EVAL-01.

| pmcp module family | rmcp counterpart | Overlap | Gap from rmcp (if pmcp-superset) | Migration LOC impact |
|---|---|---|---|---|
| `src/types/jsonrpc.rs` (615 LOC) | `rmcp::model::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, JsonRpcError, RequestId}` | **EXACT** | None | High — deletable, re-export |
| `src/types/protocol/mod.rs` + `version.rs` (723 LOC) | `rmcp::model::{Request, RequestNoParam, RequestOptionalParam, Notification}` + version negotiation | **EXACT** | None identified beyond protocol version constants (pmcp ships `LATEST_PROTOCOL_VERSION = 2025-11-25`; rmcp tracks same spec) | High — deletable, re-export |
| `src/types/capabilities.rs` (793 LOC) | `rmcp::model::{ServerCapabilities, ClientCapabilities, ToolsCapability, PromptsCapability, ResourcesCapability, SamplingCapability, TasksCapability, RootsCapabilities, ElicitationCapability, FormElicitationCapability}` | **EXACT** for spec capabilities; pmcp has extra `CompletionCapabilities`, `LoggingCapabilities` (check whether rmcp covers these in `model` or under `handler`) | Completion + Logging capabilities need verification | High — mostly deletable |
| `src/types/tools.rs` (904 LOC) | `rmcp::model::{Tool, ToolAnnotations, CallToolRequest, CallToolRequestParams, CallToolResult, ToolChoice, ToolChoiceMode, ToolUseContent, ToolResultContent}` | **EXACT** | pmcp has deeper pmcp-specific TypedTool input/output schema plumbing in `src/server/typed_tool.rs` (kept, unaffected) | High — deletable |
| `src/types/prompts.rs` (500 LOC) | `rmcp::model::{Prompt, PromptMessage, PromptArgument, PromptReference, GetPromptRequest, GetPromptResult, ListPromptsRequest, ListPromptsResult}` | **EXACT** | None identified | High — deletable |
| `src/types/resources.rs` (405 LOC) | `rmcp::model::{Resource, ResourceTemplate, RawResource, RawResourceTemplate, ResourceContents, ResourceReference, EmbeddedResource, ReadResourceRequest, ReadResourceResult, SubscribeRequest}` | **EXACT** | None identified | High — deletable |
| `src/types/sampling.rs` (556 LOC) | `rmcp::model::{SamplingMessage, CreateMessageRequest, CreateMessageRequestParams, CreateMessageResult, ModelPreferences, ModelHint, ContextInclusion}` | **EXACT** | None identified | High — deletable |
| `src/types/content.rs` (649 LOC) | `rmcp::model::{Content, TextContent, ImageContent, AudioContent, RawContent}` | **EXACT** for base; pmcp may carry extra MCP Apps-style content wrappers | None on base content | High — deletable for base, pmcp keeps Apps wrappers |
| `src/types/elicitation.rs` (198 LOC) | `rmcp::model::{ElicitationSchema, CreateElicitationRequest, ElicitationAction, CreateElicitationResult}` + `ElicitationSchemaBuilder` | **EXACT** — rmcp offers a builder that pmcp does not | Builder ergonomics could also be adopted | High — deletable, potentially inherit builder |
| `src/types/tasks.rs` (499 LOC) | `rmcp::model::{Task, TaskList, TaskStatus, TaskSupport, TasksCapability, TaskRequestsCapability, CreateTaskResult, GetTaskResult, CancelTaskRequest}` | **EXACT** for types; pmcp has deeper task orchestration (task_store, polling, ToolCallResponse) in `src/server/task_store.rs` + `client/mod.rs::call_tool_and_poll` (CLIENT-05 strength from Phase 69, **preserve**) | task_store and `call_tool_and_poll` are DX — keep; base types deletable | Medium — deletable base, keep orchestration |
| `src/types/auth.rs` (376 LOC) | `rmcp::model::` — verify whether rmcp carries `AuthInfo` / `AuthScheme` types or treats auth at the handler boundary | **UNVERIFIED** — rmcp `auth` feature provides OAuth2 but model-level AuthInfo presence is unconfirmed | If rmcp lacks, pmcp keeps | Low — verify, then decide |
| `src/types/notifications.rs` (343 LOC) | `rmcp::model::{Notification, NotificationNoParam}` + per-notification variant types | **EXACT** for base; pmcp's notification channel surface in `Client` differs (CLIENT-03 gap, Phase 69 Medium) | pmcp channel is a DX choice, keep | High — deletable |
| `src/types/completable.rs` (508 LOC) | Verify whether rmcp has completion/completable types | **UNVERIFIED** — Phase 69 did not inventory completion API on rmcp side | If rmcp covers, deletable | Low — verify first |
| `src/types/mcp_apps.rs` (1639 LOC) + `src/types/ui.rs` (808 LOC) | **NO RMCP EQUIVALENT** | rmcp has no MCP Apps SEP-1865 support in 1.5.0 [VERIFIED: docs.rs model module list does not mention `UIResource`, `WidgetMeta`, `UIContent`] | pmcp keeps (enterprise exclusive) | Zero — keep |
| `src/shared/stdio.rs` (288 LOC) | `rmcp::transport::io::stdio` | **EXACT** | None | High — deletable |
| `src/shared/websocket.rs` (499 LOC) + `src/shared/wasm_websocket.rs` (264 LOC) | **rmcp 1.5.0 has no dedicated WebSocket transport** — only a generic `Sink/Stream` adapter | pmcp has first-class WebSocket | pmcp keeps (differentiator) | Zero — keep |
| `src/shared/sse_parser.rs` (501 LOC) + `sse_optimized.rs` (486 LOC) + `reconnect.rs` (595 LOC) | rmcp has `client-side-sse` helper via `sse-stream` dep (not a full parser) | Partial — pmcp's SIMD-accelerated parser and reconnect logic is a performance/ergonomics differentiator | pmcp keeps | Zero — keep as DX/perf differentiator |
| `src/shared/streamable_http.rs` (996 LOC) + `src/server/streamable_http_server.rs` (50k-char file) | `rmcp::transport::{StreamableHttpClientTransport, transport-streamable-http-server}` | **Partial overlap** — both implement the same spec; pmcp integrates with axum + DNS rebinding protection (Lambda feedback note in MEMORY.md) | pmcp keeps (Lambda/edge differentiator) OR delegates + layers security on top | Medium — hybrid-friendly |
| `src/shared/wasm_http.rs` (265 LOC) | **rmcp 1.5.0 has no wasm32 transport story documented** | pmcp keeps (WASM differentiator) | Zero — keep |
| `src/shared/connection_pool.rs` (683 LOC) | No direct rmcp equivalent in published docs | pmcp keeps | Zero — keep |
| `src/shared/protocol.rs` (431 LOC) + `src/shared/protocol_helpers.rs` (950 LOC) | `rmcp::service` does framing and Peer construction | **Partial** — pmcp's protocol.rs is lower-level framing; helpers are DX sugar | Mixed — helpers kept, framing deletable under Hybrid | Medium |
| `src/shared/middleware.rs` (1608 LOC) | rmcp has `tower` feature for Tower integration but no equivalent first-party middleware framework | pmcp keeps (enterprise exclusive, Phase 56) | Zero — keep |
| `src/shared/simd_parsing.rs` (611 LOC) | No rmcp equivalent | pmcp keeps (perf differentiator, cited on paiml.com blog as 16x over TS SDK) | Zero — keep |
| `src/shared/cancellation.rs` (144 LOC) + `src/server/cancellation.rs` (694 LOC) | `rmcp::service::RequestContext<RoleServer>` via `tokio_util::sync::CancellationToken` | **Similar shape** — pmcp now carries Extensions + peer (Phase 70) giving shape parity | None after Phase 70 | Low — consider Hybrid integration |
| `src/shared/session.rs` (479 LOC) | rmcp handles session in `transport-streamable-http-server-session` feature | Overlap needs verification | Likely keep — pmcp session includes auth plumbing | Zero to Low |
| `src/shared/batch.rs` (233 LOC) | Verify — rmcp JSON-RPC batching status unknown | Needs verification | Possibly deletable | Low |
| `src/shared/uri_template.rs` (619 LOC) | rmcp `resources` likely uses its own URI template impl | Independent impls | Low-stakes duplication | Keep or evaluate |
| `src/shared/event_store.rs` (421 LOC) | No rmcp equivalent | pmcp keeps | Zero — keep |
| `src/shared/logging.rs` (556 LOC) | rmcp uses `tracing` directly | Overlap needs verification | Likely keep | Zero to Low |

**Rough totals:**
- **Strong-overlap, likely-deletable under Full adopt:** ~5,200 LOC in `src/types/` (jsonrpc, protocol, capabilities, tools, prompts, resources, sampling, content base, elicitation base, tasks base, notifications base) + ~1,200 LOC of `src/shared/` (stdio + protocol framing) = **~6,400 LOC deletable**.
- **Conditional / hybrid-friendly:** ~2,500 LOC (streamable_http, session, batch, logging — pmcp might delegate wire but keep enterprise concerns).
- **Pmcp-exclusive, must preserve:** mcp_apps + ui (~2,400 LOC) + websocket/wasm_websocket (~760 LOC) + sse_parser+sse_optimized+reconnect (~1,600 LOC) + middleware (~1,600 LOC) + simd_parsing (~611 LOC) + auth types (~376 LOC) + connection_pool (~683 LOC) + event_store (~421 LOC) + uri_template (~619 LOC) = **~9,000 LOC retained**.

The inversion says: **Full-adopt saves ~6k LOC from `src/types/` + ~1k from `src/shared/`**, with ~2.5k conditional depending on hybrid detail. That's a real but not enormous savings — call it ~8k LOC of mostly-boilerplate wire-format types, versus pmcp's ~78k LOC DX surface. The maintenance burden on those 8k LOC is modest because they change only when the MCP spec changes (~quarterly per rmcp's release cadence).

## Architecture Patterns

### System Architecture Diagram

```
                ┌──────────────────────────────────────────────┐
                │    pmcp USER CODE                             │
                │    (server handlers, typed tools, workflow)  │
                └───────────────────┬──────────────────────────┘
                                    │
        ┌───────────────────────────┴─────────────────────────┐
        │                pmcp DX LAYER (preserved)             │
        │  • ServerCoreBuilder, ClientBuilder                  │
        │  • #[mcp_tool] / #[mcp_server] / #[mcp_prompt]       │
        │  • TypedTool / TypedToolWithOutput / TypedPrompt     │
        │  • Workflow engine (src/server/workflow/)            │
        │  • Auth / OAuth / JWT middleware                     │
        │  • pmcp-code-mode, pmcp-tasks, MCP Apps extension    │
        │  • mcp-tester, mcp-preview, cargo-pmcp               │
        └────────────┬─────────────────────────┬───────────────┘
                     │                         │
     ┌───────────────▼──────────────┐  ┌───────▼────────────────┐
     │  pmcp-OWNED TRANSPORTS       │  │  pmcp-OWNED TYPES       │
     │  (WebSocket, WASM, SSE       │  │  (MCP Apps, UI, auth,   │
     │   parser, streamable_http    │  │   Logging extensions,   │
     │   security, connection_pool) │  │   pmcp-specific errors) │
     └───────────────┬──────────────┘  └───────┬────────────────┘
                     │                         │
      ═══════════════╪═════════════════════════╪═══════════════════
                     │                         │
                     │         HYPOTHESIS:     │
                     │         delegate below  │
                     │         this line       │
                     │                         │
      ═══════════════╪═════════════════════════╪═══════════════════
                     │                         │
     ┌───────────────▼─────────────────────────▼────────────────┐
     │  rmcp 1.5.0 FOUNDATION                                    │
     │  • model (Request/Response/JsonRpc*/capabilities/content) │
     │  • service (Peer, ServiceExt, framing)                    │
     │  • transport (stdio, streamable-http, child-process,      │
     │               Unix-socket, IO async-read-write)           │
     │  • handler (ServerHandler, ClientHandler, default impls)  │
     │  • task_manager                                            │
     └───────────────────────────────────────────────────────────┘
```

The double line shows the decision boundary Phase 72 is evaluating.

### Recommended Decision Products Structure (for the planner)

```
.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp.../
├── 72-RESEARCH.md                 # THIS FILE (planner input)
├── 72-CONTEXT.md                  # user locks scope + open-question answers
├── 72-PLAN-01.md                  # Inversion inventory + strategy matrix (RMCP-EVAL-01, RMCP-EVAL-02)
├── 72-PLAN-02.md                  # PoC scoping + decision rubric (RMCP-EVAL-03, RMCP-EVAL-04)
├── 72-PLAN-03.md                  # Recommendation doc + v3.0 roadmap sketch (RMCP-EVAL-05)
├── 72-INVENTORY.md                # Product of Plan 01
├── 72-STRATEGY-MATRIX.md          # Product of Plan 01
├── 72-POC-PROPOSAL.md             # Product of Plan 02
├── 72-DECISION-RUBRIC.md          # Product of Plan 02
└── 72-RECOMMENDATION.md           # Product of Plan 03 — the go/no-go
```

### Pattern 1: Inversion Inventory Pattern
**What:** For every module in `src/types/` and `src/shared/`, answer three questions with citations: (a) What does this module do? (b) What rmcp module/type provides equivalent functionality? (c) What would be lost if pmcp replaced this module with a re-export of rmcp? Cite file:line for every claim on both sides.

**When to use:** Any time a project considers delegating a foundational capability to an external crate. This is the Phase 69 methodology inverted.

**Example (seeded from Phase 69):**
```
Row: jsonrpc types
pmcp: src/types/jsonrpc.rs:1-615 — defines JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, JsonRpcError, RequestId
rmcp: rmcp::model::{JsonRpcMessage, JsonRpcRequest, …}  (docs.rs/rmcp/1.5.0/rmcp/model)
Functional loss on replace: None identified
Migration LOC impact: ~615 LOC deletable; ~N public imports need redirection
Semver impact: Major — public type identity changes
```

### Pattern 2: Strategy Matrix with Weighted Criteria
**What:** Score each of the five architectural options against a fixed criteria rubric. Criteria from the phase description: maintenance reduction, migration cost, breaking-change surface, enterprise feature preservation, upgrade agility with MCP spec.

**When to use:** Go/no-go decisions across multiple non-commensurable axes.

**Example rubric (planner picks weights in CONTEXT.md):**

| Option | Maintenance reduction | Migration cost | Breaking-change surface | Enterprise feature preservation | Spec-upgrade agility |
|---|---|---|---|---|---|
| A. Full adopt | HIGH (~6–8k LOC gone) | HIGH (every import/API shape breaks) | HIGH (pmcp 3.0 major) | 100% preserved if hybrid facade ships | HIGH (rmcp tracks spec, pmcp ships next day) |
| B. Hybrid / wrapper | MEDIUM | MEDIUM (re-export facade) | MEDIUM (some imports stable via facade) | 100% preserved | MEDIUM (still need to wrap new rmcp surfaces) |
| C. Selective borrow (transports only) | LOW-MEDIUM (~1–2k LOC) | LOW-MEDIUM | LOW (transport traits only) | 100% preserved | LOW (types still first-party) |
| D. Status quo + upstream PRs | ZERO | ZERO | ZERO | 100% preserved | LOW (pmcp tracks spec manually, rmcp doesn't help) |
| E. Fork rmcp into vendor/ | NEGATIVE (now maintaining two copies) | HIGH | HIGH | 100% preserved | LOW |

### Pattern 3: PoC Slice Discipline
**What:** A PoC that proves/disproves the hypothesis must (a) exercise the real DX surface end-to-end, not a mock, (b) fit inside a single example file or a tiny module, (c) have a clear pass/fail test: does pmcp's TypedTool still work when its underlying Request/Response type is an rmcp re-export?

**When to use:** Any migration-feasibility question. Pick the single thinnest slice that hits the hypothesis.

**Example slices (shortlisted — planner picks the minimum):**
1. **"Re-export types only"** — ~100 LOC change. Delete one pmcp type (e.g., `JsonRpcRequest`), replace with `pub use rmcp::model::JsonRpcRequest`. Run `cargo check --workspace` and measure breakage. Passes if: pmcp still compiles with ≤ 100 downstream rename changes.
2. **"Run one server example on rmcp service layer"** — ~400 LOC change. Port `examples/s01_basic_server.rs` to construct its server via `rmcp::service::serve_server(...)` instead of `ServerCoreBuilder::build()`, wrapping pmcp's `ToolHandler` in an rmcp `ServerHandler` adapter. Passes if: the example runs end-to-end against the existing mcp-tester conformance suite.
3. **"Workflow engine on rmcp Peer"** — ~500 LOC change + new adapter. Convert `src/server/workflow/task_prompt_handler.rs` to use `rmcp::service::Peer<RoleServer>` for its server-to-client callbacks instead of pmcp's `PeerHandle` (Phase 70). Passes if: workflow DSL tests still pass with identical semantics.

### Anti-Patterns to Avoid
- **"Big-bang migration plan before the PoC"** — Phase 72 must end with a recommendation and (if positive) a PoC report, not with a full migration plan. Migration planning is v3.0 work, scoped after Phase 72.
- **"Score without weights"** — An unweighted strategy matrix is an aesthetic exercise. Weights come from CONTEXT.md user decisions, not from the researcher.
- **"Compare against rmcp main instead of 1.5.0"** — Phase 69 pinned rmcp 1.5.0. Phase 72 inherits the pin. Unreleased rmcp work does not count for decision weighting (see Phase 69 D-02).
- **"Re-do Phase 69's ergonomics gap analysis"** — Phase 69 already produced the 32-row matrix. Phase 72 inverts framing, not re-performs.
- **"Propose adopting rmcp without surveying downstream pmcp users"** — pmcp 2.x has unknown external adoption; breaking-change impact is a real input that Phase 72 research alone cannot resolve (becomes CONTEXT.md input).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Decision scoring framework | Custom Rust scoring engine | Fixed markdown strategy matrix with planner-defined weights | Decisions need deliberate human weighting, not automation |
| LOC-impact measurement on a migration | Hypothetical estimates | `cargo check --message-format=json` counting actual breakage on a PoC branch | Ground truth matters — hypothetical breakage estimates are wrong 50%+ of the time |
| rmcp-surface inventory | Hand-typed from docs | `cargo expand` + `cargo public-api` on a pmcp test crate that uses each rmcp module | Machine-extracted surface avoids typos and catches re-exports |
| Maintenance-hours estimate | Guessing | Count git-log commits touching `src/types/` and `src/shared/` over the last 6 months, categorize by spec-tracking vs feature vs bugfix | Historical data exists; estimation is unnecessary |
| rmcp governance SLA | Infer from repo | Open an issue in modelcontextprotocol/rust-sdk asking the maintainers directly | Direct inquiry is faster and more accurate than inference |

**Key insight:** Decision phases have different "don't hand-roll" hazards than implementation phases. The hazard is over-building scoring tools and under-investing in ground-truth data collection (git history, actual rmcp API usage counts, live PoC).

## Comparable Precedents

Rust ecosystem SDKs that delegated protocol/wire concerns to a foundation crate and focused their identity on DX above it. Each row shows what worked and what pitfalls surfaced.

| SDK | Foundation | Pattern | What worked | Pitfall |
|-----|-----------|---------|-------------|---------|
| **tonic** (gRPC) | **prost** (protobuf codegen) + **hyper** (HTTP/2) + **tower** (service) | Layered — prost owns types, tonic owns DX + codegen integration | tonic never re-implemented protobuf; prost ships codegen, tonic glues to tower | Version-pin coupling: tonic's prost minor version must match — breaking changes ripple [VERIFIED: medium post, hyperium/tonic README] |
| **axum** (web framework) | **hyper** (HTTP) + **tower** (middleware) | Thin layer on hyper, deep tower integration | axum handlers *are* tower services; no middleware reinvention | axum 0.7 broke almost every handler sig when upgrading hyper 1.0 — breaking-change surface is inherited [CITED: tokio.rs/blog/2023-11-27-announcing-axum-0-7-0] |
| **reqwest** (HTTP client) | **hyper** (HTTP) | Full ergonomic wrapper hiding hyper | reqwest is the most-downloaded HTTP client in Rust; hiding hyper worked | Advanced users need `hyper` escape hatch regularly; reqwest ships `hyper` types intentionally |
| **sqlx** (DB) | Various database drivers (per-DB) | Selective borrow — sqlx owns DX, borrows wire-level drivers per backend | One DX API across Postgres/MySQL/SQLite | Macro-based compile-time query checking is sqlx-exclusive and adds its own maintenance burden |
| **pmcp** (MCP) | **? rmcp (candidate)** | TBD | **Decision product of Phase 72** | Unique risk: unlike hyper (stable since 1.0), rmcp is on 1.x.0 but has released 74 versions and has no published semver SLA |

**Patterns that worked:**
1. Keep protocol/wire code in the foundation; keep DX/macros/middleware in the SDK (tonic/prost model).
2. Expose the foundation's types publicly so advanced users can escape-hatch (reqwest/hyper model).
3. Pin the foundation version conservatively and batch upgrades (axum/hyper model).

**Pitfalls that hurt:**
1. Ripple breaking changes — every foundation minor can force SDK major (axum 0.7 on hyper 1.0).
2. Feature-flag fragmentation between foundation and SDK (tonic+prost feature flag matrix).
3. Double documentation burden (users need both foundation and SDK docs) — reqwest solves this by redocumenting hyper types in-place.

**Implication for pmcp/rmcp:** A Hybrid strategy matching tonic/prost is the canonical precedent. pmcp keeps its DX, workflow, macros, and enterprise features; rmcp owns wire and types. Ripple-breaking-change risk is real and should shape the semver policy Phase 72 recommends.

## Strategy Matrix (the second anchor product)

A first-pass strategy comparison for planner to refine. Scoring is directional, not final — CONTEXT.md weights will finalize.

### Option A: Full adopt

- **What:** pmcp re-exports all of rmcp's model + service + transport. Deletes `src/types/` (except mcp_apps, ui, auth extensions) and `src/shared/{stdio,protocol,transport}.rs`. Refactors handlers to consume rmcp's `RequestContext<RoleServer>` under the hood (possibly wrapped by pmcp's `RequestHandlerExtra` for API continuity).
- **Savings:** ~8k LOC gone; spec-tracking burden transfers to rmcp.
- **Cost:** pmcp 3.0 major; every public type identity changes; every integration downstream breaks unless carefully facaded.
- **Breaking-change surface:** Maximum — `pmcp::types::X` stops being `struct pmcp::types::X` and becomes `pub use rmcp::model::X`. Downstream code that matches on pmcp-specific enum variants, uses trait impls owned by pmcp, or pattern-matches on struct field ordering all break.
- **Enterprise feature preservation:** 100% in theory, risky in practice — pmcp's workflow engine and typed tools touch types at every boundary; a type re-identity change at the foundation cascades. Needs a PoC to verify.
- **Semver compatibility with v2.x breaking-change window:** Compatible but costly — MEMORY.md notes "During breaking-change window, consolidate aggressively — don't defer as 'not worth the churn'" which supports this option **if** the breaking window is still open. Needs CONTEXT.md decision.

### Option B: Hybrid / wrapper (recommended directionally — needs CONTEXT.md confirmation)

- **What:** pmcp depends on rmcp as a foundation crate. Imports rmcp's model types and re-exports them at the pmcp crate root with a compatibility facade (`pub use rmcp::model::Request as Request;`). Keeps pmcp's public API shape stable where possible. Uses rmcp for transport where the pmcp transport has no enterprise value-add (stdio, child-process, Unix-socket). Keeps pmcp transports where pmcp has a real differentiator (WebSocket first-class, WASM, security-hardened streamable-HTTP, SIMD SSE parser).
- **Savings:** ~5–6k LOC gone (types + stdio), ~2–3k LOC conditional.
- **Cost:** Medium refactor; facade maintenance overhead replaces spec-tracking overhead; some API shape stabilizes, some breaks.
- **Breaking-change surface:** Medium — a v3.0 bump is plausible but much less severe than Option A. Facade can mask most type-identity changes except where pmcp owned impls rmcp doesn't ship (e.g., custom `From`s).
- **Enterprise feature preservation:** 100%, naturally — pmcp keeps ownership of MCP Apps, workflow, auth, middleware, and all its pmcp-exclusive transports.
- **Precedent:** tonic/prost, axum/hyper. Proven pattern.

### Option C: Selective borrow (transports-only or types-only)

- **What:** pmcp depends on rmcp only for a narrow slice. Example: "pmcp adopts rmcp's Unix-socket transport because pmcp has none" or "pmcp adopts rmcp's `task_manager` because pmcp-tasks is still experimental." Types and handlers stay first-party.
- **Savings:** Small (~500–1500 LOC per subsystem borrowed).
- **Cost:** Low — limited ripple, but pmcp keeps spec-tracking burden for everything else.
- **Breaking-change surface:** Minimal — borrowed subsystem is new surface, not a replacement of existing surface.
- **Enterprise feature preservation:** 100%.
- **Trade-off:** Low savings. Only worth it if a specific rmcp subsystem is a large pmcp gap (e.g., WASM transport story, Unix-socket) and pmcp is not going to build it independently.

### Option D: Status quo + upstream contribution

- **What:** pmcp keeps its protocol layer fully first-party. Any ergonomic gaps identified in rmcp that pmcp has better answers to get upstreamed as PRs to rmcp (unilateral contribution). Zero migration.
- **Savings:** ZERO maintenance savings; potentially small "ecosystem goodwill" savings.
- **Cost:** ZERO migration; small ongoing cost of upstream PRs if pmcp pursues them.
- **Breaking-change surface:** ZERO.
- **Enterprise feature preservation:** 100%.
- **Risk:** The "credibility gap" the v2.1 Core Value names remains — rmcp stays the canonical "official" SDK, pmcp is still "the other one." Not a migration question; a positioning question. Also: pmcp continues to pay the spec-tracking maintenance tax every ~6 weeks when MCP spec revs.

### Option E: Fork rmcp

- **What:** pmcp vendors rmcp source into `crates/vendor/rmcp/` and owns all changes locally. Never takes upstream updates; becomes a permanent pmcp-controlled fork.
- **Savings:** NEGATIVE — pmcp now maintains its own protocol layer AND rmcp's.
- **Cost:** HIGH — bifurcates the ecosystem and doubles the protocol-layer maintenance burden.
- **Breaking-change surface:** Low (user-facing) but high (internal).
- **Enterprise feature preservation:** 100%.
- **When to use:** Only as an escape hatch when a critical rmcp bug requires an immediate patch and upstream is unresponsive. Not a primary strategy.

## PoC Scope Sizing (third anchor product)

Planner picks one PoC slice to execute in Phase 72 or a follow-on phase. Each slice proves or disproves a specific sub-hypothesis.

### Slice 1: Types re-export feasibility (~100 LOC touched, ~½ day)

**Hypothesis tested:** "pmcp's internal code can tolerate rmcp types being re-exported in place of pmcp types."

**What to do:** On a branch, replace `src/types/jsonrpc.rs` with `pub use rmcp::model::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, JsonRpcError, RequestId};`. Run `cargo check --workspace --all-features`. Measure:
- Number of compile errors (downstream types that depended on pmcp's specific impls)
- Number of `From`/`Into` impls that need re-adding
- Whether serde derives roundtrip the same JSON (`serde_json::to_value` on both sides)

**Pass criterion:** ≤ 50 compile errors, ≤ 5 missing impls; JSON roundtrip identical.

**Disqualifying outcome:** > 200 compile errors or JSON roundtrip differs → pmcp's custom serde shape is load-bearing and Full-adopt is probably infeasible.

### Slice 2: One typed tool on rmcp service layer (~400 LOC touched, ~2 days)

**Hypothesis tested:** "pmcp's `TypedTool` + `#[mcp_tool]` can compose on top of rmcp's `ServerHandler`/`Service` without losing its ergonomics."

**What to do:** Create `examples/z01_rmcp_foundation_poc.rs` (one-off non-committed example). Construct a server using `rmcp::service::serve_server(stdio_transport, pmcp_handler_adapter)` where `pmcp_handler_adapter` wraps a pmcp `ToolHandler` and translates pmcp's `CallToolRequest` to rmcp's `CallToolRequestParams`. Ship one `#[mcp_tool]`-decorated tool. Run the example against `mcp-tester` conformance suite.

**Pass criterion:** Tool registers, responds to `tools/list` and `tools/call`, conformance suite green.

**Disqualifying outcome:** Conformance fails or adapter needs > 400 LOC → integration friction too high for Hybrid strategy.

### Slice 3: Workflow engine Peer adapter (~500 LOC touched, ~3 days)

**Hypothesis tested:** "pmcp's workflow engine (Phase 70 `PeerHandle`) can run on rmcp's `Peer<RoleServer>` without workflow DSL regressions."

**What to do:** Adapt `src/server/workflow/task_prompt_handler.rs` to optionally use `rmcp::service::Peer<RoleServer>` behind the `PeerHandle` trait. Run `examples/s31_workflow_minimal.rs` and `examples/s33_workflow_dsl_cookbook.rs` unchanged against the rmcp-backed server.

**Pass criterion:** Workflow examples run end-to-end with identical observable behavior.

**Disqualifying outcome:** DSL semantics drift or workflow-specific `Peer` method missing upstream → rmcp's peer surface is too narrow for pmcp's workflow engine, Hybrid requires surface extension PRs first.

### Recommendation

Slice 1 is a **cheap go/no-go signal on Full adopt**. Do it first; if it fails, Full adopt is off the table.

Slice 2 is the **minimum scope that proves Hybrid strategy works** for the typical pmcp user path. Do this second.

Slice 3 is only needed if pmcp's workflow engine has been identified as the critical migration risk. Defer until Slices 1+2 pass.

## Decision Framework (fourth anchor product)

Falsifiable thresholds the planner can convert into gate criteria in 72-DECISION-RUBRIC.md. All thresholds are illustrative defaults — CONTEXT.md finalizes.

### Criteria with illustrative thresholds

| Criterion | Threshold (illustrative) | Decision if met | Data source |
|-----------|---------------------------|-----------------|-------------|
| Maintenance hours/release spent on spec-tracking | ≥ 16 hours/release for the last 3 releases | → Adopt (Option A or B) | `git log --since=6-months src/types/ src/shared/ | wc -l`, classified |
| rmcp median issue response time | < 7 days on High-severity bug reports | → Adopt safe (OK to depend) | `gh issue list -R modelcontextprotocol/rust-sdk --state closed --json closedAt,createdAt` |
| rmcp median issue response time | ≥ 14 days | → Stay or Fork escape hatch | Same |
| PoC Slice 1 outcome | ≤ 50 compile errors | → Full adopt feasible | `cargo check --message-format=json` |
| PoC Slice 1 outcome | > 200 compile errors | → Full adopt infeasible; Hybrid only | Same |
| PoC Slice 2 outcome | Conformance suite green within 400 LOC | → Hybrid preserves enterprise features | mcp-tester run |
| PoC Slice 2 outcome | Conformance fails or > 400 LOC | → Hybrid has deeper blockers | Same |
| Downstream pmcp users surveyed | ≥ 3 known production users; ≥ 2 tolerate a v3.0 breaking change | → Adopt window open | CONTEXT.md or survey |
| Downstream pmcp users surveyed | ≥ 3 known production users; none tolerate v3.0 | → Status quo + Option D | Same |
| pmcp v2.x breaking-change window still open | Policy check from PROJECT.md / MEMORY.md | → Full adopt fits | PROJECT.md reading |

### Decision tree

```
1. Run Slice 1. Does it pass?
   ├─ YES → continue to step 2
   └─ NO  → Option A (Full adopt) disqualified. Consider B/C/D.

2. Run Slice 2. Does it pass within budget?
   ├─ YES → continue to step 3
   └─ NO  → Option B (Hybrid) needs deeper scope. Consider C (Selective borrow) or D (Status quo).

3. Is pmcp's v2.x breaking-change window still open per PROJECT.md?
   ├─ YES → Option A or B is in play; decide on savings vs migration cost tradeoff
   └─ NO  → Option D or C preferred; defer foundation-adoption to next major window

4. Is rmcp's governance responsiveness acceptable?
   ├─ YES → Option A or B acceptable
   └─ NO  → Reject A, reject B without an escape hatch; consider E (Fork) only as insurance

5. If A or B selected: publish v3.0 roadmap sketch with Phase 72 as the precursor document.
   If D selected: publish upstream-contribution plan and close Phase 72.
```

## Rmcp Trajectory Risk

[VERIFIED: crates.io cargo info, 2026-04-19]
- **Baseline:** rmcp 1.5.0, published 2026-04-16 (same-day as Phase 69 pinning; confirmed by GitHub release listing)
- **Cadence:** 2–6 week minor releases; 74 releases total since project start
- **Versioning:** Has reached 1.x milestone; last three minors (1.3, 1.4, 1.5) each added features without documented breaking changes per release listing
- **Ownership:** `modelcontextprotocol` GitHub org (Anthropic-affiliated)
- **License:** Apache-2.0 (MIT-compatible with pmcp's MIT)

[CITED: GitHub releases page, 2026-04-19]
- Release 1.3.0 explicitly called out "use cfg-gated Send+Sync supertraits to avoid semver break" — suggests semver awareness, no formal SLA
- No published GOVERNANCE.md, MAINTAINERS.md, or semver policy document
- `docs/CONTRIBUTE.MD` covers technical contribution hygiene only

[ASSUMED: not verified in this session]
- Whether rmcp maintainers respond to external issues within N days — unmeasured; Phase 72 should collect this via `gh issue list --state closed` historical analysis
- Whether pmcp-specific bugs in rmcp would be fast-tracked — unmeasured; depends on maintainer goodwill; precedent in the Rust ecosystem is mixed

**Version-pinning + patch-fork strategy (recommended if Hybrid adopted):**
1. Pin exact rmcp minor in pmcp Cargo.toml (not `^1.5` — use `=1.5.0` or `~1.5`)
2. Maintain a `vendor/rmcp-patches/` directory with tracked diffs Cargo can apply via `[patch.crates-io]`
3. Upstream every patch; retire vendored patches as upstream accepts them
4. Formal escape hatch: if a critical bug goes > 14 days without response, fork to `crates/vendor/rmcp/` as Option E describes

## Runtime State Inventory

Phase 72 is a **pure research/decision phase** — no code is written, no configuration is edited, no services are touched.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no database or datastore is touched | None |
| Live service config | None — no services reconfigured | None |
| OS-registered state | None — no installed-process changes | None |
| Secrets/env vars | None — no auth secrets involved | None |
| Build artifacts | None — no build products; only markdown deliverables | None |

Nothing found in any category: verified by phase description ("research / decision phase, not an implementation phase") and by the explicitly markdown-only deliverable list.

## Common Pitfalls

### Pitfall 1: Treating Phase 72 as a migration phase
**What goes wrong:** Scope creeps to "let's just do a small PoC migration" and the decision document gets buried under implementation details.

**Why it happens:** Engineers default to coding. The boundary between "decision research" and "small PoC" is culturally fuzzy.

**How to avoid:** Lock the phase boundary in CONTEXT.md: Phase 72 delivers ≤ 5 markdown files, executes ≤ 1 PoC slice, produces a recommendation. The migration itself, if recommended, is v3.0 work in a separate phase.

**Warning signs:** Plans start naming files outside `.planning/phases/72-.../`; PoC branch grows past 500 LOC; the recommendation document is < 1 page.

### Pitfall 2: Comparing rmcp main against pmcp main
**What goes wrong:** The researcher pulls rmcp main for a "current" comparison, loses reproducibility, and scores against an unreleased rmcp surface.

**Why it happens:** rmcp ships fast; the pin feels stale within weeks.

**How to avoid:** Inherit Phase 69's pin: rmcp 1.5.0. Document the pin at the top of every deliverable. If a new rmcp release ships during the phase, the pin stays — re-evaluate in a follow-on phase.

**Warning signs:** Citations link to rmcp `main` branch instead of the 1.5.0 tag.

### Pitfall 3: Conflating "we have to" with "we should"
**What goes wrong:** The v2.1 Core Value ("Close credibility and DX gaps where rmcp outshines PMCP") reads as an anxiety signal, and the team adopts rmcp to resolve the anxiety rather than because the numbers justify it.

**Why it happens:** Ecosystem positioning pressure is real and emotional.

**How to avoid:** The decision rubric must be falsifiable. If the rubric says "don't adopt," the team doesn't adopt, even if rmcp feels more popular. Ecosystem credibility is a separate research question from foundation-adoption feasibility.

**Warning signs:** Recommendation document justifies adoption primarily by "rmcp is the official SDK" rather than by measured maintenance savings or concrete DX improvements.

### Pitfall 4: Missing the downstream-user question
**What goes wrong:** Phase 72 recommends adopt; v3.0 ships; users on pmcp 2.x refuse to migrate and fork the last pmcp 2.x release.

**Why it happens:** Decision framework didn't weight downstream-user churn.

**How to avoid:** Explicit criterion in the rubric: "Are there known production users? Do they tolerate v3.0?" Get answers via CONTEXT.md or a lightweight user survey before plan-checker signs off.

**Warning signs:** CONTEXT.md has no entry for user adoption; the recommendation treats adoption as a purely technical decision.

### Pitfall 5: Treating pmcp's LOC count as the total migration cost
**What goes wrong:** Estimate says "6k LOC delete, ~1 week work"; actual migration is 3 months because of downstream ripple in mcp-tester, cargo-pmcp, pmcp-code-mode, pmcp-tasks, and every example.

**Why it happens:** LOC is a first-order estimator that ignores ripple.

**How to avoid:** PoC Slice 1 specifically measures ripple, not just deletable LOC. Migration cost estimate must account for every workspace crate, not just `pmcp` itself.

**Warning signs:** Migration-cost estimate cites only `src/types/` and `src/shared/` LOC.

## Code Examples

This is a research phase — no new pmcp code is written. The relevant "patterns" are citations to existing code that Phase 72 deliverables will reference:

### Pattern A: How pmcp currently exposes its protocol types
```rust
// Source: src/types/mod.rs:36 (current)
pub use protocol::*;  // re-exports every type from the protocol sub-module

// Source: src/lib.rs:55 (current)
pub use error::{Error, ErrorCode, Result};
```

### Pattern B: How rmcp exposes its model types
```rust
// Source: rmcp 1.5.0 docs.rs/rmcp/1.5.0/rmcp/model/index.html
use rmcp::model::{Request, JsonRpcMessage, ServerCapabilities, CallToolRequestParams, Content, …};
```

### Pattern C: What a Hybrid facade would look like (illustrative, not code to write in Phase 72)
```rust
// NOT CODE — planner only. Hypothetical v3.0 src/types/protocol/mod.rs:
pub use rmcp::model::{
    JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, JsonRpcError,
    Request, Notification, RequestId, Tool, CallToolRequest, CallToolResult,
    ServerCapabilities, ClientCapabilities, // … all spec types
};

// pmcp-exclusive types remain first-party:
pub use super::mcp_apps::*;   // MCP Apps extension
pub use super::ui::*;          // UIResource, WidgetMeta
pub use super::auth::*;        // AuthInfo, AuthScheme (verify rmcp absence first)
```

## State of the Art

Ecosystem motion relevant to the decision.

| Old approach | Current approach | When changed | Impact |
|--------------|------------------|--------------|--------|
| MCP spec re-implemented per language SDK | Official SDK per language, others layer on top | 2025–2026 | rmcp has emerged as canonical Rust SDK; pmcp has repositioning choice |
| Rust crates hand-roll wire formats | Depend on protocol crate (prost, hyper, rmcp) | 2020–present | Tonic/axum/reqwest precedent favors layering |
| SDKs pin foundations loosely | Pin exact minor with `[patch.crates-io]` override for bugfixes | 2023–present | Standard defensive practice |
| Breaking changes through deprecation | Breaking changes through feature flags + facade | 2022–present (axum, tokio) | Allows v2.x→v3.x with facade continuity |

**Deprecated/outdated:**
- pmcp's "close credibility gap" framing from v2.1 may be obsoleted by Phase 72's outcome — if pmcp adopts rmcp as a foundation, the gap framing shifts from "catch up on features" to "differentiate on DX." Worth flagging in the final recommendation.
- Deferring foundation-adoption decisions until "later" is deprecated by MEMORY.md's v2.0 cleanup philosophy: "During breaking-change window, consolidate aggressively — don't defer as 'not worth the churn'."

## Assumptions Log

All claims tagged `[ASSUMED]` in this research, listed here so the planner and CONTEXT.md surface them for user confirmation.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | rmcp maintainer responsiveness to external bug reports is unmeasured in this session | Rmcp Trajectory Risk, Decision Framework | MEDIUM — a poor responsiveness number changes the decision from "adopt" to "stay or fork escape hatch"; planner must run `gh issue list` analysis during execution |
| A2 | rmcp 1.5.0 has no MCP Apps / UI Resource types | Inversion Inventory (mcp_apps row) | LOW — verified by docs.rs `model` module listing not mentioning UIResource/WidgetMeta, but could be hidden behind a feature flag; planner should verify via `cargo public-api` on an rmcp test crate |
| A3 | rmcp 1.5.0 has no first-class WebSocket transport | Inversion Inventory (websocket row) | LOW — verified from docs.rs/rmcp transport module listing only showing generic Sink/Stream adapter; could be under a less-visible feature flag |
| A4 | rmcp's Completable / Completion types exist or are absent | Inversion Inventory (completable row) | LOW — explicitly marked UNVERIFIED in the inventory; planner's Slice-1 PoC will surface this |
| A5 | rmcp's AuthInfo / AuthScheme equivalents | Inversion Inventory (auth row) | LOW — marked UNVERIFIED; auth is pmcp-exclusive at handler boundary anyway |
| A6 | pmcp's v2.x breaking-change window is still open | Strategy Matrix Option A, Decision Framework | HIGH — determines whether Option A is even available; needs explicit CONTEXT.md decision |
| A7 | pmcp has ≥ 3 known production users | Decision Framework | HIGH — user tolerance for v3.0 is a decision input; need CONTEXT.md survey |
| A8 | Full-adopt LOC savings estimate of ~8k LOC | Inversion Inventory, Strategy Matrix | MEDIUM — based on `wc -l` totals, not per-file behavioral analysis. Actual deletable LOC could be 40% lower because pmcp-specific serde attributes, `From` impls, and helper methods are mixed into the files |
| A9 | Migration cost estimate not quantified for ripple into mcp-tester, cargo-pmcp, pmcp-code-mode, pmcp-tasks | Strategy Matrix | HIGH — these crates all import pmcp types; ripple could dominate cost. Planner's Slice 1 PoC must cover the full workspace, not just `pmcp` itself |
| A10 | Downstream pmcp-run (which pmcp supports) adoption pattern | Strategy Matrix, Decision Framework | HIGH — pmcp-run is a major pmcp user per MEMORY.md; its tolerance for breaking changes is a decision input |

## Open Questions

Questions the planner will need CONTEXT.md or user decisions to resolve. Phase 72 research cannot answer these alone.

1. **Is pmcp's v2.x breaking-change window still open, and for how long?**
   - What we know: MEMORY.md v2.0 cleanup philosophy says "consolidate aggressively."
   - What's unclear: Whether v3.0 is on the near-term roadmap; whether the team would absorb a v3.0 for the sake of Full adopt.
   - Recommendation: Explicit CONTEXT.md answer before Plan 01 starts.

2. **How many production users does pmcp have, and what's their tolerance for a v3.0 breaking change?**
   - What we know: pmcp-run is a known major consumer; the Lambda feedback note suggests at least one more production user.
   - What's unclear: Total user count, upgrade tolerance.
   - Recommendation: Either a lightweight user survey, or a CONTEXT.md assumption that the team accepts breaking changes given the v2.x window.

3. **What's rmcp's empirical issue-response responsiveness?**
   - What we know: No published SLA; 74 releases in project history indicates active maintenance.
   - What's unclear: Response time on external bug reports, especially High-severity.
   - Recommendation: Part of Plan 01 — run `gh issue list -R modelcontextprotocol/rust-sdk --state closed --json closedAt,createdAt,labels` and compute median time-to-close for the last 50 issues.

4. **Does rmcp's model/service have extension hooks pmcp's workflow engine needs?**
   - What we know: rmcp's `Peer<R>` is general-purpose; workflow engine uses pmcp's PeerHandle (Phase 70).
   - What's unclear: Whether rmcp's Peer supports pmcp-specific workflow semantics without upstream PRs.
   - Recommendation: Part of PoC Slice 3, or a separate investigation question.

5. **What's the realistic cadence for upstream PRs to rmcp?**
   - What we know: rmcp has active PR merging per release cadence.
   - What's unclear: Whether pmcp-originated ergonomic improvements (e.g., the Phase 69 gap-closing fixes) would be welcomed.
   - Recommendation: If Option D is on the table, run a low-stakes upstream PR first (e.g., `list_all_tools` auto-pagination per CLIENT-02) and measure response latency.

6. **Does the Lambda/edge deployment pattern (pmcp-server-lambda crate) depend on transports that would need pmcp first-party ownership under any adoption scenario?**
   - What we know: pmcp has `crates/pmcp-server/pmcp-server-lambda` and the Lambda feedback note about DNS rebinding protection.
   - What's unclear: Whether rmcp's streamable-HTTP transport has equivalent defense-in-depth.
   - Recommendation: Part of Inversion Inventory verification.

7. **What REQ-IDs should land in REQUIREMENTS.md to make Phase 72 scope traceable?**
   - What we know: Phase description says "TBD" for REQ-IDs.
   - Recommendation: Plan 01 adds the 5 RMCP-EVAL-* IDs proposed above to REQUIREMENTS.md under a new "v2.x / v3.0 Foundation Evaluation" heading.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo | Inspecting workspace + running `cargo info` | ✓ | 1.x (confirmed via `cargo info`) | — |
| git | `git log` maintenance-hours analysis in decision framework | ✓ | 2.x (system) | — |
| gh CLI | `gh issue list -R modelcontextprotocol/rust-sdk` for response-time data | ✓ (per HANDOFF.json context; project uses gh regularly) | — | Fall back to WebFetch of issue list with manual parsing |
| Docs.rs / crates.io access | Verify rmcp surface | ✓ | Network-dependent | Context7 / rmcp source files in the rust-sdk repo |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** None (gh CLI fallback via WebFetch is viable).

## Validation Architecture

Phase 72 is a **decision/research phase**, not a code/implementation phase. The canonical "Nyquist validation" framing — test files mapped to code requirements — doesn't apply directly. Instead, validation is **decision-quality validation**: did the phase produce a recommendation that is traceable, falsifiable, and grounded in evidence?

Recommendation to the orchestrator: **include this section, but with decision-validation framing** (not code-test framing). The `workflow.nyquist_validation` config key is absent from `.planning/config.json`, which defaults to enabled; the planner should either (a) configure `workflow.nyquist_validation = false` for Phase 72 specifically, or (b) accept the decision-validation framing below as the phase's equivalent.

### Decision Framework Validation

| Property | Value |
|----------|-------|
| Validation mode | Decision-quality (not code-test) |
| Config file | `.planning/phases/72-.../72-DECISION-RUBRIC.md` |
| Validation command | Manual review against the criteria below |
| Phase-gate command | Plan-checker agent confirms all criteria in this table pass |

### Phase Requirements → Validation Map

| Req ID | Validation Criterion | Validation Type | How Verified | Exists? |
|--------|----------------------|------------------|--------------|--------|
| RMCP-EVAL-01 | Inversion inventory covers ≥ 15 pmcp module families (`src/types/*.rs` + `src/shared/*.rs`), each with pmcp evidence (file:line) and rmcp evidence (docs.rs anchor or GitHub blob URL with line number) | Markdown audit | `grep -c '| .*:[0-9]* |' 72-INVENTORY.md` ≥ 15 | ❌ Wave 0 (Plan 01) |
| RMCP-EVAL-02 | Strategy matrix scores all 5 options across all 5 criteria from the phase description; no cell is blank; each Adopt/Stay/Hybrid/Selective/Fork row has a numeric or qualitative score with rationale | Markdown audit | `grep -E '^\| (A\. Full adopt\|B\. Hybrid\|C\. Selective\|D\. Status quo\|E\. Fork)' 72-STRATEGY-MATRIX.md ≥ 5 rows, no `TBD` | ❌ Wave 0 (Plan 01) |
| RMCP-EVAL-03 | PoC proposal names ≥ 2 candidate slices, each with LOC estimate < 500, specific files touched, and a falsifiable pass/fail criterion | Markdown audit | Inspect `72-POC-PROPOSAL.md` for `LOC: <N>`, `Files:`, `Pass:` sections for each slice | ❌ Wave 0 (Plan 02) |
| RMCP-EVAL-04 | Decision rubric has ≥ 5 falsifiable thresholds (numeric or boolean, with a data source named) | Markdown audit | Inspect `72-DECISION-RUBRIC.md` for `Threshold:`, `Data source:` fields; count ≥ 5 | ❌ Wave 0 (Plan 02) |
| RMCP-EVAL-05 | Recommendation document picks one of {A, B, C, D, E} and justifies the pick by citing each rubric criterion outcome | Markdown audit | `72-RECOMMENDATION.md` starts with `**Recommendation:** <option>` and has a subsection per criterion | ❌ Wave 0 (Plan 03) |

### Validation Sampling Rate

- **Per plan completion:** plan-checker agent reads the produced document against its criterion, PASS/FAIL.
- **Per phase gate (`/gsd-verify-work`):** verifier reads all 5 deliverables and confirms cross-document consistency (inventory row counts match strategy matrix row counts; PoC slices reference real pmcp files; rubric thresholds are all cited somewhere in inventory or strategy matrix).

### Wave 0 Gaps

- [ ] `72-INVENTORY.md` — covers RMCP-EVAL-01
- [ ] `72-STRATEGY-MATRIX.md` — covers RMCP-EVAL-02
- [ ] `72-POC-PROPOSAL.md` — covers RMCP-EVAL-03
- [ ] `72-DECISION-RUBRIC.md` — covers RMCP-EVAL-04
- [ ] `72-RECOMMENDATION.md` — covers RMCP-EVAL-05

**Framework install / test setup:** None — decision validation is markdown-audit only. No pytest/cargo-test framework setup needed.

### Falsifiability Checklist (phase-gate criteria)

Decision-quality gates the planner must pass for Phase 72 to count as "complete":

- [ ] Every claim in `72-INVENTORY.md` cites a pmcp file:line AND an rmcp docs.rs anchor or GitHub blob URL
- [ ] `72-STRATEGY-MATRIX.md` has all 25 cells filled (5 options × 5 criteria) with no `TBD`
- [ ] `72-POC-PROPOSAL.md` names at least one slice that is ≤ 500 LOC touched and ≥ one slice that could be executed in ≤ 3 days
- [ ] `72-DECISION-RUBRIC.md` has ≥ 5 falsifiable thresholds and each threshold cites its data source
- [ ] `72-RECOMMENDATION.md` picks exactly one of {A, B, C, D, E} (not "some combination") and justifies by citing rubric outcomes

## Project Constraints (from CLAUDE.md)

Phase 72 is research-only, so most Toyota Way gates don't apply to the phase itself. Two constraints DO apply:

1. **Zero SATD in deliverable markdown:** No "TODO" / "FIXME" / "we'll figure this out later" language in the 5 deliverables. Every unresolved question goes under "Open Questions" with a recommended resolution path.
2. **`make quality-gate` for any PoC branch:** If Plan 02 actually runs a PoC slice, the PoC branch must `make quality-gate` green before the slice outcome is recorded. A PoC that doesn't pass quality gate is not evidence.

The following CLAUDE.md constraints DO NOT apply to Phase 72 itself (because no production code is written):
- 80%+ test coverage
- Fuzz / property / unit / example testing
- Cognitive complexity ≤ 25
- PMAT quality-gate proxy for file writes (applies only if a PoC branch is opened)
- Pre-commit hook gates

If Phase 72 produces a PoC branch, all of the above apply to that branch.

## Sources

### Primary (HIGH confidence)

- `cargo info rmcp` executed 2026-04-19 — rmcp 1.5.0 metadata, feature flag list, dependency matrix
- `cargo info pmcp` executed 2026-04-19 — pmcp 2.4.0 local + crates.io
- `wc -l` of `src/types/*.rs`, `src/shared/*.rs`, `src/server/*.rs`, `src/client/*.rs` — LOC totals grounding the Inversion Inventory
- `.planning/phases/69-.../69-RESEARCH.md` — 32-row gap matrix, pinned rmcp 1.5.0 baseline, reproduced High-severity Row IDs
- `.planning/phases/69-.../69-PROPOSALS.md` — 3 follow-on parity phase proposals (PARITY-HANDLER-01, PARITY-CLIENT-01, PARITY-MACRO-01)
- `.planning/phases/69-.../69-CONTEXT.md` — scoping decisions D-01 through D-19 inherited by Phase 72
- `.planning/REQUIREMENTS.md` — current REQ-ID table; v2.1 requirement list
- `.planning/STATE.md` — current phase position (Phase 71 executing), v2.0 protocol modernization status, v2.0 cleanup philosophy flag
- `Cargo.toml` (workspace) — workspace crate layout (12 crates: pmcp + pmcp-macros + pmcp-macros-support + crates/mcp-tester + crates/mcp-preview + crates/pmcp-server + crates/pmcp-server/pmcp-server-lambda + crates/pmcp-tasks + crates/mcp-e2e-tests + crates/pmcp-widget-utils + crates/pmcp-code-mode + crates/pmcp-code-mode-derive + cargo-pmcp)
- `src/lib.rs` — pmcp top-level re-export surface; confirms what's publicly exposed today
- `src/types/mod.rs` — confirms `pub use protocol::*` pattern and flat re-export style
- `src/server/traits.rs` — `ToolHandler`, `PromptHandler`, `ResourceHandler`, `SamplingHandler` traits baseline

### Secondary (MEDIUM confidence — cross-verified)

- https://docs.rs/rmcp/1.5.0/rmcp/ — rmcp 1.5.0 top-level module listing; verified against `cargo info rmcp` features
- https://docs.rs/rmcp/1.5.0/rmcp/model/index.html — rmcp type inventory; grounds the "exact overlap" claims in Inversion Inventory
- https://docs.rs/rmcp/1.5.0/rmcp/transport/index.html — rmcp transport feature matrix
- https://github.com/modelcontextprotocol/rust-sdk/releases — 10 most-recent release tags with dates; grounds the release-cadence claim
- https://github.com/modelcontextprotocol/rust-sdk — ownership (Anthropic-affiliated org), governance presence (formal CONTRIBUTING, SECURITY.md)
- https://github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp/Cargo.toml — mandatory vs optional dependency split
- tonic/prost precedent — https://github.com/hyperium/tonic (verified via WebSearch)
- axum/hyper precedent — https://docs.rs/axum/latest/axum/, https://tokio.rs/blog/2023-11-27-announcing-axum-0-7-0 (verified via WebSearch)
- reqwest/hyper precedent — ecosystem-standard, cross-verified via multiple WebSearch results

### Tertiary (LOW confidence — flagged for validation during plan execution)

- rmcp maintainer responsiveness to external issues — inferred from 74-release cadence; should be verified by Plan 01 via `gh issue list` analysis (ASSUMED A1)
- rmcp's completion/completable API surface — not explicitly verified in this session (ASSUMED A4)
- rmcp's auth type coverage at the `model` layer vs handler layer — not explicitly verified (ASSUMED A5)
- Full-adopt LOC savings precision — based on `wc -l` totals; actual deletable LOC could differ by 40% (ASSUMED A8)

## Metadata

**Confidence breakdown:**
- Inversion Inventory: HIGH for the 23 rows where rmcp surface is verified by docs.rs; LOW for the 4 rows explicitly marked UNVERIFIED (completable, some auth, some batch, some session)
- Strategy Matrix: MEDIUM — each option is described with concrete tradeoffs, but scoring is directional pending CONTEXT.md weights
- PoC Slice Sizing: MEDIUM — LOC estimates are educated from `wc -l` + typical Rust-migration experience; not validated against an actual branch
- Decision Framework: HIGH for the criteria set (taken from the phase description); LOW for the illustrative thresholds (need user confirmation)
- rmcp Trajectory Risk: MEDIUM — release cadence verified; governance responsiveness unverified (see A1)
- Precedents: HIGH — tonic/prost, axum/hyper, reqwest/hyper are well-documented Rust ecosystem precedents

**Research date:** 2026-04-19
**Valid until:** 2026-05-19 (30-day window for a stable rmcp 1.x surface; shorter if rmcp ships 1.6 which would require a pin re-evaluation)

**Phase 72 planner action:** Consume this document as input to plan ≤ 3 plans producing the 5 deliverables named in "Wave 0 Gaps" above. The 5 RMCP-EVAL-* REQ-IDs listed in "Phase Requirements" should be added to REQUIREMENTS.md in the first plan.
