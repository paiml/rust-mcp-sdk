# Phase 85: Shape A Pure-Config Binary + Reference Parity - Research

**Researched:** 2026-05-26
**Domain:** Rust workspace crate scaffolding · config-driven MCP server assembly · streamable-HTTP serving · code-mode tool registration · mcp-tester scenario replay
**Confidence:** HIGH (all integration seams verified by reading source + an empirical parse probe against all four reference configs)

## Summary

Phase 85 builds `crates/pmcp-sql-server/` — a thin CLI binary that loads a `config.toml` + a `--schema` DDL file, dispatches `[database].type` to one of four Phase 84 connectors, and serves the full MCP surface over streamable HTTP, then proves parity against the SQLite Chinook reference by replaying `reference/scenarios/generated.yaml` through mcp-tester. The toolkit core (Phase 83) and connectors (Phase 84) supply almost all the domain logic; the binary is glue plus three net-new pieces the toolkit does NOT yet provide.

The research surfaced **three load-bearing gaps that block the success criteria and MUST be planned as explicit tasks**, all verified against source:

1. **REF-01 is not actually satisfied today.** An empirical parse probe (`ServerConfig::from_toml` against all four reference configs) shows the three Athena/MySQL configs parse cleanly, but **the SQLite Chinook reference config FAILS to parse** — `[database] file_path` is an unknown field under `#[serde(deny_unknown_fields)]`, and the file also carries `[server] is_reference` and a whole `[shared_policy_store]` section the toolkit doesn't model. Phase 85 must add these as additive superset fields. `[VERIFIED: cargo run parse probe, 2026-05-26]`
2. **Code-mode tools are not registered by the toolkit.** `code_mode::register_code_mode_tools` (Phase 83) is **shape-preserving** — it builds + validates a `ValidationPipeline` (so R9/secret errors fire) but registers ZERO tools. Plan 08 was deferred. There is no toolkit equivalent of the production `SqlCodeModeHandler` (an `Arc<dyn SqlConnector>` → `CodeExecutor` adapter) nor a config-driven `register_code_mode_tools` that actually wires `validate_code`/`execute_code` onto the builder. SC-3/SC-4 require these tools to exist and enforce policy. `[VERIFIED: code_mode.rs:176-192 + builder_ext.rs:281-296]`
3. **`${VAR}` token-secret syntax is unsupported.** All four reference configs use `token_secret = "${CODE_MODE_SECRET}"`. The toolkit's `resolve_token_secret` only accepts `env:VAR` and otherwise rejects inline literals (R9). The parity harness must either teach the toolkit `${VAR}` expansion (additive) or pre-resolve, but the binary must not crash at startup on the reference config. `[VERIFIED: code_mode.rs:284-309 + grep of all four configs]`

