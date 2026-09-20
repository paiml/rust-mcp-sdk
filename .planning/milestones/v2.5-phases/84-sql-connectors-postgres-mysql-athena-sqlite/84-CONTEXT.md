# Phase 84: SQL Connectors (Postgres / MySQL / Athena / SQLite) - Context

**Gathered:** 2026-05-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Land the per-backend SQL connector crates and the SQLite feature flag on the
already-published `pmcp-server-toolkit`, plus extend the toolkit's
`SqlConnector` trait from its Phase 83 2-method stub (`dialect()` +
`schema_text()`) to the 3-method shape required by CONN-01 by adding
`execute(sql, named_params) -> Vec<serde_json::Value>`. Ship the
`translate_placeholders` free helper that CONN-03 calls for, with the
binding-order-preserving struct return that P83's HIGH-severity review
demanded.

**In scope:**

- Toolkit-core additions in `crates/pmcp-server-toolkit/src/sql/`:
  - `SqlConnector::execute(sql, &[(String, Value)]) -> Result<Vec<Value>, ConnectorError>` (CONN-01)
  - `pub fn translate_placeholders(sql: &str, dialect: Dialect) -> TranslatedSql` free helper (CONN-03)
  - `pub struct TranslatedSql { pub sql: String, pub ordered_params: Vec<String> }`
  - `ConnectorError` enum extended with execute-time variants (driver / query / param-bind / connect)
  - Promote `pub(crate) MockSqlConnector` to `pub struct SqliteConnector` under the existing `sqlite` feature
  - `pub fn build_code_mode_prompt(connector) -> String` alias / rename of P83's `assemble_code_mode_prompt` to satisfy CONN-04's literal naming (planner decides rename vs re-export)
- Three new per-backend workspace crates (CONN-05 / CONN-06 / CONN-07):
  - `crates/pmcp-toolkit-postgres/` — `tokio-postgres` driver, `PostgresConnector::connect(url)`, `information_schema`-driven `schema_text()`
  - `crates/pmcp-toolkit-mysql/` — `sqlx` (MySQL feature, pure-Rust) driver, `MysqlConnector::connect(url)`, `information_schema`-driven `schema_text()`
  - `crates/pmcp-toolkit-athena/` — `aws-sdk-athena` + AWS Glue catalog client, `AthenaConnector::from_config(region, workgroup)`, Glue-catalog-driven `schema_text()`
- SQLite (CONN-08): `pub struct SqliteConnector` with `::open(path)` + `::open_in_memory()` behind the toolkit's existing `sqlite` feature, rusqlite `bundled`
- Tool handlers built by the P83 `[[tools]]` synthesizer wrap `Vec<serde_json::Value>` row results into the `tools/call` response's `structuredContent` field so MCP App widgets + downstream chained tools can consume them as structured JSON
- Per-backend integration tests against authentic in-process mocks (TEST-01) co-located in each per-backend crate's `tests/mock_*.rs` + `tests/integration.rs`; SQLite tested against real in-memory `rusqlite`
- Fuzz target on the toolkit's `config.toml` parser (TEST-07) extending Phase 77's existing `pmcp_config_toml_parser` pattern — reuse, do not duplicate
- Property tests for `translate_placeholders` invariants (idempotence per dialect, bind-order preservation, no panics on malformed input) — promotes P83's TEST-02 placeholder coverage to cover the new struct return
- Doctests on every new public type/fn (TEST-03 extension)

**Out of scope (other phases own these):**

