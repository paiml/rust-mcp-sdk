# Phase 90: OpenAPI Built-In Server (`pmcp-openapi-server`) - Research

**Researched:** 2026-05-28
**Refreshed:** 2026-05-29 (D-01/D-02 grounding — engine truth + script-tool model corrected)
**Domain:** Config-driven OpenAPI/HTTP MCP server (lift + mirror of the SQL toolkit, Phases 83–86)
**Confidence:** HIGH (grounded in actual reference + actual toolkit + actual `pmcp-code-mode 0.5.1` source, not ecosystem guesswork)

> **Refresh note (2026-05-29):** This pass CORRECTS two things the original under-weighted:
> (1) the JS engine is **NOT Boa** — it is the SDK's pure-Rust AST `PlanCompiler` + `PlanExecutor<H>`
> (the prior research's "Boa runtime" claim was wrong); (2) curated `[[tools]]` are **TWO kinds**
> (single-call AND script), and the script-tool path is **first-class**, sharing the exact same
> compile+execute engine as Code Mode (D-01/D-02). The lift/reuse story, Standard Stack, and most
> pitfalls are preserved from the strong original pass.

## Summary

Phase 90 mirrors the already-shipped SQL toolkit exactly. The reference to lift is `~/Development/mcp/sdk/pmcp-run/built-in/openapi-api/crates/mcp-openapi-server-core` — the OpenAPI sibling of `mcp-sql-server-core`. It currently depends on `shared/mcp-server-common` + `shared/mcp-lambda-proxy` path-deps, which is exactly the dependency pair the SQL lift replaced with the public `pmcp-server-toolkit`. The whole value of this phase is reuse: the backend-agnostic toolkit (Phase 83) already abstracts auth (incoming MCP), secrets, config parsing, static resources/prompts, the `[[tools]]` synthesizer, and code-mode wiring. `[VERIFIED: codebase grep of both trees]`

The genuinely-new backend pieces are: (1) an **HTTP/OpenAPI connector** analogous to `SqlConnector` (`crates/pmcp-server-toolkit/src/sql/mod.rs`) — the reference already implements it as `HttpClient` (`src/http/mod.rs`); (2) the **outgoing-HTTP auth provider** (5 variants, `auth/mod.rs` + `config.rs`); (3) the **openapiv3 spec parser** (`schema/parser.rs`); (4) the **code-mode HTTP-execution seam** `HttpCodeExecutor` (lift from reference `code_mode.rs::HttpClientExecutor`); and — newly first-classed in this refresh — (5) **script-tool support**: a `ScriptToolHandler` that runs admin-authored embedded JS through the *same* engine Code Mode uses. `[VERIFIED: reference src + pmcp-code-mode 0.5.1 source]`

**The engine truth (D-02, the headline correction):** `pmcp-code-mode 0.5.1` does **not** ship Boa. Its JS engine is a **pure-Rust, AST-based plan compiler + interpreter**: `PlanCompiler` (SWC-parses a JS subset → `ExecutionPlan` of `PlanStep`s) and `PlanExecutor<H: HttpExecutor>` (walks the plan, dispatching each `api.get()/api.post()` through the injected `HttpExecutor`). The SDK wraps these in the high-level `JsCodeExecutor<H>` adapter (which implements `CodeExecutor`). **Both surfaces share this exact path:** Code Mode wires `JsCodeExecutor<HttpCodeExecutor>` behind `validate_code`/`execute_code`; curated script tools call `PlanCompiler` + `PlanExecutor` directly (no validation/token, because admin-authored). `filter`/`map`/chaining/bounded loops are all supported (`ArrayMethodCall::{Filter,Map,Reduce,…}`, `PlanStep::{ApiCall,BoundedLoop,ParallelApiCalls}`). `[VERIFIED: executor.rs:55-104,266-335,492-583,2425-2434,3186-3187 + code_executor.rs:84-163]`

**Primary recommendation:** Treat this as a structural copy of Phases 83→85→86 with `SqlConnector`→`HttpConnector`, `[database]`→`[backend]`, `sql=`→`path`/`method` **OR** `script=`, `SqlCodeExecutor`→`HttpCodeExecutor` (impl `HttpExecutor`), and `--schema` (DDL)→`--spec` (OpenAPI, **optional at runtime**). Lift the reference's `HttpClient` + `HttpClientExecutor` + `schema/parser` (the genuinely-new HTTP+OpenAPI code). For the SQL→OpenAPI generalization to work, the toolkit's `code_mode_tools_from_executor` (today hardcoded to `Arc<SqlCodeExecutor>`) must be widened to `Arc<dyn CodeExecutor>` — then the HTTP path passes `JsCodeExecutor<HttpCodeExecutor>` and the SQL path keeps passing `SqlCodeExecutor`, both via one wiring function.

<user_constraints>
## User Constraints (from CONTEXT.md)

`90-CONTEXT.md` exists (gathered 2026-05-29, status "Ready for planning"). These are LOCKED — the planner MUST honor them verbatim.

### Locked Decisions

- **D-01 — Tools are promoted code-mode code; OpenAPI curated `[[tools]]` are TWO kinds.** Across all
  config-driven built-in servers a curated `[[tools]]` entry is *promoted code-mode code* (optimized,
  verified, simplified form of a frequent operation). Promotion lineage: SQL→SQL query, GraphQL→GQL
  query/mutation, OpenAPI→JavaScript script. For the OpenAPI built-in a curated tool is one of:
  - **single-call tool** — `path` + `method` (+ optional per-tool `base_url`): the direct one-operation
    HTTP mapping (the `sql=` analog for the simple case).
  - **script tool** — `script = """<JS>"""`: embedded JS using `api.get(path)` / `api.post(path, body)`
    for multi-call domain operations — chaining one call's output into the next, and iterating arrays
    with `filter`/`map` across steps.
  Detection: a `script` field ⇒ script tool; `path`+`method` ⇒ single-call. Script-tool params are
  declared via `[[tools.parameters]]` and bound to an `args` object inside the script.

- **D-02 — ONE JavaScript engine for BOTH Code Mode and curated script tools (hard requirement).**
  The same JS executor that translates a script → API calls in Code Mode (`validate_code`/`execute_code`,
  the long-tail) MUST also execute curated script tools. No second engine, no divergent semantics — a
  script that works in Code Mode behaves identically when promoted to a curated tool. Both route through
  the toolkit's HTTP-execution seam feeding the SDK code-mode JS engine. Shared capabilities: multi-call
  chaining (output→input), array iteration (`filter`/`map`), bounded loops. **The research must reconcile
  the engine identity** — the prior pass named "Boa" (WRONG) and the reference design names
  `PlanCompiler`/`PlanExecutor`/`PlanStep::ApiCall` (CORRECT, current). This refresh resolves it
  definitively below (Pattern 4 + Pattern 4b). This is the single most important architectural constraint.

- **D-03 — OpenAPI spec useful in BOTH aspects; `--spec` OPTIONAL at runtime.** Curated tools (single-call
  AND script) are self-contained, so a curated-only server boots from `config.toml` alone — no spec
  required. When a spec IS provided it (a) powers the Code Mode `api_schema` resource the LLM reads, and
  (b) validates/authors curated tool paths + script `api.*()` calls. `--spec` is REQUIRED at
  scaffold/discovery time (`cargo pmcp new --kind openapi-server` generates `[[tools]]` from it).

- **D-04 — Parity + demo target = `london-tube` (TfL), ONLY.** Exercises `api_key` query-parameter
  outgoing-auth — the genuinely-new auth shape. Parity asserted offline via **wiremock** (pure-Rust, no
  Docker, no live network in default CI); live replay against api.tfl.gov.uk is env-gated (mirror the SQL
  Athena `#[ignore]`+env pattern). `lichess` is NOT the parity target — optional secondary demo only.

- **D-05 — Ship all five `[backend.auth]` variants** — `none` / `api_key` / `bearer` / `basic` /
  `oauth2_client_credentials` / `oauth_passthrough` — lifted wholesale from reference `config.rs:164-235`.
  These are OUTGOING/backend auth providers (`apply(headers, query)`) — distinct from the toolkit's
  INBOUND MCP-client `AuthProvider::validate_request` (Pitfall 1). `oauth_passthrough` bridges the two via
  the pmcp `AuthContext` token-capture pattern.

- **D-06 — Additive `[backend]` on the shared toolkit `ServerConfig`.** Extend the Phase 83
  backend-agnostic `ServerConfig` with `[backend]` / `[backend.auth]` / `[backend.http]` additively,
  preserving `#[serde(deny_unknown_fields)]`. One config type spans SQL + OpenAPI. Existing SQL configs
  unaffected (additive keys). A minor `pmcp-server-toolkit` version bump accompanies the new `http` surface.

### Carried Forward (locked; not re-discussed)

- **CF-1:** Streamable HTTP only (scaffold/example/binary/Lambda) — stdio deferred.
- **CF-2:** Lazy startup — HTTP connector constructed without contacting backend or requiring live creds.
- **CF-3:** `cargo pmcp new --kind openapi-server` honored verbatim, single runnable crate.
- **CF-4:** Generated config ships `[code_mode] enabled = true` + inline DEV-ONLY `token_secret` + loud
  "replace for production" note, so `cargo run` demonstrates the long-tail JS surface immediately.
- **CF-5:** ≤15-line wiring shape (load config[+spec] → connector → `ServerBuilderExt` → serve HTTP)
  shared by Shape C example + Shape B scaffold `main.rs`.
- **CF-6:** Deploy via per-project build + asset bundle; Phase 77 `PmcpRun`/target enum unchanged;
  spec/config read-only deploy assets via `pmcp::assets`.
- **CF-7:** Static `[code_mode]` policy from config — no DynamoDB/AVP runtime overrides.
- **CF-8:** Two-input model — `config.toml` + admin-authored editable/redactable OpenAPI spec surfaced
  as the code-mode resource.

### Claude's Discretion

- `HttpConnector` as a `trait` (`Arc<dyn HttpConnector>`) vs concrete struct — research recommends a
  trait for parity with `SqlConnector` + feature-gating; lean trait unless pure over-engineering.
- Exact `script` config field name and the `[[tools.parameters]]` → `args` binding shape (mirror
  reference `OPENAPI_SCRIPT_TOOLS.md`).
- Script-tool `ExecutionConfig` bounds (`max_api_calls`, `max_loop_iterations`, `timeout_seconds`) + defaults.
- Feature-gating Code Mode / `js-runtime` on the binary (default-on, opt-out via `--no-default-features`).
- URL building: lift the reference's explicit path-concat (NOT `Url::join` — Pitfall 2).
- Error-redaction wording; wiremock fixture shape; default HTTP bind address/port + readiness-poll method.

### Deferred Ideas (OUT OF SCOPE)

- `lichess` as a second demo instance (additive later).
- Live-network parity replay against api.tfl.gov.uk (env-gated, not default CI).
- GraphQL built-in server (the third backend sibling) — its own phase/milestone.
- Full live integration tests for every auth variant (all five lifted, but only `api_key` gets live
  exercise via london-tube; `basic`/`oauth2`/`passthrough` get unit coverage now).
- Non-OpenAPI `--kind` scaffold backends / broad cargo-pmcp README rewrite (Phase 89 DOCS).
</user_constraints>

## Phase Requirements

Requirement IDs not yet formally assigned in `90-CONTEXT.md`. Proposed clusters (planner may renumber).
Note OAPI-02 is now SPLIT into single-call and script-tool synthesis, and OAPI-10 (engine unification) is added.

| Proposed ID | Description | Research Support |
|----|-------------|------------------|
| OAPI-01 | `HttpConnector` trait in toolkit (analog of `SqlConnector`) | Reference `HttpClient` (`http/mod.rs`) already has the shape |
| OAPI-02a | single-call tool synthesizer (`path`/`method`/`base_url`) | Reference `tools/mod.rs::create_tool_from_config` + toolkit `synthesize_from_config` |
| OAPI-02b | **script tool** synthesizer + `ScriptToolHandler` (`script=` + `[[tools.parameters]]`→`args`) | `OPENAPI_SCRIPT_TOOLS.md` design + SDK `PlanCompiler`/`PlanExecutor` (D-01) |
| OAPI-03 | outgoing-HTTP auth provider (5 variants) | Reference `auth/mod.rs` + `config.rs:164-235` (DIFFERENT trait — Pitfall 1) |
| OAPI-04 | OpenAPI `--spec` parser seam (optional at runtime) | Reference `schema/parser.rs` (openapiv3) |
| OAPI-05 | `HttpCodeExecutor` (impl `pmcp_code_mode::HttpExecutor`) | Reference `HttpClientExecutor` (`code_mode.rs:95-165`) |
| OAPI-06 | `pmcp-openapi-server` Shape A binary (`--config` + optional `--spec`) | Mirrors `pmcp-sql-server/src/{cli,dispatch,assemble,lib}.rs` |
| OAPI-07 | `cargo pmcp new --kind openapi-server` scaffold | Mirrors `cargo-pmcp/src/commands/new.rs` + `templates/sql_server.rs` |
| OAPI-08 | REF parity replay (london-tube, wiremock) | Reference `instances/london-tube.toml` + `servers/london-tube/` |
| OAPI-09 | Docs in three shapes | Mirrors Phase 85/89 docs work |
| OAPI-10 | **Generalize `code_mode_tools_from_executor` to `Arc<dyn CodeExecutor>`** so SQL + OpenAPI share one wiring fn | toolkit `code_mode.rs:155` is hardcoded to `Arc<SqlCodeExecutor>` (the seam both backends need, D-02) |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Inbound MCP request auth | toolkit `AuthProvider` (pmcp) | — | Reused verbatim; `StaticAuthProvider` + pmcp OAuth/JWT validate the MCP client |
| Outbound HTTP backend auth | NEW toolkit `http::AuthProvider` | secrets | 5 variants applied to the *outgoing* reqwest request |
| Single-call tool mapping | NEW toolkit `http` synthesizer | config | `path`/`method`/`base_url` per `[[tools]]` (the `sql=` analog) |
| **Script tool execution** | **NEW `ScriptToolHandler`** | **`HttpCodeExecutor` + `PlanCompiler`/`PlanExecutor`** | Admin-authored JS, no approval cycle; SAME engine as Code Mode (D-02) |
| HTTP execution | NEW `HttpConnector` (reqwest) | — | The genuinely-new backend; analog of `SqlConnector` |
| Code-mode validation (SWC parse) | `pmcp-code-mode` (SDK) | — | `openapi-code-mode`/`js-runtime`; `JavaScriptValidator` |
| **Code-mode execution engine (AST plan, NOT Boa)** | **`pmcp-code-mode` `PlanCompiler`+`PlanExecutor<H>`** | — | Pure-Rust AST interpreter; wrapped by `JsCodeExecutor<H>` |
| Code-mode↔backend seam | **NEW `HttpCodeExecutor` (impl `HttpExecutor`)** | auth | The single seam feeding BOTH script tools and Code Mode |
| Secrets resolution | toolkit `SecretsProvider` | AWS feature | Reused verbatim (env + Secrets Manager + SSM) |
| Static resources/prompts | toolkit `StaticResourceHandler`/`StaticPromptHandler` | — | Reused verbatim |
| Streamable-HTTP transport | `pmcp` `StreamableHttpServer` | — | Reused verbatim (Phase 56 Tower/axum adapter) |
| Lambda deploy | Phase 86 deploy path / `pmcp::assets` | — | Reused verbatim |

## Standard Stack

### Core (lifted from the reference, verified against its Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp` (workspace) | 2.8.1 | MCP server, streamable-HTTP transport, `ToolInfo`/`ToolHandler` | The SDK itself `[VERIFIED: root Cargo.toml]` |
| `pmcp-server-toolkit` | 0.1.0→bump | Auth (inbound), secrets, config, resources/prompts, `[[tools]]` synth, builder ext | The Phase 83 reuse target `[VERIFIED]` |
| `pmcp-code-mode` | 0.5.1 | `PlanCompiler`/`PlanExecutor`/`HttpExecutor`/`JsCodeExecutor`/`CodeExecutor`, SWC validation | Long-tail + script-tool JS engine, SDK-resident `[VERIFIED: lib.rs:140-151]` |
| `openapiv3` (workspace) | per workspace | Parse OpenAPI 3.0/3.1 specs | Reference uses it in `schema/parser.rs` `[VERIFIED: parser.rs:9]` |
| `reqwest` | 0.13 (features `json,rustls,form,query`) | Outgoing HTTP client | Reference `http/mod.rs`; rustls = pure-Rust, no OpenSSL `[VERIFIED]` |
| `serde_yaml` | 0.9 | YAML OpenAPI specs | Reference parser fallback `[VERIFIED: parser.rs:30]` |
| `url` (workspace) | 2.5 | base_url + path joining | Reference `http/mod.rs:16` `[VERIFIED]` |
| `async-trait` | 0.1 | connector + executor traits | Matches `SqlConnector` style `[VERIFIED]` |
| `clap` | 4 (`derive`,`env`) | `--config` / `--spec` / `--http` CLI | Mirrors `pmcp-sql-server/src/cli.rs` `[VERIFIED]` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `regex` | 1 | path-parameter extraction (`{id}`) | Reference uses it; pick one extraction strategy consistently |
| `base64` (workspace) | — | Basic-auth header encoding | Only for `[backend.auth] type="basic"` |
| `secrecy` | 0.10 | secret wrapping in code-mode path | Toolkit already owns `SecretValue` (prefer toolkit's) |
| `thiserror` | 2 | `HttpConnectorError` / `DispatchError` | Mirrors SQL `ConnectorError` / `DispatchError` |
| `mcp-tester` | 0.7.0 (dev) | parity-replay test harness | Mirrors `pmcp-sql-server` dev-dep |
| `wiremock` | 0.6 (dev) | mock HTTP backend in tests | Reference uses it; pure-Rust, no Docker (D-04) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `openapiv3` | hand-rolled spec walk | openapiv3 handles 3.0/3.1 `$ref` resolution — never hand-roll |
| `reqwest 0.13` | `hyper` directly | reqwest gives retries/timeouts/query/json out of the box; matches the reference |
| Lifting `HttpClient` as a struct | a 1-method `HttpConnector` trait | Trait keeps parity with `SqlConnector` + feature-gating; **recommend the trait** |
| SDK `JsCodeExecutor<H>` wrapper | calling `PlanCompiler`+`PlanExecutor` directly | Code Mode uses the wrapper (it's a `CodeExecutor`); script tools call the raw pair (no token) — **use BOTH, per surface** |

**Installation (binary crate `pmcp-openapi-server`, mirrors `pmcp-sql-server`):**
```toml
pmcp = { version = "2.8.1", path = "../..", features = ["streamable-http"] }
pmcp-server-toolkit = { version = "...", path = "../pmcp-server-toolkit", features = ["code-mode"] }
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3.20", features = ["env-filter"] }
thiserror = "2"
```

Toolkit's `[backend]`/code-mode surface needs `pmcp-code-mode` with `js-runtime` (which enables
`openapi-code-mode` → SWC parser) — feature-gate it behind the toolkit's `code-mode` feature so
curated-only builds stay light (Pitfall 4).

**Version verification:** Workspace versions confirmed live: `pmcp = 2.8.1`, `pmcp-code-mode = 0.5.1`,
`pmcp-server-toolkit = 0.1.0` (needs a minor bump for the new `http`/connector surface, mirroring
Phase 84). `[VERIFIED: cargo grep 2026-05-28/29]`

## Architecture Patterns

### System Architecture Diagram

```
                          pmcp-openapi-server  (Shape A binary — mirrors pmcp-sql-server)
                          │
  --config config.toml ──▶│ load_config_and_spec()  (lib.rs::run)
  --spec api.yaml ───────▶│   • ServerConfig::from_toml_strict_validated  (+ [backend], D-06)
  --http 127.0.0.1:8080 ─▶│   • OpenApiSchema::parse(spec)  [OPTIONAL at runtime — D-03]
                          │
                          ▼
                  dispatch(cfg) ──▶ Arc<dyn HttpConnector>  + shared reqwest::Client + Arc<dyn HttpAuthProvider>
                          │
                          ▼
                  build_server(cfg, connector, http_executor, spec)            (assemble.rs analog)
                   │
                   ├─ for each [[tools]]:  ── DETECT (D-01) ──┐
                   │     • script.is_some()  ─▶ ScriptToolHandler ─┐
                   │     • path+method       ─▶ single-call handler│
                   │                                               │
                   │        single-call ──▶ HttpConnector.execute(op, args)
                   │                                               │
                   │        script tool ──▶ PlanCompiler.compile_code(script)
                   │                         PlanExecutor::new(HttpCodeExecutor, cfg)
                   │                         .set_variable("args", tool_args)
                   │                         .execute(&plan)  ──┐   (NO validate/token — admin-authored)
                   │                                            │
                   ├─ try_code_mode_from_config_with_executor ──┤   validate_code + execute_code  (~80%)
                   │     JsCodeExecutor<HttpCodeExecutor> (CodeExecutor adapter)
                   │       └─ internally: PlanCompiler + PlanExecutor<HttpCodeExecutor>
                   │                                            │
                   │            ┌───────────────────────────────┘
                   │            ▼   BOTH paths converge on the SAME engine + seam (D-02)
                   │     HttpCodeExecutor (impl pmcp_code_mode::HttpExecutor)
                   │       async fn execute_request(method, path, body) ─▶ reqwest + auth.apply()
                   │
                   ├─ merged resources (spec resource + code-mode instructions/policies)
                   └─ configured prompts (start_code_mode)
                          │
                          ▼
                  StreamableHttpServer  (pmcp Phase 56 Tower/axum adapter — reused verbatim)
                          │
   MCP client ───────────┘   tools/list, tools/call ─┐
                                                       ▼
                              HttpCodeExecutor ──reqwest──▶  REST backend (base_url + auth)
                                  • {id} path params substituted; query/header/body built
                                  • bearer/apiKey/basic/oauth2/passthrough applied to OUTGOING request
```

### Recommended Project Structure (mirror the SQL tree exactly)
```
crates/pmcp-server-toolkit/src/
├── http/                  # NEW — analog of src/sql/
│   ├── mod.rs             # HttpConnector trait + HttpConnectorError (analog of SqlConnector)
│   ├── client.rs          # reqwest-backed impl (lift reference http/mod.rs)
│   ├── auth.rs            # outgoing-HTTP AuthProvider, 5 variants (lift reference auth/mod.rs)
│   └── schema.rs          # OpenApiSchema parser (lift reference schema/parser.rs, openapiv3)
├── code_mode.rs           # ADD HttpCodeExecutor (impl HttpExecutor) alongside SqlCodeExecutor;
│                          #   WIDEN code_mode_tools_from_executor to Arc<dyn CodeExecutor> (OAPI-10)
├── tools.rs               # ADD script-tool branch + ScriptToolHandler (OAPI-02b)
└── (auth.rs, secrets.rs, config.rs, resources.rs, prompts.rs, builder_ext.rs — REUSED/extended)

crates/pmcp-openapi-server/    # NEW binary — structural copy of crates/pmcp-sql-server/
├── src/{main.rs, lib.rs, cli.rs, dispatch.rs, assemble.rs}
└── Cargo.toml

cargo-pmcp/src/
├── commands/new.rs            # ADD Some("openapi-server") arm (mirror sql-server arm)
└── templates/openapi_server.rs # NEW — mirror templates/sql_server.rs
```

### Pattern 1: The HTTP connector trait (analog of `SqlConnector`)
**What:** A `Send + Sync + 'static` trait the binary dispatches to `Arc<dyn HttpConnector>`, threaded into each single-call tool handler. The reference's `HttpClient` already IS this — lift it behind a trait for parity.
**When to use:** Single-call tools (OAPI-01 / OAPI-02a). Script tools and Code Mode do NOT use this; they use `HttpCodeExecutor` (Pattern 4).
```rust
// NEW: crates/pmcp-server-toolkit/src/http/mod.rs
#[async_trait]
pub trait HttpConnector: Send + Sync + 'static {
    async fn execute(
        &self,
        operation: &Operation,           // path/method/params (from config or spec)
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError>;
    fn base_url(&self) -> &str;          // analog of SqlConnector::dialect
}
// Source: pmcp-run .../http/mod.rs (HttpClient::execute_with_options) +
//         rust-mcp-sdk crates/pmcp-server-toolkit/src/sql/mod.rs:100-142
```

### Pattern 2: Outgoing-HTTP auth provider (DISTINCT from toolkit's inbound auth)
**What:** Apply credentials to the *outgoing* reqwest request. The reference trait is `apply(&mut HeaderMap, &mut query)` — NOT pmcp's inbound `validate_request`.
```rust
// Reference: .../auth/mod.rs (re-exported from mcp_server_common::auth)
// trait AuthProvider { async fn apply(&self, headers: &mut HeaderMap,
//                                     query: &mut HashMap<String,String>) -> Result<()>; }
// Five concrete variants in the AuthConfig enum (VERIFIED .../config.rs:164-235):
//   None | ApiKey{query_params,headers,required} | Bearer{token,required}
//   | Basic{username,password,required} | OAuth2ClientCredentials{...} | OAuthPassthrough{...}
```
**When to use:** OAPI-03 (D-05). Lift `mcp_server_common::auth` into `toolkit::http::auth`. Name it
distinctly (`HttpAuthProvider`) to avoid conflation with inbound auth (Pitfall 1).

### Pattern 3: Single-call tool synthesis (`path`/`method`/`base_url` = the `sql=` analog)
**What:** A `[[tools]]` entry WITHOUT a `script` field names an operation by `path`+`method` (+ optional `base_url`). Path params `{id}` become required tool params; the rest become query/body per method.
```rust
// Reference: .../tools/mod.rs::create_tool_from_config (VERIFIED — single-call ONLY; the reference
//   does NOT yet implement script tools, see Pattern 4b).
//   (path, method) required; path params extracted from "{...}" segments;
//   inputs.parameters → query/path schema; POST/PUT/PATCH → request_body.
// Toolkit analog: tools.rs::synthesize_from_config_with_connector (the SQL precedent).
```

### Pattern 4: The shared engine + seam (D-02 — THE headline pattern)

**Engine identity — DEFINITIVE.** `pmcp-code-mode 0.5.1` uses a **pure-Rust AST plan compiler +
interpreter**, NOT Boa. The prior research's "Boa runtime" claim is WRONG and is corrected here.
The real types (`[VERIFIED: crates/pmcp-code-mode/src/executor.rs + code_executor.rs]`):

| Type | Role | Location |
|------|------|----------|
| `PlanCompiler` | SWC-parses a JS subset → `ExecutionPlan` (`Vec<PlanStep>`) | `executor.rs:701` (`new()`, `with_config()`, `compile_code()`) |
| `PlanExecutor<H: HttpExecutor>` | walks the plan; each `api.get/post` → `H::execute_request` | `executor.rs:2737` (`new(http, config)`, `set_variable`, `execute`) |
| `HttpExecutor` (low-level trait) | `async fn execute_request(method, path, body) -> Result<Value, ExecutionError>` | `executor.rs:2425-2434` |
| `JsCodeExecutor<H>` | high-level adapter: wraps an `HttpExecutor`, impls `CodeExecutor`; internally `PlanCompiler`+`PlanExecutor` | `code_executor.rs:131-163` |
| `CodeExecutor` (high-level trait) | `async fn execute(code, variables) -> Result<Value, ExecutionError>` — the toolkit's wiring boundary | `code_executor.rs:54-68` |
| `ExecutionConfig` | `max_api_calls=50`, `timeout_seconds=30`, `max_loop_iterations=100`, `blocked_fields`, `output_blocked_fields` | `executor.rs:55-84` |
| `JsExecutor` | **legacy type alias = `PlanCompiler`** (`pub type JsExecutor = PlanCompiler;`) — do NOT treat as a separate engine | `executor.rs:3186-3187` |

Supported JS surface (`[VERIFIED: PlanStep`/`ArrayMethodCall` enums]`): `api.get/post/put/patch/delete`
(`PlanStep::ApiCall`), `Promise.all` (`ParallelApiCalls`), `const`/assignment, `if/else`, bounded
`for…of` (`BoundedLoop`, capped by `max_loop_iterations`), `try/catch`, `return`, and array/string
methods including `.filter`, `.map`, `.reduce`, `.find`, `.some`, `.every`, `.slice`, `.sort`,
`.flatMap`, `.length`, etc. **Chaining one call's output into the next and `filter`/`map` over arrays
across steps are both fully supported** — satisfying D-02's hard requirement.

**The single seam: `HttpCodeExecutor: HttpExecutor`.** Implement `pmcp_code_mode::HttpExecutor` ONCE
(lift the reference `HttpClientExecutor` body verbatim — it already does path-param substitution,
query/body split, and `auth.apply()`). This one impl feeds BOTH surfaces:

```rust
// NEW: crates/pmcp-server-toolkit/src/code_mode.rs (or src/http/code_exec.rs), behind `code-mode`.
// Lift the reference body from pmcp-run .../code_mode.rs:95-165 (HttpClientExecutor).
#[derive(Clone)]
pub struct HttpCodeExecutor {
    client: reqwest::Client,
    base_url: String,
    auth: Arc<dyn crate::http::auth::HttpAuthProvider>,
}

#[pmcp_code_mode::async_trait]
impl pmcp_code_mode::HttpExecutor for HttpCodeExecutor {
    async fn execute_request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, pmcp_code_mode::ExecutionError> {
        // 1. substitute {placeholder} path params from `body` object keys
        // 2. GET-like → remaining keys become query; else → JSON body
        // 3. self.auth.apply(&mut headers, &mut query)
        // 4. reqwest send; map status/body → Value or ExecutionError (REDACT url/token — Pitfall 5)
    }
}
// Source: pmcp-run .../code_mode.rs:95-165 (HttpClientExecutor — VERIFIED liftable) +
//         SDK trait pmcp-code-mode/src/executor.rs:2425
```

**Code Mode wiring (the long-tail ~80%):** wrap the seam in `JsCodeExecutor`, pass it to the toolkit's
(generalized) `code_mode_tools_from_executor` which registers `validate_code`/`execute_code`:

```rust
let http_exec = HttpCodeExecutor { client, base_url, auth };
let code_exec = pmcp_code_mode::JsCodeExecutor::new(http_exec.clone(), ExecutionConfig::default());
// OAPI-10: code_mode_tools_from_executor must accept Arc<dyn CodeExecutor> (see note below).
let builder = code_mode_tools_from_executor(builder, &cfg, Arc::new(code_exec))?;
```

**CRITICAL planner gap (OAPI-10).** The toolkit's `code_mode_tools_from_executor` is **hardcoded to
`Arc<SqlCodeExecutor>`** today (`crates/pmcp-server-toolkit/src/code_mode.rs:155-187`, and its internal
`ExecuteCodeHandler.executor: Arc<SqlCodeExecutor>`). To let the HTTP path reuse the same wiring, widen
the parameter (and the handler field) to `Arc<dyn pmcp_code_mode::CodeExecutor>`. Both `SqlCodeExecutor`
(Pattern A, no JS) and `JsCodeExecutor<HttpCodeExecutor>` (Pattern B, JS) implement `CodeExecutor`, so
one generalized function serves both backends — this IS the "backend-agnostic toolkit" goal (D-06).
The `validate_code` handler also hardcodes `validate_sql_query` + `CodeModeToolBuilder::new("sql")`; the
OpenAPI path needs the JS validation surface (`JavaScriptValidator` / the `"openapi"` builder flavor) —
the planner must parameterize the validation flavor alongside the executor. `[VERIFIED: code_mode.rs:155-381]`

### Pattern 4b: Script tools (D-01 — admin-authored, SAME engine, NO approval cycle)

**What:** A `[[tools]]` entry WITH a `script` field is a script tool. Its handler compiles the embedded
JS once and executes it against the SAME `HttpCodeExecutor` seam — but WITHOUT the validate/approve/token
cycle (the script is admin-authored and trusted, like a `sql=` curated query). Tool `args` (validated
against `[[tools.parameters]]`) are bound to the `args` variable inside the script.

**Engine sharing is by construction:** the handler calls the same `PlanCompiler` + `PlanExecutor<H>`
that `JsCodeExecutor` uses internally — so a script promoted from Code Mode to a curated tool behaves
identically (D-02). The reference `OPENAPI_SCRIPT_TOOLS.md` (v0.1, **Status: Proposed** — NOT yet
implemented in the reference `tools/mod.rs`) prescribes exactly this; Phase 90 IMPLEMENTS it (there is
no verbatim handler to lift — only the design + the SDK's existing engine).

```rust
// NEW: crates/pmcp-server-toolkit/src/tools.rs — ScriptToolHandler (OAPI-02b).
// Mirrors OPENAPI_SCRIPT_TOOLS.md Step 2 but over the SDK's current PlanCompiler/PlanExecutor.
pub struct ScriptToolHandler {
    script: String,                          // the [[tools]] `script = """..."""`
    http_exec: HttpCodeExecutor,             // SAME seam as Code Mode (D-02)
    exec_config: pmcp_code_mode::ExecutionConfig,
    tool_info: pmcp::types::ToolInfo,        // built from [[tools.parameters]] (synth path)
}

#[pmcp_code_mode::async_trait]
impl pmcp::ToolHandler for ScriptToolHandler {
    async fn handle(&self, args: serde_json::Value, _extra: pmcp::RequestHandlerExtra)
        -> pmcp::Result<serde_json::Value>
    {
        let mut compiler = pmcp_code_mode::PlanCompiler::with_config(&self.exec_config);
        let plan = compiler.compile_code(&self.script)
            .map_err(|e| pmcp::Error::Internal(format!("script compile failed: {e}")))?;
        let mut executor =
            pmcp_code_mode::PlanExecutor::new(self.http_exec.clone(), self.exec_config.clone());
        executor.set_variable("args", args);   // [[tools.parameters]] → `args` (D-01)
        let result = executor.execute(&plan).await
            .map_err(|e| pmcp::Error::Internal(format!("script execution failed: {e}")))?;
        Ok(result.value)
    }
    fn metadata(&self) -> Option<pmcp::types::ToolInfo> { Some(self.tool_info.clone()) }
}
// Source: OPENAPI_SCRIPT_TOOLS.md Steps 1-3 (design) +
//         SDK executor.rs PlanCompiler::compile_code / PlanExecutor::{new,set_variable,execute} (VERIFIED)
```

**Equivalent alternative:** the handler could instead hold an `Arc<JsCodeExecutor<HttpCodeExecutor>>`
and call `code_exec.execute(&self.script, Some(&args_wrapped))` — `JsCodeExecutor::execute` binds
`variables` to the `args` variable internally (`code_executor.rs:100-102`). Either is correct and both
hit the identical engine; the raw `PlanCompiler`/`PlanExecutor` form (above) mirrors the reference design
doc literally and avoids re-wrapping. Planner's discretion (it's a Claude's-Discretion item).

**Detection branch (D-01) in the synthesizer:**
```rust
for tool_cfg in &config.tools {
    if tool_cfg.script.is_some() {
        builder = builder.tool_arc(&tool_cfg.name, Arc::new(ScriptToolHandler::new(tool_cfg, http_exec.clone(), exec_config.clone())));
    } else { /* path+method single-call handler (Pattern 3) */ }
}
```

### Pattern 5: Shape A binary pipeline (copy `pmcp-sql-server`)
**What:** `run()` → `load_config_and_spec()` → `dispatch()` → `build_server()` → `serve()` over `StreamableHttpServer`. Identical control flow to `pmcp-sql-server/src/lib.rs`; only `--schema`→`--spec` (now OPTIONAL, D-03) and connector/executor types change.
```rust
// Mirror crates/pmcp-sql-server/src/lib.rs::run_serving (VERIFIED):
//   let (cfg, spec) = load_config_and_spec(args)?;        // spec: Option<OpenApiSchema> (D-03)
//   let (connector, http_exec) = dispatch(&cfg).await?;   // base_url + auth → HttpConnector + HttpCodeExecutor
//   let server = build_server(&cfg, connector, http_exec, spec)?;
//   serve(server, addr).await                              // StreamableHttpServer::with_config
```

### Anti-Patterns to Avoid
- **Claiming the engine is Boa / adding a second JS engine.** There is no Boa. The engine is
  `PlanCompiler`+`PlanExecutor<H>` (AST interpreter). Script tools and Code Mode MUST share it (D-02).
- **Giving script tools a different executor than Code Mode.** Both use the same `HttpCodeExecutor` seam
  and the same `PlanCompiler`/`PlanExecutor` — a promoted script must behave identically.
- **Conflating inbound and outbound auth.** Toolkit `AuthProvider::validate_request` authenticates the
  MCP *client*; the HTTP `HttpAuthProvider::apply` authenticates the *backend* call (Pitfall 1).
- **Leaving `code_mode_tools_from_executor` hardcoded to `Arc<SqlCodeExecutor>`.** It must accept
  `Arc<dyn CodeExecutor>` (OAPI-10) or the HTTP path cannot reuse it.
- **`Url::join()` for base_url + path.** Concatenate (Pitfall 2).
- **Struct-literal `ToolInfo`/`ToolAnnotations`.** Both `#[non_exhaustive]`; use the toolkit synth path.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OpenAPI spec parsing | custom YAML/JSON walker | `openapiv3` | Handles 3.0/3.1, `$ref` resolution, parameter locations |
| HTTP client + retries/timeouts | raw `hyper` loop | `reqwest 0.13` (rustls) | Reference already uses it; retries/backoff/query/json built in |
| **JS parse + plan compile + execute** | **a JS engine / interpreter / Boa** | **`pmcp-code-mode` `PlanCompiler` + `PlanExecutor<H>`** | The AST engine is SDK-resident; chaining/filter/map/loops already implemented (D-02) |
| Script-tool execution | a bespoke script runner | the SAME `PlanCompiler`/`PlanExecutor` Code Mode uses | D-02 hard requirement — identical semantics |
| Code-mode validation | SWC plumbing | `pmcp-code-mode` `openapi-code-mode` (`JavaScriptValidator`) | Already SDK-exported |
| Inbound MCP auth | new auth trait | toolkit `AuthProvider` / `StaticAuthProvider` | Phase 83 already lifted it |
| Secrets resolution (env/AWS) | env-var string munging | toolkit `SecretsProvider` + `SecretsProviderChain` | Phase 83 already lifted it |
| Static resources/prompts | per-server handlers | toolkit `StaticResourceHandler` / `StaticPromptHandler` | Phase 83 already lifted it |
| Streamable-HTTP transport + CORS/DNS-rebinding | axum routing | `pmcp::StreamableHttpServer` | Phase 56; security layers applied by the SDK |
| JSON-Schema input validation | custom validator | the toolkit's synth (object envelope + `additionalProperties:false`) | `tools.rs` already enforces it |
| Tool synthesis from config | per-tool Rust handlers | `synthesize_from_config_with_connector` + (NEW) script branch | The "no Rust" promise lives here |

**Key insight:** After lifting the reference's three genuinely-new files (`http/`, `schema/parser.rs`,
the `HttpClientExecutor` from `code_mode.rs`) and IMPLEMENTING the script-tool handler over the SDK's
existing `PlanCompiler`/`PlanExecutor`, every remaining capability already exists. The phase is ~85%
wiring, ~15% net-new HTTP/OpenAPI + script-tool glue.

## What's New vs What's Reused (THE key deliverable)

| Capability | Status | Where it lives now | Action for Phase 90 |
|------------|--------|--------------------|---------------------|
| Config parse + `${ENV}` resolution | **REUSED** | toolkit `config.rs` | Add `[backend]`/`[backend.auth]`/`[backend.http]` (additive, D-06) |
| Inbound MCP auth | **REUSED** | toolkit `auth.rs` | none |
| Secrets (env + AWS SM + SSM) | **REUSED** | toolkit `secrets.rs` | none |
| Static resources / prompts | **REUSED** | toolkit `resources.rs` / `prompts.rs` | none |
| `[[tools]]` → `ToolInfo`+handler synth | **REUSED (extend)** | toolkit `tools.rs` | single-call handler calls `HttpConnector` + ADD script branch |
| Builder extension wiring | **REUSED (extend)** | toolkit `builder_ext.rs` | Add `try_tools_from_config_with_connector` HTTP variant + code-mode HTTP variant |
| **Code-mode JS engine (AST `PlanCompiler`/`PlanExecutor`, NOT Boa)** | **REUSED** | `pmcp-code-mode 0.5.1` (`js-runtime`) | none — enable the feature; wrap with `JsCodeExecutor` / call directly for script tools |
| Code-mode validation (SWC) | **REUSED** | `pmcp-code-mode` (`openapi-code-mode`) | none — select the JS validation flavor |
| Code-mode token/HMAC/policy | **REUSED** | toolkit `code_mode.rs` (`validation_pipeline_from_config`) | none |
| Streamable-HTTP transport | **REUSED** | `pmcp::StreamableHttpServer` | none |
| Lambda deploy / asset resolution | **REUSED** | Phase 86 deploy + `pmcp::assets` | Spec/config read-only assets |
| **HTTP connector (reqwest)** | **NEW (lift)** | reference `http/mod.rs` (`HttpClient`) | Lift into `toolkit::http` behind `HttpConnector` |
| **Outgoing-HTTP AuthProvider (5 variants)** | **NEW (lift)** | reference `auth/mod.rs` + `config.rs:164-235` | Lift into `toolkit::http::auth` (SQL lift skipped this) |
| **OpenAPI spec parser (openapiv3)** | **NEW (lift)** | reference `schema/parser.rs` | Lift into `toolkit::http::schema` |
| **`HttpCodeExecutor` (impl `HttpExecutor`)** | **NEW (lift)** | reference `code_mode.rs::HttpClientExecutor` (`:95-165`) | Lift into `toolkit::code_mode`; feeds BOTH script tools + Code Mode |
| **Script-tool support (`ScriptToolHandler` + `script=` detection)** | **NEW (implement)** | `OPENAPI_SCRIPT_TOOLS.md` design (Status: Proposed) — NOT in reference src yet | IMPLEMENT over SDK `PlanCompiler`/`PlanExecutor` (D-01/D-02) |
| **Generalize `code_mode_tools_from_executor` → `Arc<dyn CodeExecutor>`** | **NEW (refactor)** | toolkit `code_mode.rs:155` (hardcoded `Arc<SqlCodeExecutor>`) | Widen so SQL + OpenAPI share one wiring fn (OAPI-10) |
| **`pmcp-openapi-server` binary** | **NEW (copy)** | — | Structural copy of `crates/pmcp-sql-server` |
| **`--kind openapi-server` scaffold** | **NEW (copy)** | — | Copy `cargo-pmcp` sql-server arm + template |
| **Docs (3 shapes)** | **NEW (copy)** | — | Mirror SQL docs |

## Common Pitfalls

### Pitfall 0 (NEW — the engine-identity trap)
**What goes wrong:** Assuming `pmcp-code-mode` runs a general JS engine (Boa/V8/QuickJS) — the prior
research did. Implementers then look for a `boa::Context`, or try to add one, or write a second engine
for script tools.
**Why it happens:** The terms "JS engine" / "JS runtime" suggest a general interpreter; the original
research stated "Boa runtime" without verifying.
**How to avoid:** The engine is `PlanCompiler` (SWC-parse → AST `ExecutionPlan`) + `PlanExecutor<H>`
(AST interpreter), pure Rust, no Boa. It accepts a JS *subset* (the `PlanStep`/`ValueExpr`/
`ArrayMethodCall` grammar). Scripts that use unsupported syntax fail at `compile_code`. Both Code Mode
and script tools use this exact pair.
**Warning signs:** A dependency hunt for a JS VM; a script tool that "works" but diverges from Code Mode;
`compile_code` errors treated as runtime faults rather than "unsupported JS construct."

### Pitfall 1: Two different `AuthProvider` traits
**What goes wrong:** The toolkit has `pmcp::server::auth::AuthProvider` (inbound, `validate_request`). The reference has a *second* `AuthProvider` (outbound, `apply(headers, query)`). Same name, opposite direction.
**How to avoid:** Name the new one `HttpAuthProvider` (or module-qualify in `toolkit::http::auth`). The OAuth-passthrough case bridges them: inbound `TokenCaptureAuthProvider` stores the client token in `AuthContext`, which the outbound provider forwards.
**Warning signs:** A tool call that compiles but never sends the API key; an MCP client that authenticates fine but the backend returns 401.

### Pitfall 2: `Url::join` drops API-Gateway stage prefixes
**What goes wrong:** `base_url="https://x/v1"` + path `/users` yields `https://x/users` (drops `/v1`) under RFC-3986 `join`.
**How to avoid:** Lift the reference's explicit path-concat (`http/mod.rs:212-218`), which has a regression test (`test_build_url_with_path_prefix`). Note `HttpCodeExecutor::execute_request` receives an already-relative `path` from the plan; the concat happens where base_url is prepended.
**Warning signs:** Backend 404s only on deployed (Gateway-fronted) instances, not local.

### Pitfall 3: Spec required at runtime vs scaffold-time (D-03)
**What goes wrong:** Forcing `--spec` at runtime when curated `[[tools]]` already carry everything needed.
**Why it happens:** The spec is needed for *discovery* (scaffold `[[tools]]` + the code-mode `api_schema` resource) but NOT to execute a curated single-call or script tool.
**How to avoid:** `--spec` is **optional at runtime** (D-03, now LOCKED). Required only at scaffold time and useful whenever Code Mode wants the full `api_schema` resource. A curated-only server boots from `config.toml` alone.
**Warning signs:** A demo that won't start without a spec file.

### Pitfall 4: `js-runtime` pulls SWC (build weight)
**What goes wrong:** Enabling `js-runtime`/`openapi-code-mode` unconditionally bloats the default binary (SWC parsers) and slows CI.
**How to avoid:** Feature-gate code-mode AND script-tool support on the binary behind the toolkit's `code-mode` feature (default-on, opt-out via `--no-default-features`). Curated single-call-only deployments compile without SWC. Note: script tools ALSO need `js-runtime` (they use `PlanCompiler`), so the script-tool branch is gated together with Code Mode.
**Warning signs:** Cold `cargo build` minutes; binary size jump.

### Pitfall 5: Secret leakage in connector/executor errors
**What goes wrong:** A reqwest error or auth failure echoes the bearer token / base_url credentials via `Display`.
**How to avoid:** Mirror the SQL `ConnectorError`/`DispatchError` redaction (`sql/mod.rs:262-272`): `HttpConnectorError` and the `ExecutionError` surfaced from `HttpCodeExecutor` must name the operation/status only, never the URL or `Authorization` value. Add `test_*_display_does_not_echo_secret`.
**Warning signs:** A token substring appearing in an MCP error response.

### Pitfall 6: The reference `pmcp_server.rs` uses pre-toolkit helpers
**What goes wrong:** Lifting `pmcp_server.rs` verbatim drags in `mcp_server_common::resolve_extra_prompt_content`, `compile_validator`, etc.
**How to avoid:** Do NOT lift `pmcp_server.rs` as-is. Replace its body with the toolkit's `assemble.rs` pattern. The Shape A `assemble.rs` is the canonical assembler.
**Warning signs:** `mcp_server_common::` paths surviving in lifted code.

### Pitfall 7 (NEW): Script tools skip validation by design — don't add a token cycle
**What goes wrong:** Treating script tools like Code Mode and forcing them through `validate_code`/token. Script tools are admin-authored and trusted (like a `sql=` curated query); they have NO approval cycle (`OPENAPI_SCRIPT_TOOLS.md` §Security).
**How to avoid:** `ScriptToolHandler` compiles + executes directly (Pattern 4b). It still honors `ExecutionConfig` bounds (`max_api_calls`, `max_loop_iterations`, timeout) and inherits the same backend path/method restrictions. The validate/approve/token machinery is only for the LLM-generated long-tail.
**Warning signs:** A curated script tool that returns "approval token required."

## Code Examples

### Config: single-call AND script tools (the D-01 two-kind model)
```toml
# Verified against instances/london-tube.toml + OPENAPI_SCRIPT_TOOLS.md design.
[server]
name = "london-tube"
version = "1.0.0"

[backend]
base_url = "https://api.tfl.gov.uk"

[backend.auth]            # 5 variants: none | api_key | bearer | basic | oauth2_client_credentials | oauth_passthrough
type = "api_key"          # london-tube (D-04): api_key in query param — the new auth shape
query_params = { app_key = "${TFL_APP_KEY}" }
required = false

[backend.http]
timeout_seconds = 30
retries = 3
retry_backoff_ms = 1000

# ── Single-call tool (path + method = the `sql=` analog) ──
[[tools]]
name = "get-tube-status"
description = "Status of all tube lines"
path = "/Line/Mode/tube/Status"
method = "GET"

[tools.annotations]
read_only_hint = true
idempotent_hint = true

# ── Script tool (script = embedded JS; multi-call domain operation, D-01) ──
[[tools]]
name = "disrupted-lines-with-detail"
description = "List tube lines currently reporting disruption, with the disruption detail for each"
script = """
const statuses = await api.get('/Line/Mode/tube/Status');
const disrupted = statuses.filter(line => line.lineStatuses.some(s => s.statusSeverity < 10));
const out = [];
for (const line of disrupted.slice(0, args.maxLines)) {
  const detail = await api.get('/Line/' + line.id + '/Disruption');
  out.push({ line: line.name, detail });
}
return { count: out.length, lines: out };
"""

[[tools.parameters]]      # bound to `args` inside the script (D-01)
name = "maxLines"
type = "integer"
description = "Max number of disrupted lines to detail"
required = false

[tools.annotations]
read_only_hint = true

[code_mode]               # the long-tail ~80% — SAME engine as the script tool above (D-02)
enabled = true
token_secret = "${PMCP_HMAC_SECRET}"   # CF-4: dev configs use an inline DEV-ONLY secret with a loud note

[secrets]
provider = "auto"         # toolkit SecretsProviderChain: AWS SM → SSM → env
```

### The shared seam (impl once, feeds both surfaces) — see Pattern 4 for the full body
```rust
#[pmcp_code_mode::async_trait]
impl pmcp_code_mode::HttpExecutor for HttpCodeExecutor {
    async fn execute_request(&self, method: &str, path: &str, body: Option<Value>)
        -> Result<Value, pmcp_code_mode::ExecutionError> { /* lift reference code_mode.rs:130-165 */ }
}
```

### Script-tool handler (implement over the SDK engine) — see Pattern 4b for the full handler
```rust
let mut compiler = pmcp_code_mode::PlanCompiler::with_config(&exec_config);
let plan = compiler.compile_code(&script)?;                       // SWC parse → ExecutionPlan
let mut executor = pmcp_code_mode::PlanExecutor::new(http_exec.clone(), exec_config.clone());
executor.set_variable("args", tool_args);                          // [[tools.parameters]] → args
let result = executor.execute(&plan).await?;                       // walks PlanSteps, calls execute_request
Ok(result.value)
```

### Code Mode wiring (the generalized executor seam — OAPI-10)
```rust
// SDK adapter wraps the SAME HttpCodeExecutor used by script tools:
let code_exec = pmcp_code_mode::JsCodeExecutor::new(http_exec.clone(), ExecutionConfig::default());
// AFTER OAPI-10 widens the signature to Arc<dyn CodeExecutor>:
let builder = code_mode_tools_from_executor(builder, &cfg, Arc::new(code_exec))?;
// SQL path is unchanged: code_mode_tools_from_executor(builder, &cfg, Arc::new(sql_code_executor))?;
// Source: toolkit code_mode.rs:155 (widen) + SDK code_executor.rs:131-163 (JsCodeExecutor)
```

### Dispatch seam (mirror SQL `dispatch.rs`, but for `[backend]`)
```rust
// NEW: crates/pmcp-openapi-server/src/dispatch.rs (analog of pmcp-sql-server/src/dispatch.rs)
pub async fn dispatch(cfg: &ServerConfig)
    -> Result<(Arc<dyn HttpConnector>, HttpCodeExecutor), DispatchError>
{
    let backend = cfg.backend.as_ref().ok_or(DispatchError::MissingBackend)?;
    let auth = toolkit::http::auth::create_auth_provider(&backend.auth)?;  // 5 variants
    let client = reqwest::Client::builder().build().map_err(DispatchError::from)?;
    let connector = toolkit::http::HttpClient::new(client.clone(), backend.base_url.clone(), auth.clone());
    let http_exec = HttpCodeExecutor { client, base_url: backend.base_url.clone(), auth };
    Ok((Arc::new(connector), http_exec))
}
// Source pattern: crates/pmcp-sql-server/src/dispatch.rs:126-135
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| "Boa runtime" (prior research claim) | pure-Rust AST `PlanCompiler` + `PlanExecutor<H>` | always (research error) | No Boa dependency; engine accepts a JS subset, not arbitrary JS |
| `shared/mcp-server-common` + `shared/mcp-lambda-proxy` path-deps | public `pmcp-server-toolkit` | Phase 83 | The lift target; replace both path-deps |
| `pmcp-code-mode 0.4.0` (reference pin, `PlanCompiler`/`PlanExecutor`) | `pmcp-code-mode 0.5.1` (same `PlanCompiler`/`PlanExecutor` + `JsCodeExecutor`/`CodeExecutor` adapters) | since reference snapshot | The 0.4.0 design is NOT obsolete — the SAME types exist in 0.5.1, now additionally wrapped by the high-level `CodeExecutor` adapter layer. Use 0.5.1; reconcile by using `JsCodeExecutor` for Code Mode and raw `PlanCompiler`/`PlanExecutor` for script tools. |
| Reference `OpenApiPmcpBuilder` (`pmcp_server.rs`) | toolkit `assemble.rs` + `ServerBuilderExt` | Phase 85 | Use the toolkit assembler, not the reference builder |
| `code_mode_tools_from_executor(Arc<SqlCodeExecutor>)` | `code_mode_tools_from_executor(Arc<dyn CodeExecutor>)` | Phase 90 (OAPI-10) | Generalize so SQL + OpenAPI share one wiring fn |
| Script tools: `OPENAPI_SCRIPT_TOOLS.md` "Status: Proposed" | implemented in Phase 90 | this phase | No verbatim handler to lift; implement over the SDK engine |

**Reconciliation verdict (D-02):** The reference design doc's `PlanCompiler`/`PlanExecutor`/
`PlanStep::ApiCall` (0.4.0 era) and the prior research's `JsCodeExecutor` (0.5.1) are **NOT competing
engines** — they are the SAME engine at two layers. `PlanCompiler`+`PlanExecutor<H>` is the engine;
`JsCodeExecutor<H>` is a thin 0.5.x adapter that wraps them behind the high-level `CodeExecutor` trait
(see `code_executor.rs::compile_and_execute`, which literally calls `PlanCompiler::with_config` then
`PlanExecutor::new`). **Use the current SDK (0.5.1):** `JsCodeExecutor` for Code Mode (it's a
`CodeExecutor`, which is what the toolkit wires), and raw `PlanCompiler`/`PlanExecutor` for script tools
(no token cycle). Both hit the identical compile+execute path. The prior research's only error was the
"Boa" label; the `JsCodeExecutor` naming was correct.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `HttpConnector` should be a trait (parity with `SqlConnector`) | Pattern 1 | Mild over-engineering if only one impl ever exists; preserves feature-gating + parity (Claude's Discretion in CONTEXT) |
| A2 | `code_mode_tools_from_executor` can be widened to `Arc<dyn CodeExecutor>` without breaking the SQL path | Pattern 4 / OAPI-10 | If `ExecuteCodeHandler`'s `validate_sql_query`/`"sql"` builder coupling is deeper than the executor field, the refactor is larger — but both are in the same module and clearly parameterizable `[VERIFIED: code_mode.rs:155-381]` |
| A3 | The `validate_code` validation flavor can be selected (SQL vs JS/OpenAPI) alongside the executor | Pattern 4 | If `CodeModeToolBuilder` / `validate_sql_query` are not parameterizable, the toolkit needs a JS-flavored handler variant (still small) `[VERIFIED: builder has per-language flavor — `CodeModeToolBuilder::new("sql")`/`"openapi"`]` |
| A4 | Script tools need no approval-token cycle (admin-authored, trusted) | Pattern 4b / Pitfall 7 | Stated explicitly by `OPENAPI_SCRIPT_TOOLS.md` §Security; if a future policy wants gated script tools, that's additive |
| A5 | `wiremock` (pure-Rust) is the right test backend, honoring no-Docker rule | Supporting stack | none material |
| A6 | Reference `HttpClientExecutor::execute_request` body is liftable into the toolkit verbatim (minus `crate::auth` path) | Pattern 4 | If it pulls `mcp_server_common`-only helpers, swap to toolkit equivalents (none observed in `:95-165`) `[VERIFIED: only `crate::auth::AuthProvider` + reqwest]` |

## Open Questions (RESOLVED)

1. **Validation flavor selection for `validate_code` in the OpenAPI path.**
   - What we know: the SDK exposes `JavaScriptValidator` and `CodeModeToolBuilder::new("openapi")`; the
     toolkit's current `ValidateCodeHandler` hardcodes `validate_sql_query` + `"sql"` (`code_mode.rs:264-311`).
   - What's unclear: whether OAPI-10 should generalize ONLY the executor, or also the validation/tool-builder
     flavor in the same refactor (cleaner) — vs. an OpenAPI-specific handler variant.
   - Recommendation: parameterize both (executor + flavor) in one refactor; it's a small, well-bounded change.
     Confidence MEDIUM (verified the coupling exists and is local).
   - **RESOLVED:** Plan 90-04 Task 2 generalizes `code_mode_tools_from_executor` + `ValidateCodeHandler`
     to `Arc<dyn CodeExecutor>` + a parameterized validation flavor in ONE refactor (executor + flavor
     together), gated so the existing SQL path stays green (`cargo test -p pmcp-sql-server` in `<automated>`).

2. **Script-tool `ExecutionConfig` defaults.**
   - What we know: SDK defaults are `max_api_calls=50`, `timeout_seconds=30`, `max_loop_iterations=100`.
   - What's unclear: whether script tools should inherit Code Mode's `ExecutionConfig` or take per-tool
     overrides from `[[tools]]` (a Claude's-Discretion item in CONTEXT).
   - Recommendation: start with the shared `[code_mode]`-derived `ExecutionConfig`; allow per-tool override
     later if needed. Confidence HIGH (defaults verified).
   - **RESOLVED:** Plan 90-05 adopts the shared `[code_mode]`-derived `ExecutionConfig` for script tools;
     per-tool `[[tools]]` overrides are deferred (Claude's-Discretion / future additive change).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `pmcp-code-mode` `js-runtime`/`openapi-code-mode` (SWC) | code-mode long-tail + script tools | ✓ | 0.5.1 (workspace) | curated single-call-only build (feature off) |
| `openapiv3` | spec parsing | ✓ (workspace dep) | per workspace | inline `[[tools]]` (no spec) |
| `reqwest 0.13` (rustls) | HTTP connector + executor | ✓ (reference uses it) | 0.13 | none needed |
| Network egress to api.tfl.gov.uk | REF parity replay | ✗ in CI sandbox | — | `wiremock` mock backend (pure-Rust, no Docker) for offline tests (D-04) |
| Docker | NOT used in tests | n/a | n/a | per project memory, no Docker in test harness; GCR deploy is the only Docker target |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** live network for parity replay → use `wiremock` for default CI;
gate live-network replays behind an env var (mirror the SQL Athena `#[ignore]`+env pattern).

## Validation Architecture

`.planning/config.json` — `workflow.nyquist_validation` not explicitly `false`; treating as enabled.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]`/`#[tokio::test]` + `proptest` + `wiremock` (HTTP mock) |
| Config file | none (cargo); run with `--test-threads=1` per CLAUDE.md |
| Quick run command | `cargo test -p pmcp-openapi-server -- --test-threads=1` |
| Full suite command | `make quality-gate` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OAPI-01 | `HttpConnector::execute` GET/POST against mock | integration | `cargo test -p pmcp-server-toolkit http_connector -- --test-threads=1` | ❌ Wave 0 |
| OAPI-02a | single-call `[[tools]]` path/method → ToolInfo schema | unit | `cargo test -p pmcp-server-toolkit synth_http -- --test-threads=1` | ❌ Wave 0 |
| OAPI-02b | script tool: `args` binding + multi-call chain + filter/map | integration (wiremock) | `cargo test -p pmcp-server-toolkit script_tool -- --test-threads=1` | ❌ Wave 0 |
| OAPI-03 | bearer/apiKey/basic apply to outgoing request | unit | `cargo test -p pmcp-server-toolkit http_auth -- --test-threads=1` | ❌ Wave 0 (reference tests liftable, `auth/mod.rs:104`) |
| OAPI-04 | openapiv3 parse JSON+YAML | unit | `cargo test -p pmcp-server-toolkit schema_parse -- --test-threads=1` | ❌ Wave 0 (reference `schema/parser.rs` tests liftable) |
| OAPI-05 | `HttpExecutor::execute_request` round-trip + path-param substitution | integration | `cargo test -p pmcp-server-toolkit http_executor -- --test-threads=1` | ❌ Wave 0 |
| OAPI-10 | script tool + Code Mode produce IDENTICAL output for the SAME script | integration (wiremock) | `cargo test -p pmcp-server-toolkit engine_parity -- --test-threads=1` | ❌ Wave 0 (D-02 proof) |
| OAPI-06 | binary serves over streamable-HTTP | integration | `cargo test -p pmcp-openapi-server http_smoke -- --test-threads=1` | ❌ Wave 0 (mirror SQL SC-1) |
| OAPI-08 | london-tube parity replay (api_key query param) | integration (wiremock) | `cargo test -p pmcp-openapi-server parity -- --test-threads=1` | ❌ Wave 0 |
| — | error Display redacts credentials | unit | `cargo test -p pmcp-server-toolkit display_no_secret -- --test-threads=1` | ❌ Wave 0 (mirror `sql/mod.rs`) |