**Primary recommendation:** Plan three foundation tasks BEFORE the binary — (a) extend `ServerConfig` with `file_path` / `is_reference` / `[shared_policy_store]` (REF-01 superset) + `${VAR}` env expansion, (b) ship a toolkit `SqlCodeExecutor` adapter (`Arc<dyn SqlConnector>` → `pmcp_code_mode::CodeExecutor`) and a real `register_code_mode_tools` that registers `validate_code`/`execute_code` via `CodeModeToolBuilder`, (c) re-generate + commit a Chinook DDL fixture. Then the binary is: parse config → dispatch connector → `Server::builder().tools_from_config_with_connector(...).code_mode_from_config(...).resources_arc(schema resource).prompt_arc(start_code_mode)` → `StreamableHttpServer::with_config(addr, Arc::new(Mutex::new(server)), cfg).start()`. The parity harness spawns the binary, polls readiness, and drives `mcp_tester::{ServerTester, ScenarioExecutor, TestScenario}` as a **library** dependency (no subprocess for mcp-tester).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| CLI arg parsing (`--config`/`--schema`/`--http`) | Binary (`pmcp-sql-server`) | — | clap `Parser` derive; matches mcp-tester/cargo-pmcp `clap 4 derive,env` |
| Config parse + validate | Toolkit (`ServerConfig`) | Binary | `from_toml_strict_validated` already owns this; binary calls it |
| Backend dispatch `[database].type` → connector | Binary | Connector crates | the one net-new dispatch seam; produces `Arc<dyn SqlConnector>` |
| Tool synthesis from `[[tools]]` | Toolkit (`tools::synthesize_from_config_with_connector`) | — | TKIT-07; binary just calls `tools_from_config_with_connector` |
| Code-mode validate/execute tools | Toolkit (NEW: real `register_code_mode_tools` + `SqlCodeExecutor`) | pmcp-code-mode | SC-3/SC-4 gap; must be built in the toolkit, not the binary |
| Schema resource (`docs://.../schema`) | Toolkit (`StaticResourceHandler`) | Binary (injects `--schema` text) | binary builds a `ResourceConfig` from the file, hands it to the handler |
| Code-mode prompt (`start_code_mode`) | Toolkit (`StaticPromptHandler` + `assemble_code_mode_prompt`) | Binary | binary feeds `--schema` text into the prompt body |
| Streamable-HTTP serving | SDK (`StreamableHttpServer`, `streamable-http` feature) | Binary | reuse Phase 56 adapter; binary binds addr + `.start()` |
| Parity replay | Test (mcp-tester lib) | — | integration test under `tests/`, spawns binary + replays |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp-server-toolkit` | 0.1.0 (path) | `ServerConfig`, `ServerBuilderExt`, tool synth, code-mode wiring, static resources/prompts | The crate Phase 85 is the thin CLI over `[VERIFIED: Cargo.toml]` |
| `pmcp` | 2.8.1 (path) | `Server`, `ServerBuilder`, `StreamableHttpServer` (feature `streamable-http`) | SDK; HTTP transport lives here `[VERIFIED: streamable_http_server.rs]` |
| `pmcp-code-mode` | 0.5.1 (path) | `CodeExecutor`, `CodeModeToolBuilder`, `ValidationPipeline`, `CodeModeHandler` | source of the validate/execute tool definitions `[VERIFIED: handler.rs]` |
| `pmcp-toolkit-postgres` | (Phase 84) | Postgres `SqlConnector` (tokio-postgres, `connect_lazy`) | D-07 backend, feature `postgres` `[CITED: CLAUDE.md publish order]` |
| `pmcp-toolkit-mysql` | (Phase 84) | MySQL `SqlConnector` (sqlx, `connect_lazy`) | D-07 backend, feature `mysql` |
| `pmcp-toolkit-athena` | (Phase 84) | Athena `SqlConnector` (aws-sdk-athena) | D-07 backend, feature `athena` |
| `clap` | 4 (`derive`, `env`) | CLI arg parsing | workspace convention (mcp-tester + cargo-pmcp) `[VERIFIED: Cargo.toml]` |
| `tokio` | 1 (`macros`, `rt-multi-thread` or `rt`) | async runtime; HTTP server needs an executor | `StreamableHttpServer::start` spawns a task |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | structured logging (binary `main` inits subscriber) | matches reference main.rs |

### Supporting (dev / test)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `mcp-tester` (lib `mcp_tester`) | (workspace) | `ServerTester`, `ScenarioExecutor`, `TestScenario` | parity integration test (D-10) `[VERIFIED: lib.rs exports]` |
| `proptest` | 1.7 | property tests (dispatch, `${VAR}` expansion) | ALWAYS matrix |
| `tempfile` | 3 | temp DB / schema fixtures in tests | spawn-and-replay harness |
| SQLite Chinook DDL fixture | committed `.sql` | `--schema` input + parity (D-06) | regenerate via `sqlite3 chinook.db .schema` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| mcp-tester as a **library** dep in tests | spawn `mcp-tester scenario <url> <file>` subprocess | Library is cleaner (typed `ScenarioResult`, no PATH dependency, single `--test-threads=1` process). Subprocess avoids a dev-dep cycle but is brittle. **Recommend library.** `[VERIFIED: main.rs:1014-1018 shows the lib path the CLI itself uses]` |
| `Arc<dyn SqlConnector>` dispatch | enum `Backend { Sqlite(..), Postgres(..) }` | `builder_ext` already takes `Arc<dyn SqlConnector>`, so trait-object is the path of least resistance — no monomorphization needed `[VERIFIED: builder_ext.rs:117-121]` |
| `StreamableHttpServer` | hand-rolled axum router via `pmcp::axum::router()` | `StreamableHttpServer::with_config(...).start()` is one call and applies the security layer stack (CORS/DNS-rebind/headers). **Recommend it.** `[VERIFIED: streamable_http_server.rs:347-388]` |

**Installation (new crate `crates/pmcp-sql-server/Cargo.toml`):**
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
tracing-subscriber = "0.3"

[features]
default = ["sqlite", "postgres", "mysql", "athena"]   # D-07 all default-on
sqlite   = ["pmcp-server-toolkit/sqlite"]
postgres = ["dep:pmcp-toolkit-postgres"]
mysql    = ["dep:pmcp-toolkit-mysql"]
athena   = ["dep:pmcp-toolkit-athena"]

[dev-dependencies]
mcp-tester = { path = "../mcp-tester" }
tempfile = "3"
proptest = "1.7"
```

**Version verification:** path deps — versions tracked by workspace. `pmcp = "2.8.1"`, `pmcp-server-toolkit = "0.1.0"`, `pmcp-code-mode = "0.5.1"` confirmed from `crates/pmcp-server-toolkit/Cargo.toml` `[VERIFIED: 2026-05-26]`. Confirm the connector-crate versions from their own `Cargo.toml` during planning (they are unpublished Phase 84 crates).

## Architecture Patterns

### System Architecture Diagram

```
                   ┌────────────────────────────────────────────────────────────┐
  CLI args  ─────► │  pmcp-sql-server (binary main)                              │
  --config         │                                                            │
  --schema         │  1. clap parse → { config_path, schema_path, http_addr }   │
  --http           │  2. read config_path → ServerConfig::from_toml_strict_…    │ ◄── REF-01 superset
                   │  3. read schema_path → schema_ddl: String                  │     gap (file_path,
                   │  4. dispatch(config.database.backend_type) ────────────┐   │     is_reference,
                   │         "sqlite"  → SqliteConnector::open(file_path)    │   │     shared_policy_store)
                   │         "postgres"→ PostgresConnector::connect_lazy(url)│   │
                   │         "mysql"   → MysqlConnector::connect_lazy(url)   │   │
                   │         "athena"  → AthenaConnector::from_config(...)   │   │
                   │         (compiled-out feature → clear error)           │   │
                   │                         │ Arc<dyn SqlConnector>         │   │
                   │                         ▼                               │   │
                   │  5. Server::builder()                                   │   │
                   │       .name/.version(from config.server)                │   │
                   │       .tools_from_config_with_connector(&cfg, conn) ────┼───┼─► [[tools]] → tools/call
                   │       .code_mode_from_config(&cfg)  ← NEW: real reg ────┼───┼─► validate_code/execute_code
                   │       .resources_arc(StaticResourceHandler w/ schema) ──┼───┼─► docs://…/schema resource
                   │       .prompt_arc("start_code_mode", prompt w/ schema) ─┼───┼─► get_prompt
                   │       .build()  → pmcp::Server                          │   │
                   │                         │                               │   │
                   │                         ▼                               │   │
                   │  6. StreamableHttpServer::with_config(                  │   │
                   │        addr, Arc::new(Mutex::new(server)), cfg)         │   │
                   │       .start().await → (SocketAddr, JoinHandle)         │   │
                   └───────────────────────────────┬────────────────────────┘   │
                                                   │ HTTP :PORT (JSON-RPC + SSE) │
                   ┌───────────────────────────────▼────────────────────────────┘
  PARITY TEST ───► │  tests/parity_chinook.rs                                   │
  (cargo test)     │  spawn binary (or in-proc) → poll readiness →             │
                   │  ServerTester::new(url, force_transport="http")           │
                   │  → ScenarioExecutor::execute(                             │
                   │       TestScenario::from_file("generated.yaml"))          │
                   │  → assert all 31 steps pass (success/exists/contains/     │
                   │     failure)                                              │
                   └────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```