- `pmcp-sql-server` pure-config binary + reference parity → Phase 85
- Scaffolding, ≤15-line library example, `cargo pmcp deploy` → Phase 86
- `pmcp-config-helper` Type 2 authoring Skills MCP server → Phase 87
- `crates/pmcp-server` dogfood rewrite → Phase 88
- Book chapter / course tutorial / migration recipe → Phase 89
- Streaming `execute_stream()` method on `SqlConnector` → deferred to a future semver-additive release (P84 documents the evolution plan in trait rustdoc, same pattern P83 used)
- Transaction support on `SqlConnector` → deferred (none of the v2.2 reference scenarios need it; Athena has no real tx model)
- External pool injection (`with_pool(Arc<Pool>)`) → deferred (Shape C ≤15-line target wins for v0.2; can land additively later)
- GraphQL / OpenAPI connector crates → deferred (GQL-TKIT-01 / OAPI-TKIT-01 are next-milestone)
- Docker / `testcontainers` infrastructure of any kind → explicitly forbidden (out-of-scope per ROADMAP §"Out of Scope" + `feedback_avoid_docker_pure_rust_lambda` memory)

</domain>

<decisions>
## Implementation Decisions

### `SqlConnector` Trait Surface (CONN-01)

- **D-01:** **`execute()` returns `Result<Vec<serde_json::Value>, ConnectorError>`.**
  Each row is a JSON object. This is exactly the shape MCP transport
  needs at the boundary (`tools/call` response → `structuredContent` field
  in D-05), and matches what the reference servers
  (open-images / imdb / msr-vtt) already emit. Per-backend impls convert
  driver-native rows (`tokio_postgres::Row` / `sqlx::mysql::MySqlRow` /
  `aws_sdk_athena::types::Row` / `rusqlite::Row`) into
  `serde_json::Value` inside the connector. No typed `Row` trait. No
  streaming. Simplest API surface that satisfies the v2.2 thesis.
