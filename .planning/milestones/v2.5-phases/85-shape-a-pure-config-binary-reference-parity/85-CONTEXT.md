# Phase 85: Shape A Pure-Config Binary + Reference Parity - Context

**Gathered:** 2026-05-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Build `pmcp-sql-server` — a generic CLI binary (crate `crates/pmcp-sql-server/`)
that takes `--config <toml> --schema <ddl-file>`, reads `[database] type`,
instantiates the matching Phase 84 connector (sqlite / postgres / mysql /
athena), and serves a live MCP server over **streamable HTTP** with **zero Rust
written by the user**. Then prove it reproduces a production pmcp-run reference
server (same tools, same code-mode policy, same observable behavior) by
replaying the canonical reference scenarios and asserting result parity.

**In scope:**

- New workspace crate `crates/pmcp-sql-server/` with binary `pmcp-sql-server`,
  publishing AFTER the per-backend connector crates (`pmcp-toolkit-postgres`,
  `-mysql`, `-athena`) per CLAUDE.md §"Release & Publish Workflow".
- CLI surface `pmcp-sql-server --config <toml> --schema <ddl> [--http <addr>]`
  built on the toolkit's `ServerConfig` parser (Phase 83) + the four Phase 84
  connectors, all linked behind cargo features (all default-on).
- Runtime backend dispatch: map `[database] type` → connector; a `type` whose
  feature was compiled-out produces a clear, actionable error.
- `--schema` DDL-file ingestion → fed to the code-mode prompt assembly
  (`build_code_mode_prompt`) AND surfaced as the MCP schema **resource**
  (replacing live `schema_text()` for Phase 85).
- Streamable-HTTP serving via the Phase 56 Tower/axum adapter; binary binds a
  local address and serves the full MCP surface (tools/list, tools/call,
  resources, prompts, code-mode validate_code/execute_code).
- **REF-01 superset (SC-2):** assert `reference/config.toml` (SQLite) AND all
  three Athena/MySQL server configs (open-images / imdb / msr-vtt) parse
  cleanly into the toolkit `ServerConfig`.
- **SC-1 non-SQLite coverage:** assert the binary builds the right connector
  and serves `tools/list` for non-SQLite configs via **lazy** connection (no
  live backend, no creds) — Phase 84 connectors already use `connect_lazy`.
- **REF-02 / SC-3 / SC-4 parity (SQLite Chinook):** spawn the Shape A repro
  against `reference/config.toml` + a committed Chinook DDL fixture, replay
  `reference/scenarios/generated.yaml` via mcp-tester, assert the 29 encoded
  scenarios pass — covering curated tools, the `validate_code`/`execute_code`
  code-mode pair, prompts, and resources in one harness.
- ALWAYS coverage (CLAUDE.md): unit + property + integration + doctests +
  a runnable example + fuzz reuse (the config-parser fuzz target already
  extended in Phase 84).

**Out of scope (other phases own these):**

- `cargo pmcp new --kind sql-server` scaffold, ≤15-line library example,
  `cargo pmcp deploy` config-only target → Phase 86 (Shapes B/C/D).
- `pmcp-config-helper` Type 2 authoring Skills MCP server → Phase 87.
- `crates/pmcp-server` dogfood rewrite → Phase 88.
- Book chapter / course tutorial / migration recipe → Phase 89.
- **Live introspection** (`--introspect` / `--schema-from-db`) — deferred; the
  first built-in (SQL) type ships file-based `--schema` only.
- **stdio transport** — deferred; streamable HTTP only for Phase 85.
- **Live Athena/MySQL/Postgres query execution** in CI — covered by Phase 84
  per-backend authentic mocks; Phase 85 only proves lazy startup for them.
- **DynamoDB/AVP runtime policy overrides** — pmcp-run Lambda-only; Shape A
  enforces `[code_mode]` policy statically from config.

</domain>

<decisions>
## Implementation Decisions

### Parity Target Backend (REF-02, SC-4)