crates/pmcp-sql-server/
├── Cargo.toml             # feature-gated connector deps (above)
├── src/
│   ├── main.rs            # clap CLI + tokio main → calls lib::run
│   ├── lib.rs             # run(args) -> Result; testable entry (no process::exit)
│   ├── cli.rs             # #[derive(Parser)] Args { config, schema, http }
│   ├── dispatch.rs        # backend_type → Arc<dyn SqlConnector> (+ compiled-out error)
│   └── assemble.rs        # ServerConfig + connector + schema → built pmcp::Server
├── examples/
│   └── chinook_http.rs    # ALWAYS: runnable example serving chinook over HTTP
└── tests/
    ├── parity_chinook.rs  # D-10 spawn+replay generated.yaml (SC-3/SC-4)
    ├── superset_parse.rs  # SC-2 all four configs parse (REF-01)
    └── lazy_startup.rs    # SC-1 non-SQLite builds connector + tools/list, no creds
```

### Pattern 1: Build the `pmcp::Server` from config + connector + schema
**What:** Compose the toolkit builder extensions into one `Server`.
**When to use:** the binary's assembly step (and the parity test reuses it).
**Example (verified shape — combines `e01_toolkit_minimal.rs` + `sqlite_minimal.rs`):**
```rust
// Source: crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs (assembly)
//       + builder_ext.rs (tools_from_config_with_connector signature)
use std::sync::Arc;
use pmcp::Server;
use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig, StaticResourceHandler};
use pmcp_server_toolkit::sql::SqlConnector;

async fn build_server(
    cfg: &ServerConfig,
    connector: Arc<dyn SqlConnector>,
    schema_ddl: String,           // from --schema
) -> Result<Server, Box<dyn std::error::Error>> {
    // NOTE: code_mode_from_config TODAY is shape-preserving — see Gap #2.
    // After the Phase 85 toolkit task, this call registers validate_code/execute_code.
    let server = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version)
        .try_tools_from_config_with_connector(cfg, connector.clone())?   // [[tools]]
        .try_code_mode_from_config(cfg)?                                  // validate/execute (post-gap-fix)
        .resources_arc(Arc::new(schema_resource_handler(cfg, &schema_ddl)?)) // docs://…/schema
        // prompt_arc("start_code_mode", …) — see Pattern 3
        .build()?;
    Ok(server)
}
```

### Pattern 2: Serve over streamable HTTP
**What:** Bind an address and serve the built `Server`.
**Example (verified API):**
```rust
// Source: src/server/streamable_http_server.rs:347-388
use std::sync::Arc;
use tokio::sync::Mutex;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};

let addr: std::net::SocketAddr = http_addr.parse()?;       // default e.g. 127.0.0.1:8080
let shared = Arc::new(Mutex::new(server));
let http = StreamableHttpServer::with_config(addr, shared, StreamableHttpServerConfig::default());
let (bound_addr, handle) = http.start().await?;            // returns the actual bound addr
// bound_addr is the URL base the parity harness points mcp-tester at.
```
The exact public path may be `pmcp::server::streamable_http_server::StreamableHttpServer` or a re-export; confirm the re-export path during planning (`grep "pub use.*StreamableHttpServer" src/`). Requires `pmcp` feature `streamable-http`.

### Pattern 3: Schema → resource + code-mode prompt (the `--schema` seam, D-05)
**What:** The `--schema` DDL text feeds BOTH the `docs://…/schema` resource AND the `start_code_mode` prompt body.
**Key constraint discovered:** `assemble_code_mode_prompt(connector, config)` reads `connector.schema_text()` — it does NOT take a schema string param. For Phase 85 the `--schema` file content must REPLACE live `schema_text()`. Two viable seams:
- **(A) New overload (recommended):** add `assemble_code_mode_prompt_with_schema(schema_text: &str, dialect: Dialect, config) -> String` to the toolkit (pure string assembly, no connector call). Additive, keeps the connector-based variant.
- **(B) Wrapper connector:** wrap the real connector in a decorator whose `schema_text()` returns the file content. Heavier; not recommended.

For the resource: build a `ResourceConfig { uri: "docs://chinook/schema", content: Some(schema_ddl_or_wrapped), mime_type: "text/markdown", .. }` and pass to `StaticResourceHandler::from_configs`. D-05 allows verbatim OR header/footer-wrapped — planner's discretion.
```rust
// Source: crates/pmcp-server-toolkit/src/resources.rs:151,244,294
//       + prompts.rs:54 CODE_MODE_PROMPT_NAME = "start_code_mode"
let res_cfg = pmcp_server_toolkit::resources::ResourceConfig {
    uri: "docs://chinook/schema".into(),
    name: Some("Chinook Database Schema".into()),
    description: None,
    mime_type: Some("text/markdown".into()),
    content: Some(schema_ddl),       // verbatim (D-05); or wrap with header/footer
    content_file: None,
};
let res_handler = StaticResourceHandler::from_configs(&[res_cfg])?;
```