- **D-02:** **Defer streaming and transactions to a later semver-additive
  release.** v0.2 ships `execute()` only. None of the v2.2 reference
  scenarios need either capability, and Athena has no real transaction
  model. The trait's rustdoc must document the deferred-evolution plan
  (mirroring P83's pattern for the trait stub itself): a future
  `execute_stream()` method with a default impl backed by
  `execute().map(|v| stream::iter(v))` is additive on a
  `Send + Sync + 'static` trait without a semver break, and
  transaction support can land as a separate trait extension
  (`SqlTransactional`) when an actual consumer needs it. `ConnectorError`
  stays `#[non_exhaustive]` so query/timeout/transaction variants are
  additive too.
- **D-03:** **Parameter shape: `&[(String, serde_json::Value)]` slice of
  named pairs.** Canonical user-facing binding: the config-author writes
  `WHERE id = :id` in `config.toml`, and the per-tool handler builds
  `&[("id".into(), Value::Number(...))]` from the validated tool args.
  Slice (not `HashMap` / `IndexMap`) because the caller controls order
  and `(name, value)` pairs round-trip cleanly through `serde_json`.

### `translate_placeholders` Helper (CONN-03)

- **D-04:** **Return `TranslatedSql { sql: String, ordered_params: Vec<String> }`.**
  Named-field struct, not a tuple, not a bare `String`. Per-backend
  `execute()` impls do
  `let TranslatedSql { sql, ordered_params } = translate_placeholders(canonical, dialect);`
  then iterate `ordered_params` to bind positional driver params from the
  caller's `&[(String, Value)]`. Resolves P83 review R2's HIGH-severity
  ask. CONN-03's literal text says `-> String` — that text was
  retrofitted from spike 005 before review feedback; treat it as
  satisfied by the binding-order-preserving struct shape.
- **D-05:** **Free helper, NOT a trait method.** Per spike 005's "Don't
  make `translate_placeholders` a trait method" rule — every connector
  calls it the same way, putting it on the trait invites per-backend
  drift. Lives at `pmcp_server_toolkit::sql::translate_placeholders`.
- **D-06:** **Tool handlers emit StructuredOutput.** Tool handlers built
  by P83's `synthesize_from_config` (TKIT-07, already shipped) populate
  the `tools/call` response's `structuredContent` field with the
  `Vec<serde_json::Value>` rows returned by `connector.execute()` — not
  just text content. This is the contract that downstream MCP Apps UI
  widgets and chained tool calls depend on. P84 plans must verify this
  in the per-tool handler shape, since the synthesizer landed before
  `execute()` existed.

### Per-Backend Crate Shape (CONN-05/06/07/08)

- **D-07:** **Per-backend authentic in-process mocks live in each
  crate's `tests/` dir.** `crates/pmcp-toolkit-postgres/tests/mock_postgres.rs`
  models `$1`-placeholder binding + `information_schema` SELECTs;
  `…-mysql/tests/mock_mysql.rs` models `?` + `information_schema`;
  `…-athena/tests/mock_athena.rs` models `?` + Glue catalog. No shared
  `pmcp-toolkit-test-support` crate in v0.2 — no external consumer has
  asked for one, and per-backend `tests/` is exactly where the spike 005
  reference mocks lived. Each integration test (`tests/integration.rs`)
  consumes its mock as a dev-dep module via
  `mod mock_postgres;` (Rust 2021 `tests/` layout).
- **D-08:** **Connector constructor = URL string + internal pool.**
  `PostgresConnector::connect(url) -> Result<Self, ConnectorError>` /
  `MysqlConnector::connect(url) -> …` /
  `AthenaConnector::from_config(region, workgroup) -> …`. Pool is owned
  by the connector and constructed on demand. Keeps the Shape C
  ≤15-line `main.rs` target reachable for Phase 86's library example.
  No external `with_pool(Arc<Pool>)` injection in v0.2 — additive
  later if a real consumer hits a tuning ceiling.
- **D-09:** **SQLite ships as a public `SqliteConnector` type behind
  the existing `sqlite` feature on the toolkit core crate.**
  P83 already ships `pub(crate) MockSqlConnector` under
  `cfg(any(test, feature = "sqlite"))`; P84 evolves it into
  `pub struct SqliteConnector` with `::open(path: &Path) -> Result<Self>`
  and `::open_in_memory() -> Result<Self>`, plus the full
  `SqlConnector` trait impl driven by `rusqlite` (bundled). No separate
  `pmcp-toolkit-sqlite` crate — that's what CONN-08 explicitly says.
  Delete the old `MockSqlConnector` once `SqliteConnector` covers its
  test-fixture role.
- **D-10:** **Per-backend crate names + drivers are LOCKED by
  REQUIREMENTS.md, no renaming.** `pmcp-toolkit-postgres` →
  `tokio-postgres`. `pmcp-toolkit-mysql` → `sqlx` with MySQL feature
  (pure-Rust, not C bindings). `pmcp-toolkit-athena` →
  `aws-sdk-athena` + `aws-sdk-glue` (or `aws-sdk-athena`'s catalog
  client if it covers the schema use case). SQLite → `rusqlite` with
  `bundled` feature. All four drivers are pure-Rust and Lambda-deployable
  (`feedback_avoid_docker_pure_rust_lambda`).
- **D-11:** **Per-backend `schema_text()` is dialect-styled, not
  normalized.** Postgres + MySQL emit `information_schema`-derived
  CREATE-TABLE-shaped text. Athena emits Glue-catalog-derived
  CREATE-EXTERNAL-TABLE-shaped text (with partitions / SerDe).
  SQLite emits `sqlite_master`-derived text. Per spike 005's
  "Don't normalize `schema_text()` output across dialects" — the LLM
  benefits from native introspection format. Each backend's
  `schema_text()` ALSO folds in `[[database.tables]]` curated
  descriptions per CONN-01's MUST clause (this is what P83 already
  does in `assemble_code_mode_prompt`; per-backend impls must
  cooperate with it).

### Renaming / Compatibility (CONN-04)

- **D-12:** **Resolve the `assemble_code_mode_prompt` ↔ `build_code_mode_prompt`
  naming.** P83 shipped `pmcp_server_toolkit::code_mode::assemble_code_mode_prompt(connector, config) -> String`.
  CONN-04 spec text says `build_code_mode_prompt(connector) -> String`.
  Planner decides: (a) rename and ship a deprecated `pub use` alias for
  one minor version, OR (b) add `build_code_mode_prompt` as a thin alias
  next to the existing name. Either way the public surface satisfies
  CONN-04's literal naming requirement. Pick whichever is less
  churn for in-tree consumers (P83 example + Plan 08 smoke test).