- **D-01:** **Reproduce the SQLite Chinook reference server, not Athena
  open-images.** The binding result-parity test targets
  `pmcp-run/built-in/sql-api/reference/config.toml` + `chinook.db`. This is
  pure-Rust, runs in CI with zero cloud creds, and the 29 scenarios in
  `reference/scenarios/generated.yaml` already encode that server's behavior.
  This is also the *more faithful* reading of REF-02 — its verification clause
  literally says "replaying … `reference/scenarios/`", which target Chinook.
  REF-02's "(open-images recommended)" parenthetical is satisfied in intent by
  the SQLite reference that actually owns the scenarios. **Deviation note for
  the verifier:** REF-02's literal "open-images" wording is intentionally NOT
  the parity target; record this as an approved scope reading, not a gap.
- **D-02:** **open-images / imdb / msr-vtt are parse-only here (SC-2).** Their
  configs must parse cleanly into the toolkit `ServerConfig` (REF-01 superset),
  and the binary must build the right connector + serve `tools/list` via lazy
  connection (D-09), but full live-query parity against Athena/MySQL is out of
  automated scope (needs cloud creds).

### `--schema` Flag Semantics (SHAP-A-01)

- **D-03:** **Two distinct config inputs with clear roles.** `config.toml` =
  server + operation configuration *including* the `[code_mode]` block
  (operation→query mapping, security policy, table allow/block lists).
  `--schema <file>` = the **code-mode schema resource** the LLM sees to generate
  long-tail queries. Format is standardized per backend type: **DDL for SQL**
  (Swagger/OpenAPI and GraphQL SDL are future-milestone, but the flag is
  designed format-agnostic / backend-typed now).
- **D-04:** **Admin-provided schema FILE is the default and only path in Phase
  85.** Live introspection (direct DB/Glue/endpoint calls) is explicitly
  deferred — it may require extra build-time permissions, and a file lets the
  admin **edit/redact** the schema before it becomes a public MCP resource.
  `--schema` is therefore effectively **required** for the SQL built-in. A
  future `--introspect` opt-in (calling the already-shipped
  `connector.schema_text()`) is a deferred idea.
- **D-05:** **The `--schema` file content is surfaced as the MCP schema
  resource** (the `docs://…/schema` analog). Planner's discretion: serve it
  **verbatim**, OR prepend a header / append a footer with guidance text to
  improve LLM SQL-generation success. Either way, what's in the file *is* what
  the client/LLM can read — preserving the redaction guarantee.
- **D-06:** **Chinook parity fixture:** generate the Chinook DDL once (e.g.
  `sqlite3 chinook.db .schema` or dumping `SqliteConnector::schema_text()`),
  commit it as a test fixture, and pass it via `--schema`. This keeps the Shape
  A repro behaviorally identical to production (whose code-mode prompt is
  seeded from equivalent schema text).

### Backend Dispatch & Bundling (SHAP-A-01, SC-1)

- **D-07:** **All four connectors linked, feature-gated, all default-on.** The
  binary depends on sqlite + postgres + mysql + athena connectors; each behind a
  cargo feature, all enabled by default so `cargo install pmcp-sql-server`
  works against any reference config out of the box. Athena pulls `aws-sdk-*`
  (heavy) but a universal binary is the non-developer promise. Lean
  single-backend builds remain possible via `--no-default-features --features
  sqlite`.
