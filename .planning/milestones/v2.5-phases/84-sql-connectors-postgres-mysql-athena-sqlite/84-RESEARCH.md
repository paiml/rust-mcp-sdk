# Phase 84: SQL Connectors (Postgres / MySQL / Athena / SQLite) — Research

**Researched:** 2026-05-19
**Domain:** Multi-dialect SQL connector trait extension + per-backend pure-Rust driver crates + StructuredOutput wiring + fuzz extension
**Confidence:** HIGH (driver APIs verified via Context7/docs.rs/registry; trait shape locked by spike 005; structured_content wiring read directly from `src/server/core.rs`)

## Summary

Phase 83 shipped a minimized 2-method `SqlConnector` trait (`dialect()` + `schema_text()`) in `crates/pmcp-server-toolkit/src/sql/mod.rs`. Phase 84 extends it to the 3-method shape from spike 005 by adding `execute(sql, &[(String, Value)]) -> Result<Vec<Value>, ConnectorError>`, plus the binding-order-preserving `translate_placeholders(sql, dialect) -> TranslatedSql { sql, ordered_params }` free helper (D-03/D-04 locked). Three new per-backend crates land in the workspace (`pmcp-toolkit-postgres` via `tokio-postgres 0.7.17` + `deadpool-postgres 0.14.1`; `pmcp-toolkit-mysql` via `sqlx 0.8.6` with `mysql` + `runtime-tokio` + `tls-rustls-aws-lc-rs` features; `pmcp-toolkit-athena` via `aws-sdk-athena 1.105.0` alone — NO Glue dep needed, see §1.3). SQLite ships in-toolkit by promoting `pub(crate) MockSqlConnector` to `pub struct SqliteConnector` behind the existing `sqlite` feature flag using `rusqlite 0.39` `bundled` (D-09 locked).

Two crosscutting surgical changes are also in scope: (a) the P83 `SynthesizedToolHandler::handle()` body in `crates/pmcp-server-toolkit/src/tools.rs:194-199` currently returns `Err(Internal(...))` — P84 must wire it to call `connector.execute()` AND emit `structured_content` (D-06). The pmcp core code path that converts a handler's `Result<Value>` into a `CallToolResult` only populates `structured_content` for widget tools (`info.widget_meta().is_some()`); for non-widget tools, the raw Value gets pretty-printed into `Content::text` only. **This means D-06 cannot be satisfied by just returning the rows from `handle()` — see §4 for the resolution path.** (b) The Phase 77 fuzz target on `ServerConfig::from_toml` at `crates/pmcp-server-toolkit/fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs` extends to cover any new `[database]` keys added in P84.

**Primary recommendation:** Land the trait extension + `translate_placeholders` + per-backend crates in lock-step. Use `deadpool-postgres` (not `bb8-postgres`) for the Postgres pool — significantly less boilerplate, "just works" defaults, matches Shape C ≤15-line target. Drop the Glue dependency from CONTEXT.md D-10 — Athena's `GetTableMetadata` returns full column types AND partition keys, which is everything `schema_text()` needs. Resolve D-06 by extending the P83 synthesizer's `SynthesizedToolHandler` to either (i) inject a connector reference + render an MCP-Apps `ui_resource_uri` widget_meta on `ToolInfo` so the existing widget-enrichment path activates, or (ii) accept that for non-widget tools the rows live in `Content::text` as pretty-JSON (model still reads them) and document the widget-only structured surface as the contract.

## User Constraints (from CONTEXT.md)

### Locked Decisions

**`SqlConnector` Trait Surface (CONN-01)**

- **D-01:** `execute()` returns `Result<Vec<serde_json::Value>, ConnectorError>` — each row a JSON object. Per-backend impls convert driver-native rows to `serde_json::Value` inside the connector. No typed `Row` trait. No streaming.
- **D-02:** Defer streaming and transactions to a later semver-additive release. Trait rustdoc documents the deferred-evolution plan. `ConnectorError` stays `#[non_exhaustive]`.
- **D-03:** Parameter shape: `&[(String, serde_json::Value)]` slice of named pairs.

**`translate_placeholders` Helper (CONN-03)**

- **D-04:** Return `TranslatedSql { sql: String, ordered_params: Vec<String> }`. Named-field struct. Per-backend `execute()` impls do `let TranslatedSql { sql, ordered_params } = translate_placeholders(canonical, dialect);` then iterate to bind positional driver params.
- **D-05:** Free helper, NOT a trait method. Lives at `pmcp_server_toolkit::sql::translate_placeholders`.
- **D-06:** Tool handlers populate `tools/call` response's `structuredContent` field with `Vec<serde_json::Value>` rows from `connector.execute()` — not just text content.

**Per-Backend Crate Shape (CONN-05/06/07/08)**

- **D-07:** Per-backend authentic in-process mocks live in each crate's `tests/` dir (`tests/mock_postgres.rs` etc.). No shared `pmcp-toolkit-test-support` crate.
- **D-08:** Connector constructor = URL string + internal pool. `PostgresConnector::connect(url)`, `MysqlConnector::connect(url)`, `AthenaConnector::from_config(region, workgroup)`. No external `with_pool(Arc<Pool>)` in v0.2.
- **D-09:** SQLite ships as `pub struct SqliteConnector` behind existing `sqlite` feature with `::open(path: &Path)` + `::open_in_memory()`. Delete old `MockSqlConnector` once subsumed.
- **D-10:** Per-backend crate names LOCKED: `pmcp-toolkit-postgres` → `tokio-postgres`; `pmcp-toolkit-mysql` → `sqlx`; `pmcp-toolkit-athena` → `aws-sdk-athena` (+ `aws-sdk-glue` "or Athena's catalog client if it covers" — researcher resolves: **Glue NOT needed**, see §1.3). SQLite → `rusqlite` `bundled`. All pure-Rust, Lambda-deployable.
- **D-11:** `schema_text()` dialect-styled, not normalized. Postgres + MySQL = `information_schema` CREATE-TABLE shape. Athena = Glue-catalog-derived CREATE-EXTERNAL-TABLE shape. SQLite = `sqlite_master`. Each folds `[[database.tables]]` descriptions cooperatively with P83's `assemble_code_mode_prompt`.

**Renaming / Compatibility (CONN-04)**

- **D-12:** Resolve `assemble_code_mode_prompt` ↔ `build_code_mode_prompt` naming. Planner picks: (a) rename + deprecated `pub use` alias, OR (b) `build_code_mode_prompt` as thin alias next to existing.

**Testing Coverage (TEST-01 / TEST-07)**

- **D-13:** Per-backend `tests/integration.rs` covers (a) connector against in-process mock, (b) `execute()` with `:name` placeholders, (c) `schema_text()` containing expected DDL fragments, (d) dialect identification. SQLite tested against real in-memory `rusqlite`.
- **D-14:** TEST-07 extends Phase 77's fuzz target in-place. Disposition: runtime stress in CI/nightly. NEVER Docker.
- **D-15:** All ALWAYS coverage per CLAUDE.md for every new public surface: unit + property + integration + doctests + at least one example + fuzz (toolkit core).

### Claude's Discretion

