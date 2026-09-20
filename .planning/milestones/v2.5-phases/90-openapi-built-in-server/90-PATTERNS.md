# Phase 90: OpenAPI Built-In Server (`pmcp-openapi-server`) - Pattern Map

**Mapped:** 2026-05-29
**Files analyzed:** 18 (12 new, 6 modified)
**Analogs found:** 17 / 18 (only the dispatch seam is partial-analog; see "No Analog Found")

> This phase is a STRUCTURAL LIFT/MIRROR of the shipped SQL toolkit (Phases 83–86).
> Almost every new file has a precise existing analog. Two sources feed each new file:
> 1. **In-tree analog** (`rust-mcp-sdk`) — for the SHAPE (trait surface, redaction discipline,
>    binary control flow, scaffold structure). Copy the structure from here.
> 2. **Reference body** (`~/Development/mcp/sdk/pmcp-run/built-in/openapi-api`) — for the
>    HTTP/OpenAPI BUSINESS LOGIC (path-concat, header guards, auth variants, openapiv3 parse).
>    Lift the body from here, swapping `crate::auth`/`mcp_server_common` → toolkit paths.
>
> The global substitutions across the whole phase:
> `SqlConnector → HttpConnector` · `[database] → [backend]` · `sql= → path/method OR script=` ·
> `SqlCodeExecutor → HttpCodeExecutor` · `--schema (required) → --spec (OPTIONAL, D-03)` ·
> `Dialect → base_url/HttpConfig` · `dispatch on [database] type → dispatch on [backend]`.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog (in-tree) | Reference Body to Lift | Match |
|-------------------|------|-----------|--------------------------|------------------------|-------|
| **NEW** `crates/pmcp-server-toolkit/src/http/mod.rs` | trait/model | request-response | `…/src/sql/mod.rs` (`SqlConnector`, `ConnectorError`) | — (shape only) | exact |
| **NEW** `…/src/http/client.rs` | service (connector) | request-response | `…/src/sql/sqlite.rs` (impl pattern) | ref `http/mod.rs` (`HttpClient`) | exact |
| **NEW** `…/src/http/auth.rs` | provider (outbound auth) | request-response | `…/src/auth.rs` (inbound, CONTRAST only) | ref `auth/mod.rs` + `config.rs:164-235` | role-match |
| **NEW** `…/src/http/schema.rs` | utility (parser) | transform | `…/src/sql/mod.rs::schema_text` (role) | ref `schema/parser.rs` (openapiv3) | role-match |
| **NEW** `HttpCodeExecutor` in `…/src/code_mode.rs` | service (code-mode seam) | request-response | `SqlCodeExecutor` (`code_mode.rs:428-542`) | ref `code_mode.rs:95-204` (`HttpClientExecutor`) | exact |
| **NEW** `ScriptToolHandler` in `…/src/tools.rs` | handler (curated script) | event-driven (multi-call chain) | `SynthesizedToolHandler` (`tools.rs`) | `OPENAPI_SCRIPT_TOOLS.md` (design, not impl) | role-match |
| **MOD** `code_mode_tools_from_executor` (`code_mode.rs:155`) | wiring fn | — | itself (hardcoded `Arc<SqlCodeExecutor>`) | — | refactor |
| **MOD** `ValidateCodeHandler`/`ExecuteCodeHandler` (`code_mode.rs:243-381`) | handler | request-response | itself (`"sql"` flavor hardcoded) | — | refactor |
| **MOD** `ServerConfig` `[backend]` section (`config.rs`) | config | — | `DatabaseSection` (`config.rs:307-345`) | ref `config.rs` `BackendConfig`/`AuthConfig`/`HttpConfig` | exact |
| **MOD** `ToolDecl` add `path`/`method`/`script`/`base_url` (`config.rs:490`) | config | — | `ToolDecl.sql` (`config.rs:499`) | ref `tools/mod.rs` config shape | exact |
| **MOD** `synthesize_inner` script/single-call branch (`tools.rs`) | synthesizer | — | `synthesize_from_config_with_connector` (`tools.rs:112`) | ref `tools/mod.rs::create_tool_from_config` | role-match |
| **NEW** `crates/pmcp-openapi-server/src/lib.rs` | binary pipeline | request-response | `pmcp-sql-server/src/lib.rs` | — (structural copy) | exact |
| **NEW** `…/src/cli.rs` | config (CLI) | — | `pmcp-sql-server/src/cli.rs` | — | exact |
| **NEW** `…/src/dispatch.rs` | service (backend select) | — | `pmcp-sql-server/src/dispatch.rs` | ref `pmcp_server.rs` (auth+client construction only) | role-match |
| **NEW** `…/src/assemble.rs` | service (server assembly) | — | `pmcp-sql-server/src/assemble.rs` | — (do NOT lift ref `pmcp_server.rs`, Pitfall 6) | exact |
| **NEW** `…/src/main.rs` | binary entry | — | `pmcp-sql-server/src/main.rs` | — | exact |
| **MOD** `cargo-pmcp/src/commands/new.rs` (`--kind openapi-server` arm) | CLI command | — | `new.rs:69-73` + `execute_sql_server` (`:139-160`) | — | exact |
| **NEW** `cargo-pmcp/src/templates/openapi_server.rs` | template emitter | — | `templates/sql_server.rs` | — | exact |