### Pattern 4: Backend dispatch (Q4 answered)
**What:** `[database].type` string → `Arc<dyn SqlConnector>`; the toolkit builder takes the trait object, so NO enum/monomorphization needed.
```rust
// builder_ext takes Arc<dyn SqlConnector> (builder_ext.rs:117-121) — dispatch returns that.
fn dispatch(cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    let path_or_url = /* cfg.database.file_path / cfg.database.url */;
    match cfg.database.backend_type.as_deref() {
        Some("sqlite") => {
            #[cfg(feature = "sqlite")]
            { Ok(Arc::new(SqliteConnector::open(Path::new(file_path))?)) }
            #[cfg(not(feature = "sqlite"))]
            { Err(DispatchError::feature_missing("sqlite")) }
        }
        Some("postgres") => { #[cfg(feature="postgres")] {/*connect_lazy*/} #[cfg(not)]{err} }
        Some("mysql")    => { /* same shape */ }
        Some("athena")   => { /* AthenaConnector::from_config */ }
        Some(other) => Err(DispatchError::unknown_backend(other)),
        None => Err(DispatchError::missing_type()),
    }
}
```
Error UX (D-08): a compiled-out backend yields e.g. `"config requires backend 'athena' but this binary was built without the 'athena' feature; rebuild with --features athena"`.

### Anti-Patterns to Avoid
- **Re-implementing HTTP serving with raw hyper/axum.** Use `StreamableHttpServer` — it ships the CORS/DNS-rebinding/security-headers layer stack. `[VERIFIED: streamable_http_server.rs:364-378]`
- **Spawning `mcp-tester` as a subprocess in tests.** Use the `mcp_tester` library types directly; it's what the CLI itself does internally.
- **Loosening `#[serde(deny_unknown_fields)]` to make the reference config parse.** The toolkit explicitly forbids this (`config.rs:24` "Always ADD the missing field"). REF-01 requires *additive* fields.
- **Putting code-mode SQL execution logic in the binary.** It belongs in a toolkit `SqlCodeExecutor` (mirrors production `SqlCodeModeHandler`) so Shape C/B reuse it.
- **Using `tracing_subscriber().json()`** like the Lambda reference — fine, but for a local HTTP binary a plain `fmt()` subscriber reading `RUST_LOG` is friendlier.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| MCP-over-HTTP transport | custom axum router + SSE | `pmcp::StreamableHttpServer` (`streamable-http` feature) | security layer stack + protocol correctness `[VERIFIED]` |
| Tool synthesis from `[[tools]]` | per-tool handlers | `tools::synthesize_from_config_with_connector` | TKIT-07 already does it `[VERIFIED]` |
| HMAC tokens / validation pipeline | own crypto | `pmcp_code_mode::{ValidationPipeline, HmacTokenGenerator, CodeModeToolBuilder}` | re-exported via toolkit `code_mode::*` `[VERIFIED]` |
| Scenario YAML parse + assertions | custom YAML walker | `mcp_tester::{TestScenario, ScenarioExecutor}` | exact schema match (below) `[VERIFIED]` |
| Static resource/prompt serving | own `ResourceHandler` | `StaticResourceHandler` / `StaticPromptHandler` | TKIT-04/05 `[VERIFIED]` |
| CLI parsing | manual `std::env::args` | `clap` derive | workspace convention |

**Key insight:** The binary is ~80% wiring of existing toolkit/SDK primitives. The only genuinely new domain logic is (1) backend dispatch, (2) the `SqlCodeExecutor` adapter + real code-mode registration the toolkit deferred in Phase 83, and (3) the `--schema` injection seam.

## Runtime State Inventory

