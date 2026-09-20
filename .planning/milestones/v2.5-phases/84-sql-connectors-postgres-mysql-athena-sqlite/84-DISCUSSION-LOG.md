# Phase 84: SQL Connectors (Postgres / MySQL / Athena / SQLite) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-19
**Phase:** 84-sql-connectors-postgres-mysql-athena-sqlite
**Areas discussed:** Trait execute() surface, translate_placeholders return shape, Mock + constructor shape, SQLite public surface scope

---

## Trait execute() surface

### Q1 — Row shape

| Option | Description | Selected |
|--------|-------------|----------|
| Vec<serde_json::Value> | Each row = JSON object. Matches MCP transport. Reference servers already emit JSON. Drivers convert internally. | ✓ |
| Typed Row trait + Vec<Row> | Custom Row trait with typed accessors. Faster but more API surface. | |
| Streaming: impl Stream<Item = Result<Value>> | Pin<Box<dyn Stream>>. Backpressure-friendly but adds complexity v2.2 doesn't need. | |

**User's choice:** Vec<serde_json::Value>.
**Notes:** This row shape is exactly what tool handlers feed into `tools/call` response → `structuredContent`.

### Q2 — Streaming + transactions in v0.2

| Option | Description | Selected |
|--------|-------------|----------|
| Punt both — minimal MVP | execute() only. Document deferred-evolution plan in rustdoc per P83 pattern. | ✓ |
| Add transactions, skip streaming | Land a `transaction()` continuation. Athena has no tx model. | |
| Add streaming, skip transactions | Separate execute_stream(). Lambda doesn't benefit from streaming. | |
| Add both | Maximum forward compat but biggest surface to validate before downstream uses it. | |

**User's choice:** Punt both.
**Notes:** Trait is `#[non_exhaustive]` and on a `Send + Sync + 'static` trait; methods with default impls can be added in a minor release without semver break. Document the evolution path in rustdoc the same way P83 documented the 2-method stub's evolution.

---

## translate_placeholders return shape

### Q1 — Return shape

| Option | Description | Selected |
|--------|-------------|----------|
| TranslatedSql { sql, ordered_params } | Named-field struct. Bind-order is part of the contract. P83 review HIGH-severity ask. | ✓ (confirmed via Q2) |
| (String, Vec<String>) tuple | Spike 005's shape. Less ceremony but downstream callers must remember positions. | |
| Just String per CONN-03 literal | Drops bind-order. Forces per-backend re-parse. Likely a requirement-writing oversight. | |

**User's choice:** No direct selection initially — user added a note: *"Please remember that we want to return from the tool calls StructuredOutput to make it easy to chain them together and to allow MCP App UI widgets."*

### Q2 — Confirm structure locks

| Option | Description | Selected |
|--------|-------------|----------|
| Both locked as stated | TranslatedSql struct return + tool handlers populate structuredContent with Vec<Value> rows | ✓ |
| Different shape — tell me how | (user had a different idea) | |

**User's choice:** Both locked as stated.
**Notes:** The "structured" framing carried to both: (a) the helper returns a struct, not a tuple; (b) tool handlers built by P83's TKIT-07 synthesizer populate `structuredContent` (not just `text`) so MCP App widgets and chained tool calls can consume rows as structured JSON.

Spike 005's "free helper, NOT a trait method" rule was carried forward without re-asking (locked from spike findings).

---

## Mock + constructor shape

### Q1 — Where authentic in-process mocks live

| Option | Description | Selected |
|--------|-------------|----------|
| Per-backend tests/ dir | Each crate owns its `tests/mock_*.rs` + `tests/integration.rs`. Matches spike 005 layout. | ✓ |
| Shared pmcp-toolkit-test-support crate | New workspace crate. Premature — no external consumer asked for it. | |
| Inside toolkit core under feature flag | Bloats core with backend-specific code. Conflicts with "per-backend crate is small" spike principle. | |

**User's choice:** Per-backend tests/ dir.
**Notes:** No new workspace crate. Each per-backend `tests/integration.rs` consumes its mock via `mod mock_postgres;` (Rust 2021 tests/ layout).

### Q2 — Connector constructor shape

| Option | Description | Selected |
|--------|-------------|----------|
| URL string + pool internal | `PostgresConnector::connect(url)` etc. Connector owns its pool. Shape C 15-line target reachable. | ✓ |
| Pool injection (Arc<Pool>) | User constructs pool externally. Breaks Shape C ≤15-line target. | |
| Builder pattern per connector | Matches PMCP's ServerCoreBuilder pattern but over-engineered for the simple case. | |
| Both connect(url) + with_pool(Arc<Pool>) | Two impls to test per backend. Could land additively later. | |

**User's choice:** URL string + internal pool.
**Notes:** External pool injection deferred to a future minor release if a real consumer hits a tuning ceiling. Lambda cold-start: pool is single-conn-on-demand.

---

## SQLite public surface scope

### Q1 — Public surface

| Option | Description | Selected |
|--------|-------------|----------|
| Public SqliteConnector for production+test | `pub struct SqliteConnector` with `::open(path)` + `::open_in_memory()` behind sqlite feature. P83's MockSqlConnector evolves into this. | ✓ |
| Internal-only — keep MockSqlConnector | SQLite strictly for toolkit's own coverage. Phase 86 scaffolds use rusqlite directly. | |
| Two-tier: public SqliteConnector + retain pub(crate) Mock | Both kept. More API surface, more docs. | |

**User's choice:** Public SqliteConnector.
**Notes:** P83's `pub(crate) MockSqlConnector` (gated `cfg(any(test, feature = "sqlite"))`) gets promoted to the public `pub struct SqliteConnector` and the old Mock is deleted once the real impl covers its test-fixture role.

---

## Claude's Discretion

- Exact rename strategy for CONN-04 (`assemble_code_mode_prompt` ↔ `build_code_mode_prompt`): alias vs deprecated rename.
- Concrete extended variants of `ConnectorError` for execute-time failures (likely `Driver`, `QuerySyntax`, `Connection`, `ParameterBind`).
- Whether the Athena connector uses `aws-sdk-athena` alone or also `aws-sdk-glue` for catalog access.
- `MysqlConnector` pool type naming (`sqlx::Pool<MySql>` vs `sqlx::MySqlPool`) — picked from the current published `sqlx` API.
- Internal mutex/connection structure of `SqliteConnector` — likely `Arc<Mutex<rusqlite::Connection>>` with `spawn_blocking` inside `async fn execute()`.
- Workspace publish-order slot edit in CLAUDE.md for the three new per-backend crates.

## Deferred Ideas

- Streaming `execute_stream()` method — future semver-additive release.
- Transaction support on `SqlConnector` — future release (likely Phase 88 dogfood timing).
- External pool injection (`with_pool(Arc<Pool>)`) — future minor release.
- Shared `pmcp-toolkit-test-support` crate — only if an external consumer asks.
- GraphQL / OpenAPI connector crates — next-milestone (`GQL-TKIT-01` / `OAPI-TKIT-01`).
- `#[pmcp::sql_server]` proc-macro — future milestone (toolkit-on-crates.io prereq met by P83).
- Cross-backend tool federation (`FED-01`) — future milestone.
- Type 1 ai-agents/ skill updates (`SKLL-07`) — Phase 87.

## Reviewed Todos (not folded)

- `2026-03-04-create-readme-docs-for-cargo-pmcp-cli.md` — owned by Phase 89.
- `2026-05-18-ship-sqlite-from-config-example-as-phase-86-shape-b-c-dogfood.md` — explicitly Phase 86.