---

## Pattern Assignments

### NEW `crates/pmcp-server-toolkit/src/http/mod.rs` (trait/model, request-response)

**Analog:** `crates/pmcp-server-toolkit/src/sql/mod.rs` (read fully).

Mirror three things from the SQL analog: (1) the `#[async_trait] Send + Sync + 'static` trait,
(2) the `#[non_exhaustive]` `thiserror` error enum, (3) the **redaction discipline + its test**.

**Trait shape** (analog `sql/mod.rs:100-142` `SqlConnector`):
```rust
// SQL analog — copy this shape, swap dialect()→base_url(), sql/params→operation/args:
#[async_trait]
pub trait SqlConnector: Send + Sync + 'static {
    fn dialect(&self) -> Dialect;
    async fn execute(&self, sql: &str, params: &[(String, serde_json::Value)])
        -> Result<Vec<serde_json::Value>, ConnectorError>;
    async fn schema_text(&self) -> Result<String, ConnectorError>;
}
```
New `HttpConnector` (per RESEARCH Pattern 1 — `HttpConnector` as a trait is a Claude's-Discretion
item, research recommends the trait for parity + feature-gating):
```rust
#[async_trait]
pub trait HttpConnector: Send + Sync + 'static {
    async fn execute(&self, operation: &Operation, args: &serde_json::Value)
        -> Result<serde_json::Value, HttpConnectorError>;
    fn base_url(&self) -> &str;   // analog of dialect()
}
```

**Error enum + redaction** (analog `sql/mod.rs:218-272`): copy the `#[non_exhaustive]` enum with the
`#[error("connection error: {0}")]` variant and the **Security doc-comment** at `:262-272` verbatim:
"Implementors MUST redact credentials … the inner `String` reaches MCP clients via `Display`."
`HttpConnectorError` MUST name operation/status only, NEVER URL or `Authorization` (Pitfall 5).

**Redaction test to copy** (analog `sql/mod.rs:417-426` `test_connection_display_does_not_echo_password`):
write `test_http_error_display_does_not_echo_secret` asserting the rendered `Display` contains no
`Bearer`, no `Authorization`, no `app_key`, no URL host.

---

### NEW `crates/pmcp-server-toolkit/src/http/client.rs` (service/connector, request-response)

**Analog (shape):** `crates/pmcp-server-toolkit/src/sql/sqlite.rs` (a concrete `SqlConnector` impl).
**Reference body to LIFT:** `~/Development/mcp/sdk/pmcp-run/built-in/openapi-api/crates/mcp-openapi-server-core/src/http/mod.rs` (`HttpClient`, read fully — 554 lines, liftable).

Lift the reference `HttpClient` near-verbatim. The genuinely-new logic is already correct there:

**Path-concat (NOT `Url::join` — Pitfall 2)** — reference `http/mod.rs:212-218`, has a regression test:
```rust
// Concatenate paths instead of using Url::join() which follows RFC 3986.
// join() … silently drops API Gateway stage prefixes like "/v1".
let mut url = base_url.clone();
let base_path = url.path().trim_end_matches('/');
let tool_path = path.trim_start_matches('/');
url.set_path(&format!("{}/{}", base_path, tool_path));
```
Lift the test `test_build_url_with_path_prefix` (`http/mod.rs:507-527`) too — it proves the `/v1` survives.

**Header injection guards** — reference `http/mod.rs:137-143`: `HeaderName::try_from` / `HeaderValue::try_from`
reject invalid header names/values (threat: header injection via tool args).

