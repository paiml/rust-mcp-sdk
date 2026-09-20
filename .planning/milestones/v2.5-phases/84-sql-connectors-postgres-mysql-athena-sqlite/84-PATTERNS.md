# Phase 84: SQL Connectors (Postgres / MySQL / Athena / SQLite) — Pattern Map

**Mapped:** 2026-05-19
**Files analyzed:** 18 new/modified targets
**Analogs found:** 18 / 18

## File Classification

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/pmcp-server-toolkit/src/sql/translate.rs` (NEW) | utility (state machine) | transform | `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:85-122` + `crates/pmcp-server-toolkit/src/tools.rs:99-149` (split-helper Pattern G) | role + flow match |
| `crates/pmcp-server-toolkit/src/sql/mod.rs` (MODIFY) | model / trait | request-response | itself (Phase 83 baseline) `crates/pmcp-server-toolkit/src/sql/mod.rs:60-77` | identity |
| `crates/pmcp-server-toolkit/src/sql/sqlite.rs` (NEW) | service / driver impl | request-response | `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:228-322` (`sqlite_backend`) + existing `MockSqlConnector` at `src/sql/mod.rs:182-199` | exact |
| `crates/pmcp-toolkit-postgres/Cargo.toml` (NEW) | config | — | `crates/pmcp-code-mode/Cargo.toml:1-67` (workspace member with optional features + path dep) | role match |
| `crates/pmcp-toolkit-postgres/src/lib.rs` (NEW) | service / driver impl | request-response | spike 005 `postgres_mock` shape (`main.rs:340-466`) + `pmcp-code-mode/src/lib.rs:56-167` (crate-root re-exports) | exact |
| `crates/pmcp-toolkit-postgres/tests/mock_postgres.rs` (NEW) | test fixture | request-response | `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:340-466` | exact |
| `crates/pmcp-toolkit-postgres/tests/integration.rs` (NEW) | test (integration) | request-response | `crates/pmcp-server-toolkit/tests/code_mode_wiring.rs:1-60` + `tests/backend_core_smoke.rs:1-60` | role match |
| `crates/pmcp-toolkit-postgres/examples/postgres_minimal.rs` (NEW) | example | demo | `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` | exact |
| `crates/pmcp-toolkit-mysql/{Cargo.toml,src/lib.rs,tests/*,examples/*}` (NEW) | same as Postgres | — | mirror Postgres skeleton + spike 005 `mysql_mock` (main.rs:476-575) | exact |
| `crates/pmcp-toolkit-athena/{Cargo.toml,src/lib.rs,tests/*,examples/*}` (NEW) | same as Postgres | — | mirror Postgres skeleton + spike 005 `athena_mock` (main.rs:587-697); Cargo.toml AWS-SDK feature flags from `crates/pmcp-server-toolkit/Cargo.toml:42-48` | exact |
| `crates/pmcp-server-toolkit/src/tools.rs` (MODIFY) | controller / synthesizer | request-response | itself — `synthesize_from_config` at `tools.rs:74-92` + `SynthesizedToolHandler` at `tools.rs:185-204` | identity |
| `crates/pmcp-server-toolkit/src/code_mode.rs` (MODIFY — CONN-04 alias) | utility | request-response | itself — `assemble_code_mode_prompt` at `code_mode.rs:365-390` | identity |
| `crates/pmcp-server-toolkit/tests/synthesizer_structured_content.rs` (NEW) | test (integration) | request-response | `crates/pmcp-server-toolkit/tests/code_mode_wiring.rs` + `pmcp::types::tools::CallToolResult::with_widget_enrichment` (`src/types/tools.rs:582-600`) | role match |
| `crates/pmcp-server-toolkit/fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs` (MODIFY — extend) | test (fuzz) | input fuzz | itself (`pmcp_server_toolkit_config_parser.rs:30-46`) — `[database].url` key inherits automatically | identity |
| `crates/pmcp-server-toolkit/src/config.rs` (MODIFY — add `url`) | model / config | parse | itself — `DatabaseSection` at `config.rs:286-314` | identity |
| `Cargo.toml` (root `[workspace.members]` at line 541) (MODIFY) | config | — | itself — line 541 | identity |
| `CLAUDE.md` §"Release & Publish Workflow" lines 223-231 (MODIFY) | docs | — | itself (lines 223-231) | identity |
| `crates/pmcp-server-toolkit/examples/sqlite_minimal.rs` (NEW) | example | demo | `examples/e01_toolkit_minimal.rs` (Shape C ≤15-line `main`) | exact |

## Pattern Assignments

### `crates/pmcp-server-toolkit/src/sql/translate.rs` (NEW) — utility, transform

**Analogs:**
- Reference state machine: `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:85-122`
- Split-helper template (Pattern G): `crates/pmcp-server-toolkit/src/tools.rs:99-149` (`build_input_schema` / `build_param_property` / `build_annotations`)

**Imports pattern** (from `crates/pmcp-server-toolkit/src/sql/mod.rs:31-32`):
```rust
use async_trait::async_trait;
use thiserror::Error;
// std::fmt::Write is also needed in the spike — pull in at translate.rs scope:
use std::fmt::Write as _;
```

**Core pattern — split-helper state machine** (extracted from RESEARCH §2 + spike `main.rs:85-122` + tools.rs decomposition):
```rust
// Public entry point — total over &str input.
pub fn translate_placeholders(sql: &str, dialect: Dialect) -> TranslatedSql {
    let mut walker = SqlWalker::new(sql, dialect);
    walker.run();
    walker.into_translated()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatedSql {
    pub sql: String,
    pub ordered_params: Vec<String>,
}

struct SqlWalker<'a> { /* sql: &'a str, state: State, out: String, order: Vec<String>, pg_index: usize */ }

