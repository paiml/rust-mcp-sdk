# pmcp-sql-server

Shape A pure-config SQL MCP server binary — point it at a `config.toml` + a schema file and serve a production MCP server with **no Rust required**.

**Status:** 0.1.0 — early access. The pipeline is fully implemented; the public config/CLI surface may evolve as DX matures across Phases 85–89.

> **Most people don't run this binary directly.** The recommended way to build and
> ship a config-driven SQL MCP server is the `cargo pmcp` CLI:
> `cargo pmcp new my-server --kind sql-server` scaffolds a project, `cargo run`
> serves it locally, and `cargo pmcp deploy` ships it to AWS Lambda / Cloud Run /
> Cloudflare / pmcp.run. See the user guide in
> [`cargo-pmcp/README.md`](../../cargo-pmcp/README.md#config-driven-sql-server-new---kind-sql-server)
> (and the *Config-Driven SQL Servers* chapters in the PMCP book and course).
>
> **This README is for the other path:** running the prebuilt `pmcp-sql-server`
> binary as-is, or **extending the toolkit** (adding a new SQL dialect/connector).
> The CLI scaffold ("Shape B") generates a small crate that uses the
> [`pmcp-server-toolkit`](../pmcp-server-toolkit) *library* with its own `main.rs`;
> this crate ("Shape A") is the standalone, no-Rust-required *binary*. They are
> siblings built on the same toolkit — the scaffold does not invoke this binary.

## The improvement (why this exists)

To expose a SQL database over the Model Context Protocol today, you hand-write a Rust binary against the SDK: wire a `ServerBuilder`, implement a tool handler for every query, construct and manage the database connector, wire up Code Mode policy, stand up the HTTP transport, and **recompile for every schema or tool change**.

`pmcp-sql-server` collapses that into two inputs and one binary:

- A **`config.toml`** declaring `[server]`, `[database]`, `[code_mode]`, `[[tools]]`, and optional `[[resources]]` / `[[prompts]]`.
- A **schema file** (DDL text) served as the Code Mode schema resource.

You run one binary, change tools or schema by editing the config, and never recompile. It is the runnable binary built on top of the [`pmcp-server-toolkit`](../pmcp-server-toolkit) library — everything the binary does is wiring of toolkit primitives plus a `[database] type` → connector dispatch seam.

It follows the **Pareto model**: roughly 20% of operations are covered by curated `[[tools]]` rows, and the ~80% long tail is handled by **Code Mode** generating SQL against the schema resource. You curate the common operations; Code Mode handles the rest.

## What this crate is NOT

- **Not the library.** The reusable building blocks (config types, connectors, the `[[tools]]` synthesizer, Code Mode wiring) live in `pmcp-server-toolkit`. This crate is the runnable binary on top of it.
- **Not a DynamoDB / NoSQL toolkit.** It serves SQL backends only (SQLite, Postgres, MySQL, Athena).
- **Not a SQL dialect of its own.** You supply the schema DDL and the backend URL/path; the binary connects to your real database and runs your declared SQL.

## Supported backends

The `[database] type` value in your config selects the connector. Each backend requires a small, fixed set of `[database]` fields:

| `type`     | Connector           | Required `[database]` fields                                                                                         |
| ---------- | ------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `sqlite`   | `SqliteConnector`   | `file_path` **or** `database` (a path, or `:memory:`)                                                                 |
| `postgres` | `PostgresConnector` | `url`                                                                                                                |
| `mysql`    | `MysqlConnector`    | `url`                                                                                                                |
| `athena`   | `AthenaConnector`   | `workgroup` (region from `AWS_REGION` / `AWS_DEFAULT_REGION`, default `us-east-1`; optional `output_location`, `database`) |

All four backends are compiled in by default, so the published binary serves any of them from config alone. For a lean single-backend build, opt out of the connectors you don't need:

```bash
cargo build -p pmcp-sql-server --release --no-default-features --features sqlite
```

If a config names a backend whose feature was compiled out, the binary fails fast with an actionable error — `rebuild with --features <name>` — rather than falling back to the wrong backend. An unrecognized `type` reports `supported: sqlite, postgres, mysql, athena`.

## Quickstart

> **Fastest path — the shipped `sqlite-explorer` example.** This crate ships a
> complete, runnable SQLite config under `examples/` (only `tests/` is excluded
> from the published crate). From a checkout of this repo (paths are relative to
> this crate directory; copy the two files anywhere to run them standalone):
>
> ```bash
> # 1. seed a tiny demo database (the binary connects to an EXISTING db; it does
> #    not create or seed one)
> sqlite3 /tmp/pmcp-sqlite-explorer.db < examples/sqlite-explorer.sql
>
> # 2. serve it — two curated tools (list_books, books_by_author) + Code Mode
> #    for the long tail, all from config alone
> pmcp-sql-server \
>   --config examples/sqlite-explorer.toml \
>   --schema examples/sqlite-explorer.sql \
>   --http 127.0.0.1:8080
> ```
>
> `examples/sqlite-explorer.sql` is used twice: to seed the demo DB (step 1) and
> as the `--schema` Code Mode resource (step 2). The walkthrough below builds the
> same shape by hand.

### 1. Build / install

```bash
# Build the all-backends binary (default features).
cargo build -p pmcp-sql-server --release

# Or install it on your PATH.
cargo install --path crates/pmcp-sql-server
```

### 2. A minimal `config.toml`

```toml
[server]
name = "SQL Server Min Demo"
version = "0.1.0"
type = "sql-api"

[database]
type = "sqlite"
file_path = ":memory:"

# Every `${ENV_VAR}` / `env:VAR` reference needs a `[[config_slots]]` entry
# naming its key. That list is what tells a target environment what it must
# supply, and `cargo pmcp package save` REFUSES a config that defers a value
# nothing declares (pmcp-package 0.3.1). `kind = "secret"` is identity-bearing,
# so it carries no `tested_value`.
[[config_slots]]
key = "code_mode.token_secret"
kind = "secret"
name = "CODE_MODE_SECRET"

[code_mode]
enabled = true
allow_writes = false
# Supports "${ENV_VAR}" interpolation — resolved at wiring time.
token_secret = "${CODE_MODE_SECRET}"

# ~20% curated tools; Code Mode covers the long tail against the schema resource.
[[tools]]
name = "list_artists"
description = "List all artists"
sql = "SELECT ArtistId, Name FROM Artist ORDER BY Name LIMIT :limit"

[[tools.parameters]]
name = "limit"
type = "integer"
description = "Maximum number of artists"
required = false
default = 20
```

A complete, runnable config ships with the crate at [`examples/sqlite-explorer.toml`](examples/sqlite-explorer.toml) (with its seed/schema [`examples/sqlite-explorer.sql`](examples/sqlite-explorer.sql)) — see the callout at the top of this section. A larger reference — a Chinook demo with curated tools, schema/examples/learnings `[[resources]]`, and a `start_code_mode` `[[prompts]]` entry — lives at [`tests/fixtures/reference-config.toml`](tests/fixtures/reference-config.toml) (in-repo only; `tests/` is excluded from the published crate).

### 3. Run it

```bash
# Required: --config (server config) and --schema (DDL served as the schema resource).
pmcp-sql-server --config config.toml --schema schema.ddl

# Override the bind address (default 127.0.0.1:8080).
pmcp-sql-server --config config.toml --schema schema.ddl --http 0.0.0.0:9000

# Control log verbosity via RUST_LOG (the binary inits a tracing EnvFilter).
RUST_LOG=info pmcp-sql-server --config config.toml --schema schema.ddl
```

The server is served over the streamable-HTTP transport. By default it binds loopback (`127.0.0.1:8080`) and restricts origins to localhost, so the out-of-the-box binary does not expose a public listener.

### 4. Selecting a backend

Backend selection is the `[database] type` value in your config — no flag. Set `type = "postgres"` (plus `url`), `type = "mysql"` (plus `url`), or `type = "athena"` (plus `workgroup`) to point at a different database. The connector for that type must be compiled in (it is by default; see [Supported backends](#supported-backends) for lean builds and the rebuild error).

## Design context

For the architectural responsibility map, the `[database] type` → connector dispatch seam, and the locked decisions behind the two-input (`--config` / `--schema`) model, see [`.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md`](../../.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md) and the `.planning/phases/85-*` design log.