### Testing Coverage (TEST-01 / TEST-07)

- **D-13:** **Per-backend `tests/integration.rs` covers the
  authentic-mock contract:** each test (a) constructs the connector
  against its in-process mock, (b) calls `execute()` with a
  representative parameterized query containing `:name` placeholders
  (verifying `translate_placeholders` is wired through), (c) calls
  `schema_text()` and asserts the result contains the mock's
  expected DDL fragments, (d) asserts the dialect identification
  via `connector.dialect()`. SQLite gets the same shape but against
  a real in-memory `rusqlite` DB instead of a mock.
- **D-14:** **TEST-07 fuzz target extends Phase 77's `pmcp_config_toml_parser`
  in-place — does NOT duplicate.** P83 ships a fuzz target in
  `crates/pmcp-server-toolkit/fuzz/`. P84 either adds a new
  `fuzz_targets/` entry exercising the now-richer config-shape
  surface (P84 may add new `[database]` keys per backend), or extends
  the existing target. Disposition: runtime stress in CI / nightly,
  same disposition as Phase 77 Plan 08. NEVER use Docker.
- **D-15:** **All ALWAYS coverage (CLAUDE.md) for every new public
  surface.** Each per-backend crate ships: (a) unit tests covering the
  connector impl, (b) property tests on placeholder translation
  applied through the per-backend execute path, (c) integration tests
  via authentic in-process mocks (D-13), (d) doctests on every public
  type/fn, (e) at least one example demonstrating Shape-C-shaped use.
  Fuzz coverage lives in the toolkit core (D-14).

### Claude's Discretion

- Exact rename strategy for CONN-04 (alias vs deprecated rename — see D-12).
- Concrete extended variants of `ConnectorError` for execute-time
  failures (likely `Driver(String)`, `QuerySyntax(String)`,
  `Connection(String)`, `ParameterBind { name: String, reason: String }`).
- Whether the AWS Athena connector needs both `aws-sdk-athena` AND
  `aws-sdk-glue` deps, or whether Athena's StartQueryExecution +
  GetTableMetadata cover the schema needs without a separate Glue
  client (researcher resolves from current AWS SDK docs).
- Whether `MysqlConnector` uses `sqlx::Pool<MySql>` or `sqlx::MySqlPool` —
  pure surface naming, picked from the current published `sqlx` API.
- Internal mutex/connection structure of `SqliteConnector` — rusqlite is
  sync, so the connector wraps `Arc<Mutex<Connection>>` (or `tokio::sync::Mutex`
  if test runs in a single-threaded runtime per project CLAUDE.md) and
  blocks under `spawn_blocking` from inside the `async fn execute()`.
- Workspace publish-order slot for the three new per-backend crates —
  CLAUDE.md §"Release & Publish Workflow" update edit (after
  `pmcp-server-toolkit`, before `mcp-tester` / `mcp-preview`).

### Reviewed Todos (not folded)

Two todos surfaced as matches but neither belongs in Phase 84 scope:

- `2026-03-04-create-readme-docs-for-cargo-pmcp-cli.md` — `cargo-pmcp`
  CLI docs. Owned by Phase 89 (DOCS-05); not Phase 84's surface.
- `2026-05-18-ship-sqlite-from-config-example-as-phase-86-shape-b-c-dogfood.md`
  — explicitly Phase 86 by the todo's own title. Phase 84 only needs
  to ship `SqliteConnector` (D-09); Shape B/C dogfood scaffolding is
  Phase 86's job.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spike Findings (Primary — implementation blueprint)

- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — ordering, requirements baseline, dual-surface invariant
- `.claude/skills/spike-findings-rust-mcp-sdk/references/schema-server-sql-dialects.md` — `SqlConnector` 3-method trait shape, `Dialect` enum, `translate_placeholders` reference impl, per-backend connector skeleton (Postgres example), "What to avoid" rules (no Docker, no normalization, no trait-method translation, no shared pools)
- `.claude/skills/spike-findings-rust-mcp-sdk/references/schema-server-architecture.md` — broader toolkit shape this connector trait fits into
- `.planning/spikes/005-multi-dialect-sql-connector/` — VALIDATED spike source for the trait + per-dialect mock pattern (`postgres_mock` / `mysql_mock` / `athena_mock`)
- `.planning/spikes/004-schema-server-thin-slice-sql/` — Shape C 12-line user surface validation (Phase 86 target the connectors must support)

