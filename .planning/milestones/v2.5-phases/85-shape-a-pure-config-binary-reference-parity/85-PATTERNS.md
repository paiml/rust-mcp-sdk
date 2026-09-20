# Phase 85: Shape A Pure-Config Binary + Reference Parity - Pattern Map

**Mapped:** 2026-05-26
**Files analyzed:** 17 (5 toolkit-core mods + 9 new-crate files + root Cargo.toml + 2 vendored fixtures)
**Analogs found:** 15 / 17 (2 novel: backend dispatch + `SqlCodeExecutor` adapter)

> Reading note for the planner: this phase is ~80% wiring of existing toolkit/SDK
> primitives. The genuinely-novel code is (1) backend dispatch (`[database].type`
> → `Arc<dyn SqlConnector>`), (2) the toolkit `SqlCodeExecutor` adapter + a *real*
> `register_code_mode_tools` body, and (3) the `--schema` injection seam. Three
> RESEARCH gaps (REF-01 superset fields, code-mode registration, `${VAR}` expansion)
> are toolkit-core modifications and MUST land before the binary compiles against
> the reference config.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/pmcp-sql-server/Cargo.toml` | config | — | `crates/mcp-tester/Cargo.toml` | role-match |
| `crates/pmcp-sql-server/src/main.rs` | binary entry | request-response | `pmcp-run/.../sql-reference-lambda/src/main.rs` + `crates/mcp-tester/src/main.rs` Cli | role-match |
| `crates/pmcp-sql-server/src/lib.rs` | binary entry (testable `run`) | request-response | `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` (assembly) | role-match |
| `crates/pmcp-sql-server/src/cli.rs` | config (clap Parser) | transform | `crates/mcp-tester/src/main.rs:21-90` (`#[derive(Parser)] Cli`) | role-match |
| `crates/pmcp-sql-server/src/dispatch.rs` | service | transform | `crates/pmcp-server-toolkit/examples/sqlite_minimal.rs` (connector construction) | **NOVEL** (no `type`→connector switch exists) |
| `crates/pmcp-sql-server/src/assemble.rs` | service | request-response | `examples/e01_toolkit_minimal.rs` + `builder_ext.rs` | exact |
| `crates/pmcp-sql-server/examples/chinook_http.rs` | example | request-response | `examples/sqlite_minimal.rs` + `tests/streamable_http_integration.rs:32-50` | role-match |
| `crates/pmcp-sql-server/tests/parity_chinook.rs` | test (integration) | event-driven | `crates/mcp-tester/src/lib.rs:205-255` (`run_scenario`) + `tests/streamable_http_integration.rs:32-88` (spawn+exercise) | exact |
| `crates/pmcp-sql-server/tests/superset_parse.rs` | test (integration) | CRUD | `crates/pmcp-server-toolkit/tests/reference_configs.rs:19-54` | exact |
| `crates/pmcp-sql-server/tests/lazy_startup.rs` | test (integration) | request-response | `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs:34-96` | role-match |
| `crates/pmcp-sql-server/tests/fixtures/chinook.sql` | fixture (DDL) | file-I/O | vendored from `sqlite3 chinook.db .schema` (Open Q1) | vendored |
| `crates/pmcp-sql-server/tests/fixtures/generated.yaml` | fixture (scenarios) | file-I/O | `pmcp-run/.../reference/scenarios/generated.yaml` | vendored (copy) |
| `crates/pmcp-server-toolkit/src/config.rs` (MOD) | model | transform | self (`DatabaseSection`/`ServerSection` lines 234-322) | exact (additive fields) |
| `crates/pmcp-server-toolkit/src/code_mode.rs` (MOD) | service | request-response | `pmcp-run/.../mcp-sql-server-core/src/code_mode.rs:75-204` (`SqlCodeModeHandler`+`SqlCodeModeServer`) | role-match (cross-repo mirror) |
| `crates/pmcp-server-toolkit/src/resources.rs` (use) | model | file-I/O | self (`ResourceConfig` lines 66-90 + `from_configs` line 244) | exact (no change, just consume) |
| `crates/pmcp-server-toolkit/src/builder_ext.rs` (use/MOD) | service | request-response | self (`tools_from_config_with_connector` lines 117-121) | exact |
| `Cargo.toml` (root, MOD) | config | — | self (`[workspace] members` line 541) | exact (one-token insert) |