impl<'a> SqlWalker<'a> {
    fn run(&mut self) { /* cog ~10 — outer dispatch */ }
    fn handle_normal(&mut self, c: char) { /* cog ~12 */ }
    fn handle_string(&mut self, c: char, q: char) { /* cog ~6 */ }
    fn handle_line_comment(&mut self, c: char) { /* cog ~3 */ }
    fn handle_block_comment(&mut self, c: char, depth: usize) { /* cog ~5 */ }
    fn emit_placeholder(&mut self, name: &str) { /* cog ~6 — dispatch per dialect */ }
}
```

**Per-dialect placeholder emission** (from spike `main.rs:108-116`):
```rust
match dialect {
    Dialect::Postgres => { pg_index += 1; write!(out, "${pg_index}").unwrap(); },
    Dialect::MySql | Dialect::Athena => out.push('?'),
    Dialect::Sqlite => write!(out, ":{name}").unwrap(),
}
```

**Lone-`:` handling** (from spike `main.rs:103-107` — keep verbatim for `'foo'::text` etc.):
```rust
if name.is_empty() {
    out.push(':');
    continue;
}
```

**PMAT disposition:** RESEARCH §2 mandates the split-helper form (Pattern G). The flat impl exceeds cog 25; the split form puts every helper well under. NO `#[allow(clippy::cognitive_complexity)]` — split first, allow second.

**Property test invariants — 5 from RESEARCH §2.4** (these go in `src/sql/translate.rs#[cfg(test)] mod proptests` or a sibling `tests.rs` module):
1. Idempotence for `:name`-free SQL (any dialect): `translate.sql == input`.
2. Bind-order preservation: `ordered_params == [n_1, n_2, ..., n_n]` (positional order).
3. Postgres positional indexing: `$1, $2, ..., $n` (repeats get fresh `$k`).
4. Sqlite identity: `Dialect::Sqlite` ⇒ `translate.sql == input` (spike `main.rs:291` `debug_assert_eq!` codifies this).
5. No panic on adversarial input (fuzz-style + arbitrary `&str`).

---

### `crates/pmcp-server-toolkit/src/sql/mod.rs` (MODIFY) — model / trait

**Analog:** itself. Read first: `src/sql/mod.rs:60-77` (current 2-method trait).

**Current trait body** (Phase 83 baseline at `src/sql/mod.rs:60-77`):
```rust
#[async_trait]
pub trait SqlConnector: Send + Sync + 'static {
    fn dialect(&self) -> Dialect;
    async fn schema_text(&self) -> Result<String, ConnectorError>;
}
```