### Phase 83 Predecessor Artifacts (MUST read first)

- `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/83-CONTEXT.md` — toolkit publish/cadence decisions, D-12 trait-stub justification, D-14 feature flag matrix
- `crates/pmcp-server-toolkit/src/sql/mod.rs` — current 2-method `SqlConnector` stub + `Dialect` enum + `ConnectorError` + the `pub(crate) MockSqlConnector` that becomes `SqliteConnector` in P84
- `crates/pmcp-server-toolkit/src/code_mode.rs` — P83 `assemble_code_mode_prompt(connector, config)` that CONN-04 renames/aliases as `build_code_mode_prompt`
- `crates/pmcp-server-toolkit/src/tools.rs` — P83 `synthesize_from_config` that builds the tool handlers that must emit `structuredContent` (D-06)
- `crates/pmcp-server-toolkit/src/config.rs` — `ServerConfig` parser whose fuzz target P84 extends (D-14)
- `crates/pmcp-server-toolkit/Cargo.toml` — current feature matrix; P84 may add per-backend `[features]` declarations (`postgres`, `mysql`, `athena`) if it chooses to wire feature-gated test paths

### Requirements & Roadmap

- `.planning/REQUIREMENTS.md` §"SQL Connectors" — CONN-01..08 full text
- `.planning/REQUIREMENTS.md` §"Testing" — TEST-01 (per-backend integration tests + no Docker), TEST-07 (fuzz extends P77)
- `.planning/ROADMAP.md` §"Phase 84" — goal, depends-on (Phase 83), 5 success criteria (SC-1..5)

### Reference Server Configs (REF-01 superset anchor — must parse cleanly into ServerConfig with per-backend connectors)

- `pmcp-run/built-in/sql-api/servers/open-images/config.toml` — Athena backend exemplar (394 lines, `[[database.tables]]` + `[code_mode]` + `[[tools]]`)
- `pmcp-run/built-in/sql-api/servers/imdb/config.toml` — MySQL backend parity
- `pmcp-run/built-in/sql-api/servers/msr-vtt/config.toml` — third reference parity
- `pmcp-run/built-in/sql-api/reference/scenarios/` — Phase 85 result-parity replay target; P84 connectors must produce row shapes compatible with these scenarios

### Workspace & Release Conventions

- `Cargo.toml` (root) §`[workspace.members]` — insertion point for `crates/pmcp-toolkit-postgres`, `…-mysql`, `…-athena`
- `CLAUDE.md` §"Release & Publish Workflow" — current publish order (`widget-utils → pmcp → pmcp-server-toolkit → mcp-tester → mcp-preview → cargo-pmcp`); P84 inserts the three per-backend crates after `pmcp-server-toolkit` (D-10)
- `CLAUDE.md` §"ALWAYS Requirements for New Features" — defines the unit + property + integration + doctest + example + fuzz coverage matrix D-15 commits to
- `CLAUDE.md` §"PMAT Quality-Gate Proxy Mode" + §"Pre-Commit Quality Gates" — cognitive complexity ≤25 per function; clippy zero-warning; doctest green

### Memory & Conventions

- `.claude/projects/-Users-guy-Development-mcp-sdk-rust-mcp-sdk/memory/feedback_avoid_docker_pure_rust_lambda.md` — pure-Rust drivers only; `tokio-postgres` / `sqlx` / `aws-sdk-athena` / `rusqlite bundled`. No Docker / `testcontainers` anywhere. Authentic in-process mocks + pure-Rust drivers cover TEST-01.
- `.claude/projects/-Users-guy-Development-mcp-sdk-rust-mcp-sdk/memory/feedback_v2_cleanup.md` — during the v2.x breaking-change window, consolidate aggressively. CONN-04 rename (D-12) and the `MockSqlConnector → SqliteConnector` promotion (D-09) both fit this disposition.