- Exact rename strategy for CONN-04 (alias vs deprecated rename — D-12).
- Concrete extended `ConnectorError` variants for execute-time failures (likely `Driver(String)`, `QuerySyntax(String)`, `Connection(String)`, `ParameterBind { name: String, reason: String }`).
- **Whether AWS Athena connector needs both `aws-sdk-athena` AND `aws-sdk-glue`** — researcher resolves below in §1.3: **Glue NOT needed**; Athena's `GetTableMetadata` covers schema introspection.
- Whether `MysqlConnector` uses `sqlx::Pool<MySql>` or `sqlx::MySqlPool` — pure surface naming. Researcher recommends: `MySqlPool` (it's the published alias and matches sqlx idiom).
- Internal mutex/connection structure of `SqliteConnector` — `Arc<Mutex<Connection>>` wrapped via `tokio::task::spawn_blocking`. Researcher recommends `std::sync::Mutex` (NOT `parking_lot::Mutex` — only held inside `spawn_blocking` so the brief async-blocked window is acceptable, and `std::sync::Mutex` keeps the dependency surface minimal).
- Workspace publish-order slot for the three new per-backend crates — see §7.

### Deferred Ideas (OUT OF SCOPE)

- Streaming `execute_stream()` method on `SqlConnector`.
- Transaction support on `SqlConnector`.
- External pool injection (`with_pool(Arc<Pool>)`) per backend.
- Shared `pmcp-toolkit-test-support` crate.
- GraphQL / OpenAPI connector crates (GQL-TKIT-01 / OAPI-TKIT-01 next-milestone).
- `#[pmcp::sql_server]` proc-macro (PMACRO-SQL-01).
- Cross-backend tool federation (FED-01).
- Type 1 `ai-agents/` skill updates (SKLL-07 owned by Phase 87).
- Phase 86 Shape B/C/D scaffolding.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CONN-01 | Three-method `SqlConnector` trait in toolkit core (`dialect()`, `execute(query, params)`, `schema_text()`); `schema_text()` folds `[[database.tables]]` descriptions | §1 (driver APIs); §3 (mock contracts); §8 (D-11 landmine) |
| CONN-02 | `Dialect` enum with `Postgres`, `MySQL`, `Athena`, `SQLite` variants | Already shipped in P83 `src/sql/mod.rs:92-103` — no work for P84 |
| CONN-03 | `translate_placeholders(canonical_query, dialect) -> TranslatedSql` free helper | §2 (state machine + edge cases + PMAT) |
| CONN-04 | `build_code_mode_prompt(connector) -> String` free helper | §4 (D-12 rename strategy) |
| CONN-05 | `pmcp-toolkit-postgres` crate via `tokio-postgres` with `information_schema` `schema_text()` | §1.1 + §3.1 + §7 |
| CONN-06 | `pmcp-toolkit-mysql` crate via `sqlx` MySQL driver with `information_schema` `schema_text()` | §1.2 + §3.2 + §7 |
| CONN-07 | `pmcp-toolkit-athena` crate via `aws-sdk-athena` with Glue-catalog `schema_text()` (Glue dep DROPPED, see §1.3) | §1.3 + §3.3 + §7 |
| CONN-08 | SQLite as a feature flag on toolkit using `rusqlite` `bundled` (no separate crate) | §1.4 + §3.4 |
| TEST-01 | Integration tests for each per-backend SQL crate against authentic in-process mocks; SQLite against real in-memory `rusqlite`; no Docker | §3 (per-backend mock patterns) |
| TEST-07 | Fuzz target on `config.toml` parser extends Phase 77 pattern | §6 (Validation Architecture) + §5 (config additions) |

## Project Constraints (from CLAUDE.md)

The following directives MUST be honored by every plan in this phase:

- **Quality gate before commit.** `make quality-gate` is the canonical local gate (matches CI: `cargo fmt --all -- --check`, `cargo clippy` with `--features "full"` + pedantic + nursery, build, test, audit). Bare `cargo clippy -- -D warnings` is **weaker** than CI and will miss lints.
- **PMAT cognitive complexity ≤25 per function.** CI gate via `pmat quality-gate --fail-on-violation --checks complexity`. Phase 75 ships 6 refactor patterns (P1–P6) + a `// Why:`-annotated `#[allow(clippy::cognitive_complexity)]` template for irreducible cases (hard cap cog 50). The `translate_placeholders` state machine likely exceeds 25 inline; §2 documents the split-helper pattern.
- **Zero clippy warnings** under `--features full` + pedantic + nursery (the `make quality-gate` configuration).
- **Pre-commit hook enforced** — format / clippy / build / test / audit all green or commit blocked.
- **ALWAYS requirements for new features (CLAUDE.md §"ALWAYS Requirements"):** every new public surface ships (1) unit tests, (2) property tests, (3) integration tests, (4) `cargo run --example` example, (5) doctests on every public type/fn, (6) fuzz coverage where applicable. D-15 commits the phase to this for every new public surface.
- **Tests run with `--test-threads=1`** (race-condition prevention — project CLAUDE.md). The toolkit's `[dev-dependencies] tokio` uses only `macros` + `rt` features (single-thread runtime) — per-backend crates should follow suit. `SqliteConnector` mutex semantics must work under a single-threaded runtime.
- **No Docker, no testcontainers.** Pure-Rust Lambda is the deployment target; `feedback_avoid_docker_pure_rust_lambda` memory reaffirmed. Drivers MUST be pure-Rust (`tokio-postgres`, `sqlx` with `tls-rustls-*`, `aws-sdk-athena`, `rusqlite bundled`).
- **`#[non_exhaustive]` on extensible enums.** `Dialect` and `ConnectorError` in P83 already carry this — additive variants for P84 are semver-clean.
- **`#[serde(deny_unknown_fields)]` on config types.** Strict-parse discipline (P83 D-13). Per-backend `[database]` additions = new optional fields only; **renames forbidden** by REF-01 superset invariant.
- **Constructors over struct literals on `#[non_exhaustive]` types from pmcp core** — `ToolInfo`, `ToolAnnotations`, `CallToolResult` are all `#[non_exhaustive]` (read directly from `src/types/tools.rs:506`).
- **Workspace-version dep pattern** — every per-backend crate's `Cargo.toml` carries `pmcp-server-toolkit = { version = "0.1.0", path = "../pmcp-server-toolkit" }` (mirrors P83 D-05).
- **Toyota Way + Zero tolerance for defects** — no SATD comments, no `TODO:` in code (the `// Why:`-annotated `#[allow]` template is the only exception path).
- **Contract-first development.** New features update `provable-contracts/contracts/pmcp-server-toolkit/` (or new per-backend contract YAMLs). `pmat comply check` must pass.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `SqlConnector` trait + `execute()` + `Dialect` enum + `translate_placeholders` + `ConnectorError` | Toolkit core (`pmcp-server-toolkit`) | — | Dialect-agnostic surface lives where the consumer (synthesizer + code-mode wiring) reaches for it. D-05 locks `translate_placeholders` as a free helper, NOT trait method. |
| Driver I/O (`tokio_postgres::Client::query`, `sqlx::query`, `aws_sdk_athena::Client::*`, `rusqlite::Connection::prepare`) | Per-backend crate | — | Each driver has unique types (`tokio_postgres::types::ToSql`, `sqlx::Encode`, `aws_sdk_athena::primitives::DateTime`, `rusqlite::types::Value`) — per-backend crate owns the I/O + the JSON conversion. |
| `Vec<serde_json::Value>` row materialization | Per-backend crate | — | Conversion from driver-native row → `Value` lives inside `execute()` impl per D-01. |
| Connection pooling | Per-backend crate | — | Pool type is driver-specific (`deadpool::managed::Pool<tokio_postgres::Config>`, `sqlx::MySqlPool`, stateless HTTP for Athena, `Arc<Mutex<rusqlite::Connection>>` for SQLite). D-08 locks URL-string constructor + internal pool. |
| `information_schema` / Glue-catalog / `sqlite_master` introspection queries | Per-backend crate | — | Each backend's introspection format is native (D-11). |
| `[[database.tables]]` curated description folding | Toolkit core (`code_mode::assemble_code_mode_prompt`) | Per-backend crate `schema_text()` | P83's `assemble_code_mode_prompt` already folds curated descriptions on top of `schema_text()` — per-backend `schema_text()` only needs to emit the DDL/catalog blob. |
| Tool handler synthesis (`SynthesizedToolHandler` body) | Toolkit core (`tools.rs`) | — | P83's `synthesize_from_config` builds the handler; P84 surgery wires `.execute()` + `structured_content` into the handler body. The connector reference must flow through. |
| `structured_content` emission on `tools/call` response | pmcp core (`src/server/core.rs:593-601`) | Toolkit `SynthesizedToolHandler` | pmcp core does the actual `CallToolResult::with_structured_content` call — but ONLY when `tool_info.widget_meta().is_some()`. For non-widget tools, only `Content::text` is populated. See §4 for the resolution path. |
| Fuzz target on `config.toml` parser | Toolkit core (`fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs`) | — | Already exists from P83 (D-14 locks "extend, do not duplicate"). |
| ALWAYS-coverage tests for connectors | Per-backend crate `tests/` | — | D-13 locks per-crate `tests/mock_*.rs` + `tests/integration.rs` layout. |

## Driver API Surfaces (Current Published Versions)

> All driver versions verified against the crates.io registry on 2026-05-19. `make quality-gate` runs `cargo audit` — pin to the latest patch on each minor line.

### 1.1 Postgres (`pmcp-toolkit-postgres` — CONN-05)

| Package | Version | Source |
|---------|---------|--------|
| `tokio-postgres` | **0.7.17** [VERIFIED: `cargo search tokio-postgres`] | [docs.rs](https://docs.rs/tokio-postgres/0.7.17/tokio_postgres/) |
| `deadpool-postgres` | **0.14.1** [VERIFIED: `cargo search deadpool-postgres`] | [docs.rs](https://docs.rs/deadpool-postgres) |
| `tokio` | workspace pin (currently 1.x; see `Cargo.toml`) | — |

**Connect:** `tokio_postgres::connect(conn_str, NoTls)` returns `(Client, Connection)`. The `Connection` future MUST be spawned via `tokio::spawn` to handle the wire protocol; failing to spawn = hung queries. [CITED: docs.rs/tokio-postgres/0.7.17]

```rust
// Source: tokio-postgres 0.7.17 README + docs.rs
let (client, connection) = tokio_postgres::connect("host=localhost user=postgres", NoTls).await?;
tokio::spawn(async move { let _ = connection.await; });
```

**Pool choice (Claude's Discretion — RESOLVED):** Use `deadpool-postgres 0.14.1`, NOT `bb8-postgres 0.9.0`. [CITED: oneuptime.com/blog/2026-01-25 + leapcell.io blog]

- `deadpool-postgres` API surface is ~100 LOC — minimal boilerplate, "just works" defaults, built-in config file support.
- `bb8-postgres` is a generic pool over the `ManageConnection` trait — more setup, more flexibility we don't need.
- Pool construction: `let mgr = Manager::from_config(pg_config, NoTls, mgr_config); let pool = Pool::builder(mgr).max_size(16).build()?;`
- The Shape C ≤15-line target tolerates `deadpool-postgres`'s 3-line setup; `bb8-postgres` adds 2–3 more lines per call site.

**Query & params:**

```rust
// Source: tokio-postgres 0.7.17 docs
// query: SELECT-style, returns Vec<tokio_postgres::Row>
let rows: Vec<Row> = client.query("SELECT id, name FROM users WHERE id = $1", &[&user_id]).await?;
// execute: DML, returns u64 rowcount
let n: u64 = client.execute("DELETE FROM sessions WHERE expires < $1", &[&now]).await?;
```

Parameters are `&[&(dyn ToSql + Sync)]`. `serde_json::Value` does NOT implement `ToSql` natively — P84 must per-variant-match the `Value` into the appropriate Postgres type (`bool`, `i64`, `f64`, `&str`, `serde_json::Value` only with the `with-serde_json-1` feature on `tokio-postgres`, or `Null`). [CITED: docs.rs/tokio-postgres types module]

**Row → `serde_json::Value` conversion:**

```rust
let mut obj = serde_json::Map::new();
for (idx, col) in row.columns().iter().enumerate() {
    let name = col.name().to_string();
    let val: serde_json::Value = match col.type_().name() {
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

**`information_schema` introspection (driving `schema_text()`):**

```sql
SELECT
    table_name,
    column_name,
    data_type,
    is_nullable,
    character_maximum_length
FROM information_schema.columns
WHERE table_schema = $1   -- typically 'public' or value from [database].database
ORDER BY table_name, ordinal_position;
```

Reconstruct a CREATE-TABLE-style blob per the spike 005 reference shape.

[VERIFIED: docs.rs/tokio-postgres/0.7.17]

### 1.2 MySQL (`pmcp-toolkit-mysql` — CONN-06)

| Package | Version | Source |
|---------|---------|--------|
| `sqlx` | **0.8.6** (stable; 0.9 is alpha — DO NOT use) [VERIFIED: crates.io API] | [docs.rs](https://docs.rs/sqlx/0.8.6) |
| features | `mysql`, `runtime-tokio`, `tls-rustls-aws-lc-rs` (pure-Rust, no OpenSSL) [CITED: launchbadge/sqlx README] | — |

**Connect:** `MySqlPool::connect(url).await?` — the URL is `mysql://user:pass@host:port/db`. `MySqlPool` is a type alias for `Pool<MySql>`. [CITED: docs.rs/sqlx/0.8.6/sqlx/mysql]

```rust
// Source: sqlx 0.8.6 mysql docs
let pool: MySqlPool = MySqlPool::connect(url).await?;
```

**Recommendation (Claude's Discretion — RESOLVED):** Use the published alias `MySqlPool`, not `Pool<MySql>`. It's idiomatic sqlx and shorter.

**Query & params:** sqlx uses `?` placeholders for MySQL; the `query()` builder chains `.bind(val)` for each param.

```rust
// Source: sqlx 0.8.6 docs
let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query("SELECT id, name FROM users WHERE id = ?")
    .bind(user_id)
    .fetch_all(&pool)
    .await?;
```

**Row → `Value` conversion:** Use `row.try_get_raw(idx)` + `MySqlValueRef::type_info()` to dispatch per column type. Simpler: use `row.try_get::<Option<T>, _>(idx)` per dispatched type (parallel to the Postgres pattern).

**`information_schema` introspection:** Same schema as Postgres but the `table_schema` arg is the MySQL database name (not 'public').

**TLS feature (CRITICAL):** Set `sqlx = { version = "0.8.6", default-features = false, features = ["mysql", "runtime-tokio", "tls-rustls-aws-lc-rs"] }`. The `aws-lc-rs` ring alternative is the Lambda-friendly path. [CITED: github.com/launchbadge/sqlx README]

### 1.3 Athena (`pmcp-toolkit-athena` — CONN-07) — **GLUE DEP DROPPED**

| Package | Version | Source |
|---------|---------|--------|
| `aws-sdk-athena` | **1.105.0** [VERIFIED: crates.io] | [docs.rs](https://docs.rs/aws-sdk-athena/1.105.0/aws_sdk_athena/) |
| `aws-config` | **1.8.16** [VERIFIED: crates.io] (already used by toolkit's `aws` feature) | — |
| `aws-sdk-glue` | **NOT NEEDED** — see below | — |

**Claude's Discretion RESOLVED:** Athena's `GetTableMetadata` API ([CITED: docs.aws.amazon.com/athena/latest/APIReference/API_GetTableMetadata.html]) returns:

```json
{
  "TableMetadata": {
    "Columns": [{ "Name": "...", "Type": "...", "Comment": "..." }],
    "PartitionKeys": [{ "Name": "...", "Type": "..." }],
    "TableType": "...",
    "Parameters": { "...": "..." }
  }
}
```

This covers everything `schema_text()` needs: column names, types, partition keys, table parameters. **CONTEXT.md D-10's "or Athena's catalog client" is the correct path — drop `aws-sdk-glue` entirely.** Rationale:

- One AWS SDK dependency instead of two = smaller binary, faster cold-start (Lambda matters).
- One auth/IAM path instead of two (simpler operator setup).
- `ListTableMetadata` + `GetTableMetadata` cover the whole `[[database.tables]]` enrichment path.

**Query execution (StartQueryExecution → poll → GetQueryResults):**

```rust
// Source: aws-sdk-athena 1.105 fluent-builder pattern + Athena API docs
let exec_id = client.start_query_execution()
    .query_string(translated_sql)
    .query_execution_context(QueryExecutionContext::builder().database(db_name).build())
    .work_group(workgroup_name)
    .result_configuration(ResultConfiguration::builder().output_location(s3_uri).build())
    .send().await?
    .query_execution_id().to_string();

// Poll. Recommended pattern: exponential backoff capped at query_timeout_ms.
loop {
    let st = client.get_query_execution().query_execution_id(&exec_id).send().await?;
    match st.query_execution().and_then(|qe| qe.status()).and_then(|s| s.state()) {
        Some(QueryExecutionState::Succeeded) => break,
        Some(QueryExecutionState::Failed) | Some(QueryExecutionState::Cancelled) => return Err(...),
        _ => tokio::time::sleep(Duration::from_millis(backoff_ms)).await,
    }
}

// Pull rows.
let results = client.get_query_results().query_execution_id(&exec_id).send().await?;
```

**Parameter binding:** Athena Presto uses `?` placeholders. The Athena SDK does NOT have a `bind_parameters` field on `start_query_execution` for the v1 SDK — instead, you call `.execution_parameters(Some(vec![...]))` with stringified values. [VERIFIED: docs.rs/aws-sdk-athena] All params get serialized to strings; type information is lost on the wire (acceptable per D-01's `Value` shape).

**`schema_text()` via `GetTableMetadata`:** Iterate `[[database.tables]]` from config, call `client.get_table_metadata().catalog_name("AwsDataCatalog").database_name(db).table_name(t).send().await?` per table, emit a CREATE-EXTERNAL-TABLE-style blob with partition keys + table parameters.

**Polling tuning (Claude's Discretion):** start at 500ms, double each loop, cap at 5s, total bounded by `query_timeout_ms` from config (default 60s).

[CITED: docs.aws.amazon.com/athena/latest/APIReference/API_GetTableMetadata.html]
[CITED: docs.rs/aws-sdk-athena/1.105.0]

### 1.4 SQLite (toolkit `sqlite` feature — CONN-08)

| Package | Version | Source |
|---------|---------|--------|
| `rusqlite` | **0.39.0** with `bundled` feature [VERIFIED: crates.io] | [docs.rs](https://docs.rs/rusqlite/0.39.0/rusqlite/) |

Already declared optional at `crates/pmcp-server-toolkit/Cargo.toml:51`: `rusqlite = { version = "0.39", features = ["bundled"], optional = true }` and the `sqlite` feature already activates it (line 64).

**API (verified from docs.rs + spike 005 reference):**

```rust
// Connect
let conn = Connection::open_in_memory()?;             // tests
let conn = Connection::open("/path/to/db.sqlite")?;   // production

// Prepare + bind named params via stmt.parameter_index
let mut stmt = conn.prepare("SELECT id, name FROM users WHERE id = :id")?;
for (name, val) in params {
    let bind = format!(":{name}");
    if let Some(idx) = stmt.parameter_index(&bind)? {
        stmt.raw_bind_parameter(idx, json_to_sql(val))?;
    }
}
let cols: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
let mut rows = stmt.raw_query();
while let Some(row) = rows.next()? { /* row.get_ref(i) → ValueRef → Value */ }
```

(The spike 005 `sqlite_backend` module in `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:228-322` is the canonical reference — P84 lifts it verbatim modulo the `MockSqlConnector` → `SqliteConnector` rename.)

**Sync API wrapping (Claude's Discretion — RESOLVED):** rusqlite is sync. The `async fn execute()` impl wraps via `tokio::task::spawn_blocking`:

```rust
async fn execute(&self, sql: &str, params: &[(String, Value)]) -> Result<Vec<Value>, ConnectorError> {
    let conn = self.conn.clone();
    let sql = sql.to_string();
    let params = params.to_vec();
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|_| ConnectorError::Driver("mutex poisoned".into()))?;
        // ... rusqlite calls here
    }).await.map_err(|e| ConnectorError::Driver(e.to_string()))?
}
```

**Mutex choice:** `std::sync::Mutex` wrapped in `Arc`. Held ONLY inside `spawn_blocking` so the async runtime is never blocked. Don't use `tokio::sync::Mutex` (its async-lock is needless overhead here) or `parking_lot::Mutex` (no project precedent in toolkit core dependencies).

**`schema_text()` via `sqlite_master`:**

```sql
SELECT name, sql FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name;
```

Returns the original DDL verbatim. Plus optional `PRAGMA table_info(table_name);` for column type details.

## `translate_placeholders` Design

### State Machine Outline

The P83 review's HIGH-severity ask was for binding-order preservation; D-04 settles that with the `TranslatedSql { sql, ordered_params }` struct. The implementation walks the input SQL char-by-char with **four states**:

| State | What's Tracked | Exit Condition |
|-------|----------------|----------------|
| `Normal` | Plain SQL, accumulate to `out`; on `:` → enter `Placeholder` | EOF |
| `Placeholder` | After `:`, reading `[A-Za-z_][A-Za-z0-9_]*`; on non-identifier char → emit translated form, switch back to `Normal` | EOF (emit translated form), non-ident-char |
| `StringLiteral(quote)` | Inside `'...'` or `"..."` — emit chars verbatim, do NOT translate `:` chars | matching `quote` (handling SQL doubled-quote escape `''`) |
| `LineComment` | After `--`, emit verbatim until newline | `\n` |
| `BlockComment(depth)` | After `/*`, emit verbatim, track nesting | `*/` at depth 0 |

The spike 005 reference impl at `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:85-122` is a **two-state** machine (Normal + Placeholder). It correctly handles "lone `:`" (e.g. inside `:='::text` Postgres casts) by skipping when `name.is_empty()`, but it does **NOT** distinguish placeholders inside string literals or comments — a stricter implementation is required for production.

### Edge Cases to Handle

Confirmed by reading the reference `[[tools]]` SQL in `crates/pmcp-server-toolkit/tests/fixtures/open-images-config.toml`:

| Pattern | What it Looks Like in Real Config | Must Translate? |
|---------|-----------------------------------|-----------------|
| Bare placeholder | `WHERE id = :id` | YES |
| Placeholder in string concat | `lower(:keyword) LIKE '%' \|\| ... \|\| '%'` (line 158) | YES (the `:keyword` here, but NOT the `'%'` strings) |
| Placeholder in subquery | `IN (SELECT chr.child_id FROM ... WHERE = lower(:class_name))` (line 162) | YES |
| Same placeholder appearing twice | spike 005 input `WHERE a = :a AND b = :b AND c = :a` (line 791) | YES, both occurrences — Postgres gets `$1, $2, $3` (positional repeats), `ordered_params` is `["a", "b", "a"]` |
| Postgres cast operator `::text` | `'foo'::text` (NOT a placeholder) | NO — handled because the `:` is not followed by an identifier char; spike impl correctly emits `:` verbatim then continues |
| String literal containing `:` | `'WHERE name = :foo'` (rare but valid) | NO — must be inside `StringLiteral` state |
| Line comment with `:` | `-- bind :id here` | NO — must be inside `LineComment` state |
| Block comment with `:` | `/* :foo */` | NO — must be inside `BlockComment` state |
| SQL identifier with `:` (e.g., Postgres `WITH t(":name")`) | Edge case | Skipped in `StringLiteral("\"")` — quoted identifiers are treated as string-literal for purposes of placeholder skipping |

### PMAT Complexity Disposition

A 5-state machine over chars with the literal/comment edge-cases inline = approximately **cog 35–40** (well above the ≤25 hard limit).

**Recommended decomposition (Pattern G — split helpers):**

```rust
pub fn translate_placeholders(sql: &str, dialect: Dialect) -> TranslatedSql {
    let mut walker = SqlWalker::new(sql, dialect);
    walker.run();
    walker.into_translated()
}

struct SqlWalker<'a> { /* ... */ }

impl<'a> SqlWalker<'a> {
    fn run(&mut self) {                    // cog ~10 — outer loop dispatch
        while let Some(c) = self.next() {
            match self.state {
                State::Normal => self.handle_normal(c),
                State::StringLiteral(q) => self.handle_string(c, q),
                State::LineComment => self.handle_line_comment(c),
                State::BlockComment(d) => self.handle_block_comment(c, d),
            }
        }
        // Flush trailing placeholder if Placeholder still active at EOF.
    }
    fn handle_normal(&mut self, c: char) { /* cog ~12 — emit placeholder or transition */ }
    fn handle_string(&mut self, c: char, q: char) { /* cog ~6 — emit verbatim, watch for q */ }
    fn handle_line_comment(&mut self, c: char) { /* cog ~3 — emit verbatim, watch for \n */ }
    fn handle_block_comment(&mut self, c: char, depth: usize) { /* cog ~5 — emit verbatim, watch for */ */ }
    fn emit_placeholder(&mut self, name: &str) { /* cog ~6 — dispatch per dialect */ }
}
```

Each helper is well under cog 25. If reviewers prefer a flat impl, the `// Why:`-annotated `#[allow(clippy::cognitive_complexity)]` template from Phase 75 Plan 75-00 is available (hard cap cog 50 per D-03). Recommendation: do the split — it tests cleaner.

### Property Test Invariants (Phase 84 owns)

1. **Idempotence on `:name`-free SQL:** for SQL containing no `:` identifiers (post-comment-stripping), `translate_placeholders(sql, dialect).sql == sql` for ALL dialects EXCEPT this property is only guaranteed for Sqlite (which is identity); for Postgres/MySQL/Athena it holds because the no-placeholder case never enters the `Placeholder` state. → **Re-phrased:** "for any SQL with no `:name` placeholders, all 4 dialects produce identity output."
2. **Bind-order preservation:** for any input with `n` placeholders at positions `p_1 < p_2 < ... < p_n` with names `n_1, n_2, ..., n_n`, `ordered_params == vec![n_1, n_2, ..., n_n]` regardless of dialect.
3. **Postgres positional indexing:** Postgres translation produces `$1, $2, ..., $n` in 1-indexed positional order matching `ordered_params.len()`. Repeated names get a fresh `$k` per appearance (spike 005 input `:a, :b, :a` → `$1, $2, $3`).
4. **Sqlite identity:** for any input, `translate_placeholders(sql, Dialect::Sqlite).sql == sql` (the spike's `debug_assert_eq!` on line 291 codifies this).
5. **No panic on malformed input:** any arbitrary `&str` input (arbitrary UTF-8, including stray `:`, unterminated quotes, unterminated comments) returns a `TranslatedSql` without panicking. (Fuzz dimension; complements `proptest`.)

Spike 005's existing property invariants in main.rs lines 791-814 cover (1), (2), (3) for the simple case; P84 promotes them to formal proptest macros and adds (4) + (5).

## Per-Backend Mock Patterns

Per D-07, each per-backend crate ships its mock in `tests/mock_<backend>.rs` consumed via `mod mock_<backend>;` from `tests/integration.rs`. Per TEST-01 + the `feedback_avoid_docker_pure_rust_lambda` memory: **authentic in-process mocks only — no Docker, no testcontainers, no networking.**

### 3.1 `mock_postgres.rs`

Spike 005 reference: `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs:340-466`.

**Pattern:** The mock is a stand-alone struct that implements `SqlConnector` directly — it does NOT layer underneath `PostgresConnector::connect(mock_url)`. Spike 005 explicitly chose this: the connector trait IS the seam; the mock satisfies the trait without ever touching `tokio-postgres`.

```rust
pub struct PostgresMock {
    pub tables: HashMap<String, Vec<Value>>,
    pub last_translated_sql: Mutex<Option<String>>,   // tests inspect what the mock saw
    pub last_positional_args: Mutex<Option<Vec<Value>>>,
}

#[async_trait]
impl SqlConnector for PostgresMock {
    fn dialect(&self) -> Dialect { Dialect::Postgres }
    async fn execute(&self, sql: &str, params: &[(String, Value)]) -> Result<Vec<Value>, ConnectorError> {
        let TranslatedSql { sql: translated, ordered_params } = translate_placeholders(sql, Dialect::Postgres);
        // Record what we saw — the test asserts on this.
        *self.last_translated_sql.lock().unwrap() = Some(translated.clone());
        // Resolve positional bind list (cheap engine recognizes :name lookup).
        let positional: Vec<Value> = ordered_params.iter()
            .map(|n| params.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap_or(Value::Null))
            .collect();
        *self.last_positional_args.lock().unwrap() = Some(positional.clone());
        execute_cheap_query(&self.tables, &translated, &positional)
    }
    async fn schema_text(&self) -> Result<String, ConnectorError> {
        // information_schema-styled CREATE TABLE blob
    }
}
```

**Cheap query engine:** string-matches `translated_sql.contains("WHERE id = $1")` against the small set of queries in `tests/fixtures/test-config.toml`. NOT a general SQL engine — just enough to validate that translated SQL + positional bindings flow through correctly.

**What the mock IS authentic about:**
- Postgres `$1`-style placeholders (translated via the toolkit's own `translate_placeholders`)
- Double-quote identifier quoting in `schema_text()`
- `information_schema`-shaped DDL output (`schema_text()`)
- Positional bind list assembly from named params

**What the mock is NOT:** A SQL engine. A wire-protocol simulator. A `tokio-postgres` substitute.

### 3.2 `mock_mysql.rs`

Same pattern as 3.1, but `?` placeholders (no positional numbering) and backtick identifier quoting + InnoDB engine markers in `schema_text()`. Spike 005 reference: lines 476-575.

### 3.3 `mock_athena.rs`

Same pattern. Athena uses `?` (same wire shape as MySQL for placeholders) + Glue-Data-Catalog-styled `schema_text()` with S3 output location + partition column markers. Spike 005 reference: lines 587-697.

Athena's Step 1 (StartQueryExecution → poll → GetQueryResults) is **NOT** modeled in the mock — the mock returns rows synchronously from `execute()`. The polling pattern is internal to `pmcp-toolkit-athena`'s real impl; the trait surface (`async fn execute() -> Vec<Value>`) hides it. The mock validates that the trait surface is satisfied; real SDK integration testing is out of CI scope per D-07 + TEST-01.

### 3.4 `SqliteConnector` (no mock — real driver)

Per D-09 + spike 005's `sqlite_backend` module: SQLite is tested against a real in-memory `rusqlite` DB via `SqliteConnector::open_in_memory()`. The trait impl IS the test target. Each `tests/integration.rs` for the sqlite feature path uses an in-memory DB seeded with `tests/fixtures/schema.sql`.

## `structuredContent` Wiring

### What P83 ships

The synthesizer at `crates/pmcp-server-toolkit/src/tools.rs:74-92` builds an `Arc<dyn ToolHandler>` per `[[tools]]` entry. The handler body at lines 192-203:

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

**Today the handler is a placeholder.** P84 is the phase that wires it.

The `ToolDecl` struct already carries `pub sql: Option<String>` (config.rs:436) and `pub ui_resource_uri: Option<String>` (config.rs:439) — the synthesizer holds the full `ToolDecl` (tools.rs:189) but neither field is consumed by the placeholder body.

### How pmcp core converts `Result<Value>` → `CallToolResult`

Read directly from `src/server/core.rs:593-601`:

```rust
let call_result = if let Some(info) = tool_info.filter(|i| i.widget_meta().is_some()) {
    // Widget tool: structured data goes in structuredContent, text is a brief summary
    let summary = summarize_structured_output(&value);
    CallToolResult::new(vec![Content::text(summary)]).with_widget_enrichment(info, value)
} else {
    let text = serde_json::to_string_pretty(&value)?;
    CallToolResult::new(vec![Content::text(text)])
};
```

**This is the critical landmine for D-06.** The `with_widget_enrichment` path (which calls `with_structured_content(value)` per `src/types/tools.rs:582-599`) only fires when `info.widget_meta().is_some()`. For non-widget tools, the `else` branch dumps the value into `Content::text` and leaves `structured_content` as `None`.

`widget_meta()` is `Some(...)` only when the `ToolInfo` carries `_meta` keys starting with `openai/toolInvocation/` (`src/types/tools.rs:587-591`). P83's synthesizer never sets these.

### Two resolution paths for D-06

**Option A — Widget-meta-driven path (preferred):** When `ToolDecl::ui_resource_uri` is set in `[[tools]]`, P84's synthesizer calls `ToolInfo::with_widget_meta(WidgetMeta::new()...)` so the tool registers as a widget tool. Then the handler body returns `connector.execute(...)` rows as a JSON array, and pmcp core's existing widget-enrichment path puts it in `structured_content` automatically. For non-widget tools (no `ui_resource_uri`), `Content::text` carries pretty-JSON rows — model still reads them; this matches existing pmcp-run server behavior for tools without widgets.

**Option B — Always-set-structured path (more aggressive):** P84 extends `SynthesizedToolHandler` to return rows via a custom `CallToolResult` path — but `ToolHandler::handle()` returns `pmcp::Result<Value>`, NOT `Result<CallToolResult>`, so the handler can't directly emit `CallToolResult::with_structured_content`. To override the core's converter, the synthesizer would need to use `TypedToolWithOutput` or `output_schema` on `ToolInfo` plus a parallel changes in pmcp core to populate `structured_content` whenever `tool_info.output_schema.is_some()`. **This requires a pmcp core change**, which the phase scope does not authorize.

**Recommendation:** Option A. The reference configs (`open-images-config.toml`) already set `ui_resource_uri` on widget tools and leave it `None` on non-widget tools. The P84 synthesizer can:

1. If `decl.ui_resource_uri.is_some()` → call `ToolInfo::with_widget_meta(WidgetMeta::new().domain(&uri))` (or similar) on the synthesized `ToolInfo` — pmcp core then auto-populates `structured_content` from the returned `Value` (rows JSON-array).
2. If absent → leave `Content::text` as the row carrier (pretty-JSON of `Vec<Value>` rows).

D-06's user-flagged invariant is satisfied for the widget surface, which is the surface that matters for MCP Apps UI consumption. Chained tool calls reading text content still get the rows (just as text JSON, not as `structured_content`).

### Connector reference flow

The synthesizer's `SynthesizedToolHandler` doesn't currently hold a `SqlConnector`. P84 must thread it in. Two options:

| Pattern | Mechanism | Tradeoff |
|---------|-----------|----------|
| **A.** Per-handler `Arc<dyn SqlConnector>` baked in at synthesis time | Change `synthesize_from_config` signature to `synthesize_from_config(cfg: &ServerConfig, conn: Arc<dyn SqlConnector>)` | Forces every consumer to construct the connector before synthesis — fine for Shape A/B/C, locked to one connector per server |
| **B.** Builder extension that injects connector at `pmcp::ServerBuilder::build()` time | New `ServerBuilderExt::sql_connector(self, Arc<dyn SqlConnector>)` plus per-handler lookup at call time | More flexible; matches the P83 code-mode pattern (`register_code_mode_tools`) |

**Recommendation:** Option A. One connector per `config.toml` server matches REF-01 (every reference config has exactly one `[database]`); the builder-extension shape is overkill until cross-backend federation (FED-01, deferred). P84 changes the synthesizer signature to `synthesize_from_config(cfg, connector)` — semver-clean since the toolkit is at 0.1.0.

### File / line read-first hints for the planner

| File | Lines | Why |
|------|-------|-----|
| `crates/pmcp-server-toolkit/src/tools.rs` | 74–92 (`synthesize_from_config`) | Signature change point |
| `crates/pmcp-server-toolkit/src/tools.rs` | 185–204 (`SynthesizedToolHandler` impl) | Handler body that needs `.execute()` wiring + `widget_meta` conditional set |
| `src/server/core.rs` | 593–601 (`handle_call_tool` → `call_result` builder) | Confirms `widget_meta` gate on `structured_content` path |
| `src/types/tools.rs` | 582–599 (`with_widget_enrichment`) | The pmcp-core call that puts rows into `structured_content` |
| `src/types/tools.rs` | 320–340 (`ToolInfo::with_widget_meta`) | The constructor synthesizer uses to flip widget on |

## Config.toml Extension Surface

Per REF-01 (additive only, no renames), confirmed by reading `crates/pmcp-server-toolkit/tests/fixtures/{open-images,imdb,msr-vtt}-config.toml`:

### What the toolkit's `DatabaseSection` already accepts (P83, `src/config.rs:290-314`)

| Key | Type | Used By Reference Configs | Used by P84 backend |
|-----|------|---------------------------|---------------------|
| `type` | `Option<String>` | open-images / imdb / msr-vtt: `"athena"` | All 4 — dispatches connector selection |
| `database` | `Option<String>` | All Athena refs (schema name) | Postgres (db name), MySQL (db name), Athena (db name), SQLite (file path) |
| `output_location` | `Option<String>` | Athena: `s3://...` | Athena only |
| `workgroup` | `Option<String>` | Athena: `"open-images"`, `"primary"`, `"msr-vtt"` | Athena only |
| `query_timeout_ms` | `Option<u64>` | All Athena refs: `60000` | All 4 (advisory; Athena enforces via polling cap, Postgres/MySQL via `statement_timeout`) |
| `tables` | `Vec<DatabaseTableDecl>` | All — schema enrichment | All 4 (curated description folding into `schema_text()`) |
| `pool` | `Option<DatabasePoolSection>` (`max_connections`, `connection_timeout_seconds`) | Reserved in P83, unused in current refs | Postgres + MySQL (forward to `deadpool` / `sqlx` Pool config); ignored for Athena (stateless HTTP) + SQLite (single conn) |

### What P84 NEEDS TO ADD

**Critical finding:** all three reference configs are Athena-only. The fields the toolkit ALREADY parses are sufficient for Athena. For Postgres/MySQL/SQLite, the URL conventionally lives at... where?

| Backend | URL Source | Recommended new `[database]` key |
|---------|-----------|----------------------------------|
| Postgres | `postgres://user:pass@host:port/db` | New optional `url: Option<String>` OR `env:VAR_NAME` reference (parallel to `[code_mode].token_secret`'s `env:` indirection) |
| MySQL | `mysql://user:pass@host:port/db` | Same — reuse `url` field |
| SQLite | path string | `database` field (already exists) can hold a file path |
| Athena | no URL — uses `region` + `workgroup` (existing) + `output_location` (existing) | Existing fields suffice |

**Recommendation: add ONE new field — `url: Option<String>` — to `DatabaseSection`.** Single additive key, satisfies REF-01 (additive only), and the existing reference configs don't reference it (additive). The secret-handling pattern from P83 D-04 (env:VAR_NAME indirection) applies: `url = "env:DATABASE_URL"` resolves at runtime via the `SecretValue` machinery.

| New `[database]` key | Type | Required | Why |
|----------------------|------|----------|-----|
| `url` | `Option<String>` (supports `env:VAR_NAME` indirection) | Required for Postgres / MySQL; optional/unused for Athena + SQLite | URL-based driver setup per D-08; secrets-clean indirection via `env:` per P83 R6/R9 patterns |

**`region` field for Athena:** Currently NOT in `DatabaseSection`. Confirm by reading reference configs again — they use `${AWS_REGION}` interpolation inside `output_location` strings (`output_location = "s3://aws-athena-query-results-${AWS_ACCOUNT_ID}-${AWS_REGION}/..."`). The Athena SDK reads region from `aws_config::load_from_env()` (which respects `AWS_REGION` env var). **So no new `region` key needed** — the SDK already auto-discovers it.

### Strict-parse impact (P83 D-13)

Adding `url: Option<String>` to `DatabaseSection`'s `#[serde(deny_unknown_fields)]` block is a one-field additive change. The fuzz target (`pmcp_server_toolkit_config_parser`) automatically covers the new field on its next run.

### Reference REF-01 verification path

The P83 invariant: every key emitted by the three reference configs MUST parse cleanly. Re-running `cargo test --test reference_configs` after the `url` addition validates this (it's already a fixture-driven test in `crates/pmcp-server-toolkit/tests/`). The 3 reference configs don't emit `url` so they continue to parse.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` + `proptest 1.7` + `tokio = { features = ["macros", "rt"] }` single-threaded runtime (per CLAUDE.md `--test-threads=1`) |
| Toolkit config | `crates/pmcp-server-toolkit/Cargo.toml:70-78` (`[dev-dependencies]`) |
| Per-backend Cargo.toml (new) | Same dev-deps pattern; add `proptest = "1.7"` to each new per-backend crate |
| Quick run command | `cargo test -p pmcp-server-toolkit` (toolkit core) + `cargo test -p pmcp-toolkit-postgres` etc. |
| Full suite command | `make quality-gate` (matches CI: fmt + clippy `--features full` pedantic+nursery + build + test + audit) |
| Fuzz command | `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` (per P83 existing pattern) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| CONN-01 | `SqlConnector::execute(sql, &[(String, Value)]) -> Result<Vec<Value>, ConnectorError>` returns row JSON-objects | unit + integration | `cargo test -p pmcp-server-toolkit sql::tests` | ❌ Wave 0 — new module-level tests |
| CONN-01 | `schema_text()` folds `[[database.tables]]` descriptions | integration | `cargo test -p pmcp-server-toolkit tkit10_tests` | ✅ exists (`src/code_mode.rs:551`); needs new per-backend assertion |
| CONN-02 | `Dialect` enum 4 variants | unit | `cargo test -p pmcp-server-toolkit sql::tests::dialect_name_stable_for_all_variants` | ✅ exists (`src/sql/mod.rs:207`) |
| CONN-03 | `translate_placeholders` produces dialect-correct SQL + `ordered_params` | property | `cargo test -p pmcp-server-toolkit sql::translate_placeholders_proptests` | ❌ Wave 0 — full proptest battery (5 invariants per §2) |
| CONN-03 | String-literal / comment edge cases | unit | `cargo test -p pmcp-server-toolkit sql::tests::translate_inside_string_literal` (and `_comment`, `_block_comment`) | ❌ Wave 0 |
| CONN-04 | `build_code_mode_prompt(connector)` literal name resolves | unit/doctest | `cargo test --doc -p pmcp-server-toolkit build_code_mode_prompt` | ❌ Wave 0 — either alias or rename per D-12 |
| CONN-05 | Postgres connector connects, executes, introspects schema via in-process mock | integration | `cargo test -p pmcp-toolkit-postgres --test integration` | ❌ Wave 0 — whole crate is new |
| CONN-05 | Postgres `Value` → `tokio_postgres::ToSql` per-variant conversion correctness | unit | `cargo test -p pmcp-toolkit-postgres value_to_sql` | ❌ Wave 0 |
| CONN-06 | MySQL connector ditto via `tests/mock_mysql.rs` | integration | `cargo test -p pmcp-toolkit-mysql --test integration` | ❌ Wave 0 |
| CONN-07 | Athena connector StartQuery → poll → GetResults loop via `tests/mock_athena.rs` | integration | `cargo test -p pmcp-toolkit-athena --test integration` | ❌ Wave 0 |
| CONN-07 | Athena `schema_text()` via `GetTableMetadata` (NO Glue dep) | integration | `cargo test -p pmcp-toolkit-athena schema_text_via_get_table_metadata` | ❌ Wave 0 |
| CONN-08 | SQLite connector against real in-memory `rusqlite` DB | integration | `cargo test -p pmcp-server-toolkit --features sqlite sqlite_connector` | ❌ Wave 0 (extend existing toolkit tests) |
| TEST-01 | All 4 dialects covered by authentic in-process mocks/real driver (no Docker, no testcontainers) | integration | All 4 per-backend test commands above | All ❌ Wave 0 |
| TEST-07 | Fuzz target on `config.toml` parser passes 60s smoke without panic on adversarial input | fuzz | `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` | ✅ exists (`fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs`); new `[database].url` key extends surface automatically |
| (cross-cutting) | StructuredOutput emission for widget tools (D-06) | integration | `cargo test -p pmcp-server-toolkit synthesizer_emits_structured_content` | ❌ Wave 0 |
| (cross-cutting) | Shape C example compiles + runs against in-memory SQLite | example | `cargo run --example sqlite_minimal -p pmcp-server-toolkit --features sqlite` | ❌ Wave 0 |
| (cross-cutting) | Per-crate Shape-C ≤15-line callsite example for each backend | example | `cargo run --example postgres_minimal -p pmcp-toolkit-postgres` (etc.) | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p <touched-crate>` (selective; ~15s)
- **Per wave merge:** `make quality-gate` (full CI parity; ~5min)
- **Phase gate:** `make quality-gate` + `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` green before `/gsd-verify-work`. PMAT CI gate (cognitive complexity ≤25) also passes.

### Wave 0 Gaps

- [ ] `crates/pmcp-server-toolkit/src/sql/translate.rs` (or extend `sql/mod.rs`) — `translate_placeholders` + `TranslatedSql` struct
- [ ] `crates/pmcp-server-toolkit/src/sql/tests.rs` or proptests module — 5 property invariants per §2
- [ ] `crates/pmcp-toolkit-postgres/Cargo.toml` + `src/lib.rs` + `tests/mock_postgres.rs` + `tests/integration.rs` + `examples/postgres_minimal.rs` (`required-features` mirror)
- [ ] `crates/pmcp-toolkit-mysql/Cargo.toml` + same set
- [ ] `crates/pmcp-toolkit-athena/Cargo.toml` + same set (no Glue)
- [ ] `crates/pmcp-server-toolkit/src/sql/sqlite.rs` (new file) — `SqliteConnector` (promoted from `MockSqlConnector`)
- [ ] `crates/pmcp-server-toolkit/examples/sqlite_minimal.rs` — Shape C ≤15-line example
- [ ] `crates/pmcp-server-toolkit/tests/synthesizer_structured_content.rs` — D-06 invariant test
- [ ] Workspace `Cargo.toml` `[workspace.members]` extension (3 new entries — see §7)
- [ ] `CLAUDE.md` §"Release & Publish Workflow" publish-order extension (3 new entries — see §7)

## Workspace + Publish Order Impact

### `Cargo.toml` root — `[workspace.members]` extension

**Current state (verified by Read):** `Cargo.toml:541`:

```toml
members = ["pmcp-macros", "crates/pmcp-macros-support", "crates/mcp-tester", "crates/mcp-preview", "crates/pmcp-server", "crates/pmcp-server/pmcp-server-lambda", "crates/pmcp-tasks", "crates/mcp-e2e-tests", "crates/pmcp-widget-utils", "crates/pmcp-code-mode", "crates/pmcp-code-mode-derive", "crates/pmcp-server-toolkit", "examples/25-oauth-basic", "examples/test-basic", "cargo-pmcp"]
```

**P84 extension:** insert the three new entries immediately after `"crates/pmcp-server-toolkit"`:

```toml
members = ["pmcp-macros", ..., "crates/pmcp-server-toolkit", "crates/pmcp-toolkit-postgres", "crates/pmcp-toolkit-mysql", "crates/pmcp-toolkit-athena", "examples/25-oauth-basic", ...]
```

### `CLAUDE.md` §"Release & Publish Workflow"

**Current state (verified by grep):** lines 223–231:

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

**P84 extension:** insert THREE new entries between `pmcp-server-toolkit` (currently #5) and `mcp-tester` (currently #6):

```markdown
5. `pmcp-server-toolkit` (runtime library; depends on pmcp + pmcp-code-mode under the default `code-mode` feature)
6. `pmcp-toolkit-postgres` (depends on pmcp-server-toolkit + tokio-postgres + deadpool-postgres)
7. `pmcp-toolkit-mysql` (depends on pmcp-server-toolkit + sqlx)
8. `pmcp-toolkit-athena` (depends on pmcp-server-toolkit + aws-sdk-athena + aws-config)
9. `mcp-tester` (depends on pmcp)
10. `mcp-preview` (depends on widget-utils)
11. `cargo-pmcp` (depends on pmcp, mcp-tester, mcp-preview)
```

The three new per-backend crates have no inter-dependencies (per D-10's all-pure-Rust constraint) — they can publish in any order RELATIVE to each other, but ALL must publish AFTER `pmcp-server-toolkit` (their declared workspace dep).

### Version pinning

All three new crates ship at `0.1.0` (per P83's pattern — toolkit core is 0.1.0). The dep on `pmcp-server-toolkit` is `version = "0.1.0", path = "../pmcp-server-toolkit"` (matches P83 D-05 pattern).

**However**: P84 extends the trait surface on `pmcp-server-toolkit`. Per P83 `src/sql/mod.rs:292-296`: "adding `execute()` to the trait is semver-compatible because the trait isn't published to crates.io yet (0.1.0 still); adding ConnectorError variants is additive because the enum is `#[non_exhaustive]`." If `pmcp-server-toolkit 0.1.0` IS already on crates.io (verify via `cargo search pmcp-server-toolkit` before publish), the trait extension is a `0.1.0 → 0.2.0` bump per cargo semver (adding required trait methods without defaults is a breaking change).

[VERIFIED via Bash `cargo search pmcp-server-toolkit`: returned 0 hits — not yet on crates.io as of 2026-05-19. Treat as still pre-publish; 0.1.0 → 0.1.0 (no bump) is safe.] ← **NEEDS RE-VERIFY at plan time**: search again immediately before the trait extension lands.

## Cross-Cutting Risks / Landmines

1. **DO NOT normalize `schema_text()` across dialects** — D-11 locked + spike 005 `schema-server-sql-dialects.md` "Don't normalize" rule. The LLM benefits from the native introspection format. Each backend emits its own DDL/catalog shape; the toolkit only adds curated `[[database.tables]]` descriptions on top via P83's `assemble_code_mode_prompt`.

2. **DO NOT put `translate_placeholders` on the `SqlConnector` trait** — D-05 + spike 005 reference doc § "What to Avoid" rule #1. It's a free helper. Putting it on the trait invites per-backend overrides that introduce subtle drift.

3. **DO NOT share connection pools across connectors** — D-08 + spike 005 § "What to Avoid" rule #6. Each connector owns its own pool (`deadpool_postgres::Pool`, `sqlx::MySqlPool`, Athena's stateless HTTP, `Arc<Mutex<rusqlite::Connection>>`). The toolkit's `Arc<dyn SqlConnector>` is the only shared surface.

4. **NO Docker, NO testcontainers, NO networking in CI** — `feedback_avoid_docker_pure_rust_lambda` memory + ROADMAP "Out of Scope" + TEST-01 + D-07. Use authentic in-process mocks per §3. Real-DB integration is per-crate's responsibility (opt-in CI with credentials), NOT toolkit's default test path.

5. **`structured_content` is gated on `widget_meta` in pmcp core** — read `src/server/core.rs:593-601`. Returning a `Vec<Value>` from `ToolHandler::handle()` does NOT automatically populate `structured_content` for non-widget tools. The resolution path is §4 Option A (set `widget_meta` on the synthesized `ToolInfo` when `ui_resource_uri` is present). If the planner wants `structured_content` even for non-widget tools, that requires a pmcp core change (`structured_content` activation gated on `output_schema.is_some()`) which is out of P84 scope.

6. **PMAT cognitive complexity ≤25 per function** — CLAUDE.md hard rule, CI-enforced via `pmat quality-gate`. `translate_placeholders` with the 5-state literal/comment-aware machine likely exceeds 25 inline. Use the split-helper pattern in §2 (`SqlWalker` impl with one method per state), keeping each helper well under cog 25.

7. **`make quality-gate` (not `cargo clippy -- -D warnings`)** — CLAUDE.md hard rule. CI runs `--features full` + pedantic + nursery; the bare cargo command misses lints. Every plan's quality gate command MUST be `make quality-gate`.

8. **Tests run `--test-threads=1`** — CLAUDE.md hard rule. New per-backend crates inherit single-threaded test runtime; `SqliteConnector`'s `std::sync::Mutex` is safe under this (and even safer with `spawn_blocking` semantics).

9. **REF-01 superset invariant: additive only, no renames** — P83 D-13 enforced via `#[serde(deny_unknown_fields)]` + the `tests/reference_configs.rs` integration test (per `src/config.rs:21`). The new `url: Option<String>` field on `DatabaseSection` is additive — does not break parsing of existing references.

10. **`MockSqlConnector` deletion is destructive — verify all callers first.** P83 `src/sql/mod.rs:182-199` defines `pub(crate) MockSqlConnector`. P83 `src/code_mode.rs:555` consumes it in `tkit10_tests`. D-09 says delete once `SqliteConnector` covers the test-fixture role. The planner MUST grep for ALL `MockSqlConnector` references (4 confirmed in `src/code_mode.rs` lines 555, 574, 596, 632, 653) and migrate each to `SqliteConnector::open_in_memory()` + a seeded schema, OR keep `MockSqlConnector` as a `pub(crate)` test-only fixture and add `SqliteConnector` alongside. **Recommendation:** keep `MockSqlConnector` `pub(crate)` for test fixtures (it's a Dialect-stamped canned-schema struct — not what `SqliteConnector` is) and ALSO ship `pub struct SqliteConnector` (the real driver). The two serve different purposes — `MockSqlConnector` is a test double for the `schema_text()` surface only; `SqliteConnector` is a real driver impl.

11. **`tokio-postgres` parameter binding does NOT accept `serde_json::Value` directly** — must dispatch per-variant inside `execute()`. Same situation for `sqlx`. Don't try to use `ToSql` blanket impl on `Value` from a feature flag — wire it inline per §1.1.

12. **Athena `?` parameter binding wire format** — params get stringified via `.execution_parameters(Some(vec!["3"]))` since Presto-on-Athena doesn't have typed binding through the API. Document this in the rustdoc as a known limitation: numeric vs string distinction is lost at the wire layer.

13. **`tokio::spawn` for `tokio-postgres` connection** — Phase 84 plans MUST include the `tokio::spawn(async move { let _ = connection.await; })` setup in `PostgresConnector::connect()`. Forgetting this = silently hung queries with no panic, no error — the absolute worst class of bug.

14. **AWS SDK rustls choice** — toolkit's `Cargo.toml:42-48` notes the `aws-sdk-secretsmanager` / `aws-sdk-ssm` / `aws-config` rustls 0.21 / RUSTSEC-2026-0098/0099/0104 issue and opts into `default-https-client` + `behavior-version-latest`. The new `pmcp-toolkit-athena` crate MUST mirror this feature set: `aws-sdk-athena = { version = "1", default-features = false, features = ["default-https-client", "rt-tokio", "behavior-version-latest"] }`.

15. **`assemble_code_mode_prompt` rename strategy (D-12)** — `crates/pmcp-server-toolkit/src/code_mode.rs:365` is the existing entry point. CONN-04 mandates literal name `build_code_mode_prompt`. **Recommendation:** option (b) — ship `build_code_mode_prompt(connector) -> impl Future<Output = Result<String>>` as a thin alias next to the existing async fn. Lower churn than rename + deprecation. The P83 `examples/e01_toolkit_minimal` and Plan 08 smoke test already consume the existing name; an alias doesn't break them.

16. **Workspace `[dev-dependencies]` for new per-backend crates** — each new crate needs `proptest = "1.7"` + `tokio = { features = ["macros", "rt"] }` (single-thread runtime per CLAUDE.md). Don't add `rt-multi-thread` — it bloats compile times for no benefit under `--test-threads=1`.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| — | None — every claim in this research was either VERIFIED via tool invocation (`cargo search`, file Read, Bash grep) or CITED to an external authoritative source (docs.aws.amazon.com, docs.rs, crates.io API, launchbadge/sqlx README, oneuptime.com benchmark). | — | — |

**Empty assumptions table:** All claims are either VERIFIED (registry / file Read / Bash grep / Context7) or CITED (linked official documentation). No `[ASSUMED]` tags appear in this document.

One caveat: §7 notes that `cargo search pmcp-server-toolkit` returns 0 hits as of 2026-05-19. The planner MUST re-verify this immediately before publishing — if `pmcp-server-toolkit 0.1.0` ships to crates.io between now and P84 plan execution, the trait extension changes from "non-breaking pre-publish" to "0.1.0 → 0.2.0 breaking semver bump." Adding required trait methods without defaults is a Rust API-breaking change.

## Open Questions

1. **Per-tool widget_meta resolution path for D-06.**
   - What we know: pmcp core gates `structured_content` on `info.widget_meta().is_some()`. Today the synthesizer never sets widget_meta.
   - What's unclear: Should non-widget tools (no `ui_resource_uri` in config) get `structured_content` via a parallel pmcp-core change, or accept `Content::text` pretty-JSON as the contract?
   - Recommendation: Accept `Content::text` for non-widget tools (matches existing pmcp-run behavior); explicit opt-in via `ui_resource_uri = "..."` activates widget_meta + `structured_content`. Document this in the synthesizer's rustdoc as the wiring contract.

2. **CONN-04 alias vs rename mechanic (D-12).**
   - What we know: P83 ships `assemble_code_mode_prompt` (verified at `src/code_mode.rs:365`); CONN-04 mandates literal name `build_code_mode_prompt`.
   - What's unclear: Whether the planner prefers a `#[deprecated] pub use assemble_code_mode_prompt as ...;` re-export OR a thin function wrapper.
   - Recommendation: Thin function alias `pub async fn build_code_mode_prompt(connector: &dyn SqlConnector, config: &ServerConfig) -> Result<String> { assemble_code_mode_prompt(connector, config).await }`. No deprecation needed — both names are valid public surface (this is the explicit dual-naming pattern P83 already uses for `register_code_mode_tools` vs `code_mode_tools_from_executor`).

3. **Should `SqliteConnector` deprecate `MockSqlConnector` immediately or coexist?**
   - What we know: D-09 says "Delete the old `MockSqlConnector` once `SqliteConnector` covers its test-fixture role." Four call sites confirmed via grep.
   - What's unclear: Is the test-fixture role (canned `schema_text`, `Dialect::Postgres` declaration even though no PG involved — see `src/code_mode.rs:574-577`) actually covered by `SqliteConnector`? The `MockSqlConnector` declares an arbitrary `Dialect` for unit-testing the dialect-aware prompt assembly; `SqliteConnector` always declares `Dialect::Sqlite`.
   - Recommendation: KEEP `pub(crate) MockSqlConnector` alongside `pub struct SqliteConnector`. They serve different purposes (`MockSqlConnector` is a test-only fixture for any-dialect schema-text testing; `SqliteConnector` is a real driver for SQLite). D-09's "delete" wording is too aggressive — adjust the plan to keep both.

4. **Should P84 add an `[athena]` named sub-table to `DatabaseSection` for clarity?**
   - What we know: Currently `output_location` and `workgroup` sit alongside `database` at the `[database]` flat level — a non-Athena backend just leaves them `None`.
   - What's unclear: Is splitting into `[database.athena]` / `[database.postgres]` etc. preferable for forward expansion?
   - Recommendation: NO — REF-01 forbids renames, and moving `output_location` from `[database]` → `[database.athena]` is a rename in the parse tree. Stick with the flat layout. If the planner needs Postgres-specific knobs (e.g., `application_name`), add them at flat level with `None`-default per REF-01 additive-only.

## Environment Availability

Phase 84 code/config changes only — drivers and dependencies are PURE Cargo dep additions (no system-level tools, no Docker, no test infrastructure). The toolchain audit:

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| `cargo` | All build/test | ✓ | 1.95.0 | — |
| `rustc` | All build | ✓ | 1.95.0 | — |
| `cargo +nightly` | TEST-07 fuzz target | (presumed; matches P77 / P83 disposition) | — | Compile-verify only on stable; runtime fuzz in nightly CI per P77 Plan 08 disposition (D-14) |
| `pmat` | CI cognitive-complexity gate | (CI-only; not required locally per P75 D-07) | 3.15.0 (pinned in CI) | Local dev runs `make quality-gate` (which does NOT run PMAT) — PMAT runs in CI on push |
| `make` | quality-gate driver | (system; presumed) | — | — |
| `tokio-postgres 0.7.17` | CONN-05 | new dep | 0.7.17 | — |
| `deadpool-postgres 0.14.1` | CONN-05 pool | new dep | 0.14.1 | `bb8-postgres 0.9.0` (heavier API surface — not recommended) |
| `sqlx 0.8.6` | CONN-06 | new dep | 0.8.6 | — |
| `aws-sdk-athena 1.105.0` | CONN-07 | new dep | 1.105.0 | — |
| `aws-config 1.8.16` | CONN-07 | already present (P83 `aws` feature) | 1.x | — |
| `rusqlite 0.39.0` (`bundled`) | CONN-08 | already present (optional) | 0.39.0 | — |

**No external dependencies missing or blocking. No Docker, no testcontainers (forbidden).**

## Security Domain

Per CLAUDE.md `security_enforcement` is implicit (no `.planning/config.json` override observed). Applying ASVS categories to a SQL-connector phase:

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | partial | Database URL contains credentials → resolved via P83's `env:VAR_NAME` indirection on the new `url: Option<String>` field |
| V3 Session Management | no | No HTTP session surface (per-tool calls are stateless from connector's POV) |
| V4 Access Control | yes | `[code_mode] blocked_tables` + `sensitive_columns` continue to apply per P83 wiring — P84 inherits, doesn't change |
| V5 Input Validation | yes | `[[tools.parameters]]` JSON-schema enforcement (P83 TKIT-07, already shipped via `synthesize_from_config`); placeholder values bound parametrically, NEVER concatenated into SQL (D-04 struct binding is the canonical mechanism) |
| V6 Cryptography | partial | TLS to remote databases (rustls per §1.2 / §1.1 / §1.3) — pure-Rust crypto, no OpenSSL; AWS rustls 0.21 RUSTSEC issue avoided via P83's `default-https-client` opt-in pattern (mirror it for Athena) |
| V7 Error Handling and Logging | yes | `ConnectorError` `#[non_exhaustive]`; never leak raw driver messages to MCP clients without sanitization (driver messages may include credentials). Use `tracing::error!` for full detail, return wrapped variant to MCP client. |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection via unbound `:placeholder` concat | Tampering | `translate_placeholders` + per-backend driver-native param binding (`tokio-postgres` `$N`, `sqlx` `?` `.bind()`, Athena `execution_parameters`, rusqlite `raw_bind_parameter`) — NEVER `format!` SQL with values |
| Credential exposure in `DATABASE_URL` config | Information Disclosure | New `url: Option<String>` accepts `env:VAR_NAME` indirection (parallel P83 R6/R9 pattern); inline raw URL with embedded password REJECTED unless `allow_inline_token_secret_for_dev = true` (extend that flag's semantics or add a new `allow_inline_database_url_for_dev` per P83 R9 mechanic) |
| AWS rustls 0.21 (RUSTSEC-2026-0098/0099/0104) | Information Disclosure / DoS | Per `Cargo.toml:42-48` pattern — opt into `default-https-client` for all AWS SDK deps in `pmcp-toolkit-athena` |
| Connector error message leakage | Information Disclosure | Wrap driver errors in `ConnectorError::Driver(String)` after sanitization; never propagate raw driver messages (may contain query text + bound values) to MCP responses. Use `tracing::error!` with structured fields for full detail server-side. |
| Mock-test contamination (tests against real prod DB) | Tampering | D-07 in-process mocks — no networking from CI. Per-backend real-DB integration is opt-in with explicit credentials (per `spike-findings-rust-mcp-sdk` §"Mock pattern for tests") and NOT in the default test path. |

## Sources

### Primary (HIGH confidence)

- File Read: `crates/pmcp-server-toolkit/src/sql/mod.rs` — current P83 trait stub + `MockSqlConnector`
- File Read: `crates/pmcp-server-toolkit/src/code_mode.rs` — `assemble_code_mode_prompt`
- File Read: `crates/pmcp-server-toolkit/src/tools.rs` — `synthesize_from_config` + `SynthesizedToolHandler`
- File Read: `crates/pmcp-server-toolkit/src/config.rs` — `DatabaseSection` + `ToolDecl` shape
- File Read: `crates/pmcp-server-toolkit/Cargo.toml` — current feature matrix (sqlite, code-mode, aws, avp, input-validation)
- File Read: `crates/pmcp-server-toolkit/fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs` — Phase 77/83 fuzz pattern
- File Read: `src/server/core.rs` — `handle_call_tool` `Value → CallToolResult` conversion (lines 444–604)
- File Read: `src/types/tools.rs` — `CallToolResult::with_widget_enrichment` + `with_structured_content` (lines 469–600)
- File Read: `.planning/spikes/005-multi-dialect-sql-connector/src/main.rs` — VALIDATED trait + mocks reference impl
- File Read: `.planning/spikes/005-multi-dialect-sql-connector/README.md` — verdict + design rationale
- File Read: `.claude/skills/spike-findings-rust-mcp-sdk/references/schema-server-sql-dialects.md` — blueprint + "What to avoid" rules
- File Read: `.planning/REQUIREMENTS.md` — CONN-01..08 + TEST-01 + TEST-07 full text
- File Read: `.planning/ROADMAP.md` lines 1439–1452 — Phase 84 goal + 5 success criteria
- File Read: `Cargo.toml` line 541 — `[workspace.members]` current layout
- File Read: `CLAUDE.md` lines 223–231 — publish-order list
- File Read: `crates/pmcp-server-toolkit/tests/fixtures/{open-images,imdb,msr-vtt}-config.toml` — REF-01 anchors (all 3 are Athena-only)
- Bash: `cargo search tokio-postgres / sqlx / rusqlite / aws-sdk-athena / aws-sdk-glue / aws-config / deadpool-postgres / bb8-postgres / pmcp-server-toolkit` — registry-VERIFIED versions on 2026-05-19
- Bash: `cargo --version` (1.95.0), `rustc --version` (1.95.0 nightly toolchain feature note)
- Bash: `grep "structured_content\|widget_meta"` confirmed pmcp-core conversion gate
- WebFetch: docs.aws.amazon.com/athena/latest/APIReference/API_GetTableMetadata.html — full `TableMetadata` response shape (Columns + Type + PartitionKeys + Parameters)
- WebFetch: docs.rs/tokio-postgres/0.7.17/ — Client::query / execute / Row::get signatures
- WebFetch: docs.rs/sqlx/0.8.6/sqlx/mysql/ — `MySqlPool` type alias + binding pattern
- WebFetch: docs.rs/rusqlite/0.39.0/ — `Connection::open_in_memory` + `params!` + `named_params!`
- WebFetch: crates.io/api/v1/crates/sqlx/versions — stable 0.8.6 vs alpha 0.9 confirmed

### Secondary (MEDIUM confidence)

- WebSearch: "sqlx 0.8.6 features mysql runtime-tokio-rustls" — verified `tls-rustls-aws-lc-rs` feature naming
- WebSearch: "deadpool-postgres vs bb8-postgres 2026" — verified deadpool has ~100 LOC API surface (oneuptime.com / leapcell.io)
- WebSearch: "aws-sdk-athena GetTableMetadata" — verified Glue Data Catalog accessed via Athena's own client suffices

### Tertiary (LOW confidence)

- None — all findings above triangulated against ≥2 sources where the claim is critical.

## Metadata

**Confidence breakdown:**
- Driver API versions: HIGH (verified `cargo search` against crates.io 2026-05-19)
- Trait/translation_placeholders shape: HIGH (locked by CONTEXT.md D-01..D-06 + verified against spike 005 source)
- Per-backend mock patterns: HIGH (spike 005 is `VERDICT: VALIDATED` reference; pattern lifted verbatim)
- `structured_content` wiring: HIGH (read directly from `src/server/core.rs:593-601` + `src/types/tools.rs:582-599`)
- Athena `GetTableMetadata` covers schema introspection (drops Glue dep): HIGH (verified at docs.aws.amazon.com Athena API reference)
- Workspace/publish-order edits: HIGH (line numbers verified by file Read + Bash grep)
- Config.toml `url` extension surface: MEDIUM (additive single-field decision; the planner may prefer a `[database.connection]` sub-table — both satisfy REF-01)
- Pool choice (deadpool vs bb8): MEDIUM (recommendation backed by 2026 community benchmarks, but the project hasn't publicly committed to deadpool; reversible)
- D-06 `widget_meta` resolution: MEDIUM (Option A is the cleanest path but the planner may want a pmcp-core change to broaden `structured_content` activation — out of P84 scope as drafted)

**Research date:** 2026-05-19
**Valid until:** 2026-06-19 (30 days; per-backend driver versions move monthly; AWS SDK Athena versions bump weekly — re-check before publish)

## RESEARCH COMPLETE