**Extension pattern — additive 3rd method** (per D-01, CONN-01 — no default body; this is the breaking change the trait's rustdoc at `src/sql/mod.rs:41-55` already warns about):
```rust
async fn execute(
    &self,
    sql: &str,
    params: &[(String, serde_json::Value)],
) -> Result<Vec<serde_json::Value>, ConnectorError>;
```

**`ConnectorError` extension pattern** (the enum is already `#[non_exhaustive]` at `src/sql/mod.rs:152-153` — variants are additive):
```rust
// Add to the existing enum at lines 153-171:
/// Underlying driver / SDK call failed (e.g. tokio-postgres connect, sqlx query).
#[error("driver error: {0}")]
Driver(String),
/// Query syntax / planning error from the backend.
#[error("query error: {0}")]
Query(String),
/// Failed to bind a named parameter to a driver-native position.
#[error("parameter bind failed for '{name}': {reason}")]
ParameterBind { name: String, reason: String },
/// Connection-pool acquire / handshake failure.
#[error("connection error: {0}")]
Connection(String),
```

**`#[non_exhaustive]` discipline:** `Dialect` enum stays unchanged (4 variants already present). Adding `ConnectorError` variants is additive — no semver bump (per `src/sql/mod.rs:292-296` rationale).

**`MockSqlConnector` disposition (Pitfall #10):** KEEP `pub(crate) MockSqlConnector` at `src/sql/mod.rs:182-199` (test-only, dialect-stamped canned-schema fixture). Add `pub struct SqliteConnector` ALONGSIDE it in the new `src/sql/sqlite.rs` module. The two serve different roles — see RESEARCH Open Question #3.

**Need to grep before deleting `MockSqlConnector`:** `grep -rn "MockSqlConnector" crates/pmcp-server-toolkit/src crates/pmcp-server-toolkit/tests` — RESEARCH §"Cross-Cutting Risks #10" confirms 4 call sites in `src/code_mode.rs` lines 555 / 574 / 596 / 632 / 653.

---

### `crates/pmcp-server-toolkit/src/sql/sqlite.rs` (NEW) — service / driver impl, request-response

**Analog:** `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:228-322` (`mod sqlite_backend`) — VERDICT: VALIDATED.

**Imports pattern** (from spike `main.rs:230-231`):
```rust
use async_trait::async_trait;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};
```

**Cfg gate** (matches existing `MockSqlConnector` gate at `src/sql/mod.rs:180-181`):
```rust
#[cfg(feature = "sqlite")]
// (NOT `cfg(any(test, feature = "sqlite"))` for the public connector — pub
// surface requires the feature; `pub(crate) MockSqlConnector` keeps its
// test-fallback shape)
```

**Struct + constructors** (spike `main.rs:233-247`, refined per RESEARCH §1.4 + Claude's Discretion D-09):
```rust
pub struct SqliteConnector {
    conn: Arc<Mutex<Connection>>,
    schema_blob: String,
}

impl SqliteConnector {
    pub fn open(path: &std::path::Path) -> Result<Self, ConnectorError> {
        let conn = Connection::open(path).map_err(|e| ConnectorError::Connection(e.to_string()))?;
        // schema_text seeded lazily; or pre-fetched via sqlite_master here.
        Ok(Self { conn: Arc::new(Mutex::new(conn)), schema_blob: String::new() })
    }
    pub fn open_in_memory() -> Result<Self, ConnectorError> { /* same, Connection::open_in_memory() */ }
}
```

**`async fn execute` wrapping sync rusqlite via `spawn_blocking`** (RESEARCH §1.4):
```rust
async fn execute(&self, sql: &str, params: &[(String, Value)]) -> Result<Vec<Value>, ConnectorError> {
    let conn = self.conn.clone();
    let sql = sql.to_string();
    let params = params.to_vec();
    tokio::task::spawn_blocking(move || -> Result<Vec<Value>, ConnectorError> {
        let guard = conn.lock().map_err(|_| ConnectorError::Driver("mutex poisoned".into()))?;
        // ... spike main.rs:286-315 body verbatim
        Ok(out)
    })
    .await
    .map_err(|e| ConnectorError::Driver(e.to_string()))?
}
```

**`json_to_sql` / `sql_to_json` helpers** (lift verbatim from spike `main.rs:249-277`).

**`schema_text()` via `sqlite_master`** (RESEARCH §1.4 + D-11):
```sql
SELECT name, sql FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name;
```

---

### `crates/pmcp-toolkit-postgres/Cargo.toml` (NEW) — config

**Analog:** `crates/pmcp-code-mode/Cargo.toml:1-67` (workspace member with optional features + path dep) and `crates/pmcp-server-toolkit/Cargo.toml:42-48` (the RUSTSEC-safe AWS-SDK feature template, reused for AWS-SDK-Athena in `pmcp-toolkit-athena`).

**Excerpt — package + workspace dep wiring** (mirror pattern from `pmcp-code-mode/Cargo.toml:1-17` + `pmcp-server-toolkit/Cargo.toml:22-23`):
```toml
[package]
name = "pmcp-toolkit-postgres"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/paiml/rust-mcp-sdk"
description = "Postgres connector for pmcp-server-toolkit (config-driven MCP servers)"
keywords = ["mcp", "postgres", "sql", "toolkit"]
categories = ["development-tools", "database"]

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[dependencies]
pmcp-server-toolkit = { version = "0.1.0", path = "../pmcp-server-toolkit" }
tokio-postgres = "0.7.17"            # RESEARCH §1.1 — VERIFIED
deadpool-postgres = "0.14.1"         # RESEARCH §1.1 — VERIFIED
tokio = { version = "1", default-features = false, features = ["rt"] }
async-trait = "0.1"
serde_json = "1"
thiserror = "2"
tracing = "0.1"

[dev-dependencies]
proptest = "1.7"
# Why: tests run with --test-threads=1 per CLAUDE.md — single-threaded tokio is sufficient.
tokio = { version = "1", features = ["macros", "rt"] }
```

**`exclude` to keep <10MB published artifact** (from `pmcp-server-toolkit/Cargo.toml:16`):
```toml
exclude = [".planning/", ".pmat/", "fixtures/", "tests/", "fuzz/"]
```

---

### `crates/pmcp-toolkit-postgres/src/lib.rs` (NEW) — service / driver impl

**Analogs:**
- Crate-root re-export pattern: `crates/pmcp-code-mode/src/lib.rs:116-213` (re-exports + `pub use async_trait::async_trait`).
- Trait impl pattern: spike 005 `postgres_mock` at `main.rs:340-466`.
- AWS SDK opt-in features (mirrored verbatim for Athena, not Postgres): `crates/pmcp-server-toolkit/Cargo.toml:42-48`.

**Module-level allow + docstring pattern** (from `pmcp-code-mode/src/lib.rs:1-55`):
```rust
//! Postgres connector for pmcp-server-toolkit.
//!
//! Implements `pmcp_server_toolkit::sql::SqlConnector` over `tokio-postgres`
//! + `deadpool-postgres`. Pure-Rust + Lambda-deployable per
//! `feedback_avoid_docker_pure_rust_lambda` memory.
//!
//! # Example
//!
//! ```no_run
//! use pmcp_toolkit_postgres::PostgresConnector;
//! # async fn run() -> anyhow::Result<()> {
//! let conn = PostgresConnector::connect("postgres://localhost/mydb").await?;
//! # Ok(()) }
//! ```

#![allow(clippy::doc_markdown)]
```

**Imports pattern** (RESEARCH §1.1 + analog `pmcp-server-toolkit/src/sql/mod.rs:31`):
```rust
use async_trait::async_trait;
use deadpool_postgres::{Manager, ManagerConfig, Pool};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tokio_postgres::{NoTls, types::ToSql};

use pmcp_server_toolkit::sql::{
    translate_placeholders, ConnectorError, Dialect, SqlConnector, TranslatedSql,
};
```

**Connect pattern** (RESEARCH §1.1 — Pitfall #13 + #11):
```rust
impl PostgresConnector {
    pub async fn connect(url: &str) -> Result<Self, ConnectorError> {
        let pg_config: tokio_postgres::Config = url.parse()
            .map_err(|e: tokio_postgres::Error| ConnectorError::Connection(e.to_string()))?;
        let mgr = Manager::from_config(pg_config, NoTls, ManagerConfig::default());
        let pool = Pool::builder(mgr).max_size(16).build()
            .map_err(|e| ConnectorError::Connection(e.to_string()))?;
        // NOTE: deadpool spawns the Connection future internally. If you use
        // tokio_postgres::connect() directly, the `tokio::spawn(async move {
        // let _ = connection.await; })` is MANDATORY (Pitfall #13).
        Ok(Self { pool })
    }
}
```

**Per-row `Value` conversion** (RESEARCH §1.1 lines 159-173 — copy verbatim):
```rust
let mut obj = serde_json::Map::new();
for (idx, col) in row.columns().iter().enumerate() {
    let name = col.name().to_string();
    let val: Value = match col.type_().name() {
        "int8" | "int4" | "int2" => row.try_get::<_, Option<i64>>(idx)?.map_or(Value::Null, |i| json!(i)),
        "float8" | "float4" => row.try_get::<_, Option<f64>>(idx)?.map_or(Value::Null, |f| json!(f)),
        "bool" => row.try_get::<_, Option<bool>>(idx)?.map_or(Value::Null, |b| json!(b)),
        "text" | "varchar" | "char" | "bpchar" => row.try_get::<_, Option<String>>(idx)?.map_or(Value::Null, |s| json!(s)),
        "json" | "jsonb" => row.try_get::<_, Option<Value>>(idx)?.unwrap_or(Value::Null),
        _ => row.try_get::<_, Option<String>>(idx).ok().flatten().map_or(Value::Null, |s| json!(s)),
    };
    obj.insert(name, val);
}
```

**`SqlConnector` trait impl skeleton** (lifted from spike `main.rs:373-414`, adapted for real `tokio-postgres` + the new `execute()` signature):
```rust
#[async_trait]
impl SqlConnector for PostgresConnector {
    fn dialect(&self) -> Dialect { Dialect::Postgres }

    async fn execute(&self, sql: &str, params: &[(String, Value)]) -> Result<Vec<Value>, ConnectorError> {
        let TranslatedSql { sql: translated, ordered_params } =
            translate_placeholders(sql, Dialect::Postgres);
        // Build positional bind list using ordered_params + value→ToSql per-variant dispatch (Pitfall #11).
        // Execute via pool.get().await?.query(&translated, &bind_refs).await?
        // Convert each Row → Value::Object per the loop above.
        todo!("plan-driven")
    }

    async fn schema_text(&self) -> Result<String, ConnectorError> { /* information_schema SELECT */ todo!() }
}
```

---

### `crates/pmcp-toolkit-postgres/tests/mock_postgres.rs` (NEW) — test fixture, request-response

**Analog:** `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:340-466` (`mod postgres_mock`) — VALIDATED.

**Imports + struct shape** (spike `main.rs:340-369`):
```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

use pmcp_server_toolkit::sql::{
    translate_placeholders, ConnectorError, Dialect, SqlConnector, TranslatedSql,
};

pub struct PostgresMock {
    pub tables: HashMap<String, Vec<Value>>,
    pub last_translated_sql: Mutex<Option<String>>,
    pub last_positional_args: Mutex<Option<Vec<Value>>>,
}
```

**`execute()` body — translate + positional bind + cheap engine** (spike `main.rs:378-414`):
```rust
async fn execute(&self, sql: &str, params: &[(String, Value)]) -> Result<Vec<Value>, ConnectorError> {
    let TranslatedSql { sql: translated, ordered_params } =
        translate_placeholders(sql, Dialect::Postgres);
    let positional: Vec<Value> = ordered_params.iter()
        .map(|n| params.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap_or(Value::Null))
        .collect();
    *self.last_translated_sql.lock().unwrap() = Some(translated.clone());
    *self.last_positional_args.lock().unwrap() = Some(positional.clone());
    execute_cheap_query(&self.tables, &translated, &positional)
}
```

**Cheap query engine** (spike `main.rs:416-466`) — string-match the few queries from `tests/fixtures/*-config.toml`. NOT a general SQL engine.

**`schema_text()` returns information_schema-styled CREATE TABLE blob** (spike `main.rs:401-413`).

---

### `crates/pmcp-toolkit-postgres/tests/integration.rs` (NEW)

**Analog:** `crates/pmcp-server-toolkit/tests/code_mode_wiring.rs:1-60` + `tests/backend_core_smoke.rs:1-60`.

**Header pattern** (from `code_mode_wiring.rs:1-21`):
```rust
//! Phase 84 CONN-05 integration anchor — Postgres connector against in-process mock.
//!
//! Per D-13: each integration test (a) constructs the connector against its mock,
//! (b) calls `execute()` with a parameterized query (`:name` placeholders verifying
//! `translate_placeholders` is wired through), (c) calls `schema_text()` and asserts
//! the result contains the mock's expected DDL fragments, (d) asserts dialect ID.

mod mock_postgres;

use mock_postgres::PostgresMock;
use pmcp_server_toolkit::sql::{Dialect, SqlConnector};
use serde_json::Value;
```

**Test body shape** (from `tests/code_mode_wiring.rs:44-60` adapted for SQL):
```rust
#[tokio::test]
async fn execute_translates_named_to_positional_postgres() {
    let mock = PostgresMock::employee_directory();
    let rows = mock
        .execute("SELECT * FROM employees WHERE id = :id",
                 &[("id".into(), Value::from(1_i64))])
        .await
        .expect("execute");
    let translated = mock.last_translated_sql.lock().unwrap().clone().unwrap();
    assert!(translated.contains("WHERE id = $1"));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], Value::from("Ada Lovelace"));
}
```

---

### `crates/pmcp-toolkit-postgres/examples/postgres_minimal.rs` (NEW)

**Analog:** `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` (Shape C ≤15-line `main`).

**ALWAYS-requirements doctring** (from `e01_toolkit_minimal.rs:1-12`):
```rust
//! Postgres connector minimal example — Shape C ≤15-line `main.rs`.
//!
//! Build with:
//! ```sh
//! cargo run -p pmcp-toolkit-postgres --example postgres_minimal
//! ```
```

**Cargo.toml `[[example]]` declaration** (from `pmcp-server-toolkit/Cargo.toml:66-68`):
```toml
[[example]]
name = "postgres_minimal"
required-features = []  # or ["..."] if any feature gates apply
```

---

### `crates/pmcp-toolkit-mysql/*` (NEW CRATE)

Mirror Postgres skeleton verbatim with these substitutions:
- `Cargo.toml` deps: `sqlx = { version = "0.8.6", default-features = false, features = ["mysql", "runtime-tokio", "tls-rustls-aws-lc-rs"] }` (RESEARCH §1.2 + Pitfall #14 mirror). DROP `tokio-postgres` / `deadpool-postgres`.
- `Connect`: `MySqlPool::connect(url).await?` (RESEARCH §1.2 line 204).
- `Query`: `sqlx::query(&translated_sql).bind(val).fetch_all(&pool).await?` (RESEARCH §1.2 line 213).
- `mock_mysql.rs`: spike `main.rs:476-575` verbatim — `?` placeholders, backtick identifier quoting, InnoDB markers.

---

### `crates/pmcp-toolkit-athena/*` (NEW CRATE)

Mirror Postgres skeleton with these substitutions:
- `Cargo.toml` deps (CRITICAL — mirror `pmcp-server-toolkit/Cargo.toml:42-48` RUSTSEC pattern):
```toml
aws-sdk-athena = { version = "1.105.0", default-features = false, features = ["default-https-client", "rt-tokio", "behavior-version-latest"] }
aws-config = { version = "1.8.16", default-features = false, features = ["default-https-client", "rt-tokio", "credentials-process", "sso", "behavior-version-latest"] }
# NO aws-sdk-glue — RESEARCH §1.3 + Pitfall #14: GetTableMetadata covers schema introspection.
```
- `from_config(region, workgroup) -> Result<Self, ConnectorError>` constructor (per D-08).
- StartQueryExecution → poll → GetQueryResults loop (RESEARCH §1.3 lines 252-276).
- `mock_athena.rs`: spike `main.rs:587-697` verbatim — `?` placeholders, Glue-Data-Catalog-styled `schema_text()`. The mock does NOT model the polling loop (RESEARCH §3.3 footnote — trait surface hides it).
- Polling tuning: 500ms start, doubling, capped at 5s, bounded by `query_timeout_ms` (RESEARCH §1.3 final line).

---

### `crates/pmcp-server-toolkit/src/tools.rs` (MODIFY) — synthesizer, request-response

**Analog:** itself — read first `tools.rs:74-92` (`synthesize_from_config`) and `tools.rs:185-204` (`SynthesizedToolHandler`).

**Current placeholder body** (`tools.rs:192-204`):
```rust
#[async_trait]
impl ToolHandler for SynthesizedToolHandler {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Err(pmcp::Error::Internal(format!(
            "tool '{}' is not yet wired — Plan 06 (code-mode) or Phase 84 (SQL connector) required",
            self.info.name
        )))
    }
    fn metadata(&self) -> Option<ToolInfo> { Some(self.info.clone()) }
}
```

**Wiring change — thread `Arc<dyn SqlConnector>` through synthesizer (RESEARCH §4 "Connector reference flow", Option A)**:
- Change signature: `synthesize_from_config(config: &ServerConfig, connector: Arc<dyn SqlConnector>) -> Result<Vec<SynthesizedTool>>`.
- Update `SynthesizedToolHandler` struct (line 185) to hold `connector: Arc<dyn SqlConnector>`.
- New `handle()` body uses `decl.sql` (already on the held `decl` at line 189):
```rust
async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
    let sql = self.decl.sql.as_deref().ok_or_else(|| pmcp::Error::Internal(
        format!("tool '{}' has no `sql` declared", self.info.name),
    ))?;
    // Build &[(String, Value)] from validated args + declared parameter list.
    let named_params: Vec<(String, Value)> = self.decl.parameters.iter()
        .filter_map(|p| args.get(&p.name).map(|v| (p.name.clone(), v.clone())))
        .collect();
    let rows = self.connector.execute(sql, &named_params).await
        .map_err(|e| pmcp::Error::Internal(format!("connector error: {e}")))?;
    Ok(Value::Array(rows))
}
```

**`widget_meta` flip-on pattern (RESEARCH §4 Option A)**:
```rust
// In synthesize_from_config: when decl.ui_resource_uri.is_some(), apply
// ToolInfo::with_widget_meta on the info before constructing the handler.
let info = /* base ToolInfo::new(...) or ::with_annotations(...) */;
let info = if let Some(uri) = decl.ui_resource_uri.as_deref() {
    info.with_widget_meta(WidgetMeta::new().domain(uri))  // exact builder per src/types/tools.rs:334
} else {
    info
};
```

**Why this fires `structured_content`:** Read `src/server/core.rs:593-601`:
```rust
let call_result = if let Some(info) = tool_info.filter(|i| i.widget_meta().is_some()) {
    let summary = summarize_structured_output(&value);
    CallToolResult::new(vec![Content::text(summary)]).with_widget_enrichment(info, value)
} else {
    let text = serde_json::to_string_pretty(&value)?;
    CallToolResult::new(vec![Content::text(text)])
};
```
And `src/types/tools.rs:582-600` (`with_widget_enrichment` body) calls `with_structured_content(value)` ONLY when `info.widget_meta().is_some()` — confirmed at line 583.

**Read-first hints for the planner** (per RESEARCH §4):
| File | Lines | Why |
|------|-------|-----|
| `crates/pmcp-server-toolkit/src/tools.rs` | 74-92 | Signature change point |
| `crates/pmcp-server-toolkit/src/tools.rs` | 185-204 | Handler body with `.execute()` + `widget_meta` conditional |
| `src/server/core.rs` | 593-601 | Confirms `widget_meta` gate on `structured_content` |
| `src/types/tools.rs` | 582-600 | `with_widget_enrichment` body |
| `src/types/tools.rs` | 334-339 | `ToolInfo::with_widget_meta` constructor |

---

### `crates/pmcp-server-toolkit/src/code_mode.rs` (MODIFY — CONN-04 alias)

**Analog:** itself — read first `src/code_mode.rs:365-390` (`assemble_code_mode_prompt`).

**Existing function signature** (`code_mode.rs:365-368`):
```rust
pub async fn assemble_code_mode_prompt(
    connector: &(dyn SqlConnector + '_),
    config: &ServerConfig,
) -> Result<String>
```

**Per D-12 + RESEARCH Open Question #2 + Pitfall #15 — RECOMMENDATION: thin function alias** (not deprecated re-export — both names stay valid; matches P83's `register_code_mode_tools` vs `code_mode_tools_from_executor` dual-naming):
```rust
/// Alias for [`assemble_code_mode_prompt`] satisfying CONN-04's literal naming.
///
/// Identical behaviour; both names are valid public surface.
pub async fn build_code_mode_prompt(
    connector: &(dyn SqlConnector + '_),
    config: &ServerConfig,
) -> Result<String> {
    assemble_code_mode_prompt(connector, config).await
}
```

**Doctest pattern** (from `code_mode.rs:354-364`):
```rust
/// # Example
///
/// ```no_run
/// use pmcp_server_toolkit::code_mode::build_code_mode_prompt;
/// use pmcp_server_toolkit::config::ServerConfig;
/// use pmcp_server_toolkit::sql::SqlConnector;
///
/// async fn assemble<C: SqlConnector>(connector: &C, config: &ServerConfig) {
///     let prompt = build_code_mode_prompt(connector, config).await.unwrap();
///     assert!(prompt.contains("# Code Mode"));
/// }
/// ```
```

---

### `crates/pmcp-server-toolkit/src/config.rs` (MODIFY — add `url`)

**Analog:** itself — read first `src/config.rs:286-314` (`DatabaseSection`).

**Current struct** (`config.rs:290-314`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSection {
    #[serde(default, rename = "type")]
    pub backend_type: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub output_location: Option<String>,
    #[serde(default)]
    pub workgroup: Option<String>,
    #[serde(default)]
    pub query_timeout_ms: Option<u64>,
    #[serde(default)]
    pub tables: Vec<DatabaseTableDecl>,
    #[serde(default)]
    pub pool: Option<DatabasePoolSection>,
}
```

**Additive change (RESEARCH §5 — single field, additive only, REF-01 compliant):**
```rust
/// Connection URL for Postgres / MySQL backends. Supports `env:VAR_NAME`
/// indirection for secret hygiene (mirrors `[code_mode].token_secret` per
/// P83 R6/R9). Optional/unused for Athena (uses `region` + `workgroup` +
/// `output_location`) and SQLite (uses `database` for the file path).
#[serde(default)]
pub url: Option<String>,
```

**Strict-parse impact (RESEARCH §5 final paragraph):** Single additive key under `#[serde(deny_unknown_fields)]` — the fuzz target at `fuzz_targets/pmcp_server_toolkit_config_parser.rs:30-46` covers the new field automatically on next run. The 3 reference configs in `tests/fixtures/{open-images,imdb,msr-vtt}-config.toml` don't emit `url` so `tests/reference_configs.rs` continues to pass.

---

### `crates/pmcp-server-toolkit/fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs` (MODIFY — extend)

**Analog:** itself — read first `fuzz_targets/pmcp_server_toolkit_config_parser.rs:30-46`.

**Current target body** (`pmcp_server_toolkit_config_parser.rs:35-46`):
```rust
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return; };
    let _: Result<ServerConfig, _> = ServerConfig::from_toml(s);
});
```

**P84 disposition:** NO new fuzz_targets/ entries (per D-14 — extend, don't duplicate). The new `[database].url` field is automatically covered by the existing single-target body because `ServerConfig::from_toml` already exercises the full schema with `#[serde(deny_unknown_fields)]`.

Corpus seeding: drop one new seed in `fuzz/corpus/pmcp_server_toolkit_config_parser/` containing a `[database] url = "env:DATABASE_URL"` snippet so libfuzzer has a starting point for mutations.

---

### `crates/pmcp-server-toolkit/tests/synthesizer_structured_content.rs` (NEW)

**Analogs:**
- Test header / file shape: `crates/pmcp-server-toolkit/tests/code_mode_wiring.rs:1-43`.
- The gate being verified: `src/types/tools.rs:582-600` (`with_widget_enrichment`).
- Synthesizer entry point: `crates/pmcp-server-toolkit/src/tools.rs:74-92`.

**Test header pattern** (from `code_mode_wiring.rs:1-13`):
```rust
//! D-06 integration anchor — synthesizer emits structuredContent for widget tools.
//!
//! Per RESEARCH §4 Option A: when `ToolDecl.ui_resource_uri.is_some()`, the
//! synthesizer flips `widget_meta` on the synthesized `ToolInfo` so pmcp core's
//! `CallToolResult::with_widget_enrichment` populates `structured_content`
//! with the `Vec<Value>` rows returned by the connector. This test asserts the
//! contract end-to-end against an in-memory SQLite connector.

#![cfg(all(feature = "code-mode", feature = "sqlite"))]
```

**Assertion sketch:**
```rust
#[tokio::test]
async fn widget_tool_emits_structured_content() {
    let cfg = ServerConfig::from_toml_strict_validated(WIDGET_TOOL_CONFIG).expect("cfg");
    let conn: Arc<dyn SqlConnector> = Arc::new(SqliteConnector::open_in_memory().unwrap());
    let tools = synthesize_from_config(&cfg, conn).expect("synth");
    let (_name, info, handler) = &tools[0];
    assert!(info.widget_meta().is_some(), "widget tool must have widget_meta");
    // Call handle, route through pmcp core's `with_widget_enrichment`, assert
    // structured_content is populated.
}
```

---

### `Cargo.toml` root `[workspace.members]` (MODIFY at line 541)

**Analog:** itself — read first `Cargo.toml:541`.

**Current** (verified from RESEARCH §7):
```toml
members = ["pmcp-macros", "crates/pmcp-macros-support", "crates/mcp-tester", "crates/mcp-preview", "crates/pmcp-server", "crates/pmcp-server/pmcp-server-lambda", "crates/pmcp-tasks", "crates/mcp-e2e-tests", "crates/pmcp-widget-utils", "crates/pmcp-code-mode", "crates/pmcp-code-mode-derive", "crates/pmcp-server-toolkit", "examples/25-oauth-basic", "examples/test-basic", "cargo-pmcp"]
```

**P84 insertion (RESEARCH §7) — immediately after `"crates/pmcp-server-toolkit"`:**
```toml
... "crates/pmcp-server-toolkit", "crates/pmcp-toolkit-postgres", "crates/pmcp-toolkit-mysql", "crates/pmcp-toolkit-athena", "examples/25-oauth-basic", ...
```

---

### `CLAUDE.md` §"Release & Publish Workflow" lines 223-231 (MODIFY)

**Analog:** itself — read first `CLAUDE.md:223-231`.

**Current** (verified from CLAUDE.md / RESEARCH §7):
```markdown
### Workspace Crates (publish order)
1. `pmcp-widget-utils` (leaf, no internal deps)
2. `pmcp` (core SDK, depends on widget-utils)
3. `pmcp-code-mode` (depends on pmcp)
4. `pmcp-code-mode-derive` (depends on pmcp-code-mode)
5. `pmcp-server-toolkit` (runtime library; depends on pmcp + pmcp-code-mode under the default `code-mode` feature)
6. `mcp-tester` (depends on pmcp)
7. `mcp-preview` (depends on widget-utils)
8. `cargo-pmcp` (depends on pmcp, mcp-tester, mcp-preview)
```

**P84 insertion (RESEARCH §7) — three new entries between current #5 and #6:**
```markdown
5. `pmcp-server-toolkit` ...
6. `pmcp-toolkit-postgres` (depends on pmcp-server-toolkit + tokio-postgres + deadpool-postgres)
7. `pmcp-toolkit-mysql` (depends on pmcp-server-toolkit + sqlx)
8. `pmcp-toolkit-athena` (depends on pmcp-server-toolkit + aws-sdk-athena + aws-config)
9. `mcp-tester` ...
10. `mcp-preview` ...
11. `cargo-pmcp` ...
```

The three per-backend crates can publish in any order relative to each other (no inter-deps) but ALL must publish AFTER `pmcp-server-toolkit`.

---

### `crates/pmcp-server-toolkit/examples/sqlite_minimal.rs` (NEW)

**Analog:** `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` (exact role — Shape C ≤15-line `main.rs`).

**Pattern — `[[example]]` declaration in Cargo.toml** (from `pmcp-server-toolkit/Cargo.toml:66-68`):
```toml
[[example]]
name = "sqlite_minimal"
required-features = ["sqlite", "code-mode"]
```

**Main body shape — Shape C ≤15-line constraint enforced** (`e01_toolkit_minimal.rs:23-63`):
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG_TOML)?;
    let conn = Arc::new(SqliteConnector::open_in_memory()?);
    let _server = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version)
        .try_tools_from_config(&cfg, conn.clone())?   // signature includes connector per Plan tools.rs change
        .try_code_mode_from_config(&cfg)?
        .build()?;
    println!("sqlite_minimal: built");
    Ok(())
}
```

## Shared Patterns

### `#[non_exhaustive]` on extensible enums

**Source:** `crates/pmcp-server-toolkit/src/sql/mod.rs:92-93` (`Dialect`) + `src/sql/mod.rs:152-153` (`ConnectorError`).
**Apply to:** Every `ConnectorError` variant added in Phase 84.

```rust
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConnectorError {
    #[error("connector I/O error: {0}")]
    Io(String),
    // ... existing variants ...
}
```

Rationale: `#[non_exhaustive]` (RESEARCH "Established Patterns") permits Phase 84 to add `Driver` / `Query` / `Connection` / `ParameterBind` variants without semver break.

---

### `async_trait` on connector traits

**Source:** `crates/pmcp-server-toolkit/src/sql/mod.rs:31` (the toolkit) + spike `main.rs:31` (spike).
**Apply to:** All `SqlConnector` impls in every per-backend crate.

```rust
use async_trait::async_trait;

#[async_trait]
impl SqlConnector for PostgresConnector { /* ... */ }
```

---

### `#[serde(deny_unknown_fields)]` on config types

**Source:** `crates/pmcp-server-toolkit/src/config.rs:291`.
**Apply to:** ONLY if Phase 84 introduces new nested config structs. For the additive `url: Option<String>` field, no new struct is needed.

---

### Workspace-version dep pattern

**Source:** `crates/pmcp-server-toolkit/Cargo.toml:22-23`:
```toml
pmcp = { version = "2.8.1", path = "../..", default-features = false }
pmcp-code-mode = { version = "0.5.1", path = "../pmcp-code-mode", default-features = false, optional = true }
```
**Apply to:** Every new per-backend crate. Pin `pmcp-server-toolkit = { version = "0.1.0", path = "../pmcp-server-toolkit" }`.

---

### AWS SDK RUSTSEC-safe feature gate (mirror for Athena ONLY)

**Source:** `crates/pmcp-server-toolkit/Cargo.toml:42-48`.
**Apply to:** `crates/pmcp-toolkit-athena/Cargo.toml` ONLY (Pitfall #14).

```toml
# Why: default features enable `rustls` → ... rustls 0.21 / RUSTSEC-2026-0098/0099/0104.
# Opt into modern `default-https-client` (rustls 0.23 via aws-lc-rs) instead.
aws-sdk-athena = { version = "1.105.0", default-features = false, features = ["default-https-client", "rt-tokio", "behavior-version-latest"] }
aws-config = { version = "1.8.16", default-features = false, features = ["default-https-client", "rt-tokio", "credentials-process", "sso", "behavior-version-latest"] }
```

---

### Per-backend `tests/mock_*.rs` + `tests/integration.rs` layout

**Source:** Rust 2021 convention + spike 005 validation. Each per-backend crate:
- `tests/mock_<backend>.rs` — mock module (NOT a `#[test]`-flagged file; consumed via `mod mock_<backend>;`).
- `tests/integration.rs` — `mod mock_<backend>;` import + `#[tokio::test]` functions.

This is the Rust 2021 layout — every `*.rs` file directly in `tests/` is its own integration-test binary, BUT files that have no `#[test]` functions get pulled in via `mod` from sibling test files.

---

### Split-helper pattern for PMAT cog ≤25

**Source:** `crates/pmcp-server-toolkit/src/tools.rs:99-149` (`build_input_schema` / `build_param_property` / `build_annotations`) — Phase 75 PATTERNS §Pattern G applied in Phase 83.

**Apply to:** `translate_placeholders` decomposition (RESEARCH §2.3). The flat 5-state body exceeds cog 25; split into `SqlWalker::handle_normal` / `handle_string` / `handle_line_comment` / `handle_block_comment` / `emit_placeholder`. NO `#[allow(clippy::cognitive_complexity)]` — split first, allow second.

---

### ALWAYS-requirements coverage matrix (D-15)

**Source:** CLAUDE.md §"ALWAYS Requirements for New Features".
**Apply to:** Every new public type/function:

| Coverage | Where |
|----------|-------|
| Unit tests | `src/sql/translate.rs` `#[cfg(test)] mod tests` + per-backend `src/lib.rs` `#[cfg(test)]` |
| Property tests | `src/sql/translate.rs` proptest battery (5 invariants per RESEARCH §2.4) |
| Integration tests | Per-backend `tests/integration.rs` against mock or real (SQLite) |
| Doctests | Every `pub fn` / `pub struct` / `pub trait` |
| Example | Per-backend `examples/{backend}_minimal.rs` + `crates/pmcp-server-toolkit/examples/sqlite_minimal.rs` |
| Fuzz | Toolkit core `fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs` (extended automatically by the new `url` field) |

## No Analog Found

All 18 files have a close match in the codebase or the spike 005 reference. The planner does NOT need to fall back to RESEARCH-only patterns for any file.

## Metadata

**Analog search scope:**
- `crates/pmcp-server-toolkit/{src,tests,examples,fuzz}/`
- `crates/pmcp-code-mode/` + `crates/pmcp-widget-utils/` + `crates/mcp-tester/` (skeleton analogs)
- `src/server/core.rs` + `src/types/tools.rs` (pmcp core surface)
- `.planning/spikes/005-multi-dialect-sql-connector/` (VALIDATED reference impl)
- `Cargo.toml` (root) line 541
- `CLAUDE.md` lines 223-231

**Files scanned:** ~30 (toolkit src + tests + examples + spike 005 + pmcp core + analog crate roots).

**Key patterns identified:**
- All per-backend connectors share a single shape: `connect(url) -> Self`, `dialect() -> Dialect`, `execute(sql, &[(String, Value)]) -> Vec<Value>`, `schema_text() -> String`. Spike 005 is the authoritative blueprint.
- The synthesizer's wiring change is a single signature edit (`synthesize_from_config(cfg, conn)`) plus a single conditional `with_widget_meta` flip; no pmcp-core changes required (Option A, RESEARCH §4).
- The `ConnectorError` enum is already `#[non_exhaustive]` — Phase 84's new variants are additive with zero semver impact.
- The `translate_placeholders` decomposition follows Phase 83's `build_input_schema`/`build_param_property` split-helper pattern (Pattern G).
- AWS-SDK-Athena Cargo.toml MUST mirror `pmcp-server-toolkit/Cargo.toml:42-48` RUSTSEC-safe pattern verbatim — no `default-features`, opt into `default-https-client`.

**Pattern extraction date:** 2026-05-19