### Phase 77 Reuse Anchor (TEST-07)

- `fuzz/fuzz_targets/pmcp_config_toml_parser.rs` (or current location) — existing fuzz target the toolkit config parser extends per D-14

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`crates/pmcp-server-toolkit/src/sql/mod.rs`** — Phase 83 already shipped
  `SqlConnector` (2 methods), `Dialect` (4 variants with `name()` +
  `placeholder_guidance()`), `ConnectorError` (Io / Schema /
  DialectMismatch), and `pub(crate) MockSqlConnector` under
  `cfg(any(test, feature = "sqlite"))`. P84 EXTENDS this file — does not
  rewrite. Adding `execute()` to the trait is semver-compatible because
  the trait isn't published to crates.io yet (0.1.0 still); adding
  ConnectorError variants is additive because the enum is
  `#[non_exhaustive]`.
- **`crates/pmcp-server-toolkit/src/code_mode.rs`** — P83's
  `assemble_code_mode_prompt(connector, config)` calls
  `connector.schema_text()` and folds in `[[database.tables]]`
  descriptions. P84's per-backend `schema_text()` impls plug into this
  for free.
- **`crates/pmcp-server-toolkit/src/tools.rs`** — P83's
  `synthesize_from_config` builds the `Arc<dyn ToolHandler>` per
  `[[tools]]` entry. P84 must verify these handlers emit
  `structuredContent` (D-06); the synthesizer landed before `execute()`
  existed, so the handler body that calls `connector.execute()` is part
  of P84's surface to plan.
- **`pmcp::ServerBuilder::{tool_arc, prompt_arc, get_tool, get_prompt}`** —
  landed in Phase 82. P84's per-tool handlers register through these
  same primitives via P83's `code_mode_from_config` builder extension.
- **Phase 77 fuzz target on `pmcp_config_toml_parser`** — extends to
  cover the toolkit config types per D-14.

### Established Patterns

- **`async_trait` on connector traits** — already used by `SqlConnector`
  in P83. Per-backend impls inherit the same pattern.
- **`#[non_exhaustive]` on extensible enums** — `Dialect` and
  `ConnectorError` in P83 already use this. P84 adds variants without
  semver impact.
- **`#[serde(deny_unknown_fields)]` on config types** — P83's
  `ServerConfig` already enforces strict parsing. P84's per-backend
  `[database]` sections inherit this; renames are forbidden but new
  keys are additive.
- **Workspace-version dep pattern** — `pmcp = { version = "...", path = "..." }`
  used by every workspace crate. The three new per-backend crates pin
  `pmcp-server-toolkit` and `pmcp-code-mode` the same way (D-05 of P83).
- **PMAT cognitive-complexity ≤25 per function** — Phase 75 CI gate.
  `translate_placeholders` is a state machine over chars; P84 may need
  to split helpers or carry a `// Why:`-annotated `#[allow]` per the
  Phase 75 template.
- **Per-backend `tests/mock_*.rs` + `tests/integration.rs` layout** —
  matches the Rust 2021 `tests/` directory convention; spike 005
  validated this layout.

### Integration Points

- **`pmcp-server-toolkit`** — the three new per-backend crates depend on
  it as a workspace dep. The toolkit's trait extension lands first
  (within toolkit core), then the per-backend crates implement it.