---

## Pattern Assignments

### `crates/pmcp-sql-server/Cargo.toml` (config)

**Analog:** `crates/mcp-tester/Cargo.toml` (workspace binary crate with `[lib]`+`[[bin]]` split, clap, pmcp path-dep with `streamable-http`).

**Crate shape to copy:**
```toml
[package]
name = "pmcp-sql-server"
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/paiml/rust-mcp-sdk"

[lib]              # mirrors mcp-tester: lib holds testable `run`, bin is thin
name = "pmcp_sql_server"
path = "src/lib.rs"

[[bin]]
name = "pmcp-sql-server"
path = "src/main.rs"
```

**Dependency + feature block (from RESEARCH §Standard Stack, verified versions):**
```toml
[dependencies]
pmcp = { version = "2.8.1", path = "../..", features = ["streamable-http"] }
pmcp-server-toolkit = { version = "0.1.0", path = "../pmcp-server-toolkit", features = ["code-mode", "sqlite"] }
pmcp-toolkit-postgres = { path = "../pmcp-toolkit-postgres", optional = true }
pmcp-toolkit-mysql    = { path = "../pmcp-toolkit-mysql", optional = true }
pmcp-toolkit-athena   = { path = "../pmcp-toolkit-athena", optional = true }
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[features]
default = ["sqlite", "postgres", "mysql", "athena"]   # D-07 all default-on
sqlite   = ["pmcp-server-toolkit/sqlite"]
postgres = ["dep:pmcp-toolkit-postgres"]
mysql    = ["dep:pmcp-toolkit-mysql"]
athena   = ["dep:pmcp-toolkit-athena"]

[dev-dependencies]
mcp-tester = { path = "../mcp-tester" }
tempfile = "3"
proptest = "1"
```
> **Plan-time verify:** connector-crate versions (unpublished Phase 84 crates) from
> their own `Cargo.toml`; `tracing-subscriber` pinned `0.3.20` in mcp-tester.

---

### `crates/pmcp-sql-server/src/cli.rs` (config / clap Parser)

**Analog:** `crates/mcp-tester/src/main.rs:21-90` (`#[derive(Parser)] struct Cli`).

**`#[arg]` style to copy** (note `env =`, `default_value`, `Option<String>` patterns):
```rust
// Source: crates/mcp-tester/src/main.rs:38-89
#[derive(clap::Parser, Debug)]
#[command(name = "pmcp-sql-server", version, about = "Config-driven SQL MCP server")]
pub struct Args {
    /// Path to the server config TOML (server + [database] + [code_mode]).
    #[arg(long)]
    pub config: std::path::PathBuf,

    /// Path to the backend schema file (DDL for SQL) — the code-mode resource.
    #[arg(long)]
    pub schema: std::path::PathBuf,

    /// HTTP bind address (streamable-HTTP). Default loopback ephemeral-safe.
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub http: String,
}
```
> D-disc: flag shape is `--http <addr>` (chosen over `--transport ... --bind ...`).
> `env =` is available (mcp-tester uses `env = "MCP_API_KEY"` etc.) if a flag needs env fallback.

---

### `crates/pmcp-sql-server/src/main.rs` + `src/lib.rs` (binary entry)

**Analog (assembly shape):** `pmcp-run/.../sql-reference-lambda/src/main.rs` (the ~30-line production binary this generalizes) + `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` (the toolkit builder chain).

