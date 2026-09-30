# Chapter 12.10: Config-Driven SQL Servers (cargo pmcp)

The previous chapter showed how Code Mode safely executes LLM-generated SQL
against a schema. This chapter is the applied form of that idea: a complete,
deployable SQL MCP server that you describe in a `config.toml` instead of
hand-writing in Rust. You declare your tables, a handful of curated tools, and
a Code Mode policy; the `cargo pmcp` CLI scaffolds the crate, runs it locally,
and deploys it to AWS Lambda — and the long tail of queries you didn't curate
is handled by Code Mode generating SQL against your schema.

After this chapter you should be able to scaffold a config-driven SQL server,
run it, edit its tools and schema without recompiling, and ship it to Lambda
with the inline dev secret automatically swapped for a production secret.

## The Problem (Why Config, Not Code)

To expose a SQL database over MCP the conventional way, you hand-write a Rust
binary: construct a `ServerBuilder`, implement a tool handler for *every* query
you want to support, build and manage the database connector, wire up Code Mode
policy, stand up the HTTP transport — and recompile every time a tool or the
schema changes. For a database with dozens of useful query shapes, most of that
code is mechanical, and the recompile loop slows iteration to a crawl.

There is a Pareto split hiding in this work. Roughly 20% of operations are
"blessed" paths worth curating as named tools with typed parameters. The other
~80% is a long tail of ad-hoc queries you cannot enumerate in advance. Hand-coding
forces you to either write a handler for every tail query or expose a single
unsafe "run arbitrary SQL" tool.

Config-driven servers answer both halves:

```text
                       ┌─────────────────────────────┐
   config.toml  ─────► │  [[tools]]  (the curated 20%) │ ──► named MCP tools
                       └─────────────────────────────┘
                       ┌─────────────────────────────┐
   schema.sql   ─────► │  Code Mode  (the long-tail 80%) │ ──► validate_code /
                       └─────────────────────────────┘        execute_code
```

You curate the common operations as `[[tools]]`; Code Mode handles the rest by
generating SQL against the schema resource, validated and policy-checked exactly
as in Chapter 12.9. Nothing about the server is hand-coded per query — the parts
that vary live in config.

## Two Shapes

PMCP ships two ways to run this, both built on the same `pmcp-server-toolkit`
library:

| | **Shape A — the binary** | **Shape B — the scaffold** |
|---|---|---|
| What | The prebuilt `pmcp-sql-server` binary | A crate from `cargo pmcp new --kind sql-server` |
| Run | `pmcp-sql-server --config c.toml --schema s.sql` | `cargo run` inside the crate |
| Rust source? | None | A small generated `src/main.rs` you own |
| Best for | Zero-build point-and-serve; extending the toolkit | Building, customizing, and **deploying** |

This chapter uses **Shape B**, because it is the path the CLI scaffolds and the
one `cargo pmcp deploy` understands end-to-end. Shape A is covered in the
`pmcp-sql-server` crate README.

## Step 1: Scaffold

```bash
cargo pmcp new my-sql-server --kind sql-server
cd my-sql-server
```

This emits a single runnable crate (not the default multi-crate workspace):

```text
my-sql-server/
├── Cargo.toml          # pins pmcp-server-toolkit ["code-mode","sqlite","http"] + pmcp ["streamable-http"]
├── src/main.rs         # generated wiring (below)
├── config.toml         # [server] / [database] / [code_mode] + a list_books tool
├── schema.sql          # idempotent demo DDL + seed
├── deploy.toml         # deploy descriptor (human-visible)
└── .pmcp/deploy.toml   # the copy cargo pmcp deploy reads
```

