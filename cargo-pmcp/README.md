# cargo-pmcp

Production-grade MCP server development toolkit.

## Overview

`cargo-pmcp` is a comprehensive scaffolding and development tool for building Model Context Protocol (MCP) servers using the PMCP SDK. It streamlines the entire lifecycle from project creation to production deployment.

## Features

- **Project Scaffolding** - Create workspaces and add servers with best-practice templates
- **Agents & Teams** - Scaffold and run deploy-anywhere agents (`agent new`/`agent dev`), run an in-process small team over the four reference servers (`team dev`), and inspect portable AI-Package bundles (`package inspect`)
- **Development Mode** - Build and run servers with HTTP transport and live logs
- **Client Connection** - One-command setup for Claude Code, Cursor, and MCP Inspector
- **Automated Testing** - Generate and run scenario-based tests from server capabilities
- **Load Testing** - Stress-test with concurrent virtual users, latency histograms, and CI/CD reports
- **Security Pentesting** - 32 attack checks across 7 categories with SARIF output for GitHub Security tab
- **Schema Management** - Export, validate, and diff schemas from live MCP servers
- **Workflow Validation** - Catch structural errors in workflows before runtime
- **MCP Apps** - Scaffold widget projects, generate ChatGPT manifests, and build landing pages
- **Widget Preview** - Browser-based preview with dual proxy/WASM bridge modes and hot-reload
- **Multi-Target Deployment** - Deploy to AWS Lambda, Google Cloud Run, Azure Container Apps, Cloudflare Workers, or pmcp.run. The Azure Container Apps target needs no local Docker (ACR cloud-build via `az containerapp up --source`) — see [DEPLOYMENT.md § Azure Container Apps](./DEPLOYMENT.md#azure-container-apps-azure-container-apps-target).
- **Declarative IAM** - Declare AWS permissions (DynamoDB, S3, SecretsManager, …) in `.pmcp/deploy.toml` via the `[iam]` section. Translated into `mcpFunction.addToRolePolicy(...)` calls in the generated CDK stack at deploy time. See [docs/IAM.md](./docs/IAM.md) for the how-to guide, or [DEPLOYMENT.md § IAM Declarations](./DEPLOYMENT.md#iam-declarations-iam-section) for the schema reference.
- **Secrets Management** - Multi-provider secret storage (local, pmcp.run, AWS Secrets Manager)
- **OAuth Authentication** - Production-ready OAuth 2.0 with AWS Cognito, Dynamic Client Registration, and SSO
- **Landing Pages** - Create, develop, and deploy landing pages for server discovery
- **Workspace Diagnostics** - Validate project structure, toolchain, and server connectivity

## Installation

```bash
cargo install cargo-pmcp
```

## Config-Driven SQL Server (`new --kind sql-server`)

This is the recommended way to build a SQL MCP server: declare your tables,
tools, and Code Mode policy in a `config.toml`, point it at a schema, and let
`cargo pmcp` scaffold, run, and deploy it. **You do not write a tool handler
per query, and you do not recompile to change tools or schema.** A small,
generated `src/main.rs` wires the [`pmcp-server-toolkit`](../crates/pmcp-server-toolkit)
library; everything that varies between servers lives in config.

> **Two shapes, same toolkit.** This scaffold ("Shape B") generates a crate you
> own and can extend. There is also a prebuilt `pmcp-sql-server` *binary*
> ("Shape A") you run with `--config`/`--schema` and never compile — see
> [`crates/pmcp-sql-server/README.md`](../crates/pmcp-sql-server/README.md).
> Use the scaffold when you want to deploy and customize; use the binary for the
> zero-build, point-and-serve path or when extending the toolkit with a new SQL
> dialect.

### 1. Scaffold

`cargo pmcp new <name> --kind sql-server` emits a **single runnable crate**
(distinct from the default multi-crate workspace):

```
<name>/
├── Cargo.toml          # pins pmcp-server-toolkit (features ["code-mode", "sqlite", "http"]) + pmcp ["streamable-http"]
├── src/main.rs         # generated wiring: load config/schema → open SQLite → build server → serve streamable HTTP
├── config.toml         # [server] / [database] / [code_mode] + a curated list_books tool
├── schema.sql          # idempotent demo DDL + seed (CREATE TABLE IF NOT EXISTS / INSERT OR IGNORE)
├── deploy.toml         # human-visible deploy descriptor (target = pmcp-run; assets = config.toml + schema.sql)
└── .pmcp/deploy.toml   # the copy `cargo pmcp deploy` actually reads
```

The generated `src/main.rs` loads `config.toml` + `schema.sql` via
`pmcp::assets::load_string` (resolves to the cwd locally, `/var/task/assets/` on
Lambda) and opens SQLite at `pmcp_server_toolkit::demo_db_path()` (`/tmp/demo.db`
on Lambda). The **same binary runs locally and deploys unchanged** — no source
edits between `cargo run` and `cargo pmcp deploy`.

> The emitted `Cargo.toml` pins `pmcp-server-toolkit = "0.1.0"`. Until that crate
> is published to crates.io, add a local `[patch.crates-io]` (or a path
> dependency) so `cargo run` resolves against your toolkit build.

### 2. Run it locally

```bash
cargo pmcp new my-sql-server --kind sql-server
cd my-sql-server
cargo run
# prints: PMCP_SQL_SERVER_ADDR=http://127.0.0.1:<port>
```

Connect any MCP client to the printed address. Out of the box you get the
curated `list_books` tool plus Code Mode's `validate_code` / `execute_code`
tools — the LLM writes SQL against your schema for the long tail you didn't
curate. You can also drive the dev loop through the CLI, which builds the crate
and injects `.env` variables:

```bash
cargo pmcp dev --server my-sql-server --port 3000
```

### 3. Edit config (no recompile)

The generated `config.toml` is parsed with `#[serde(deny_unknown_fields)]`, so a
typo is a hard error rather than a silent no-op. Add tools and tune policy by
editing config — both `config.toml` and `schema.sql` are read at startup:

```toml
[code_mode]
enabled = true
allow_writes = false          # default-deny: read-only posture
require_limit = true
max_limit = 1000
# DEV ONLY — the deploy path rewrites this to a secrets ref automatically.
token_secret = "dev-only-insecure-secret-min-16-bytes"
allow_inline_token_secret_for_dev = true

[[tools]]
name = "list_books"
description = "List books ordered by title"
sql = "SELECT id, title, author FROM books ORDER BY title LIMIT :limit"

[[tools.parameters]]
name = "limit"
type = "integer"
required = false
default = 20
```

`schema.sql` is idempotent (`CREATE TABLE IF NOT EXISTS` / `INSERT OR IGNORE`),
so a second `cargo run` against a persisted `demo.db` succeeds. To target a
different backend, change `[database] type` to `postgres`, `mysql`, or `athena`
and add the connector feature to `Cargo.toml` (the four connectors live in
`pmcp-server-toolkit`).

### 4. Deploy to AWS Lambda

`cargo pmcp deploy` detects a config-driven project (a `config.toml` +
`schema.sql` + a `pmcp-server-toolkit` dependency) and bundles those assets
beside the binary so the deployed server resolves them under
`/var/task/assets/`. The scaffold's `deploy.toml` defaults to `pmcp-run`; for
Lambda, set the target type and deploy:

```toml
# deploy.toml  (and .pmcp/deploy.toml — keep them in sync)
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

```bash
# One-time prerequisites (see the AWS Lambda chapter in the PMCP course):
#   aws sts get-caller-identity        # credentials configured
#   cargo install cargo-lambda         # cross-compile to the Lambda runtime
#   npm install -g aws-cdk             # infra provisioning

cargo pmcp validate deploy             # pre-flight: catches IAM footguns before any AWS call
cargo pmcp deploy --target-type aws-lambda
cargo pmcp deploy outputs              # show the deployed endpoint
```

**Lambda runtime posture** (handled for you):

- **Assets** (`config.toml`, `schema.sql`) extract to `/var/task/assets/` (read-only) — exactly where `pmcp::assets::load_string` looks.
- **The mutable SQLite DB** opens at `/tmp/demo.db` (`/tmp` is the only writable path on Lambda).
- **Secret (H4):** the deploy path rewrites the bundled config's inline DEV
  `token_secret` to `${CODE_MODE_SECRET}` so the deployed artifact never ships
  the dev literal. Supply `CODE_MODE_SECRET` as a deploy secret/env. Your
  on-disk `config.toml` is left untouched.

Verify the live endpoint:

```bash
cargo pmcp test conformance <deployed-url>
```

For a guided, hands-on version of this walkthrough, see the **Config-Driven SQL
Servers** chapters in the [PMCP book](../pmcp-book) and [PMCP course](../pmcp-course).

## Config-Driven OpenAPI Server (`new --kind openapi-server`)

The OpenAPI/HTTP sibling of `--kind sql-server`: declare a REST `[backend]`, a
handful of curated `[[tools]]` (single-call or multi-call script), and a
`[code_mode]` policy in `config.toml`, optionally ship an `api.yaml` OpenAPI
spec, and serve a production MCP server over the backend's HTTP API — **no Rust
required to change behaviour, just edit the config**.

> **Two shapes, same toolkit.** This scaffold ("Shape B") generates a crate you
> own and can extend. There is also a prebuilt `pmcp-openapi-server` *binary*
> ("Shape A") you run with `--config`/`--spec` and never compile — see
> [`crates/pmcp-openapi-server`](../crates/pmcp-openapi-server).

```bash
cargo pmcp new my-openapi-server --kind openapi-server
cd my-openapi-server
cargo run
# prints: PMCP_OPENAPI_SERVER_ADDR=http://127.0.0.1:<port>
```

The scaffold emits a **single runnable crate**:

```
<name>/
├── Cargo.toml          # pmcp-server-toolkit (features ["openapi-code-mode"]) + pmcp-openapi-server (dispatch/build_server) + pmcp ["streamable-http"]
├── src/main.rs         # generated ≤15-line wiring: load config[+optional api.yaml] → dispatch → build_server → serve streamable HTTP
├── config.toml         # [server] / [backend] / [code_mode] + a single-call tool + a script tool (DEV-only inline token_secret)
├── api.yaml            # minimal OpenAPI spec (optional at runtime; exposed as the api_schema resource for Code Mode)
├── deploy.toml         # human-visible deploy descriptor (target = pmcp-run; assets = config.toml + api.yaml)
└── .pmcp/deploy.toml   # the copy `cargo pmcp deploy` actually reads
```

Out of the box you get the curated `list_widgets` tool, a `widget_with_detail`
script tool, and Code Mode's `validate_code` / `execute_code` tools — the LLM
writes JS against your backend for the long tail you didn't curate. The generated
`config.toml` carries an inline **DEV-ONLY** `token_secret` (guarded by
`allow_inline_token_secret_for_dev = true`); **replace it with a secrets ref for
production** — `cargo pmcp deploy` substitutes one automatically.

> The emitted `Cargo.toml` pins `pmcp-server-toolkit = "0.1.0"` and
> `pmcp-openapi-server = "0.1.0"`. Until those crates are published to crates.io,
> add a local `[patch.crates-io]` (or path dependencies) so `cargo run` resolves
> against your in-repo build.

## Config-Driven Workbook Server (`new --kind workbook-server`)

The spreadsheet sibling of `--kind sql-server`: a business analyst authors the
contract in **standard Excel** — name the inputs and each result block as Excel
**Tables** with columns `name | value | description | tier` — and the compiler
derives a well-named, well-typed MCP tool surface. **Each output table becomes
its own MCP tool** (e.g. `calculate_tax`, `estimate_refund`), each advertising a
DAG-derived input schema (only the inputs that table's formulas actually use) and
an emitted output schema. No per-cell named ranges, **no Rust required**.

> **Two shapes, same toolkit.** `cargo pmcp new my-workbook --kind workbook-server`
> ("Shape B") scaffolds a small crate you own and can extend. There is also a
> prebuilt `pmcp-workbook-server` *binary* ("Shape A") you point at a compiled
> bundle and never recompile — see [`crates/pmcp-workbook-server`](../crates/pmcp-workbook-server).
> Unlike the SQL/OpenAPI kinds, a workbook server takes **no `config.toml` and no
> schema file**: the single input is a compiled `bundle@version` directory.

The `cargo pmcp workbook` command turns an `.xlsx` into that bundle (and lets you
preview the surface before you commit to it):

```bash
# 1. Preview the exact tool surface an AI will see — read-only, writes nothing:
cargo pmcp workbook explain pricing.xlsx              # text; add --format json for machines

# 2. Lint the workbook against the dialect (cell-precise, fail-helpful):
cargo pmcp workbook lint pricing.xlsx

# 3. Compile to a governed, gated bundle (records an approver-bound acceptance):
cargo pmcp workbook compile pricing.xlsx --workflow quote --approver alice

# (dev) Emit an UNGATED bundle for local iteration — no approver required:
cargo pmcp workbook emit pricing.xlsx --workflow quote

# 4. Serve it — Shape A binary, or your scaffolded Shape B crate:
pmcp-workbook-server --bundle-dir bundles/quote@1.0.0
```

`workbook explain` is the habit that prevents a silently-wrong deploy: it runs the
real ingest → tool-surface projection and prints each tool's name, description,
inputs (type / unit / enum), and outputs — so you confirm the AI-facing contract
*before* compiling or shipping. See the **Workbook Table Authoring** chapter in the
[pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/) for the full authoring guide.

## Agents & Teams (`agent`, `team`, `package`)

`cargo-pmcp` is the on-ramp for agents and teams too — the same `new`/`dev` story as
servers, plus a `package` verb for portable bundles. An **agent** is an MCP *client*
with a loop; a **team** is a set of agents wired to the four reference team servers.

```bash
# 1. Scaffold a runnable agent crate (AgentPackage manifest + runner + pin tripwire):
cargo pmcp agent new my-agent
cd my-agent && cargo run                    # the generated runner drives the loop

# 2. Run an agent loop locally — offline, against Ollama, or a remote endpoint:
cargo pmcp agent dev --source fixed                          # offline smoke test
cargo pmcp agent dev --model llama3.2                        # local Ollama (default)
cargo pmcp agent dev --endpoint https://api.example.com/v1 --api-key-env MY_KEY

# 3. Run a small team in one process — member agents + team-fs, approval-mcp,
#    mem-mcp, team-mcp — and print the offline doc-review transcript:
cargo pmcp team dev
cargo pmcp team dev --serve --port 8080     # or serve team-mcp over HTTP

# 4. Inspect a portable AI-Package bundle locally, fully offline:
cargo pmcp package inspect ./some-agent.pmcp
```

> `package inspect` reads an OCI image-layout `.pmcp` bundle. The remote
> `capture`/`show` verbs (upload / fetch against the pmcp.run platform) are a
> coordinated follow-on and are intentionally not shipped here yet.

An agent deploys through the existing target adapters — an agent-as-server is just a
server binary, so `cargo pmcp deploy` applies once you wrap it. See
[`agent`](docs/commands/agent.md) · [`team`](docs/commands/team.md) ·
[`package`](docs/commands/package.md), and the crates they build on:
[`pmcp-agent`](../crates/pmcp-agent/README.md) ·
[`pmcp-team-servers`](../crates/pmcp-team-servers/README.md) ·
[`pmcp-package`](../crates/pmcp-package/README.md).

## End-to-End Example

Walk through the full lifecycle using the `complete` template calculator server.

### 1. Create workspace and add a server

```bash
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace
cargo pmcp add server calculator --template complete
```

### 2. Start the dev server

```bash
cargo pmcp dev --server calculator
```

Server starts on `http://0.0.0.0:3000` with live logs.

### 3. Connect to Claude Code

In another terminal:

```bash
cargo pmcp connect --server calculator --client claude-code
```

Now ask Claude: *"Multiply 7 and 8"* or *"Solve x^2 - 5x + 6 = 0"*.

### 4. Generate and run tests

```bash
# Generate test scenarios from server capabilities
cargo pmcp test generate --server calculator

# Run the tests
cargo pmcp test run --server calculator --detailed
```

### 5. Load test

```bash
# Generate starter config with schema discovery
cargo pmcp loadtest init https://my-server.example.com

# Run load test
cargo pmcp loadtest run https://my-server.example.com --vus 20 --duration 60
```

### 6. Security pentest

```bash
# Quick scan — MCP-specific checks (prompt injection, tool poisoning, session security)
cargo pmcp pentest http://localhost:3000

# Deep scan — all 7 categories including transport, auth, exfiltration, protocol abuse
cargo pmcp pentest http://localhost:3000 --profile deep

# SARIF output for GitHub Security tab
cargo pmcp pentest http://localhost:3000 --profile deep --format sarif -o results.sarif

# Filter to specific categories
cargo pmcp pentest http://localhost:3000 --category pi,tp,ss

# CI gate — fail on medium or higher findings
cargo pmcp pentest http://localhost:3000 --fail-on medium
```

### 7. Deploy

```bash
# Initialize for AWS Lambda with OAuth
cargo pmcp deploy init --target aws-lambda --oauth cognito

# Deploy
cargo pmcp deploy --target aws-lambda

# Or deploy to Azure Container Apps (no local Docker — ACR cloud-build)
cargo pmcp deploy init --target azure-container-apps
az login
cargo pmcp deploy --target-type azure-container-apps
```

### 8. Monitor

```bash
cargo pmcp deploy logs --tail
cargo pmcp deploy metrics --period 24h
cargo pmcp deploy test --verbose
```

## Commands

| Command | Description | Reference |
|---------|-------------|-----------|
| `new` | Create a new MCP workspace (or `--kind sql-server` / `--kind openapi-server` / `--kind workbook-server` for a single config-driven crate) | [docs/commands/new.md](docs/commands/new.md) |
| `add` | Add server, tool, or workflow to workspace | [docs/commands/add.md](docs/commands/add.md) |
| `dev` | Start development server with HTTP transport | [docs/commands/dev.md](docs/commands/dev.md) |
| `connect` | Connect server to Claude Code, Cursor, or Inspector | [docs/commands/connect.md](docs/commands/connect.md) |
| `test` | Run, generate, upload, and download test scenarios | [docs/commands/test.md](docs/commands/test.md) |
| `loadtest` | Load test with virtual users and performance reports | [docs/commands/loadtest.md](docs/commands/loadtest.md) |
| `pentest` | Security penetration testing with 32 checks across 7 categories | [src/pentest/README.md](src/pentest/README.md) |
| `doctor` | Workspace diagnostics — toolchain, dependencies, connectivity | |
| `workbook` | Preview (`explain`), lint, compile, and emit governed Excel-workbook bundles | [docs/commands/workbook.md](docs/commands/workbook.md) |
| `schema` | Export, validate, and diff MCP server schemas | [docs/commands/schema.md](docs/commands/schema.md) |
| `validate` | Validate workflows and server components | [docs/commands/validate.md](docs/commands/validate.md) |
| `deploy` | Deploy to AWS Lambda, Cloud Run, Azure Container Apps, Workers, pmcp.run | [docs/commands/deploy.md](docs/commands/deploy.md) |
| `secret` | Manage secrets across local, pmcp.run, and AWS | [docs/commands/secret.md](docs/commands/secret.md) |
| `app` | Scaffold MCP Apps projects with widgets | [docs/commands/app.md](docs/commands/app.md) |
| `preview` | Browser-based widget preview with hot-reload | [docs/commands/preview.md](docs/commands/preview.md) |
| `landing` | Create and deploy server landing pages | [docs/commands/landing.md](docs/commands/landing.md) |
| `agent` | Scaffold (`new`) and run (`dev`) deploy-anywhere agents | [docs/commands/agent.md](docs/commands/agent.md) |
| `team` | Run an in-process small team + the four reference team servers (`dev`) | [docs/commands/team.md](docs/commands/team.md) |
| `package` | Inspect (`inspect`) a local AI-Package bundle, fully offline | [docs/commands/package.md](docs/commands/package.md) |

## App Validation

`cargo pmcp test apps URL` validates MCP App metadata on a running server. It cross-references tools that declare `ui.resourceUri` against the resources they reference, validates MIME types, and (in strict modes) statically inspects the widget HTML for required protocol handler wiring.

### Modes

| Mode | Severity | Description |
|------|----------|-------------|
| `standard` (default) | Warning | Permissive — emits ONE summary Warning row per widget (MCP Apps is optional in the spec). |
| `chatgpt` | Error | Strict for ChatGPT compatibility (checks `openai/*` `_meta` keys). **For widget validation specifically, this mode is a no-op** (no widget-related rows emitted; preserves prior behavior). |
| `claude-desktop` | Error | Strict for Claude Desktop / Claude.ai. Statically inspects each widget HTML body fetched via `resources/read` for the `@modelcontextprotocol/ext-apps` import, the `new App({...})` constructor, the four required protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`), and the `app.connect()` call. Missing signals emit one Error row each. Honors `--tool` to restrict the check to a single tool's widget. |

```bash
# Default (permissive) mode
cargo pmcp test apps http://localhost:3000

# ChatGPT compatibility mode
cargo pmcp test apps http://localhost:3000 --mode chatgpt

# Claude Desktop / Claude.ai pre-deploy gate (strict static widget inspection)
cargo pmcp test apps http://localhost:3000 --mode claude-desktop

# Restrict the strict check to a single tool's widget via --tool
cargo pmcp test apps http://localhost:3000 --mode claude-desktop --tool open_dashboard
```

### Why --mode claude-desktop

Claude Desktop and Claude.ai silently tear down the entire MCP connection when a widget is missing required protocol handlers — the widget appears for a moment, then everything dies with no actionable error. `--mode claude-desktop` catches this class of failure pre-deploy by statically inspecting the widget HTML body for the SDK import, the App constructor, all four required handlers, and the `connect()` call. Missing signals are emitted as Error rows that link to the relevant section of the MCP Apps guide:

[src/server/mcp_apps/GUIDE.md#handlers-before-connect](https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#handlers-before-connect).

### MIME profile (`;profile=mcp-app`)

A common Claude Desktop failure point is omitting the `;profile=mcp-app` MIME parameter on widget resources. Claude Desktop uses this MIME parameter to identify resources that should be rendered as MCP App widgets vs plain HTML resources. Verify your server registers widget resources with `mime_type: "text/html;profile=mcp-app"` (or equivalent — `UIResource::html_mcp_app()` and `UIResourceContents::html()` both produce this MIME type by default).

### Vite singlefile minification

If your widget bundle is produced by Vite singlefile in production, the validator's regex strategy is designed to survive that minifier; if you encounter false-negatives, file an issue with a bundled HTML for empirical analysis.

> Note: The legacy widget examples under `examples/mcp-apps-chess/`, `examples/mcp-apps-dataviz/`, and `examples/mcp-apps-map/` use the older postMessage channel and will fail `--mode claude-desktop`. For a Claude-Desktop-ready widget, see `crates/mcp-tester/examples/validate_widget_pair.rs` and the corrected fixture at `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html`.

### Source-scan mode: `--widgets-dir <path>`

Two scan surfaces are supported:

| Scan mode | When to use | What it scans |
|-----------|-------------|---------------|
| **Bundle scan** (default) | CI against a deployed server | Each widget HTML body fetched via `resources/read` |
| **Source scan** (`--widgets-dir <path>`) | Local pre-deploy validation | `<path>/*.html` source files on disk |

**Why both:** Bundle scan validates what the server actually serves to clients (the post-Vite-singlefile bytes). Source scan is faster and higher-confidence pre-deploy because source files have unmangled identifiers and intact import statements — minifiers cannot defeat patterns that match against the unminified `import { App } from '@modelcontextprotocol/ext-apps'`.

The validator's regex set is minification-resistant in both modes (see [Plan 78-06 — gap closure for cost-coach minified-bundle false positives](.planning/phases/78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-/78-06-PLAN.md)). The four SDK-presence signals — `[ext-apps]` log prefix, `ui/initialize` method literal, `ui/notifications/tool-result` method literal, and the legacy `@modelcontextprotocol/ext-apps` import literal — survive Vite singlefile minification because they are protocol-level strings the SDK exposes by name.

Example:

```bash
# Source-scan local widget files (pre-deploy)
cargo pmcp test apps --mode claude-desktop --widgets-dir ./widget "http://informational"

# Bundle-scan against a deployed server (CI)
cargo pmcp test apps --mode claude-desktop https://my-server.example.com/mcp
```

Same validator, same verdict shape, two ingestion paths.

## Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Enable verbose output for debugging |
| `--no-color` | | Suppress colored output (also respects `NO_COLOR` env and non-TTY) |
| `--quiet` | | Suppress all non-error output (verbose wins if both are set) |
| `--dump-wire` | | Dump every HTTP request/response the SDK puts on the wire, credentials redacted. See below. |

### `--dump-wire` — what did we actually send?

When a deployed server rejects a request, the first question is what went on the
wire. `--dump-wire` answers it for any command that talks to a server:

```bash
cargo pmcp test conformance --dump-wire https://your-server.example.com/mcp
```

```text
DEBUG pmcp::wire: outgoing MCP request direction="request" method=POST
  headers=mcp-protocol-version: 2026-07-28
          mcp-method: resources/read
          mcp-name: ui://app/keypad
  body={"jsonrpc":"2.0","id":"…","method":"resources/read","params":{…}}
DEBUG pmcp::wire: incoming MCP response direction="response" status=400
```

`Authorization`, `Cookie`, `Mcp-Session-Id` and friends render as
`<redacted N bytes>` — the name and length survive so "present but empty" stays
distinguishable, but the secret never reaches a log or a CI artifact.

It is a preset over the SDK's `pmcp::wire` tracing target, not a separate logger,
so these are equivalent and any `tracing_subscriber` layer composes with it:

```bash
cargo pmcp test conformance --dump-wire "$URL"
RUST_LOG=pmcp::wire=debug cargo pmcp test conformance "$URL"
```

With neither, no subscriber is installed and the SDK's guards keep every wire
path from allocating — it costs a normal run nothing.

## OAuth Authentication

Enable OAuth 2.0 for your MCP server with zero code changes. Supports AWS Cognito, Microsoft Entra ID, Google, Okta, and Auth0.

**Infrastructure setup:**
```bash
cargo pmcp deploy init --target aws-lambda --oauth cognito
```

**Server-side (provider-agnostic):**
```rust
use pmcp::server::auth::AuthContext;

fn handle_tool_call(auth: &AuthContext) -> Result<Value, Error> {
    auth.require_auth()?;
    auth.require_scope("read:data")?;
    let user_id = auth.user_id();
    Ok(json!({ "user": user_id }))
}
```

**Switch providers via config only (no code changes):**
```toml
[profile.production.auth]
type = "jwt"
issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
audience = "your-app-client-id"
```

For detailed OAuth architecture, see [docs/oauth-design.md](docs/oauth-design.md) and [docs/oauth-sdk-design.md](docs/oauth-sdk-design.md).

## Secrets Management

Secrets are environment variables that your MCP server needs at runtime. They are resolved at deploy time from `.env` files and shell environment variables, then injected into the deployment target.

### Local Development

Create a `.env` file in your project root with `KEY=VALUE` pairs:

```text
ANTHROPIC_API_KEY=sk-ant-...
DATABASE_URL=postgresql://localhost/mydb
ANALYTICS_KEY=ua-12345
```

`cargo pmcp dev <server>` automatically loads `.env` into the server process. Shell environment variables take precedence over `.env` values when both define the same key.

### Declaring Secrets

Declare required secrets in `pmcp.toml` so the deploy pipeline knows what to resolve:

```toml
[[secrets.definitions]]
name = "ANTHROPIC_API_KEY"
description = "Anthropic API key for LLM calls"
required = true
env_var = "ANTHROPIC_API_KEY"
obtain_url = "https://console.anthropic.com/settings/keys"
```

### Deployment Integration

`cargo pmcp deploy` resolves secrets from your environment and `.env` file, then reports which are found and which are missing:

- **AWS Lambda:** Resolved secrets are injected as Lambda environment variables. Missing secrets produce a warning but do not block deployment.
- **pmcp.run:** The CLI performs a diagnostic check only -- secrets are never sent from your machine. For missing secrets, it shows the exact `cargo pmcp secret set` command to store each secret in pmcp.run's managed Secrets Manager. Actual env var injection happens server-side.

Missing secrets are warnings, not deployment blockers.

### Runtime Access

Server code reads secrets via the `pmcp::secrets` module:

```rust
use pmcp::secrets;

// Optional secret
if let Some(key) = secrets::get("ANALYTICS_KEY") {
    configure_analytics(&key);
}

// Required secret (returns actionable error if missing)
let api_key = secrets::require("ANTHROPIC_API_KEY")?;
```

The `require()` function returns an error message that includes the exact `cargo pmcp secret set` command to fix it.

### Secret Providers

| Provider | Storage | Commands |
|----------|---------|----------|
| Local | File-based at `.pmcp/secrets/` | `cargo pmcp secret list`, `set`, `get` |
| pmcp.run | Managed Secrets Manager | `cargo pmcp secret set --target pmcp` |

## CI/CD Integration

`cargo-pmcp` supports OAuth 2.0 client credentials flow for automated deployments.

### Post-deploy verification, with wire capture on failure

Conformance-check the deployment, and if it fails re-run with `--dump-wire` and
archive the frames — so a red build ships the evidence instead of just a verdict:

```yaml
- name: Verify deployment
  run: |
    cargo pmcp test conformance "$SERVER_URL" --format json > conformance.json \
      || {
        echo "conformance failed — re-running with wire capture"
        cargo pmcp test conformance --dump-wire "$SERVER_URL" 2> wire.log || true
        exit 1
      }

- name: Upload wire capture
  if: failure()
  uses: actions/upload-artifact@v4
  with:
    name: mcp-wire-capture
    path: wire.log
```

Credentials are redacted in the capture, so the artifact is safe to attach to a
public build. For the v1-vs-v2 era comparison and a flag that makes those
findings fail the job, see `mcp-tester conformance --dual-run
--fail-on-era-findings` in the [mcp-tester README](../crates/mcp-tester/README.md).

```bash
export PMCP_CLIENT_ID="your-client-id"
export PMCP_CLIENT_SECRET="your-client-secret"
cargo pmcp deploy --target pmcp-run
```

### GitHub Actions

```yaml
name: Deploy MCP Server
on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-pmcp
      - name: Deploy
        env:
          PMCP_CLIENT_ID: ${{ secrets.PMCP_CLIENT_ID }}
          PMCP_CLIENT_SECRET: ${{ secrets.PMCP_CLIENT_SECRET }}
        run: cargo pmcp deploy --target pmcp-run
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `PMCP_CLIENT_ID` | Cognito App Client ID (for client_credentials flow) |
| `PMCP_CLIENT_SECRET` | Cognito App Client Secret |
| `PMCP_ACCESS_TOKEN` | Direct access token (alternative to client credentials) |
| `PMCP_ID_TOKEN` | Optional ID token (when using direct access token) |

## Requirements

- Rust 1.70 or later

## License

MIT

## Contributing

See the main PMCP SDK repository for contributing guidelines.