### Sampling Rate
- **Per task commit:** `cargo test -p <crate> <module> -- --test-threads=1`
- **Per wave merge:** `cargo test --workspace -- --test-threads=1`
- **Phase gate:** `make quality-gate` green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `crates/pmcp-server-toolkit/src/http/` module + tests (lift reference `http/mod.rs` tests)
- [ ] `crates/pmcp-server-toolkit/tests/http_connector_props.rs` — proptest for URL building + path-param substitution
- [ ] `crates/pmcp-server-toolkit/tests/script_tool_engine_parity.rs` — wiremock: same script via ScriptToolHandler and via execute_code yields identical output (D-02 proof, OAPI-10)
- [ ] `crates/pmcp-openapi-server/tests/parity_replay.rs` — wiremock-backed london-tube replay
- [ ] Framework install: none (cargo + workspace deps already present)

## Security Domain

`security_enforcement` not explicitly `false` → included.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Inbound: toolkit `AuthProvider`/`StaticAuthProvider`. Outbound: HTTP `HttpAuthProvider` (5 variants) |
| V3 Session Management | partial | OAuth-passthrough binds token via pmcp `AuthContext`; code-mode token bound to user+session |
| V4 Access Control | yes | code-mode policy (`validation_pipeline_from_config`); script tools inherit backend path/method restrictions |
| V5 Input Validation | yes | JSON-Schema object envelope (`additionalProperties:false`); script `args` validated against `[[tools.parameters]]` |
| V6 Cryptography | yes | HMAC approval tokens via `HmacTokenGenerator` — never hand-roll |
| V7 Error Handling / Logging | yes | `HttpConnectorError` + `HttpCodeExecutor` `ExecutionError` MUST redact URL + `Authorization` (Pitfall 5) |
| V9 Communications | yes | reqwest `rustls` (TLS); base_url should be `https` |
| V10 Malicious Code | yes | Code-mode JS goes through `validate_code`→token→`execute_code`; script tools are admin-authored + bounded by `ExecutionConfig` (no arbitrary JS — `PlanCompiler` accepts only the supported subset) |