- **D-08:** **Runtime dispatch on `[database] type`.** Match the config's
  `type` string → connector constructor. A `type` whose feature was compiled
  out yields a clear error naming the missing feature (e.g. "config requires
  backend 'athena' but this binary was built without the `athena` feature").
- **D-09:** **Lazy connection at startup (no live backend for non-SQLite).**
  Connector construction must NOT require a live backend or creds. Phase 84
  already satisfies this: `MysqlConnector::connect` uses `connect_lazy`
  (REVIEWS M3), `PostgresConnector` is symmetric, `AthenaConnector::from_config`
  just builds an SDK client. The binary builds the connector and serves
  `tools/list` without opening a connection; real I/O surfaces on first query.

### Parity Verification Method (REF-02, SC-3, SC-4)

- **D-10:** **mcp-tester replay against the scenarios IS the parity assertion.**
  Spawn the Shape A repro, replay `reference/scenarios/generated.yaml` via
  mcp-tester, assert all 29 scenarios pass. The scenarios already encode the
  production server's expected behavior, so passing them = result parity — no
  need to build/run the pmcp-run Lambda. Rejected: dual live-run + diff (needs
  the Lambda running), golden snapshot (staleness risk).
- **D-11:** **SC-3 (code-mode policy parity) rides the same replay.**
  `generated.yaml` exercises `validate_code` (8×), the `validate_code →
  execute_code` flow, and the `start_code_mode` prompt — so the single replay
  validates curated tools + code-mode policy enforcement + prompts + resources.
  No separate SC-3 harness.
- **D-12:** **Streamable HTTP only.** The binary serves streamable HTTP (Phase
  56 Tower/axum adapter); the parity harness spawns it on a local address,
  waits for readiness, and points mcp-tester at the URL. stdio is deferred. This
  matches the pmcp-run Lambda's production transport surface.
- **D-13:** **Code-mode policy enforced statically from `[code_mode]` config.**
  No DynamoDB/AVP runtime overrides (those are pmcp-run Lambda-only). Static
  enforcement of the config's default policy == the behavior the scenarios
  encode, so static policy is the correct parity surface.

### Claude's Discretion

- Exact crate/feature layout of `crates/pmcp-sql-server/` (feature names should
  mirror the connector crate names: `sqlite`, `postgres`, `mysql`, `athena`).
- The `--schema` resource wrapper (verbatim vs header/footer-augmented — D-05).
- The HTTP CLI flag shape (`--http <addr>` vs `--transport streamable-http
  --bind <addr>`) and default bind address/port.
- HMAC `token_secret` sourcing for code-mode tokens (config `[code_mode]
  .token_secret` + env expansion — the toolkit already supports env-var
  expansion in config).
- Error-UX wording for compiled-out backends and malformed config/schema files.
- How readiness is detected by the parity harness (health poll vs fixed wait —
  prefer a poll).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Reference Implementation — Parity Target (read first)

- `pmcp-run/built-in/sql-api/reference/config.toml` — the SQLite Chinook
  reference server config; `[database] type = "sqlite"`, `file_path`,
  `[[database.tables]]` descriptions, `[code_mode]` block. The exact config
  Shape A reproduces.
- `pmcp-run/built-in/sql-api/reference/chinook.db` — the demo SQLite database
  the parity test runs against (digital media store, ~1 MB).
- `pmcp-run/built-in/sql-api/reference/scenarios/generated.yaml` — 31
  mcp-tester scenarios (list_tools/resources/prompts, `search_tracks`,
  `list_artists`, `get_album_tracks`, `validate_code`×8, `execute_code`,
  `start_code_mode` prompt). The replay target that defines parity (D-10/D-11).
- `pmcp-run/built-in/sql-api/reference/sql-reference-lambda/src/main.rs` — the
  ~30-line production binary shape Shape A generalizes (load config → build
  `SqliteConnector` → run server). Note: production uses `SqlConfig` /
  `SqlMcpServer` from `mcp-sql-server-core`; Shape A uses the **toolkit**
  `ServerConfig` + Phase 84 connectors instead.

### Reference Server Configs (REF-01 superset / SC-2 parse-only)

- `pmcp-run/built-in/sql-api/servers/open-images/config.toml` — Athena backend
  (`type = "athena"`, workgroup, output_location, Glue tables).
- `pmcp-run/built-in/sql-api/servers/imdb/config.toml` — MySQL backend.
- `pmcp-run/built-in/sql-api/servers/msr-vtt/config.toml` — third reference.

### Toolkit Core + Connectors (the surface Shape A wires together)

- `crates/pmcp-server-toolkit/src/config.rs` — `ServerConfig` parser (strict
  `deny_unknown_fields`, env expansion) that ingests all the above configs.
- `crates/pmcp-server-toolkit/src/code_mode.rs` — `build_code_mode_prompt` /
  `assemble_code_mode_prompt`; where the `--schema` DDL text feeds in (D-05).
- `crates/pmcp-server-toolkit/src/tools.rs` — `synthesize_from_config` builds
  the `[[tools]]` handlers that emit `structuredContent`.
- `crates/pmcp-server-toolkit/src/sql/mod.rs` — `SqlConnector` trait, `Dialect`,
  `SqliteConnector`, `translate_placeholders`.
- `crates/pmcp-toolkit-postgres/src/lib.rs`, `crates/pmcp-toolkit-mysql/src/lib.rs`,
  `crates/pmcp-toolkit-athena/src/lib.rs` — the three connectors the binary
  dispatches to; note `connect_lazy` (REVIEWS M3) confirming D-09 lazy startup.
- `crates/pmcp-server-toolkit/src/builder_ext.rs` — `ServerBuilderExt` /
  `code_mode_from_config` wiring the binary uses to build the `pmcp::Server`.

### HTTP Transport + Test Tooling

- `src/server/` Tower/axum streamable-HTTP adapter (Phase 56) — D-12 transport.
- `crates/mcp-tester/` — the scenario-replay engine (D-10); confirm its
  `generated.yaml` schema + how it targets an HTTP URL.

### Requirements & Roadmap

- `.planning/REQUIREMENTS.md` — SHAP-A-01, REF-01, REF-02 (and REF-03 context,
  owned by Phase 89).
- `.planning/ROADMAP.md` §"Phase 85" — goal, depends-on (Phase 84), SC-1..4.
- `.planning/phases/84-sql-connectors-postgres-mysql-athena-sqlite/84-CONTEXT.md`
  — connector trait shape (D-01..D-15), structuredContent contract (D-06),
  lazy-pool / pure-Rust / no-Docker constraints.
- `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/83-CONTEXT.md` —
  `ServerConfig`, code-mode wiring, REF-01 superset enforcement.

### Memory & Conventions

- `.claude/projects/-Users-guy-Development-mcp-sdk-rust-mcp-sdk/memory/feedback_avoid_docker_pure_rust_lambda.md`
  — pure-Rust drivers, no Docker/testcontainers; authentic in-process mocks.
- `CLAUDE.md` §"Release & Publish Workflow" — `pmcp-sql-server` publish slot
  (after the per-backend connector crates).
- `CLAUDE.md` §"ALWAYS Requirements" + §"PMAT Quality-Gate" — coverage matrix +
  cognitive-complexity ≤25.
- `.claude/skills/spike-findings-rust-mcp-sdk/` — schema-server architecture +
  SQL dialect references (auto-loaded).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`mcp-server-common` shape lifted into `pmcp-server-toolkit`** (Phase 83):
  `ServerConfig` parser, `synthesize_from_config` tool builder, code-mode prompt
  assembly, static resources/prompts, HMAC tokens. Shape A is the thin CLI that
  binds these to a connector + HTTP transport — almost no new domain logic.
- **Phase 84 connectors already lazy** — `connect_lazy` (MySQL, REVIEWS M3),
  symmetric Postgres, `from_config` Athena (SDK client, no connect). D-09 lazy
  startup needs no new connector work.
- **`SqliteConnector::open(path)` / `::open_in_memory()`** (Phase 84 D-09) — the
  parity-target connector, pure-Rust `rusqlite bundled`.
- **`generated.yaml` mcp-tester scenarios** cover curated tools + code-mode +
  prompts + resources — one replay validates SC-3 and SC-4.
- **Phase 56 Tower/axum streamable-HTTP adapter** — the transport the binary
  serves (D-12); reuse, don't re-implement.

### Established Patterns

- **`#[serde(deny_unknown_fields)]` strict config parsing** (Phase 83) — enforces
  REF-01 "additive keys allowed, renames forbidden" automatically.
- **Cargo feature gating per backend** — connector crates are already separate;
  the binary mirrors them as `sqlite`/`postgres`/`mysql`/`athena` features
  (D-07), all default-on.
- **Env-var expansion in config** (`${AWS_ACCOUNT_ID}` etc.) — toolkit
  `ServerConfig` already supports it; the binary inherits it for secrets/region.
- **Workspace-version dep pattern** — `pmcp-sql-server` pins
  `pmcp-server-toolkit` + the four connector crates the same way.
- **`--test-threads=1`** (project CI convention) — the parity harness spawns a
  server + replays; keep it single-threaded-safe.

### Integration Points

- **Root `Cargo.toml` `[workspace.members]`** — insert `crates/pmcp-sql-server`.
- **`[database] type` → connector** — the one new dispatch seam (D-08).
- **`--schema` file → code-mode prompt + schema resource** — D-05 seam between
  the new CLI and the existing `build_code_mode_prompt` / resource handler.
- **mcp-tester ← spawned HTTP server** — the parity harness integration point
  (D-10/D-12).

</code_context>

<specifics>
## Specific Ideas

- **Two-input mental model (user-stated):** `config.toml` carries server +
  operation + code-mode policy; the **schema file** carries the code-mode
  schema resource. The schema file is admin-authored in a standard format (DDL
  for SQL, Swagger for OpenAPI, SDL for GraphQL), easy to generate and editable
  so admins can redact what shouldn't be a public MCP resource. Live
  introspection is a deliberate *option*, not the default — gated by build-time
  permissions. Captured as D-03/D-04/D-05.
- **Schema-resource wrapping (user-stated):** the schema file may be served
  verbatim OR with prepended/appended header/footer text to help the LLM
  generate successful SQL. Planner picks the exact wrapper. (D-05.)
- **Universal binary promise (user choice):** `cargo install pmcp-sql-server`
  should run any reference config out of the box → all four backends default-on
  (D-07), accepting the aws-sdk weight.
- **HTTP-only transport (user choice):** the Shape A binary matches the
  production Lambda's streamable-HTTP surface rather than local stdio (D-12).
- **No cloud creds in CI (reaffirmed):** SQLite parity target + parse-only +
  lazy-startup for non-SQLite keep Phase 85 fully automatable; pure-Rust,
  no Docker, no testcontainers.

</specifics>

<deferred>
## Deferred Ideas

- **Live schema introspection (`--introspect` / `--schema-from-db`)** — opt-in
  alternative to the `--schema` file, calling the already-shipped
  `connector.schema_text()`. Deferred past the SQL built-in; lands additively
  when an admin wants build-time introspection (and accepts the credential
  cost). (D-04.)
- **stdio transport** — Phase 85 is HTTP-only; add stdio if a local-CLI/embedded
  use case (e.g. Shape B/C scaffold) needs it. (D-12.)
- **Full live Athena/MySQL/Postgres parity smoke** — manual/gated tests that
  connect to real cloud backends; reopens the credential dependency, so out of
  Phase 85's automated scope. (D-02.)
- **OpenAPI (Swagger) / GraphQL (SDL) `--schema` formats** — the flag is
  designed backend-typed for these, but the connectors are next-milestone
  (GQL-TKIT-01 / OAPI-TKIT-01). (D-03.)
- **`cargo pmcp deploy` of the Shape A binary to pmcp.run** — Shape D, Phase 86.
- **Migration recipe (REF-03)** — pmcp-run author swapping path-deps for the
  public toolkit; owned by Phase 89 (DOCS-01).

### Reviewed Todos (not folded)

None — no pending todos matched Phase 85 scope.

</deferred>

---

*Phase: 85-shape-a-pure-config-binary-reference-parity*
*Context gathered: 2026-05-26*