**Production reference `main.rs` (the shape to generalize — NOTE it uses
`mcp-sql-server-core`'s `SqlConfig`/`SqlMcpServer`, NOT the toolkit):**
```rust
// Source: pmcp-run/built-in/sql-api/reference/sql-reference-lambda/src/main.rs:24-41
#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().json().init();          // Shape A: prefer fmt() reading RUST_LOG (anti-pattern note in RESEARCH §Pattern 2)
    let config = SqlConfig::from_toml_with_env(CONFIG_TOML)?;
    let db_path = config.database.file_path.as_ref().ok_or_else(/*..*/)?;
    let connector = SqliteConnector::new(db_path)?;   // Shape A: ::open(Path) instead, dispatched on type
    let server = SqlMcpServer::new(config, connector).await?;
    run_lambda(server).await                          // Shape A: StreamableHttpServer::start instead
}
```

**Toolkit builder chain to copy into `lib::run` (the actual Shape A target):**
```rust
// Source: crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs:49-56
// — ONE crate-root import line (D-15 witness, review R3): never module-qualify.
use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig, StaticResourceHandler};

let _server = Server::builder()
    .name(&cfg.server.name)
    .version(&cfg.server.version)
    .try_tools_from_config_with_connector(&cfg, connector.clone())?  // [[tools]] + connector (Phase 84)
    .try_code_mode_from_config(&cfg)?                                // validate_code/execute_code (post-gap-fix)
    .resources_arc(Arc::new(schema_resource_handler(&cfg, &schema_ddl)?)) // docs://…/schema
    .build()?;
```
> **`lib::run(args) -> Result` pattern (testability):** `main.rs` is a 3-line
> `#[tokio::main]` shim — parse args, init subscriber, `lib::run(args).await` — so
> `tests/` and `examples/` re-enter the same assembly without `process::exit`.
> Mirrors mcp-tester's `lib.rs`/`main.rs` split.

---

### `crates/pmcp-sql-server/src/dispatch.rs` (service — **NOVEL**)

**No analog exists** — there is no `[database].type` → connector switch anywhere
in the codebase. Closest reference points for the *inputs/outputs*:

**Connector constructor signatures to dispatch to (verified):**
```rust
// SQLite — crates/pmcp-server-toolkit/src/sql/sqlite.rs:72,88 (sync, pure-Rust)
SqliteConnector::open(path: &Path) -> Result<Self, ConnectorError>
SqliteConnector::open_in_memory()  -> Result<Self, ConnectorError>

// Postgres — crates/pmcp-toolkit-postgres/src/lib.rs:117 (async; pool built, I/O deferred)
PostgresConnector::connect(url: &str) -> Result<Self, ConnectorError>     // ⚠ named `connect`, NOT `connect_lazy`

// MySQL — crates/pmcp-toolkit-mysql/src/lib.rs:144 (async; MySqlPool::connect_lazy internally)
MysqlConnector::connect(url: &str) -> Result<Self, ConnectorError>

// Athena — crates/pmcp-toolkit-athena/src/lib.rs:148 (async; builds SDK client, no connect)
AthenaConnector::from_config(region: &str, workgroup: &str) -> Result<Self, ConnectorError>
// or build AthenaConfig { region, workgroup, database, output_location, query_timeout_ms, tables } then chain
```
> **D-09 lazy-startup confirmed:** Postgres/MySQL `connect` build pools without TCP
> I/O; Athena `from_config` only builds an SDK client. SC-1 (lazy `tools/list`) holds
> as long as dispatch does NOT call `schema_text()` for non-SQLite — the schema
> resource/prompt for those comes from the `--schema` file (RESEARCH §Pitfall 4).

**Builder-side consumer signature (the trait object dispatch returns):**
```rust
// Source: crates/pmcp-server-toolkit/src/builder_ext.rs:117-121
fn tools_from_config_with_connector(self, config: &ServerConfig, connector: Arc<dyn SqlConnector>) -> Self;
```

**Dispatch shape (RESEARCH §Pattern 4; keep cog ≤25 — split per-backend arms into helpers):**
```rust
fn dispatch(cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    match cfg.database.backend_type.as_deref() {
        Some("sqlite") => {
            #[cfg(feature = "sqlite")]   { /* SqliteConnector::open(cfg.database.file_path) */ }
            #[cfg(not(feature = "sqlite"))] { Err(DispatchError::feature_missing("sqlite")) }
        }
        Some("postgres") => { /* PostgresConnector::connect(url) under #[cfg(feature="postgres")] */ }
        Some("mysql")    => { /* MysqlConnector::connect(url) */ }
        Some("athena")   => { /* AthenaConnector::from_config(region, workgroup) */ }
        Some(other) => Err(DispatchError::unknown_backend(other)),
        None        => Err(DispatchError::missing_type()),
    }
}
```
> Error UX (D-08): compiled-out feature →
> `"config requires backend 'athena' but this binary was built without the 'athena' feature; rebuild with --features athena"`.
> Error type must NOT echo arbitrary config input (V7, T-84-04-02).

---

### `crates/pmcp-sql-server/src/assemble.rs` (service)

**Analog:** `examples/e01_toolkit_minimal.rs:49-56` (exact builder chain) +
`resources.rs` `ResourceConfig`/`StaticResourceHandler` (the schema-resource seam).

**Schema-resource construction (RESEARCH §Pattern 3 / D-05):**
```rust
// Source: crates/pmcp-server-toolkit/src/resources.rs:66-90 (ResourceConfig), :244 (from_configs)
let res_cfg = pmcp_server_toolkit::resources::ResourceConfig {
    uri: "docs://chinook/schema".into(),
    name: Some("Chinook Database Schema".into()),
    description: None,
    mime_type: "text/markdown".into(),       // NOTE: ResourceConfig.mime_type is `String` (line 79), NOT Option
    content: Some(schema_ddl),               // verbatim (D-05) OR header/footer-wrapped — planner's discretion
    content_file: None,
};
let handler = StaticResourceHandler::from_configs(&[res_cfg])?;
```
> ⚠ RESEARCH §Pattern 3 example showed `mime_type: Some(..)`; the actual field is
> `pub mime_type: String` (resources.rs:79). Use a bare `String`.

**Prompt seam (D-05):** the code-mode prompt name is
`pmcp_server_toolkit::prompts::CODE_MODE_PROMPT_NAME = "start_code_mode"`
(prompts.rs:54). `assemble_code_mode_prompt(connector, config)` reads
`connector.schema_text()` (code_mode.rs:315+) — for Phase 85 the `--schema` text
must REPLACE live `schema_text()`. Add an additive
`assemble_code_mode_prompt_with_schema(schema_text, dialect, config) -> String`
(RESEARCH §Pattern 3 option A, recommended).

---

### `crates/pmcp-sql-server/tests/parity_chinook.rs` (test — REF-02/SC-3/SC-4)

**Analog A (mcp-tester library replay):** `crates/mcp-tester/src/lib.rs:205-255`
(`run_scenario_with_transport`) — the exact load-scenario → `ServerTester::new` →
`ScenarioExecutor::new` → `execute` → check result flow, used as a **library** (D-10).

**Analog B (spawn server on ephemeral port + read bound addr):**
`tests/streamable_http_integration.rs:32-50` — the `127.0.0.1:0` bind + `start()`
returns `(addr, handle)` pattern that makes the harness `--test-threads=1`-safe.

**Spawn+poll+replay shape (RESEARCH §Code Examples; A4: confirm `ScenarioResult` accessor):**
```rust
// Server side — tests/streamable_http_integration.rs:47-50 pattern
let server = build_server(&cfg, connector, schema_ddl).await?;   // reuse lib::build_server
let http = StreamableHttpServer::with_config(
    "127.0.0.1:0".parse()?, Arc::new(Mutex::new(server)), StreamableHttpServerConfig::default());
let (addr, handle) = http.start().await?;                        // read the REAL bound addr

// Replay side — mcp-tester lib (crates/mcp-tester/src/lib.rs:223-245)
let mut tester = ServerTester::new(
    &format!("http://{addr}"), Duration::from_secs(30),
    /*insecure*/ false, /*api_key*/ None, /*transport*/ Some("http"), /*middleware*/ None)?;
let scenario = TestScenario::from_file("tests/fixtures/generated.yaml")?;
let mut exec = ScenarioExecutor::new(&mut tester, /*detailed*/ true);
let result = exec.execute(scenario).await?;
assert!(result.success, "31 parity scenarios must pass: {result:?}");  // ⚠ `.success` per lib.rs:248 — verify vs all_passed()
handle.abort();
```
> ⚠ RESEARCH assumed `result.all_passed()` (A4); the real field used by the library
> is `result.success` (lib.rs:248). Confirm `ScenarioResult` shape at plan time.
> **Env setup:** harness MUST `std::env::set_var("CODE_MODE_SECRET", "<≥16 bytes>")`
> before building the server (RESEARCH §Runtime State / Pitfall 3).

---

### `crates/pmcp-sql-server/tests/superset_parse.rs` (test — REF-01/SC-2)

**Analog (near-exact, copy-and-adapt):**
`crates/pmcp-server-toolkit/tests/reference_configs.rs:19-54`.

**Test body to copy (one per config; add the SQLite reference as the 4th):**
```rust
// Source: crates/pmcp-server-toolkit/tests/reference_configs.rs:20-32
#[test]
fn reference_sqlite_config_parses_and_validates() {
    let toml = include_str!("fixtures/reference-config.toml");          // the Chinook reference (currently FAILS — Gap #1)
    let cfg = ServerConfig::from_toml_strict_validated(toml)
        .expect("reference config must parse + validate — REF-01 superset invariant");
    assert_eq!(cfg.database.backend_type.as_deref(), Some("sqlite"));
    assert!(cfg.code_mode.is_some());
}
```
> The 3 Athena/MySQL fixtures already parse (reference_configs.rs proves it). SC-2's
> NET-NEW assertion is the SQLite reference config — which fails today on `file_path`,
> `[server] is_reference`, `[shared_policy_store]` (Gap #1). This test is the
> regression gate for the `config.rs` additive-field task.

---

### `crates/pmcp-sql-server/tests/lazy_startup.rs` (test — SC-1)

**Analog:** `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs:34-96` —
builds a `Server` from a fixture config and asserts `Server::get_tool(name)` is
`Some` after `try_tools_from_config`. Adapt: dispatch a postgres/mysql/athena config
(no creds), build the server, assert `tools/list` succeeds WITHOUT a live backend.

**Construction-surface shape to copy:**
```rust
// Source: crates/pmcp-server-toolkit/tests/backend_core_smoke.rs:57-89
let mut builder = Server::builder().name(&cfg.server.name).version(&cfg.server.version)
    .try_tools_from_config(&cfg).expect("synth");
// … .build() must succeed; Server::get_tool("<curated tool>") must be Some
```

---

### `crates/pmcp-server-toolkit/src/config.rs` (MOD — REF-01 superset, Gap #1)

**Analog:** the file itself — `DatabaseSection` (lines 290-322) already carries
Athena-specific optional fields under `deny_unknown_fields` as the documented
REF-01 pattern. Add the SQLite/reference fields the **same way** (additive
`Option<…>` / `#[serde(default)]`):

| Add | To | Shape |
|-----|-----|-------|
| `file_path: Option<String>` | `DatabaseSection` (after line 318 `url`) | `#[serde(default)] pub file_path: Option<String>` |
| `is_reference: bool` | `ServerSection` (after line 250 `version`) | `#[serde(default)] pub is_reference: bool` |
| `shared_policy_store: Option<SharedPolicyStoreSection>` | `ServerConfig` (after line 127 `resources`) | new `#[serde(deny_unknown_fields)]` struct: `creates_shared_store`, `export_to_ssm`, `ssm_path` |

**Pattern witness (the existing additive field this mirrors):**
```rust
// Source: crates/pmcp-server-toolkit/src/config.rs:311-318
/// Connection URL for Postgres / MySQL backends. … Optional/unused for
/// Athena … and SQLite (uses `database` for the file path or `:memory:`).
#[serde(default)]
pub url: Option<String>,
```
> **Anti-pattern (RESEARCH §Pitfall 1 + config.rs:24):** do NOT loosen
> `deny_unknown_fields`. "Always ADD the missing field." Exact failing field today:
> `unknown field 'file_path', expected one of type/database/output_location/workgroup/query_timeout_ms/tables/url/pool`.
>
> **`${VAR}` expansion (Gap #3):** add an env-expansion pass over the raw TOML
> string before `from_toml` (covers `token_secret` AND Athena `output_location`'s
> `${AWS_ACCOUNT_ID}`), coexisting with the existing `env:VAR` convention in
> `resolve_token_secret` (code_mode.rs:290). See Shared Pattern "Secret/Env Resolution".

---

### `crates/pmcp-server-toolkit/src/code_mode.rs` (MOD — real registration + executor, Gap #2)

**Analog (cross-repo mirror):**
`pmcp-run/built-in/sql-api/crates/mcp-sql-server-core/src/code_mode.rs:75-204` —
the production `SqlCodeModeHandler` (`CodeExecutor` impl) + `SqlCodeModeServer`
(`#[derive(CodeMode)]`). The toolkit must grow an equivalent.

**Current toolkit state (shape-preserving stub — registers ZERO tools):**
```rust
// Source: crates/pmcp-server-toolkit/src/code_mode.rs:176-192
pub fn register_code_mode_tools(builder: pmcp::ServerBuilder, config: &ServerConfig)
    -> Result<pmcp::ServerBuilder> {
    if config.code_mode.is_none() { return Ok(builder); }
    let _pipeline = validation_pipeline_from_config(config)?;   // R9 gate only
    // Plan 08 … will register the validate/execute tools here … shape-preserving for now.
    Ok(builder)
}
```

**`CodeExecutor::execute` body to mirror (production — defense-in-depth re-validate then dispatch by query type):**
```rust
// Source: pmcp-run/.../mcp-sql-server-core/src/code_mode.rs:136-176
async fn execute(&self, code: &str, _vars: Option<&serde_json::Value>)
    -> std::result::Result<serde_json::Value, ExecutionError> {
    let validator = SqlValidator::new();
    let metadata = validator.validate(code, &self.config.code_mode)?;   // re-validate (token-replay guard)
    let result = match metadata.query_type {
        QueryType::Select => self.connector.execute_query(code, &[]).await?,
        QueryType::Insert | QueryType::Update | QueryType::Delete | QueryType::Truncate
            => self.connector.execute_statement(code, &[]).await?,
        QueryType::Create | QueryType::Alter | QueryType::Drop
            => self.connector.execute_statement(code, &[]).await?,
        QueryType::Other => return Err(ExecutionError::BackendError("Unsupported query type".into())),
    };
    Ok(serde_json::json!({ "columns": result.columns, "rows": result.rows, "rows_affected": result.rows_affected }))
}
```
> **Toolkit adaptation:** production's connector is `Arc<dyn DatabaseConnector>` with
> separate `execute_query`/`execute_statement`; the toolkit's `SqlConnector`
> (sql/mod.rs:101-141) has a SINGLE `execute(sql, params)`. The toolkit
> `SqlCodeExecutor` bridges this single method to SELECT/DML/DDL (RESEARCH §State of
> the Art row 2). Struct shape mirrors `SqlCodeModeHandler` (code_mode.rs:75-79):
> `{ connector: Arc<dyn SqlConnector>, config, server_name }`.

**Registration via derive (production, RESEARCH A2 recommended path):**
```rust
// Source: pmcp-run/.../mcp-sql-server-core/src/code_mode.rs:193-204
#[derive(pmcp_code_mode_derive::CodeMode)]
#[code_mode(context_from = "get_context", language = "sql")]
pub struct SqlCodeModeServer {
    pub code_mode_config: CodeModeConfig,      // ← build_cm_config (code_mode.rs:206) already produces this
    pub token_secret: TokenSecret,             // ← resolve_token_secret (code_mode.rs:284) already produces SecretValue
    pub policy_evaluator: Arc<dyn PolicyEvaluator>,
    pub code_executor: Arc<SqlCodeModeHandler>,// ← the toolkit SqlCodeExecutor
    policy_hash: String,
}
```
> The toolkit ALREADY has the config→`CodeModeConfig` mapping (`build_cm_config`,
> code_mode.rs:206-248) and the secret resolution (`resolve_token_secret`,
> code_mode.rs:284-309). The new work is: the `SqlCodeExecutor` adapter + wiring a
> `SqlCodeModeServer` inside the real `register_code_mode_tools` so the binary's
> `code_mode_from_config` call (builder_ext) registers `validate_code`/`execute_code`.
> Decide derive-macro reuse vs hand-writing two `CodeModeToolBuilder` handlers (A2).

**Registration unit-test analog (existing pipeline tests to extend):**
`crates/pmcp-server-toolkit/tests/code_mode_wiring.rs:44-184` already tests
`validation_pipeline_from_config` policy (writes/INSERT/SELECT) + R9 inline-secret
rejection. ADD: assert `register_code_mode_tools` registers `validate_code`/`execute_code`
(currently it registers none), and that DELETE/DDL are rejected by policy.

---

### `Cargo.toml` (root, MOD)

**Analog:** the file itself — `[workspace] members` line 541.

**Change:** insert `"crates/pmcp-sql-server"` into the `members` array (sits with the
other `crates/pmcp-toolkit-*` entries). Single-token edit; no `exclude` change needed.

---

## Shared Patterns

### Secret / Env Resolution (`${VAR}` + `env:VAR`)
**Source:** `crates/pmcp-server-toolkit/src/code_mode.rs:284-309` (`resolve_token_secret`).
**Apply to:** `config.rs` (env-expansion pass), the binary startup, parity harness.
```rust
// Current: env:VAR only — code_mode.rs:290-295
if let Some(var) = raw.strip_prefix("env:") {
    return std::env::var(var).map(|s| SecretValue::new(s.into_bytes())) /* … */;
}
// Gap #3: reference configs use `${CODE_MODE_SECRET}` → add ${VAR} expansion (additive, coexists).
```
HMAC secret min length is `HmacTokenGenerator::MIN_SECRET_LEN` (16 bytes); the
parity harness sets `CODE_MODE_SECRET` to ≥16 bytes before spawn (V6).

### Streamable-HTTP Serving (D-12)
**Source:** `src/server/streamable_http_server.rs:354-387` (`with_config` + `start`).
**Apply to:** binary assembly + `examples/chinook_http.rs` + `tests/parity_chinook.rs`.
```rust
// with_config(addr, Arc<Mutex<Server>>, StreamableHttpServerConfig) ; start() -> (SocketAddr, JoinHandle)
let http = StreamableHttpServer::with_config(addr, Arc::new(Mutex::new(server)), StreamableHttpServerConfig::default());
let (bound_addr, handle) = http.start().await?;   // start() applies CORS + DnsRebinding + SecurityHeaders layers
```
`StreamableHttpServerConfig::default()` is stateful (session id gen, in-mem event store,
`AllowedOrigins::localhost()`); `::stateless()` (line 249) is the Lambda-style alt
(A3 — switch if scenarios assume stateless sessions). Re-export path is the full
`pmcp::server::streamable_http_server::StreamableHttpServer` (NO short re-export found —
RESEARCH §Pattern 2 caveat confirmed). Requires `pmcp` feature `streamable-http`.

### Single Crate-Root Import (D-15 witness, review R3)
**Source:** `examples/e01_toolkit_minimal.rs:19-21`.
**Apply to:** every new file importing the toolkit.
```rust
use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig, StaticResourceHandler};
```
NEVER module-qualify (`pmcp_server_toolkit::auth::*`). If a needed symbol isn't
crate-root re-exported, fix the re-export in toolkit `lib.rs` — don't qualify the import.

### Fixture-Parse Test Convention
**Source:** `crates/pmcp-server-toolkit/tests/reference_configs.rs` — `include_str!("fixtures/<x>.toml")`
→ `ServerConfig::from_toml_strict_validated(toml).expect(...)`. Fuzz seeds live in
`fuzz/corpus/pmcp_server_toolkit_config_parser/` and the seed-parse smoke test
(reference_configs.rs:124-157) pins well-formed seeds — extend the corpus if new
config fields land (ALWAYS fuzz, CLAUDE.md).

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/pmcp-sql-server/src/dispatch.rs` | service | transform | No `[database].type` → connector switch exists anywhere; the four connector constructors are the only reference points. NOVEL seam (D-08). |
| `SqlCodeExecutor` adapter in `code_mode.rs` | service | request-response | No `Arc<dyn SqlConnector>` → `pmcp_code_mode::CodeExecutor` adapter in the toolkit. The pattern to mirror lives in a DIFFERENT repo (`pmcp-run`'s `SqlCodeModeHandler`) over a DIFFERENT trait (`DatabaseConnector`, 2-method) — must be re-derived for `SqlConnector` (1-method `execute`). |

---

## Metadata

**Analog search scope:**
`crates/pmcp-sql-server/` (target, empty), `crates/mcp-tester/` (clap binary + scenario lib),
`crates/pmcp-server-toolkit/{src,tests,examples}/`, `crates/pmcp-toolkit-{postgres,mysql,athena}/src/`,
`src/server/streamable_http_server.rs`, `tests/streamable_http_integration.rs`,
`pmcp-run/built-in/sql-api/reference/` + `crates/mcp-sql-server-core/` (cross-repo parity target).
**Files scanned:** 14 read in full/targeted + 6 grep-located.
**Pattern extraction date:** 2026-05-26
**Re-verify at plan time:** connector-crate versions; `ScenarioResult` accessor (`.success` vs `all_passed()`); `StreamableHttpServer` re-export path; whether `register_code_mode_tools` reuse of the derive macro composes with the config-driven signature (A2).