### Known Threat Patterns for OpenAPI/HTTP MCP
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Credential echo in error/log | Information Disclosure | Redact URL + token in `Display` (test it) |
| SSRF via attacker-controlled base_url/path | Tampering/EoP | base_url is operator-config; path params schema-validated; tool args never set base_url |
| Arg injection into query/path | Tampering | JSON-Schema validation + `additionalProperties:false`; path substitution only for declared `{params}` |
| Script tool runs unbounded / DoS | DoS | `ExecutionConfig` caps (`max_api_calls`, `max_loop_iterations`, `timeout_seconds`); `PlanExecutor` enforces |
| Untrusted JS escaping the engine | EoP | `PlanCompiler` accepts ONLY the supported AST subset (no `eval`, no FFI, no arbitrary host calls); Code-mode adds validate→token gate for LLM-generated code |
| Token replay across users | Spoofing | HMAC token bound to user_id+session_id (Code Mode only) |
| Header injection via tool args | Tampering | `HeaderName/HeaderValue::try_from` reject invalid (reference `http/mod.rs:138`) |
| API-Gateway stage-prefix drop | correctness boundary | path concat, not `Url::join` (Pitfall 2) |

## Sources

### Primary (HIGH confidence — read directly this session)
- `crates/pmcp-code-mode/src/lib.rs` (full) — public exports; confirms `PlanCompiler`, `PlanExecutor`, `HttpExecutor`, `JsCodeExecutor`, `JsExecutor`(=`PlanCompiler`), `CodeExecutor` under `js-runtime`
- `crates/pmcp-code-mode/src/code_executor.rs` (full) — `CodeExecutor` trait + `JsCodeExecutor<H>`/`SdkCodeExecutor`/`McpCodeExecutor` adapters; `compile_and_execute` showing `PlanCompiler`+`PlanExecutor` are the engine
- `crates/pmcp-code-mode/src/executor.rs` (outline + key blocks `:55-104,266-335,492-583,2425-2475,2737-2769,3186-3187`) — `ExecutionConfig`, `PlanStep`, `ArrayMethodCall` (filter/map/reduce/…), `HttpExecutor` trait, `PlanExecutor` API, `JsExecutor` alias
- `crates/pmcp-code-mode/Cargo.toml` — features (`js-runtime → openapi-code-mode → swc_*`); NO Boa dependency anywhere
- `crates/pmcp-server-toolkit/src/code_mode.rs` (`:1-1266`) — `SqlCodeExecutor: CodeExecutor`, `code_mode_tools_from_executor` HARDCODED to `Arc<SqlCodeExecutor>` (OAPI-10 gap), `ValidateCodeHandler`/`ExecuteCodeHandler` with `"sql"` flavor coupling
- pmcp-run `.../mcp-openapi-server-core/src/code_mode.rs:95-165` — `HttpClientExecutor` impl `HttpExecutor` (the seam to lift); confirms reqwest + `Arc<dyn AuthProvider>` + path-param substitution
- pmcp-run `.../mcp-openapi-server-core/src/tools/mod.rs` — `create_tool_from_config` (single-call ONLY; script tools NOT yet implemented in reference src)
- pmcp-run `.../mcp-openapi-server-core/src/config.rs:164-235` — the 5-variant `AuthConfig` enum
- pmcp-run `OPENAPI_SCRIPT_TOOLS.md` (full) — the script-tool design (Status: Proposed): `script`/`[[tools.parameters]]`/`args`, detection logic, `ScriptToolHandler` over `PlanCompiler`/`PlanExecutor`/`HttpExecutor`
- `.planning/phases/90-openapi-built-in-server/90-CONTEXT.md` — locked D-01..D-06 + CF-1..CF-8

