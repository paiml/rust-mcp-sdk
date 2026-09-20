# PMCP SDK Examples

Examples demonstrating the [PMCP](https://crates.io/crates/pmcp) Model Context Protocol SDK for Rust. Every example in this directory is a standalone, runnable program registered in `Cargo.toml` and ships with a copy-paste `cargo run` command.

## Conventions

Examples follow a **role-prefix** naming scheme:

| Prefix | Role | Count |
|--------|------|-------|
| `s` | Server | 44 |
| `c` | Client | 10 |
| `t` | Transport | 8 |
| `m` | Middleware | 8 |

Within each role, examples are ordered by capability (tools, resources, prompts, sampling, etc.) and then by complexity (basic to advanced). The migration table at the bottom maps the previous numeric-only names to the current role-prefixed names.

## Prerequisites

- Rust toolchain (`stable` or newer). Install with [rustup](https://rustup.rs/).
- A clone of this repository. All commands below assume you are in the repository root.
- Some examples require feature flags (e.g. `websocket`, `streamable-http`, `schema-generation`, `full`). Feature flags are noted per-example and come from `Cargo.toml` `[[example]]` entries.

Run any example using the copy-paste command block shown directly under its entry.

---

## Server Examples

### Basic Server

**s01_basic_server** — Minimal MCP server with a single tool handler over stdio.
```bash
cargo run --example s01_basic_server
```

**s02_server** — Minimal echo-tool server, the smallest usable `Server` + `ToolHandler` pair.
```bash
cargo run --example s02_server
```

**s25_refactored_server** — Transport-independent `ServerCore` + `StdioAdapter` pattern (new protocol/transport split).
```bash
cargo run --example s25_refactored_server
```

### Resources

**s03_server_resources** — Server that serves resources with URI templates and custom handlers.
```bash
cargo run --example s03_server_resources
```

**s04_server_resources_collection** — `ResourceCollection` with `StaticResource` and `DynamicResourceHandler` combined.
```bash
cargo run --example s04_server_resources_collection
```

**s14_resource_watcher** — `ResourceWatcher` for live file-system change monitoring.
```bash
cargo run --example s14_resource_watcher --features resource-watcher
```

**s15_dynamic_resources** — Dynamic resource providers with URI templates and automatic parameter extraction.
```bash
cargo run --example s15_dynamic_resources
```

### Prompts

**s05_server_prompts** — Server with prompt handlers, templates, and dynamic prompt generation.
```bash
cargo run --example s05_server_prompts
```

**s06_completable_prompts** — Completable prompt arguments (argument auto-completion).
```bash
cargo run --example s06_completable_prompts
```

### Logging, Progress, Cancellation, Errors

**s07_logging** — Server/client logging with levels, structured metadata, and filtering.
```bash
cargo run --example s07_logging
```

**s08_progress_notifications** — Protocol-level progress notifications and tokens.
```bash
cargo run --example s08_progress_notifications --features progress_example
```

**s09_progress_countdown** — Countdown tool with rate-limited progress reporting via `extra.report_count()`.
```bash
cargo run --example s09_progress_countdown
```

**s10_request_cancellation** — Request cancellation tokens and `CancelledNotification`.
```bash
cargo run --example s10_request_cancellation --features cancellation_example
```

**s11_error_handling** — Error codes, recovery strategies, and retry with backoff.
```bash
cargo run --example s11_error_handling
```

### Sampling and Elicitation

**s12_sampling_llm** — Implementing a `SamplingHandler` for LLM sampling on the server.
```bash
cargo run --example s12_sampling_llm
```

**s13_elicit_input** — User input elicitation using the 2025-11-25 spec-compliant JSON-schema elicitation API.
```bash
cargo run --example s13_elicit_input
```

**s30_tool_with_sampling** — Tool that internally calls `sampling/createMessage` (text summarization pattern).
```bash
cargo run --example s30_tool_with_sampling --features full
```

### Typed Tools

**s16_typed_tools** — `TypedTool` with automatic JSON-schema generation from Rust types.
```bash
cargo run --example s16_typed_tools --features schema-generation
```

**s17_advanced_typed_tools** — Field descriptions, validation, regex, ranges, optional fields, nested structures.
```bash
cargo run --example s17_advanced_typed_tools --features schema-generation
```

**s18_serverbuilder_typed** — Ergonomic `ServerBuilder::tool_typed` / `tool_typed_sync` methods.
```bash
cargo run --example s18_serverbuilder_typed --features schema-generation
```

**s19_wasm_typed_tools** — Typed tools compatible with WASM targets (browser, Cloudflare Workers, WASI).
```bash
cargo run --example s19_wasm_typed_tools --features schema-generation
```

**s20_typed_tool_v2** — `TypedToolWithOutput` with both input and output typing (auto-generated `outputSchema`).
```bash
cargo run --example s20_typed_tool_v2 --features schema-generation
```

**s21_description_variants** — All description-builder variants for typed tools (async, sync, with output).
```bash
cargo run --example s21_description_variants --features schema-generation
```

**s22_structured_output_schema** — Top-level `outputSchema` on `ToolInfo` per MCP 2025-11-25.
```bash
cargo run --example s22_structured_output_schema
```

### Proc Macros

**s23_mcp_tool_macro** — `#[mcp_tool]` + `#[mcp_server]` proc macros (compared to `TypedTool`/`TypedToolWithOutput`).
```bash
cargo run --example s23_mcp_tool_macro --features full
```

**s24_mcp_prompt_macro** — `#[mcp_prompt]` proc macro with mixed tools + prompts via `#[mcp_server]`.
```bash
cargo run --example s24_mcp_prompt_macro --features full
```

### Currency Domain Example

**s26_currency_server** — Full currency exchange MCP server (rates, trends, predictions, ASCII sparklines).
```bash
cargo run --example s26_currency_server
```

**s27_test_currency_server** — Test harness that prints the expected protocol messages for `s26_currency_server`.
```bash
cargo run --example s27_test_currency_server
```

### Authentication and OAuth

**s28_authentication** — Client-side `AuthInfo` + `AuthScheme` for authenticated MCP calls.
```bash
cargo run --example s28_authentication --features authentication_example
```

**s29_oauth_server** — Full OAuth 2.0 server with `InMemoryOAuthProvider`, bearer tokens, and scope middleware.
```bash
cargo run --example s29_oauth_server
```

### Workflow System

**s31_workflow_minimal** — Minimal `SequentialWorkflow` (quadratic-formula solver) with DSL helpers and bindings.
```bash
cargo run --example s31_workflow_minimal
```

**s32_workflow_error_messages** — Common workflow validation errors and how to diagnose them.
```bash
cargo run --example s32_workflow_error_messages
```

**s33_workflow_dsl_cookbook** — Recipes covering `prompt_arg`, `from_step`, `field`, `constant`, and binding patterns.
```bash
cargo run --example s33_workflow_dsl_cookbook
```

**s34_typed_tools_workflow** — Typed tools + workflow with server-side tool execution during `prompts/get`.
```bash
cargo run --example s34_typed_tools_workflow --features schema-generation
```

**s35_hybrid_workflow** — Hybrid execution model: server runs deterministic steps, client LLM handles fuzzy-matching steps.
```bash
cargo run --example s35_hybrid_workflow
```

**s36_dynamic_resource_workflow** — Template-bound resource URIs built from previous-step outputs.
```bash
cargo run --example s36_dynamic_resource_workflow
```

**s37_resource_only_steps** — Workflow steps that fetch resources without executing any tool.
```bash
cargo run --example s37_resource_only_steps
```

**s38_prompt_workflow_progress** — Multi-step prompt workflow with progress reporting and cancellation.
```bash
cargo run --example s38_prompt_workflow_progress
```

### MCP Apps (HTML Widgets)

**s39_mcp_app_venue_map** — MCP Apps (SEP-1865) interactive map built with Leaflet.js and `UIResourceBuilder`.
```bash
cargo run --example s39_mcp_app_venue_map --features schema-generation
```

**s40_mcp_app_hotel_gallery** — MCP Apps image gallery with lightbox and responsive grid.
```bash
cargo run --example s40_mcp_app_hotel_gallery --features schema-generation
```

### Agent Skills (SEP-2640)

**s44_server_skills** — Three-tier Agent Skills demo registering `hello-world`,
`refunds`, and `code-mode` skills via `pmcp::Server::builder()`. Uses
`.bootstrap_skill_and_prompt(...)` to expose the same skill content on BOTH
a SEP-2640 skill surface AND a parallel MCP prompt fallback for hosts that
don't yet speak SEP-2640 — the two surfaces are byte-equal by construction.
```bash
cargo run --example s44_server_skills --features skills,full
```

### Client Host Surface (Sampling / Elicitation / Roots)

**s49_sampling_host** — A sampling **host**: a `Client` that ANSWERS an inbound
spec-direction `sampling/createMessage` from its registered
`pmcp::client::host::HostSamplingHandler` (the MCP host direction — the inverse
of the legacy LLM-server pattern used by `Client::create_message`). Registers the
handler via `ClientBuilder::on_sampling(..)` and drives a nested round-trip over
an in-process duplex transport, printing the completion the host handler produced.
```bash
cargo run --example s49_sampling_host
```

### MCP Tasks

**s45_tool_as_task_lifecycle** — THE canonical tools-as-Tasks example: the
recommended, all-typed pattern for exposing a tool as an async MCP Task. Register
a `with_task_support(TaskSupport::Required)` tool plus an `InMemoryTaskStore` on
`ServerCoreBuilder` (the store auto-advertises the `tasks` capability and mints
the task id) — you never hand-write any `tasks/*` wire JSON. Drives the full path
through a LIVE in-process client round-trip (`initialize → call(task) → tasks/get
poll → tasks/result`), proving the four original tools-as-tasks wire-shape bugs
are impossible on the SDK path: store-minted id consistency, auto-advertised
`tasks` capability, typed `tasks/get`, and a typed non-empty `tasks/result`.
```bash
cargo run --example s45_tool_as_task_lifecycle --features full
```

---

## Client Examples

### Initialization

**c01_client_initialize** — Client initialization, capability negotiation, and server-capability inspection.
```bash
cargo run --example c01_client_initialize
```

**c05_client** — Bare-minimum MCP client setup over stdio.
```bash
cargo run --example c05_client
```

### Tools, Resources, Prompts

**c02_client_tools** — Listing, calling, and error-handling MCP tools from the client.
```bash
cargo run --example c02_client_tools
```

**c03_client_resources** — Listing resources, reading content, handling content types, pagination.
```bash
cargo run --example c03_client_resources
```

**c04_client_prompts** — Listing prompts, passing arguments, and consuming prompt responses.
```bash
cargo run --example c04_client_prompts
```

### Concurrency and Auth

**c06_multiple_clients_parallel** — Multiple MCP clients running in parallel with independent per-client state.
```bash
cargo run --example c06_multiple_clients_parallel
```

**c07_oidc_discovery** — OIDC discovery, token exchange, and refresh against an OAuth 2.0 provider (with CORS/retry handling).
```bash
cargo run --example c07_oidc_discovery --features http-client
```

### Typed Helpers and Pagination (Phase 73)

**c09_client_list_all** — Phase 73 PARITY-CLIENT-01 demo. Shows `Client::with_client_options`,
`call_tool_typed`, `get_prompt_typed`, and all four `list_all_*` helpers (including
`list_all_resource_templates`, which uses the distinct `resources/templates/list`
capability). This example drives an MCP server over **stdio** and is not
self-contained — see the source-file header for pairing instructions.

```bash
cargo run --example c09_client_list_all --features full
```

### Agent Skills (SEP-2640)

**c10_client_skills** — Walks BOTH host flows side-by-side against an in-process
server: (a) the SEP-2640 flow enumerates skills via `resources/list`, reads each
`skill://…/SKILL.md` + reference URI, and asserts `Content::Resource { uri,
text, mime_type }` wire shape; (b) the legacy flow retrieves the parallel
prompt handler via `get_prompt("start_code_mode")` and invokes it directly.
The example `assert_eq!`s both flows' resulting context byte-for-byte —
proving SEP-2640-capable and SEP-2640-blind hosts see the same content.
```bash
cargo run --example c10_client_skills --features skills,full
```

---

## Transport Examples

### WebSocket

**t01_websocket_transport** — WebSocket client transport with `WebSocketConfig`.
```bash
cargo run --example t01_websocket_transport --features websocket
```

**t02_websocket_server_enhanced** — Multi-client `EnhancedWebSocketServer` with heartbeats and connection management.
```bash
cargo run --example t02_websocket_server_enhanced --features websocket
```

### SSE

**t03_sse_optimized** — `OptimizedSseTransport` with compression, batching, and reconnection.
*Deprecated since 2.18.0* — retained for 2.x compatibility; use `StreamableHttpTransport` for new
code, which bounds every peer-controlled read and carries a configurable cap.
```bash
cargo run --example t03_sse_optimized --features sse
```

### Streamable HTTP

**t04_streamable_http_stateful** — Stateful HTTP server with session management and `mcp-protocol-version` header handling.
```bash
cargo run --example t04_streamable_http_stateful --features streamable-http
```

**t05_streamable_http_stateless** — Stateless HTTP server (ideal for AWS Lambda and serverless deployments).
```bash
cargo run --example t05_streamable_http_stateless --features streamable-http
```

**t06_streamable_http_client** — HTTP client for both stateful and stateless streamable-HTTP servers.
```bash
cargo run --example t06_streamable_http_client --features streamable-http
```

### Connection Pooling and Performance

**t07_connection_pool** — Connection pool with round-robin / least-connections load balancing strategies.
```bash
cargo run --example t07_connection_pool --features full
```

**t08_simd_parsing_performance** — SIMD-accelerated JSON-RPC, SSE, Base64, and HTTP-header parsing with benchmarks.
```bash
cargo run --example t08_simd_parsing_performance
```

---

## Middleware Examples

### Protocol Middleware

**m01_basic_middleware** — Basic `Middleware` trait usage with `LoggingMiddleware` and a `MiddlewareChain`.
```bash
cargo run --example m01_basic_middleware
```

**m02_enhanced_middleware** — Priority ordering, rate limiting, circuit breaker, metrics, and context propagation.
```bash
cargo run --example m02_enhanced_middleware --features full
```

**m03_middleware_demo** — End-to-end flow combining protocol middleware with `StreamableHttpTransport` and OAuth.
```bash
cargo run --example m03_middleware_demo
```

### HTTP / Server Middleware

**m04_server_http_middleware** — `ServerHttpLoggingMiddleware` with header redaction, CORS, and body gating.
```bash
cargo run --example m04_server_http_middleware --features streamable-http
```

### OAuth Pass-Through

**m05_tool_middleware_oauth** — Tool middleware that extracts OAuth tokens from `AuthContext` and injects them into tool calls.
```bash
cargo run --example m05_tool_middleware_oauth
```

**m06_oauth_transport_to_tools** — Complete transport → middleware → tools OAuth flow (production-ready pattern).
```bash
cargo run --example m06_oauth_transport_to_tools
```

### Error Recovery and Observability

**m07_advanced_error_recovery** — Adaptive retry with jitter, partial-failure bulk recovery, deadline-aware timeouts, health monitoring.
```bash
cargo run --example m07_advanced_error_recovery --features full
```

**m08_observability_middleware** — Tracing, metrics, and logging via the built-in observability middleware (console + CloudWatch EMF).
```bash
cargo run --example m08_observability_middleware
```

---

## Standalone Example Projects

The following subdirectories contain full standalone Cargo projects (excluded from the root workspace). Each has its own `Cargo.toml` and build instructions — see the `README.md` inside each directory:

| Directory | What it demonstrates |
|-----------|----------------------|
| `examples/mcp-apps-chess/` | MCP Apps: interactive chess board widget |
| `examples/mcp-apps-map/` | MCP Apps: map widget with `WidgetDir` hot-reload |
| `examples/mcp-apps-dataviz/` | MCP Apps: data visualization widget |
| `examples/wasm-client/` | Browser WASM MCP client |
| `examples/wasm-mcp-server/` | WASM MCP server target |
| `examples/wasm/` | Shared WASM helpers |
| `examples/scenarios/` | End-to-end scenario harnesses |
| `examples/test-basic/` | Basic smoke-test fixture |
| `examples/25-oauth-basic/` | Basic OAuth 2.0 scaffold project |
| `examples/26-server-tester/` | `mcp-tester` integration harness |
| `examples/27-course-server-minimal/` | Minimal server used by the PMCP course |

---

## Migration Reference

If you were using examples from a previous version of PMCP, the table below maps the old example names to the current role-prefixed names. Any old `cargo run` invocations should be updated to the new role-prefixed names.

### Server Examples

| Old Name | New Name |
|----------|----------|
| 02_server_basic | s01_basic_server |
| server | s02_server |
| 04_server_resources | s03_server_resources |
| 08_server_resources | s04_server_resources_collection |
| 06_server_prompts | s05_server_prompts |
| 17_completable_prompts | s06_completable_prompts |
| 08_logging | s07_logging |
| 10_progress_notifications | s08_progress_notifications |
| 11_progress_countdown | s09_progress_countdown |
| 11_request_cancellation | s10_request_cancellation |
| 12_error_handling | s11_error_handling |
| 14_sampling_llm | s12_sampling_llm |
| 19_elicit_input | s13_elicit_input |
| 18_resource_watcher | s14_resource_watcher |
| 56_dynamic_resources | s15_dynamic_resources |
| 32_typed_tools | s16_typed_tools |
| 33_advanced_typed_tools | s17_advanced_typed_tools |
| 34_serverbuilder_typed | s18_serverbuilder_typed |
| 35_wasm_typed_tools | s19_wasm_typed_tools |
| 36_typed_tool_v2_example | s20_typed_tool_v2 |
| 37_description_variants_example | s21_description_variants |
| 48_structured_output_schema | s22_structured_output_schema |
| 63_mcp_tool_macro | s23_mcp_tool_macro |
| 64_mcp_prompt_macro | s24_mcp_prompt_macro |
| refactored_server_example | s25_refactored_server |
| currency_server | s26_currency_server |
| test_currency_server | s27_test_currency_server |
| 09_authentication | s28_authentication |
| 16_oauth_server | s29_oauth_server |
| 49_tool_with_sampling_server | s30_tool_with_sampling |
| 50_workflow_minimal | s31_workflow_minimal |
| 51_workflow_error_messages | s32_workflow_error_messages |
| 52_workflow_dsl_cookbook | s33_workflow_dsl_cookbook |
| 53_typed_tools_workflow_integration | s34_typed_tools_workflow |
| 54_hybrid_workflow_execution | s35_hybrid_workflow |
| 59_dynamic_resource_workflow | s36_dynamic_resource_workflow |
| 60_resource_only_steps | s37_resource_only_steps |
| 12_prompt_workflow_progress | s38_prompt_workflow_progress |
| conference_venue_map | s39_mcp_app_venue_map |
| hotel_gallery | s40_mcp_app_hotel_gallery |

### Client Examples

| Old Name | New Name |
|----------|----------|
| 01_client_initialize | c01_client_initialize |
| 03_client_tools | c02_client_tools |
| 05_client_resources | c03_client_resources |
| 07_client_prompts | c04_client_prompts |
| client | c05_client |
| 47_multiple_clients_parallel | c06_multiple_clients_parallel |
| 20_oidc_discovery | c07_oidc_discovery |

### Transport Examples

| Old Name | New Name |
|----------|----------|
| 13_websocket_transport | t01_websocket_transport |
| 27_websocket_server_enhanced | t02_websocket_server_enhanced |
| 28_sse_optimized | t03_sse_optimized |
| 22_streamable_http_server_stateful | t04_streamable_http_stateful |
| 23_streamable_http_server_stateless | t05_streamable_http_stateless |
| 24_streamable_http_client | t06_streamable_http_client |
| 29_connection_pool | t07_connection_pool |
| 32_simd_parsing_performance | t08_simd_parsing_performance |

### Middleware Examples

| Old Name | New Name |
|----------|----------|
| 15_middleware | m01_basic_middleware |
| 30_enhanced_middleware | m02_enhanced_middleware |
| 40_middleware_demo | m03_middleware_demo |
| 55_server_middleware | m04_server_http_middleware |
| 57_tool_middleware_oauth | m05_tool_middleware_oauth |
| 58_oauth_transport_to_tools | m06_oauth_transport_to_tools |
| 31_advanced_error_recovery | m07_advanced_error_recovery |
| 61_observability_middleware | m08_observability_middleware |