> Phase 85 is a NEW crate (greenfield within the workspace) — no rename/refactor of existing runtime state. The only "state" considerations:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | Chinook SQLite DB (`reference/chinook.db`, ~1MB) — read-only fixture for parity | Commit a DDL `.sql` fixture (D-06); the `.db` is downloaded via `make download-chinook` and may be `.gitignore`d. Confirm CI can obtain it or commit a small seeded copy. |
| Live service config | None | None — Shape A is self-contained; no n8n/Datadog/cloud config |
| OS-registered state | None | None |
| Secrets/env vars | `CODE_MODE_SECRET` (referenced as `${CODE_MODE_SECRET}` in all 4 configs); HMAC `token_secret` | Parity harness must SET this env var before spawning; binary must resolve `${VAR}` (Gap #3) or `env:VAR`. Min 16 bytes (`HmacTokenGenerator::MIN_SECRET_LEN`). |
| Build artifacts | New crate target dir | Standard cargo; add to `[workspace.members]` |

**`reference/chinook.db` availability:** verified present at `/Users/guy/Development/mcp/sdk/pmcp-run/built-in/sql-api/reference/chinook.db` (984 KB) but the *toolkit* repo (`rust-mcp-sdk`) is a different repo — the parity test needs the DB and DDL inside `rust-mcp-sdk`. **Open item: where does the Chinook fixture live for CI in the SDK repo?** Recommend committing a generated `tests/fixtures/chinook.sql` (DDL) + a small `chinook.db` or a build step that creates one from the DDL. `[VERIFIED: file exists in pmcp-run; NOT yet in rust-mcp-sdk]`

## Common Pitfalls

### Pitfall 1: Reference config does not parse (REF-01 not actually met)
**What goes wrong:** `ServerConfig::from_toml(reference/config.toml)` returns `unknown field 'file_path'`. The Chinook config also has `[server] is_reference = true` and `[shared_policy_store]` (creates_shared_store/export_to_ssm/ssm_path) — all unknown.
**Why it happens:** Phase 83 modeled `DatabaseSection` around the Athena/MySQL configs (which use `database`/`url`/`output_location`), not the SQLite reference (which uses `file_path`); `deny_unknown_fields` rejects the rest.
**How to avoid:** Add to the toolkit (additive, REF-01): `DatabaseSection.file_path: Option<String>`; `ServerSection.is_reference: bool` (default false); a new `Option<SharedPolicyStoreSection>` on `ServerConfig`. Then re-run the parse probe for all four configs as the SC-2 gate.
**Warning signs:** SC-2 test fails on the reference config only.
`[VERIFIED: empirical parse probe 2026-05-26 — exact error: "unknown field `file_path`, expected one of `type`, `database`, `output_location`, `workgroup`, `query_timeout_ms`, `tables`, `url`, `pool`"]`

### Pitfall 2: `code_mode_from_config` registers nothing
**What goes wrong:** Server builds, `tools/list` shows the curated `[[tools]]` but NOT `validate_code`/`execute_code`; the 8 `validate_code` scenarios and `execute_code` scenario fail.
**Why it happens:** `code_mode::register_code_mode_tools` is shape-preserving (validates pipeline, returns builder unchanged); Plan 08 (actual registration) was deferred in Phase 83.
**How to avoid:** Build the toolkit pieces production uses (`SqlCodeModeHandler` analog):
  1. `SqlCodeExecutor { connector: Arc<dyn SqlConnector>, config }` impl `pmcp_code_mode::CodeExecutor` — `execute(code, vars)` validates SQL (defense-in-depth) then `connector.execute(...)`.
  2. Make `register_code_mode_tools` actually register: build `validate_code` + `execute_code` `ToolInfo` from `CodeModeToolBuilder::new("sql")`, wrap handlers that drive `ValidationPipeline` + the executor, register via `tool_arc`/`tool`.
  3. Production uses the `#[derive(CodeMode)]` macro (`pmcp-code-mode-derive`) which generates exactly this. **Decision for planner:** reuse the derive macro on a toolkit `SqlCodeModeServer` struct (fastest, matches production) vs hand-write the two handlers. The derive path is proven `[VERIFIED: mcp-sql-server-core/src/code_mode.rs:194-204]`.
**Warning signs:** `tools/list` count = number of `[[tools]]` only; no `validate_code`.
`[VERIFIED: code_mode.rs:176-192 + builder_ext.rs:281-296]`

### Pitfall 3: `${CODE_MODE_SECRET}` token_secret rejected at startup
**What goes wrong:** Binary panics/errors on `code_mode_from_config` because `resolve_token_secret` sees `${CODE_MODE_SECRET}` (not `env:…`), treats it as an inline literal, and rejects it (R9).
**Why it happens:** toolkit supports `env:VAR` only; reference configs use shell-style `${VAR}`.
**How to avoid:** Add `${VAR}` expansion (additive) in `resolve_token_secret` (and ideally a general config env-expansion pass for `output_location`'s `${AWS_ACCOUNT_ID}` etc.), OR document that the binary expands `${VAR}` before parse. Parity harness must `set_var("CODE_MODE_SECRET", "<≥16 bytes>")`.
**Warning signs:** `ToolkitError::Validation(InlineSecretRejected)` or `CodeMode("env var ... not set")` at startup.
`[VERIFIED: code_mode.rs:284-309 + all 4 configs use ${VAR}]`

### Pitfall 4: Non-SQLite configs need lazy startup (SC-1)
**What goes wrong:** Building the Postgres/MySQL/Athena connector tries to open a live connection and hangs/fails in CI (no creds).
**Why it happens:** eager connect.
**How to avoid:** Phase 84 connectors already use `connect_lazy` / SDK-client-only `from_config` (D-09). Verify the binary's dispatch path does NOT call any method that forces a round-trip before `tools/list`. `schema_text()` for non-SQLite WILL hit the backend — so the schema resource/prompt for non-SQLite must come from `--schema` (file), not `schema_text()`. This reinforces Pattern 3's file-based seam.
**Warning signs:** lazy-startup test hangs.
`[CITED: 85-CONTEXT.md D-09; CONN connectors Phase 84]`

### Pitfall 5: HTTP transport handshake / Accept headers
**What goes wrong:** mcp-tester gets 406/415 because POST requires `Content-Type: application/json` and `Accept: application/json` or `text/event-stream`.
**Why it happens:** the streamable-HTTP server strictly validates headers.
**How to avoid:** `ServerTester::new(url, .., force_transport = Some("http"), ..)` uses the SDK `StreamableHttpTransport` which sends correct headers. Confirm the harness passes `force_transport="http"` (or a `http://` URL that auto-selects streamable HTTP). `[VERIFIED: tester.rs:90-160 transport selection + streamable_http_server.rs:390-428 header validation]`

## Code Examples

### Parity harness: spawn + poll + replay (library API)
```rust
// Source: crates/mcp-tester/src/lib.rs (exports) + main.rs:1014-1018 (the lib path the CLI uses)
use std::time::Duration;
use mcp_tester::{ServerTester, ScenarioExecutor, TestScenario};

async fn run_parity(url: &str, scenario_path: &str) -> anyhow::Result<()> {
    // force_transport="http" → SDK StreamableHttpTransport (correct headers)
    let mut tester = ServerTester::new(
        url, Duration::from_secs(30), /*insecure*/ false,
        /*api_key*/ None, /*force_transport*/ Some("http"), /*middleware*/ None,
    )?;
    tester.initialize().await?;                       // MCP handshake
    let scenario = TestScenario::from_file(scenario_path)?;   // YAML or JSON
    let mut exec = ScenarioExecutor::new(&mut tester, /*verbose*/ true);
    let result = exec.execute(scenario).await?;
    assert!(result.all_passed(), "parity scenarios failed: {result:?}");  // confirm field name
    Ok(())
}
```
Readiness poll (D-disc: prefer poll over fixed wait): loop `reqwest`/`tester.initialize()` with backoff until the bound port answers, then run scenarios.

### generated.yaml schema confirmation (consumable as-is)
```text
# Source: pmcp-run/.../reference/scenarios/generated.yaml — operation/assertion histogram (verified):
#   operations: list_tools×1, list_resources×1, list_prompts×1, tool_call×22,
#               get_prompt×1 (start_code_mode), read_resource×3
#   assertions: success×24, contains×8, exists×7, failure×5
# ALL map 1:1 to mcp_tester::Operation and ::Assertion variants. No unsupported types.
# validate_code calls pass {code, dry_run:true}; failure assertions on DELETE/DDL = policy enforcement.
```
`[VERIFIED: scenario.rs Operation/Assertion enums + grep histogram of generated.yaml]`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `mcp-sql-server-core::SqlMcpServer<C>` + `run_lambda` (production) | toolkit `ServerConfig` + `ServerBuilderExt` + `StreamableHttpServer` (Shape A) | Phase 83–85 | Shape A uses the *public* toolkit, not the pmcp-run path-deps; transport is local HTTP not Lambda |
| `DatabaseConnector` (pmcp-run trait, `execute_query`/`execute_statement`) | `SqlConnector` (toolkit, 3-method `dialect`/`execute`/`schema_text`) | Phase 84 | the executor adapter must bridge the toolkit trait's single `execute` to SELECT/DML/DDL |
| `#[derive(CodeMode)]` on a server struct (production) | same macro is available in-workspace (`pmcp-code-mode-derive`) | Phase 67.1 | Phase 85 can reuse it; the toolkit's `register_code_mode_tools` is the config-driven wrapper that still needs the real body |

**Deprecated/outdated:** none relevant. Note `connect_lazy` (MySQL REVIEWS M3) is current and required for D-09.

## Project Constraints (from CLAUDE.md)

- **Zero defects / Toyota Way:** `make quality-gate` before every commit/PR (fmt --all, clippy pedantic+nursery, build, test, audit).
- **Cognitive complexity ≤25 per function** (PMAT CI gate, cap 50 with annotated `#[allow]`). Keep `dispatch`/`assemble` small — split per-backend arms into helpers.
- **Zero SATD comments.**
- **ALWAYS testing matrix per feature:** fuzz + property + unit + `cargo run --example` + integration + doctests. Fuzz is covered by the existing `pmcp_config_toml_parser` target (extend corpus if new config fields added).
- **No Docker / testcontainers** — pure-Rust. SQLite parity (rusqlite bundled), lazy startup for non-SQLite. `[CITED: feedback_avoid_docker_pure_rust_lambda]`
- **Publish order:** add `pmcp-sql-server` to CLAUDE.md §Release after the three per-backend connector crates (slot #9 region, before/after mcp-tester per planner — it depends on toolkit + connectors).
- **`--test-threads=1`** CI convention — the spawn+replay harness must be single-process-safe (bind to an ephemeral port `127.0.0.1:0` and read back `bound_addr`).
- **Contract-first:** consider a contract YAML for the new `SqlCodeExecutor` if it carries security invariants (token-bind-to-code-hash already in pmcp-code-mode).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The Chinook `.db`/DDL fixture can be committed or generated inside `rust-mcp-sdk` for CI | Runtime State | If the DB can't be obtained in CI, the parity test can't run; mitigate by committing a generated DDL + a seeded small `.db`, or building the DB from DDL in a test setup step |
| A2 | Reusing `#[derive(CodeMode)]` (`pmcp-code-mode-derive`) on a toolkit struct is the fastest path to real `validate_code`/`execute_code` registration | Pitfall 2 | If the macro's `context_from`/`Arc<Self>` requirements don't compose with the config-driven builder, hand-writing two `ToolHandler`s via `CodeModeToolBuilder` is the fallback (more code) |
| A3 | `StreamableHttpServerConfig::default()` (stateful) is acceptable for parity; `stateless()` is the Lambda-style alternative | Pattern 2 | If scenarios assume stateless session handling, switch to `stateless()`; verify against scenario session expectations |
| A4 | `ScenarioResult` exposes an all-passed accessor (e.g. `all_passed()`/`failures()`) for the assert | Code Examples | Field/method name unverified — confirm by reading `ScenarioResult` (scenario.rs:200) during planning; may need to inspect `step_results` manually |
| A5 | The `--schema` file content as a plain DDL string is acceptable verbatim as the `docs://chinook/schema` resource body | Pattern 3 / D-05 | D-05 explicitly allows verbatim OR wrapped — low risk; planner's discretion |
| A6 | Adding `file_path`/`is_reference`/`[shared_policy_store]` as additive optional fields satisfies REF-01 without breaking the 3 Athena/MySQL configs | Pitfall 1 | Those 3 already parse OK; additive optional fields can't break them. Low risk. |

## Open Questions

1. **Chinook fixture location in the SDK repo (A1).**
   - What we know: `chinook.db` (984 KB) + scenarios live in `pmcp-run`, a *separate* repo. Parity runs in `rust-mcp-sdk`.
   - What's unclear: whether to commit the `.db`, commit DDL + build the DB in test setup, or copy `generated.yaml` into the SDK repo's test fixtures.
   - Recommendation: commit `tests/fixtures/chinook.sql` (DDL, the `--schema` input per D-06) AND either a committed `chinook.db` (~1MB, under crates.io exclude so it doesn't bloat the published crate) or a `#[ctor]`/test-setup that builds the DB from the DDL + a minimal seed. Also copy `generated.yaml` into the SDK test fixtures (it's the parity contract). Decide in planning.

2. **Reuse derive macro vs hand-write code-mode handlers (A2).**
   - What we know: production uses `#[derive(CodeMode)] #[code_mode(context_from="get_context", language="sql")]`; the macro generates `register_code_mode_tools(self: &Arc<Self>, builder)`.
   - What's unclear: cleanest composition with the toolkit's config-driven `register_code_mode_tools(builder, &ServerConfig)` signature.
   - Recommendation: build a toolkit `SqlCodeModeServer` (mirror production), wire it inside the toolkit's `register_code_mode_tools` so the binary just calls `code_mode_from_config`. Confirm the macro is usable from the toolkit crate (it depends on `pmcp` + `pmcp-code-mode`).

3. **`${VAR}` expansion scope (Gap #3).**
   - What we know: all 4 configs use `${VAR}` for `token_secret` and Athena `output_location`.
   - What's unclear: whether to expand `${VAR}` globally at parse time (one pass over the TOML string) or only in `resolve_token_secret`.
   - Recommendation: a single env-expansion pass on the raw TOML string before `from_toml` is simplest and covers `output_location` too — but verify it doesn't conflict with the existing `env:VAR` convention (they can coexist).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / Rust stable | build/test | ✓ | rustup stable (run `rustup update stable` per CLAUDE.md) | — |
| `sqlite3` CLI | regenerate Chinook DDL fixture (D-06, dev-time) | likely ✓ | — | dump `SqliteConnector::schema_text()` instead |
| `pmcp` `streamable-http` feature | HTTP serving | ✓ (feature exists) | pmcp 2.8.1 | none — required |
| `chinook.db` in SDK repo | parity test | ✗ (lives in pmcp-run) | — | commit DDL + build DB in test setup (A1) |
| `generated.yaml` in SDK repo | parity test | ✗ (lives in pmcp-run) | — | copy into `tests/fixtures/` |
| AWS creds | NOT required (lazy startup, parse-only for Athena) | n/a | — | D-02/D-09 keep cloud out of CI |

**Missing dependencies with no fallback:** none blocking — all gaps have an in-repo fallback (commit/generate fixtures).
**Missing dependencies with fallback:** Chinook DB + scenario YAML must be vendored into `rust-mcp-sdk` (see Open Q1).

## Validation Architecture

> nyquist_validation key is ABSENT in `.planning/config.json` → treated as ENABLED.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` + `proptest` 1.7 |
| Config file | none (cargo) — tests run with `--test-threads=1` (CLAUDE.md) |
| Quick run command | `cargo test -p pmcp-sql-server --no-default-features --features sqlite -- --test-threads=1` |
| Full suite command | `make quality-gate` (workspace fmt/clippy/build/test/audit) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SHAP-A-01 | binary serves MCP from config+schema, zero user Rust | integration | `cargo test -p pmcp-sql-server --test parity_chinook` | ❌ Wave 0 |
| SHAP-A-01 (SC-1) | non-SQLite builds connector + `tools/list` lazily, no creds | integration | `cargo test -p pmcp-sql-server --test lazy_startup` | ❌ Wave 0 |
| REF-01 (SC-2) | all four reference configs parse into `ServerConfig` | unit/integration | `cargo test -p pmcp-sql-server --test superset_parse` | ❌ Wave 0 |
| REF-02 (SC-3+SC-4) | replay 31 generated.yaml scenarios → all pass (curated tools + code-mode + prompt + resource) | integration | `cargo test -p pmcp-sql-server --test parity_chinook` | ❌ Wave 0 |
| (toolkit gap) | `register_code_mode_tools` registers validate_code/execute_code; policy rejects DELETE/DDL | unit | `cargo test -p pmcp-server-toolkit code_mode::` | partial (pipeline tests exist; registration tests ❌) |
| (dispatch) | `type` → connector; compiled-out backend errors clearly | property/unit | `cargo test -p pmcp-sql-server dispatch::` | ❌ Wave 0 |
| (`${VAR}`) | env expansion resolves token_secret from `CODE_MODE_SECRET` | property | `cargo test -p pmcp-server-toolkit env_expand::` | ❌ Wave 0 |
| ALWAYS example | runnable HTTP example | example | `cargo run -p pmcp-sql-server --example chinook_http` | ❌ Wave 0 |
| ALWAYS doctest | public API doctests compile/run | doctest | `cargo test -p pmcp-sql-server --doc` | ❌ Wave 0 |
| ALWAYS fuzz | config parser never panics (incl. new fields) | fuzz | `cargo fuzz run pmcp_config_toml_parser -- -max_total_time=60` | ✓ (extend corpus) |

### Sampling Rate
- **Per task commit:** `cargo test -p pmcp-sql-server --no-default-features --features sqlite -- --test-threads=1`
- **Per wave merge:** `cargo test -p pmcp-sql-server -p pmcp-server-toolkit -- --test-threads=1`
- **Phase gate:** `make quality-gate` green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `tests/superset_parse.rs` — REF-01/SC-2 (all four configs parse) — this is also the regression test for Pitfall 1
- [ ] `tests/lazy_startup.rs` — SC-1 (non-SQLite connector + tools/list, no creds)
- [ ] `tests/parity_chinook.rs` — REF-02/SC-3/SC-4 (spawn + replay generated.yaml)
- [ ] `tests/fixtures/chinook.sql` + Chinook DB provisioning (Open Q1)
- [ ] `tests/fixtures/generated.yaml` — vendored from pmcp-run
- [ ] toolkit: code-mode registration unit tests (validate_code/execute_code exist; DELETE/DDL rejected)
- [ ] `examples/chinook_http.rs` — ALWAYS example

## Security Domain

> `security_enforcement` absent in config.json → treated as ENABLED.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | partial | `StaticAuthProvider` (toolkit) optional; reference scenarios don't require auth, but the binary should support `auth_provider_arc` if configured |
| V3 Session Management | yes | streamable-HTTP session handling is SDK-owned; `StreamableHttpServerConfig` stateful/stateless |
| V4 Access Control | yes | code-mode policy (`allow_writes`/`allow_deletes`/`allow_ddl`/`blocked_tables`) enforced STATICALLY from config (D-13) via `ValidationPipeline` |
| V5 Input Validation | yes | `ServerConfig` `deny_unknown_fields`; `validate_code` runs SQL validation before token issuance; connectors translate `:name` placeholders (no string interpolation) |
| V6 Cryptography | yes | HMAC approval tokens via `pmcp_code_mode::HmacTokenGenerator` (NEVER hand-rolled); `TokenSecret` is `secrecy`-backed; min 16-byte secret |
| V7 Error Handling | yes | bounded connector error messages (T-84-04-02); compiled-out-backend errors must not echo arbitrary input |

### Known Threat Patterns for {SQL MCP server + code-mode}
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection via code-mode | Tampering | `SqlValidator` parse + policy eval before execute; defense-in-depth re-validation in `execute()` (mirror `SqlCodeModeHandler`) |
| Token replay / forgery | Spoofing | HMAC token bound to code hash + user/session context (`ValidationContext`); TTL `token_ttl_seconds` |
| Inline secret in committed config | Information disclosure | R9 inline-secret rejection; `${VAR}`/`env:VAR` resolution only; min secret length |
| DNS rebinding on local HTTP | Spoofing | `DnsRebindingLayer` applied by `StreamableHttpServer::start` (origin-locked) |
| Writes/DDL on read-only intent | Elevation | `allow_writes=false`/`allow_ddl=false` policy; scenarios assert DELETE/DDL → `failure` |
| Schema resource leaking sensitive DDL | Information disclosure | admin redacts `--schema` file (D-04 rationale: file is editable before becoming a public resource) |

## Sources

### Primary (HIGH confidence — read in this session)
- `crates/pmcp-server-toolkit/src/builder_ext.rs` — `ServerBuilderExt` (`tools_from_config_with_connector` takes `Arc<dyn SqlConnector>`; `code_mode_from_config`)
- `crates/pmcp-server-toolkit/src/code_mode.rs` — `register_code_mode_tools` (shape-preserving), `validation_pipeline_from_config`, `assemble_code_mode_prompt`/`build_code_mode_prompt` (reads `connector.schema_text()`), `resolve_token_secret` (`env:` only)
- `crates/pmcp-server-toolkit/src/config.rs` — `ServerConfig`/`DatabaseSection`/`ServerSection`/`CodeModeSection`/`ResourceDecl` (all `deny_unknown_fields`); NO `file_path`/`is_reference`/`shared_policy_store`
- `crates/pmcp-server-toolkit/src/sql/mod.rs` + `sql/sqlite.rs` — `SqlConnector` trait (3 methods), `SqliteConnector::open`/`open_in_memory`
- `crates/pmcp-server-toolkit/src/resources.rs` / `prompts.rs` — `StaticResourceHandler::from_configs`/`from(&cfg)`, `StaticPromptHandler::from_configs`, `CODE_MODE_PROMPT_NAME = "start_code_mode"`
- `crates/pmcp-server-toolkit/examples/{e01_toolkit_minimal,sqlite_minimal}.rs` — verified assembly pattern
- `src/server/streamable_http_server.rs` — `StreamableHttpServer::{new,with_config,start}`, `StreamableHttpServerConfig::{default,stateless}`, header validation, security layer stack
- `crates/mcp-tester/src/{scenario.rs,scenario_executor.rs,tester.rs,main.rs,lib.rs}` — `Operation`/`Assertion` enums, `TestScenario::from_file`, `ScenarioExecutor::execute`, `ServerTester::new`, lib exports
- `crates/pmcp-code-mode/src/handler.rs` — `CodeModeToolBuilder::{build_validate_tool,build_execute_tool}` (input schema incl. `dry_run`), `CodeModeHandler` trait
- `crates/pmcp-code-mode-derive/src/lib.rs` — `#[derive(CodeMode)]` generates real `register_code_mode_tools` (`.tool("validate_code"…).tool("execute_code"…)`)
- `pmcp-run/built-in/sql-api/crates/mcp-sql-server-core/src/code_mode.rs` — production `SqlCodeModeHandler` (CodeExecutor impl) + `SqlCodeModeServer` (`#[derive(CodeMode)]`) — the pattern to mirror
- `pmcp-run/built-in/sql-api/reference/{config.toml,scenarios/generated.yaml,sql-reference-lambda/src/main.rs}` — parity target; scenario operation/assertion histogram
- **Empirical parse probe** (cargo run against all four configs) — REF-01 status `[VERIFIED 2026-05-26]`

### Secondary (MEDIUM confidence)
- `.planning/phases/84-.../84-CONTEXT.md` (referenced) — connector lazy-startup (D-09), `connect_lazy`
- `CLAUDE.md` — publish order, ALWAYS matrix, quality gates, no-Docker

### Tertiary (LOW confidence)
- None — all claims verified against source or empirical probe.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates/versions read from Cargo.toml; APIs read from source.
- Architecture/wiring: HIGH — assembly pattern verified against two working toolkit examples + production reference; HTTP API verified in source.
- REF-01 status: HIGH — empirical parse probe gives the exact failing field.
- Code-mode gap: HIGH — `register_code_mode_tools` body read directly; confirmed no tool registration.
- Pitfalls: HIGH — each backed by a source line or probe result.
- Open Q1 (fixture location): MEDIUM — depends on a planning decision, not a fact gap.

**Research date:** 2026-05-26
**Valid until:** 2026-06-25 (stable workspace; re-verify connector-crate versions + `StreamableHttpServer` re-export path at plan time)