### Secondary (MEDIUM confidence)
- pmcp-run design docs (skimmed): `OPENAPI_CODE_MODE_DESIGN.md`, `..._POLICY_DESIGN.md`, `..._ACCESS_CONTROL.md`, `BUILTIN_SERVER_ARCHITECTURE.md`, `DEPLOYMENT.md`
- `crates/pmcp-sql-server/src/{lib,cli,dispatch,assemble}.rs` (Shape A binary to mirror — from prior pass)

### Tertiary (LOW confidence)
- none — all engine + tool-model claims are grounded in directly-read source.

## Metadata

**Confidence breakdown:**
- Engine identity (D-02): HIGH — `PlanCompiler`/`PlanExecutor`/`JsCodeExecutor`/`CodeExecutor` read directly; "no Boa" verified by Cargo.toml + lib.rs (no boa dep, AST interpreter present)
- Script-tool model (D-01): HIGH for the design + SDK engine; MEDIUM that the reference has no impl to lift (it's "Proposed") — so Phase 90 implements it (small, well-specified)
- `code_mode_tools_from_executor` generalization (OAPI-10): HIGH that the coupling is local + parameterizable; MEDIUM on exact refactor size (validation-flavor coupling)
- Standard stack: HIGH — versions verified live; reference Cargo.toml read
- Lift map / What's-new-vs-reused: HIGH — both trees read side-by-side
- Spec-optional-at-runtime (D-03): HIGH — now LOCKED in CONTEXT

**Research date:** 2026-05-28 (original) / 2026-05-29 (refresh)
**Valid until:** 2026-06-28 (stable — lift target frozen; toolkit/code-mode versions may bump)