**Auth application point** — reference `http/mod.rs:146`: `self.auth.apply(&mut headers, &mut query_params).await?`.

**Retry/backoff** — reference `http/mod.rs:329-384` (`execute_with_retries`): exponential backoff,
retry on 5xx / connect / timeout. Carries over unchanged.

**What changes on lift:** `use crate::auth::AuthProvider` → `use crate::http::auth::HttpAuthProvider`;
`crate::error::{Error,Result}` → toolkit `HttpConnectorError`; `crate::config::HttpConfig` → toolkit's
new `[backend.http]` struct. Implement the new `HttpConnector` trait `execute()` over `execute_with_options`.

---

### NEW `crates/pmcp-server-toolkit/src/http/auth.rs` (provider, outbound auth) — OAPI-03 / D-05

**CONTRAST analog (DO NOT copy — Pitfall 1):** `crates/pmcp-server-toolkit/src/auth.rs` is the
**inbound** MCP-client `AuthProvider::validate_request`. The new provider is **outbound**
`apply(&mut HeaderMap, &mut query)`. Name it `HttpAuthProvider` to avoid conflation.

**Reference body to LIFT (the 5-variant enum, near-verbatim):** ref `config.rs:164-235`:
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    None,
    ApiKey { #[serde(default)] query_params: HashMap<String,String>,
             #[serde(default)] headers: HashMap<String,String>,
             #[serde(default = "default_true")] required: bool },
    Bearer { token: String, #[serde(default = "default_true")] required: bool },
    Basic { username: String, password: String, #[serde(default = "default_true")] required: bool },
    OAuth2ClientCredentials { token_url, client_id, client_secret, scopes, required },
    OAuthPassthrough { #[serde(default = "default_auth_header")] target_header, required },
}
```
**Constructor pattern to LIFT:** ref `auth/mod.rs:26-30` `create_auth_provider(&AuthConfig) -> Arc<dyn AuthProvider>`
and `:47-53` `create_passthrough_auth_provider(config, incoming_token)`. The reference re-exports the
`apply()` impls from `mcp_server_common::auth` — the lift must **inline those impls** into
`toolkit::http::auth` (the SQL lift replaced exactly this `mcp_server_common` path-dep). The 5 unit tests
at ref `auth/mod.rs:104-152` (`test_no_auth`/`test_bearer_auth`/`test_basic_auth`) are directly liftable.

**D-04 note:** london-tube uses `ApiKey { query_params }` (api_key in query param) — the one variant the
parity replay exercises live. `oauth_passthrough` bridges to inbound via pmcp `AuthContext` (D-05, Pitfall 1).

---

### NEW `crates/pmcp-server-toolkit/src/http/schema.rs` (utility/parser, transform) — OAPI-04

**Role analog:** `sql/mod.rs::schema_text` (the thing that produces the code-mode schema resource).
**Reference body to LIFT:** ref `schema/parser.rs` (openapiv3; `serde_yaml` fallback for YAML specs).
Defines `Operation` (used by `HttpConnector::execute` and the single-call synthesizer) with
`path_parameters()`/`query_parameters()`/`header_parameters()` accessors — see how `http/mod.rs`
consumes them at `:188,230,264`.

**D-03 (the deliberate divergence from SQL):** `--spec` is **OPTIONAL at runtime**. `OpenApiSchema::parse`
returns into an `Option<OpenApiSchema>` threaded through the binary. A curated-only server boots from
`config.toml` alone. Contrast SQL's `--schema` which is effectively required (`cli.rs:46-47` `pub schema: PathBuf`).

---

### NEW `HttpCodeExecutor` in `crates/pmcp-server-toolkit/src/code_mode.rs` (service/seam) — OAPI-05

**In-tree analog (CodeExecutor shape):** `SqlCodeExecutor` at `code_mode.rs:428-542`. Mirror the
struct-holds-deps + `#[pmcp_code_mode::async_trait] impl CodeExecutor` shape — but `HttpCodeExecutor`
implements the **low-level `HttpExecutor`** trait (feeds `JsCodeExecutor` + script tools), NOT `CodeExecutor`.

**Reference body to LIFT (near-verbatim):** ref `code_mode.rs:95-204` (`HttpClientExecutor`). The
`execute_request` body already does path-param substitution, GET→query / POST→body split, and `auth.apply()`:
```rust
// ref code_mode.rs:128-169 — lift this body:
impl HttpExecutor for HttpClientExecutor {
    async fn execute_request(&self, method: &str, path: &str, body: Option<Value>)
        -> Result<Value, pmcp_code_mode::ExecutionError> {
        let is_get_like = matches!(method.to_uppercase().as_str(), "GET"|"HEAD"|"OPTIONS");
        // substitute {placeholder} path params from body object keys …
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), resolved_path);
        self.auth.apply(&mut headers, &mut auth_params).await
            .map_err(|e| ExecutionError::RuntimeError { message: format!("Authentication failed: {}", e) })?;
        // GET-like → remaining keys become query; else → JSON body …
    }
}
```
Note ref uses `format!("{}{}", base_url.trim_end_matches('/'), resolved_path)` (already-relative path
from the plan) — this is the executor-side analog of the connector's path-concat (Pitfall 2 still applies).

**What changes on lift:** `crate::auth::AuthProvider` → `crate::http::auth::HttpAuthProvider`; gate behind
the toolkit's `code-mode` feature (Pitfall 4 — script tools also need `js-runtime`). **Redaction (Pitfall 5):**
the `ExecutionError` surfaced here must never echo URL/token — the ref `RuntimeError { message }` already
avoids the URL; keep it that way and add a `test_*_display_no_secret`.

**`Clone` impl:** lift ref `code_mode.rs:117-126` (the seam is cloned into every script tool + the JsCodeExecutor).

---

### NEW `ScriptToolHandler` in `crates/pmcp-server-toolkit/src/tools.rs` (handler) — OAPI-02b / D-01 / D-02

**No verbatim body to lift** — `OPENAPI_SCRIPT_TOOLS.md` is "Status: Proposed", the reference
`tools/mod.rs` does NOT implement script tools. IMPLEMENT it over the SDK's `PlanCompiler`/`PlanExecutor`.

**In-tree analog (ToolHandler shape):** the toolkit's existing `SynthesizedToolHandler` in `tools.rs`
(the `#[async_trait] impl ToolHandler { async fn handle(...); fn metadata() -> Some(ToolInfo) }` pattern)
+ the `ValidateCodeHandler`/`ExecuteCodeHandler` handlers in `code_mode.rs:243-381` for how a handler
wires a `pmcp_code_mode::*` type.

**The handler body (RESEARCH Pattern 4b — D-02: SAME engine as Code Mode, NO token cycle):**
```rust
#[pmcp_code_mode::async_trait]
impl pmcp::ToolHandler for ScriptToolHandler {
    async fn handle(&self, args: serde_json::Value, _extra: pmcp::RequestHandlerExtra)
        -> pmcp::Result<serde_json::Value> {
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
```
The `tool_info` is built from `[[tools.parameters]]` via the EXISTING synth path (`build_input_schema`
in `tools.rs`) — same as a single-call tool, so `args` is schema-validated before the script runs.

**Pitfall 7:** NO `validate_code`/token cycle — admin-authored + trusted, like a `sql=` query. Bound only
by `ExecutionConfig` (`max_api_calls=50`, `max_loop_iterations=100`, `timeout_seconds=30` defaults).

---

### MOD `code_mode_tools_from_executor` + the two handlers (`code_mode.rs`) — OAPI-10 (CRITICAL)

**Analog = the CURRENT hardcoded code itself.** Three coupled changes (all in `code_mode.rs`):

1. **Widen the wiring fn** (`code_mode.rs:155-187`): `executor: Arc<SqlCodeExecutor>` →
   `executor: Arc<dyn pmcp_code_mode::CodeExecutor>`. Both `SqlCodeExecutor` (no JS) and
   `JsCodeExecutor<HttpCodeExecutor>` (JS) impl `CodeExecutor`, so one fn serves both backends.
2. **Widen `ExecuteCodeHandler.executor`** (`code_mode.rs:318-321`): field type
   `Arc<SqlCodeExecutor>` → `Arc<dyn CodeExecutor>`. The `handle` body at `:370-374` already calls
   `self.executor.execute(code, input.variables.as_ref())` through the trait — no body change.
3. **Parameterize the validation flavor** — both handlers hardcode the `"sql"` flavor:
   - `ValidateCodeHandler::metadata` (`code_mode.rs:311`): `CodeModeToolBuilder::new("sql")` → take a
     `flavor: &str` field (`"sql"` | `"openapi"`).
   - `ValidateCodeHandler::handle` (`code_mode.rs:273`): `self.pipeline.validate_sql_query(...)` — the
     OpenAPI path needs the JS validation surface (`JavaScriptValidator` per RESEARCH Open Question 1).
   - `ExecuteCodeHandler::metadata` (`code_mode.rs:379`): same `new("sql")` → flavor field.

   Recommendation (RESEARCH OQ-1, confidence MEDIUM): parameterize executor + flavor in ONE refactor;
   the coupling is local to this module and parameterizable (`[VERIFIED: code_mode.rs:155-381]`).
   Keep the SQL call site unchanged: `code_mode_tools_from_executor(builder, &cfg, Arc::new(sql_exec))`.

---

### MOD `ServerConfig` `[backend]` section (`config.rs`) — D-06

**Analog:** `DatabaseSection` (`config.rs:307-345`) and how it hangs off `ServerConfig`
(`config.rs:112-113` `pub database: DatabaseSection`). Add additively, preserving
`#[serde(deny_unknown_fields)]` on every new struct (the whole-file invariant, `config.rs:8-24`).

Add to `ServerConfig` (mirror `:111-113`):
```rust
/// `[backend]` — OpenAPI/HTTP backend connection + auth + http tuning (D-06).
#[serde(default)]
pub backend: Option<BackendSection>,
```
New structs (all `#[serde(deny_unknown_fields)]`, mirroring `DatabaseSection`'s `:306-307` derive line):
- `BackendSection { base_url: String, #[serde(default)] auth: AuthConfig, #[serde(default)] http: HttpConfig }`
  — lift from ref `config.rs:140-159`.
- `AuthConfig` (5-variant, `#[serde(tag="type", rename_all="snake_case")]`) — lift ref `config.rs:164-245`
  (incl. `Default → None`, `default_true`, `default_auth_header`).
- `HttpConfig { timeout_seconds, retries, retry_backoff_ms, user_agent, default_headers }` — lift ref `config.rs` `HttpConfig`.

Existing SQL configs are unaffected (additive `Option<BackendSection>`). Extend the config-parser fuzz
target for `[backend]` (CONTEXT ALWAYS coverage).

---

### MOD `ToolDecl` (`config.rs:490-509`) — D-01 two-kind detection

**Analog:** the existing `ToolDecl.sql` field (`config.rs:499`):
```rust
/// SQL template (uses `:param` placeholders bound by [`ParamDecl`]).
#[serde(default)]
pub sql: Option<String>,
```
Add, mirroring that exact pattern (all `#[serde(default)] Option<…>`, preserving `deny_unknown_fields`):
```rust
/// Single-call tool: HTTP path (with `{param}` placeholders). Mutually exclusive with `script`.
#[serde(default)] pub path: Option<String>,
/// Single-call tool: HTTP method (GET/POST/…). Pairs with `path`.
#[serde(default)] pub method: Option<String>,
/// Single-call tool: optional per-tool base_url override.
#[serde(default)] pub base_url: Option<String>,
/// Script tool: embedded JS (`api.get(...)` / `api.post(...)`). Presence ⇒ script tool (D-01).
#[serde(default)] pub script: Option<String>,
```
`ParamDecl` (`config.rs:518-546`) is reused unchanged — for single-call it maps `{path}`+query/body,
for script tools it binds to `args` (D-01).

**Detection rule (D-01):** `script.is_some()` ⇒ `ScriptToolHandler`; else `path`+`method` ⇒ single-call.

---

### MOD `synthesize_inner` (`tools.rs`) — single-call + script branch (OAPI-02a/b)

**Analog:** `synthesize_from_config_with_connector` (`tools.rs:112-117`) and the shared `synthesize_inner`
body it delegates to. Mirror it as `synthesize_from_config_with_http_connector(cfg, Arc<dyn HttpConnector>, http_exec)`.

**Single-call body to LIFT (operation building):** ref `tools/mod.rs::create_tool_from_config`
(single-call ONLY — the reference has no script tools). Path params extracted from `"{...}"` segments;
inputs → query/path schema; POST/PUT/PATCH → request_body. The synthesized handler calls
`HttpConnector::execute(&operation, &args)`.

**Detection branch (RESEARCH Pattern 4b):**
```rust
for tool_cfg in &config.tools {
    if tool_cfg.script.is_some() {
        builder = builder.tool_arc(&tool_cfg.name,
            Arc::new(ScriptToolHandler::new(tool_cfg, http_exec.clone(), exec_config.clone())));
    } else { /* path+method single-call handler over HttpConnector */ }
}
```
Reuse the EXISTING `build_input_schema`/`build_param_property`/`build_annotations` helpers (`tools.rs`)
for both kinds (object envelope + `additionalProperties:false`, the no-Rust-required guarantee).

---

### NEW `crates/pmcp-openapi-server/src/lib.rs` (binary pipeline) — OAPI-06

**Analog:** `crates/pmcp-sql-server/src/lib.rs` (read fully — structural copy). Mirror:
- `RunError` enum (`lib.rs:52-97`) — same variants; rename `schema`→`spec`, keep `Io`/`Config`/`Dispatch`/`Assemble`/`Addr`/`Serve`/`Serving`.
- `load_config_and_schema` (`lib.rs:108-119`) → `load_config_and_spec` returning `(ServerConfig, Option<OpenApiSchema>)`
  (D-03: spec OPTIONAL — `args.spec.as_ref().map(|p| parse...)` instead of an unconditional `read_to_string`).
- `serve` (`lib.rs:156-164`) — copy VERBATIM (`StreamableHttpServer::with_config` + `StreamableHttpServerConfig::default()`, CF-1).
- `run_serving` (`lib.rs:200-211`) — same shape:
  ```rust
  let (cfg, spec) = load_config_and_spec(args)?;
  let (connector, http_exec) = dispatch(&cfg).await?;   // returns the pair (see dispatch.rs)
  let server = build_server(&cfg, connector, http_exec, spec)?;
  let addr: SocketAddr = args.http.parse()...;
  serve(server, addr).await
  ```
- `run` (`lib.rs:245-265`) — copy VERBATIM (tracing init + `handle.await.map_err(RunError::Serving)`, threat T-85-10-02).

---

### NEW `crates/pmcp-openapi-server/src/cli.rs` (CLI) — OAPI-06

**Analog:** `pmcp-sql-server/src/cli.rs` (read fully). Copy the `Args` struct + tests; change ONLY:
- `pub schema: PathBuf` (required, `cli.rs:46-47`) → `pub spec: Option<PathBuf>` (**OPTIONAL**, D-03 —
  drop `#[arg(long)]` requiredness, use `Option`). Document "required at scaffold time, optional at runtime".
- `name`/`about` strings → openapi-server.
- Keep `pub config: PathBuf` (required) and `pub http: String` default `"127.0.0.1:8080"` (`cli.rs:50-51`) unchanged.

---

### NEW `crates/pmcp-openapi-server/src/dispatch.rs` (backend select)

**Analog (shape + redaction):** `pmcp-sql-server/src/dispatch.rs` (read fully). The SQL version matches
on `cfg.database.backend_type` and returns `Arc<dyn SqlConnector>`. The OpenAPI version is SIMPLER —
ONE backend (HTTP), so no per-backend feature-gated arms; instead it builds the connector + executor pair.

**RESEARCH Pattern (Dispatch seam):**
```rust
pub async fn dispatch(cfg: &ServerConfig)
    -> Result<(Arc<dyn HttpConnector>, HttpCodeExecutor), DispatchError> {
    let backend = cfg.backend.as_ref().ok_or(DispatchError::MissingBackend)?;
    let auth = toolkit::http::auth::create_auth_provider(&backend.auth)?;   // 5 variants
    let client = reqwest::Client::builder().build().map_err(DispatchError::from)?;  // lazy, CF-2
    let connector = toolkit::http::HttpClient::new(client.clone(), backend.base_url.clone(), auth.clone());
    let http_exec = HttpCodeExecutor { client, base_url: backend.base_url.clone(), auth };
    Ok((Arc::new(connector), http_exec))
}
```
**Copy from SQL dispatch:** the `DispatchError` `#[non_exhaustive]` enum + its Security doc-comment
(`dispatch.rs:42-103`) — `Display` names backend/field ONLY, never URL/credential (V7 / Pitfall 5).
**CF-2 lazy startup:** the reqwest client is constructed without contacting the backend (mirror SQL's
"no `schema_text()`/`execute()` at dispatch time", `dispatch.rs:113-115`).

---

### NEW `crates/pmcp-openapi-server/src/assemble.rs` (server assembly) — OAPI-06

**Analog:** `pmcp-sql-server/src/assemble.rs` (read first 200 lines). **Pitfall 6: do NOT lift the
reference `pmcp_server.rs`** — replace it with this toolkit `assemble.rs` pattern.

Mirror:
- `AssembleError` (`assemble.rs:71-83`) — `Toolkit(#[from] ToolkitError)` + `Build(#[from] pmcp::Error)`.
- `build_server(&cfg, connector, http_exec, spec)` — uses the toolkit builder chain via `ServerBuilderExt`
  (single crate-root import, `assemble.rs:42-44`).
- The resource/prompt merge helpers (`merge_schema_resource` `assemble.rs:98-152`, the synthesized
  `code-mode://instructions`/`code-mode://policies` resources `:155-200`). For OpenAPI the schema
  resource is the OpenAPI spec (D-03) — when `spec` is `None`, skip the spec-resource override; when
  `Some`, merge it (the `/schema`-suffix override logic at `:111` generalizes).
- Code-mode wired via the (now generalized, OAPI-10) `try_code_mode_from_config_with_connector` analog —
  build a `JsCodeExecutor<HttpCodeExecutor>` and pass `Arc::new(code_exec)` to `code_mode_tools_from_executor`.

---

### NEW `crates/pmcp-openapi-server/src/main.rs` + `Cargo.toml`

**Analog:** `pmcp-sql-server/src/main.rs` (492-byte thin `#[tokio::main]` shim → `run(Args::parse())`).
**Cargo.toml:** mirror `pmcp-sql-server/Cargo.toml`; deps per RESEARCH "Installation" (`pmcp` w/ `streamable-http`,
`pmcp-server-toolkit` w/ `code-mode`, `clap`, `tokio`, `thiserror`, dev: `mcp-tester`, `wiremock`).
Add `crates/pmcp-openapi-server` to root `Cargo.toml` `[workspace.members]`. Publish slot: after the
toolkit + connector crates (CLAUDE.md Release order).

---

### MOD `cargo-pmcp/src/commands/new.rs` — `--kind openapi-server` arm (OAPI-07 / CF-3)

**Analog:** the existing sql-server arm. Add to the `match kind.as_deref()` block (`new.rs:69-73`):
```rust
// CURRENT:
match kind.as_deref() {
    Some("sql-server") => return execute_sql_server(&workspace_dir, &name, global_flags),
    Some(k) => anyhow::bail!("unknown --kind '{}'; supported: sql-server", k),
    None => {},
}
// ADD an openapi-server arm + widen the error message to "sql-server, openapi-server".
```
Copy `execute_sql_server` (`new.rs:139-160`) → `execute_openapi_server` (validate name via
`validate_crate_name` `:109-133`, `fs::create_dir_all(src)`, call `templates::openapi_server::generate`)
and `print_sql_server_next_steps` (`:162-191`) → `print_openapi_server_next_steps`.
**Folded todo (CONTEXT):** document the new `--kind openapi-server` surface in the cargo-pmcp README/help
(scoped — broad rewrite stays Phase 89).

---

### NEW `cargo-pmcp/src/templates/openapi_server.rs` (OAPI-07 / CF-3/4/5)

**Analog:** `cargo-pmcp/src/templates/sql_server.rs` (read first 60 lines — raw-string emission, no template
engine; `format!` escapes literal braces as `{{`/`}}`). Mirror the `generate` orchestrator (`sql_server.rs:33-44`)
+ one `generate_<file>` per file. Emit a SINGLE runnable crate:
- `Cargo.toml` — features `["code-mode", "http"]` (mirror `sql_server.rs:58`; drop `sqlite`, add `http`).
- `src/main.rs` — the ≤15-line wiring (CF-5: load config[+spec] → connector → `ServerBuilderExt` → serve HTTP).
- `config.toml` — `[backend]` + a single-call AND a script `[[tools]]` (RESEARCH "Code Examples" london-tube),
  `[code_mode] enabled=true` + inline DEV `token_secret` + `allow_inline_token_secret_for_dev=true` (CF-4).
- `api.yaml` (or omit — D-03: spec optional; ship a minimal one for the scaffold-discovery story).
- `deploy.toml` + `.pmcp/deploy.toml` (mirror `generate_deploy_toml`, CF-6).

---

## Shared Patterns

### Error redaction (Pitfall 5 — apply to ALL new error types)
**Source:** `crates/pmcp-server-toolkit/src/sql/mod.rs:262-272` (the `Connection` Security doc-comment)
and the test `:417-426` (`test_connection_display_does_not_echo_password`).
**Apply to:** `HttpConnectorError`, `DispatchError`, and the `ExecutionError` surfaced from `HttpCodeExecutor`.
Each MUST name operation/status/backend/field ONLY — never URL, `Authorization`, bearer token, or `app_key`.
Each gets a `test_*_display_does_not_echo_secret`.

### CodeExecutor trait wiring (the SDK seam)
**Source:** `crates/pmcp-server-toolkit/src/code_mode.rs:509-542` (`SqlCodeExecutor: CodeExecutor`) +
the re-export block at `:56-60` (`CodeExecutor`, `ExecutionError`, `ValidationPipeline`, …).
**Apply to:** `HttpCodeExecutor` (impl `HttpExecutor`, wrapped by `JsCodeExecutor` for Code Mode) and the
OAPI-10 generalization. **D-02:** the SAME `HttpCodeExecutor` instance feeds BOTH `ScriptToolHandler`
(raw `PlanExecutor`) AND `code_mode_tools_from_executor` (via `JsCodeExecutor`) — one seam, two surfaces.

### `#[serde(deny_unknown_fields)]` strict config (REF-01 superset)
**Source:** `crates/pmcp-server-toolkit/src/config.rs:8-24` (whole-file invariant) + every section derive.
**Apply to:** `BackendSection`, `AuthConfig`, `HttpConfig`, and the new `ToolDecl` fields. Renames forbidden;
additive `Option<…>` keys allowed (existing SQL configs must keep parsing).

### ToolHandler synthesis (object envelope, no struct-literals)
**Source:** `crates/pmcp-server-toolkit/src/tools.rs:11-28` (invariants) + `build_input_schema` helper.
**Apply to:** both single-call and script tool handlers. `ToolInfo`/`ToolAnnotations` are `#[non_exhaustive]`
— use constructors, never struct-literals. `metadata()` MUST return `Some(ToolInfo)`.

### Binary control flow (Shape A pipeline)
**Source:** `crates/pmcp-sql-server/src/lib.rs` (`run` → `run_serving` → `load_config_and_*` → `dispatch` →
`build_server` → `serve`). **Apply to:** the whole `pmcp-openapi-server` binary — structural copy, only
`--schema`→`--spec` (optional), connector/executor types, and the dispatch arity change.

### Streamable-HTTP only (CF-1) + lazy startup (CF-2)
**Source:** `lib.rs:156-164` (`serve`, `StreamableHttpServerConfig::default()`) + `dispatch.rs:113-115`
(offline-safe construction). **Apply to:** binary, scaffold `main.rs`, example, Lambda. No stdio.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `…/src/http/dispatch` arity `(Arc<dyn HttpConnector>, HttpCodeExecutor)` | service | — | SQL `dispatch` returns a single `Arc<dyn SqlConnector>`; the OpenAPI binary must return BOTH the single-call connector AND the code-mode/script seam from one call. Shape is new (build both from the same `reqwest::Client` + auth) but each half has an analog. Partial-analog only. |

`ScriptToolHandler` (OAPI-02b) has a **design** anchor (`OPENAPI_SCRIPT_TOOLS.md`, Status: Proposed) but
**no implementation to lift** in either tree — it is IMPLEMENTED in Phase 90 over the SDK's existing
`PlanCompiler`/`PlanExecutor`. Treated as role-match (the `ToolHandler` shape is well-precedented) rather
than no-analog, since the handler skeleton mirrors `SynthesizedToolHandler` + the code_mode handlers.

---

## Metadata

**Analog search scope:**
- `crates/pmcp-server-toolkit/src/` (sql/, code_mode.rs, config.rs, tools.rs, builder_ext.rs, auth.rs)
- `crates/pmcp-sql-server/src/` (lib.rs, cli.rs, dispatch.rs, assemble.rs, main.rs)
- `cargo-pmcp/src/commands/new.rs` + `cargo-pmcp/src/templates/sql_server.rs`
- `~/Development/mcp/sdk/pmcp-run/built-in/openapi-api/crates/mcp-openapi-server-core/src/` (http/, auth/, code_mode.rs, config.rs, tools/, schema/)

**Files scanned:** 14 read in full or in targeted ranges; reference `http/mod.rs` (554 lines) and
`auth/mod.rs` + `config.rs:150-259` + `code_mode.rs:85-204` read directly.

**Pattern extraction date:** 2026-05-29