The generated `src/main.rs` is the load-bearing wiring — and it is the *only*
Rust you get, deliberately small:

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ServerConfig::from_toml_strict_validated(
        &pmcp::assets::load_string("config.toml")?,
    )?;
    // Open the concrete SQLite connector and bootstrap the schema on it.
    let conn = SqliteConnector::open(pmcp_server_toolkit::demo_db_path().as_ref())?;
    conn.execute_batch(&pmcp::assets::load_string("schema.sql")?).await?;
    let conn: Arc<dyn SqlConnector> = Arc::new(conn);

    let server = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version)
        .try_tools_from_config_with_connector(&cfg, conn.clone())?    // the curated 20%
        .try_code_mode_from_config_with_connector(&cfg, conn)?        // the long-tail 80%
        .resources_arc(Arc::new(StaticResourceHandler::from(&cfg)))
        .build()?;

    let (addr, handle) = serve(server).await?;     // streamable HTTP
    println!("PMCP_SQL_SERVER_ADDR=http://{addr}");
    handle.await?;
    Ok(())
}
```

Two design choices make this binary portable:

1. **`pmcp::assets::load_string`** resolves `config.toml`/`schema.sql` from the
   working directory locally and from `/var/task/assets/` on Lambda — so the same
   compiled binary works in both places.
2. **`demo_db_path()`** returns `/tmp/demo.db` on Lambda (the only writable path),
   and `schema.sql` is idempotent, so re-running against a persisted DB is safe.

## Step 2: Run It

```bash
cargo run
# prints: PMCP_SQL_SERVER_ADDR=http://127.0.0.1:<port>
```

Point an MCP client at the printed address. You will see the curated
`list_books` tool plus Code Mode's `validate_code` and `execute_code` tools.
Ask the model for something you *didn't* curate ("which authors have more than
one book?") and it will write SQL against your schema, have it validated, and
execute it under the policy you configured.

`cargo pmcp dev --server my-sql-server` wraps the same run loop, building the
crate and loading any `.env` you provide.

## Step 3: Customize Through Config

The config is parsed with `#[serde(deny_unknown_fields)]`, so a misspelled key
is a hard parse error — not a silent default. Add a curated tool by appending a
`[[tools]]` block; tune the safety posture under `[code_mode]`:

```toml
[code_mode]
enabled = true
allow_writes = false        # default-deny — reads only
require_limit = true        # every generated query must bound its rows
max_limit = 1000
token_secret = "dev-only-insecure-secret-min-16-bytes"   # DEV ONLY (see deploy)
allow_inline_token_secret_for_dev = true
# Optional: text appended to BOTH the validate_code and execute_code tool
# descriptions, so the model reads what this deployment enforces before it calls.
description_notice = "Queries are read-only and capped at 1000 rows."

[[tools]]
name = "books_by_author"
description = "Books written by a given author"
sql = "SELECT id, title FROM books WHERE author = :author ORDER BY title LIMIT :limit"

[[tools.parameters]]
name = "author"
type = "string"
required = true

[[tools.parameters]]
name = "limit"
type = "integer"
required = false
default = 20
```

Edit `schema.sql` to change the tables or seed data. Both files are read at
startup — restart the server and the new tool and schema are live. No recompile.

To use a different backend, set `[database] type` to `postgres`, `mysql`, or
`athena` and enable the matching connector feature in `Cargo.toml`. The
connector trait and all four implementations live in `pmcp-server-toolkit`.

## Step 4: Deploy to AWS Lambda

The scaffold's `deploy.toml` defaults to the `pmcp-run` target. To deploy to
Lambda, change the target type:

```toml
# deploy.toml  (mirror the change into .pmcp/deploy.toml)
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "my-sql-server"
memory_mb = 512
timeout_seconds = 30

[assets]
include = ["config.toml", "schema.sql"]
```

Then validate and ship:

```bash
cargo pmcp validate deploy                  # catches IAM footguns before any AWS call
cargo pmcp deploy --target-type aws-lambda
cargo pmcp deploy outputs                    # the deployed endpoint URL
```

`cargo pmcp deploy` recognizes a config-driven project (config.toml + schema.sql
+ a `pmcp-server-toolkit` dependency) and bundles the two assets into the
deployment package. On Lambda:

```text
  deployment zip ──► /var/task/assets/config.toml   (read-only)
                     /var/task/assets/schema.sql     (read-only)
                     /tmp/demo.db                     (writable — opened at runtime)
```

The most important production detail is handled for you: the deploy path
**rewrites the bundled config's inline DEV `token_secret` to
`${CODE_MODE_SECRET}`** so the deployed artifact never ships the dev literal.
Supply `CODE_MODE_SECRET` as a Lambda environment variable / deploy secret. Your
on-disk `config.toml` is left unchanged — only the bundled copy is sanitized.

Verify the live server with the same conformance suite you would run locally:

```bash
cargo pmcp test conformance <deployed-url>
```

## What You Built

You now have a SQL MCP server that:

- exposes curated, typed tools for the common 20% of operations,
- safely answers the long-tail 80% through Code Mode,
- is changed by editing config — not by recompiling,
- runs identically on your laptop and on Lambda, and
- ships without ever leaking a dev secret into the deployed artifact.

For the standalone no-Rust binary form, see the `pmcp-sql-server` crate README;
for the Code Mode internals that make the long-tail path safe, revisit
[Chapter 12.9](ch12-9-code-mode.md).