- **`Cargo.toml` root `[workspace.members]`** — three new entries get
  inserted (current location: lines ~540–543 per P83's STATE.md note).
- **`CLAUDE.md` Release & Publish Workflow** — publish-order update edit:
  insert `pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`,
  `pmcp-toolkit-athena` immediately after `pmcp-server-toolkit`.
- **`pmcp-run/built-in/shared/mcp-server-common/`** — P83's re-export
  shim is in operator-handoff territory; P84 should not regress
  whatever shape the shim was published under (the toolkit's
  `[[tools]]` synthesizer + `code_mode` wiring is what those backend
  cores will eventually consume through the public API).

</code_context>

<specifics>
## Specific Ideas

- **StructuredOutput everywhere on the tool surface** (user-stated
  during discussion): every tool handler built by the
  P83 synthesizer wraps `Vec<serde_json::Value>` row results into the
  `tools/call` response's `structuredContent` field so MCP App UI
  widgets can render them as structured JSON and downstream chained
  tool calls can consume them without re-parsing. Captured as D-06;
  flagged here so the planner and researcher both know this is a
  user-flagged invariant, not just a "default."
- **Shape C ≤15-line target preserves** — every per-backend constructor
  shape choice (D-08 URL string + internal pool) is evaluated against
  this constraint. Phase 86's library example will assert it
  in-binary; P84's constructor choices must not break it.
- **REF-01 superset invariant remains in force** — the three reference
  configs (`open-images` / `imdb` / `msr-vtt`) parse cleanly into the
  toolkit's `ServerConfig`. P84 may add new `[database]` keys
  per-backend; renames are forbidden. Strict parsing (P83 D-13)
  enforces this; the per-backend `[database]` shapes need to align
  with what the reference servers already write.
- **Lambda-deployable, pure-Rust, no Docker, no testcontainers** —
  reaffirmed for the third phase running. Constrains test
  infrastructure (D-07 in-process mocks), driver selection (D-10
  all pure-Rust), and fuzz infrastructure (D-14 no containerized
  runtime).

</specifics>

<deferred>
## Deferred Ideas

- **Streaming `execute_stream()` method on `SqlConnector`** — defer to a
  semver-additive minor release once a real consumer hits the
  large-result-scan use case (e.g., a future Athena warehousing tool).
  Trait rustdoc documents the deferred evolution plan per D-02.
- **Transaction support on `SqlConnector`** — defer; v2.2 reference
  scenarios are read-only and Athena has no real tx model. Land as a
  separate `SqlTransactional` trait extension when a real consumer
  needs it (likely Phase 88 dogfood, if pmcp-server has stateful ops).
- **External pool injection (`with_pool(Arc<Pool>)`) per backend** —
  defer; Shape C ≤15-line target wins for v0.2. Add additively in a
  future minor release if a tuning ceiling shows up in production
  deployments.
- **Shared `pmcp-toolkit-test-support` crate** — defer; per-backend
  `tests/` dir covers TEST-01 today and no external consumer has asked
  for a shared mock crate yet.
- **GraphQL / OpenAPI connector crates** — `GQL-TKIT-01` and
  `OAPI-TKIT-01` are explicitly next-milestone (out of scope per
  ROADMAP "Future Requirements" + spike 007 gating).
- **`#[pmcp::sql_server]` proc-macro over the toolkit** — `PMACRO-SQL-01`
  in ROADMAP "Future Requirements" — gated on the toolkit being public
  on crates.io (which P83 already delivered, so this could light up in
  a future milestone).
- **Cross-backend tool federation (FED-01)** — one MCP server serving
  tools from Postgres + Athena + … composed via toolkit core. Not in
  v2.2 scope; deferred per ROADMAP "Future Requirements."
- **Type 1 ai-agents/ skill updates (SKLL-07)** — Phase 87 owns the
  Type-2 authoring skills server AND Type-1 skill refreshes for
  toolkit-authoring patterns. P84 does NOT touch `ai-agents/`.

### Reviewed Todos (not folded)

- `2026-03-04-create-readme-docs-for-cargo-pmcp-cli.md` — owned by
  Phase 89 (DOCS-05), not Phase 84.
- `2026-05-18-ship-sqlite-from-config-example-as-phase-86-shape-b-c-dogfood.md`
  — explicitly Phase 86 by the todo's own title. P84 ships the
  `SqliteConnector` infrastructure (D-09); Shape B/C dogfood
  scaffolding is Phase 86.

</deferred>

---

*Phase: 84-sql-connectors-postgres-mysql-athena-sqlite*
*Context gathered: 2026-05-19*
