# PMCP - Pragmatic Model Context Protocol
<!-- QUALITY BADGES START -->
[![Quality Gate](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-gate.yml/badge.svg?branch=main)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-gate.yml)
[![TDG Score](https://img.shields.io/badge/TDG%20Score-0.00-brightgreen)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
[![Complexity](https://img.shields.io/badge/Complexity-clean-brightgreen)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
[![Technical Debt](https://img.shields.io/badge/Tech%20Debt-0h-brightgreen)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
<!-- QUALITY BADGES END -->

[![CI](https://github.com/paiml/rust-mcp-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/badge/coverage-52%25-yellow.svg)](https://github.com/paiml/rust-mcp-sdk)
[![Crates.io](https://img.shields.io/crates/v/pmcp.svg)](https://crates.io/crates/pmcp)
[![Documentation](https://docs.rs/pmcp/badge.svg)](https://docs.rs/pmcp)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust 1.83+](https://img.shields.io/badge/rust-1.83+-orange.svg)](https://www.rust-lang.org)
[![MCP Compatible](https://img.shields.io/badge/MCP-v2025--11--25-blue.svg)](https://modelcontextprotocol.io)

> **Production-grade Rust implementation of the [Model Context Protocol](https://modelcontextprotocol.io) (MCP) - 16x faster than TypeScript, built with Toyota Way quality principles**

## Overview

PMCP is a complete MCP ecosystem for Rust, providing everything you need to build, test, and deploy production-grade MCP servers — in Rust, **or from configuration alone**:

- **🧩 Config-Driven Servers** - Build SQL & OpenAPI/HTTP MCP servers from a `config.toml`, or serve a governed Excel workbook from a compiled bundle — all **no Rust required** (`pmcp-server-toolkit`, `pmcp-sql-server`, `pmcp-openapi-server`, `pmcp-workbook-server`)
- **🤝 Agents & Teams** - Build deploy-anywhere agents and small teams: the agent loop (`pmcp-agent`), four reference team servers (`pmcp-team-servers`), and a portable AI-Package format (`pmcp-package`) — driven from the `cargo pmcp agent`/`team`/`package` verbs
- **🦀 pmcp SDK** - High-performance Rust crate with full MCP protocol support
- **⚡ cargo-pmcp** - CLI toolkit for scaffolding, testing, and development
- **📚 pmcp-book** - Comprehensive reference guide with 27 chapters
- **🎓 pmcp-course** - Hands-on course with quizzes and exercises
- **🤖 AI Agents** - Kiro and Claude Code configurations for AI-assisted development

**Why PMCP?**
- **Performance**: 16x faster than TypeScript SDK, 50x lower memory
- **Safety**: Rust's type system + zero `unwrap()` in production code
- **Quality**: Toyota Way principles - zero technical debt tolerance
- **Complete**: SDK, tooling, documentation, and AI assistance in one ecosystem

## Quick Start

Choose your path based on experience and preference:

### 🧩 Path 1: Config-Only Servers — No Rust Required (SQL & OpenAPI)

**New in v2.9 — this removes the biggest blocker to putting organizational data behind MCP: you no longer need a Rust programmer.** Describe a production MCP server over a **SQL database** or any **OpenAPI / HTTP backend** in a `config.toml` — declare the backend, a handful of curated tools, and a Code Mode policy — and a prebuilt binary serves it. No Rust, no recompiling. Curated tools cover the common ~20%; **Code Mode** handles the long-tail ~80% by generating queries against your schema/spec under a static, default-deny policy. A business analyst curates the API slice in config; the toolkit does the rest.

**SQL — SQLite / Postgres / MySQL / Athena** (runnable from a checkout of this repo):
```bash
cargo install pmcp-sql-server

# Seed a tiny demo DB, then serve it from config alone — two curated tools
# (list_books, books_by_author) + Code Mode for the long tail.
sqlite3 /tmp/pmcp-sqlite-explorer.db < crates/pmcp-sql-server/examples/sqlite-explorer.sql
pmcp-sql-server \
  --config crates/pmcp-sql-server/examples/sqlite-explorer.toml \
  --schema crates/pmcp-sql-server/examples/sqlite-explorer.sql
```

**OpenAPI / HTTP — any REST backend**, with six outgoing-auth models including OAuth **passthrough** (the server holds no standing credential and forwards the caller's own token, so it can only act as the signed-in user):
```bash
cargo install pmcp-openapi-server

# Curated configs ship with the crate — e.g. a London Tube (api_key) showcase and
# a Microsoft-Graph / Excel "Contoso" (oauth_passthrough) example. These talk to a
# live backend, so supply any required credential per the example's comments.
#
# `base_url` is a SLOT, not a baked literal, so the endpoint comes from the
# environment too — the same package moves between environments unchanged.
TFL_BASE_URL=https://api.tfl.gov.uk TFL_APP_KEY=<your-key> \
  pmcp-openapi-server --config crates/pmcp-openapi-server/examples/london-tube.toml
```

**Excel workbook — a governed spreadsheet, served as one MCP tool per output table** (no `config.toml`, no schema; the single input is a compiled `bundle@version` directory). Author inputs and outputs as named Excel **Tables** and each output table becomes its own well-named, well-typed MCP tool with a DAG-derived input schema:
```bash
cargo install pmcp-workbook-server

# Preview the exact tool surface an AI will see — BEFORE you compile or deploy:
cargo pmcp workbook explain pricing.xlsx          # e.g. calculate_tax, estimate_refund

# Compile a governed workbook to a deterministic bundle (ingest → lint → compile →
# reconcile → fail-closed gate → write), then serve one calculation tool per output
# table — plus explain / get_manifest / diff_version / render_workbook — from the bundle alone.
cargo pmcp workbook compile pricing.xlsx --workflow quote --approver alice
pmcp-workbook-server --bundle-dir bundles/quote@1.0.0
```

**Want to extend and deploy it?** `cargo pmcp new my-server --kind sql-server` (or `--kind openapi-server` / `--kind workbook-server`) scaffolds the same config-driven server as a small crate, ready for `cargo pmcp deploy` to AWS Lambda / Google Cloud Run / Cloudflare / pmcp.run.

**Learn more**: the *Config-Driven SQL Servers*, *OpenAPI*, and *Config-Driven Workbook Servers* chapters in the [pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/) · [`pmcp-sql-server`](crates/pmcp-sql-server/README.md) · [`pmcp-openapi-server`](crates/pmcp-openapi-server/README.md) · [`pmcp-workbook-server`](crates/pmcp-workbook-server/README.md) · [`pmcp-server-toolkit`](crates/pmcp-server-toolkit/README.md)

---

### 🚀 Path 2: AI-Assisted (Recommended for Rapid Prototyping)

Build production-ready MCP servers with AI assistance in minutes:

**Prerequisites:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update

# Install cargo-pmcp
cargo install cargo-pmcp
```

**Install Claude Code AI Agent:**
```bash
# Install the mcp-developer subagent (user-level - works across all projects)
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md

# Restart Claude Code
```

**Build your server:**
```
You: "Create a weather forecast MCP server with tools for getting current conditions and 5-day forecasts"

Claude Code: [Invokes mcp-developer subagent]

I'll create a production-ready weather MCP server using cargo-pmcp.

$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace
$ cargo pmcp add server weather --template minimal

[Implements type-safe tools with validation]
[Adds comprehensive tests and observability]
[Validates quality gates]

✅ Production-ready server complete with 85% test coverage!
```

**What you get**: Production-ready code following Toyota Way principles, with comprehensive tests, structured logging, metrics collection, and zero clippy warnings.

**Learn more**: [AI-Assisted Development Course](https://paiml.github.io/rust-mcp-sdk/course/part6-ai-dev/ch15-ai-assisted.html) | [AI Agents README](ai-agents/README.md)

---

### ⚡ Path 3: cargo-pmcp Toolkit (Recommended for Manual Development)

Scaffold and build servers using the cargo-pmcp CLI:

**Installation:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update

# Install cargo-pmcp
cargo install cargo-pmcp
```

**Create a server:**
```bash
# Create workspace
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace

# Add a server using a template
cargo pmcp add server myserver --template minimal

# Start development server with hot-reload
cargo pmcp dev --server myserver

# Generate and run tests
cargo pmcp test --server myserver --generate-scenarios
cargo pmcp test --server myserver

# Build for production
cargo build --release
```

**Available templates** (`cargo pmcp add server <name> --template <t>`):
- `minimal` - Empty structure for custom servers
- `calculator` - Arithmetic operations (learning)
- `complete_calculator` - Full-featured reference implementation
- `sqlite_explorer` - Hand-coded Rust database browser (escape hatch)

**Config-driven kinds** (`cargo pmcp new <name> --kind <k>` — TOML-driven, no per-tool Rust):
- `sql-server` - SQL MCP server over SQLite / Postgres / MySQL / Athena from `config.toml`
- `openapi-server` - MCP server over any OpenAPI / HTTP backend from `config.toml`
- `workbook-server` - MCP server over a governed Excel workbook, served from a compiled `bundle@version` directory

**Learn more**: [cargo-pmcp Guide](cargo-pmcp/README.md)

---

### 🦀 Path 4: pmcp SDK Directly (For Fine-Grained Control)

Use the pmcp crate directly for maximum control:

**Installation:**
```toml
[dependencies]
pmcp = "2.0"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
schemars = "0.8"  # For type-safe tools
```

**Type-safe server example:**
```rust
use pmcp::{ServerBuilder, TypedTool, RequestHandlerExtra, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
struct WeatherArgs {
    #[schemars(description = "City name")]
    city: String,

    #[schemars(description = "Number of days (1-5)")]
    days: Option<u8>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct WeatherOutput {
    temperature: f64,
    conditions: String,
}

async fn get_weather(args: WeatherArgs, _extra: RequestHandlerExtra) -> pmcp::Result<WeatherOutput> {
    // Validate
    if args.city.is_empty() {
        return Err(Error::validation("City cannot be empty"));
    }

    let days = args.days.unwrap_or(1);
    if !(1..=5).contains(&days) {
        return Err(Error::validation("Days must be 1-5"));
    }

    // Call weather API...
    Ok(WeatherOutput {
        temperature: 72.0,
        conditions: "Sunny".to_string(),
    })
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = ServerBuilder::new()
        .name("weather-server")
        .version("1.0.0")
        .tool("get-weather", TypedTool::new("get-weather", |args, extra| {
            Box::pin(get_weather(args, extra))
        }).with_description("Get weather forecast for a city"))
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

**Learn more**: [pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/) | [pmcp-course](https://paiml.github.io/rust-mcp-sdk/course/) | [API Documentation](https://docs.rs/pmcp)

---

## PMCP Ecosystem Components

### 🦀 pmcp SDK (The Crate)

High-performance Rust implementation of the MCP protocol.

**Key Features:**
- **Type-Safe Tools**: Automatic JSON schema generation from Rust types
- **Multiple Transports**: stdio, HTTP/SSE, WebSocket, WASM
- **OAuth Support**: Full auth context pass-through
- **Workflows**: Multi-step orchestration with array indexing support
- **MCP Apps**: Rich HTML UI widgets with live preview and browser DevTools
- **MCP Tasks**: Shared client/server state with task lifecycle management — see the [*Task-Augmented Tool Results (SEP-1686)*](https://paiml.github.io/rust-mcp-sdk/book/task-augmented-results.html) chapter for returning a `CallToolResult` that points at a related task
- **Agent Skills (SEP-2640)**: Register an Agent Skill in ~5 lines and serve it on BOTH a SEP-2640 skill surface AND a parallel MCP prompt fallback — byte-equal by construction (`skills` feature, opt-in)
- **Tower Middleware**: DNS rebinding protection, CORS, security headers
- **Typed Client Helpers**: `call_tool_typed`, `get_prompt_typed`, and auto-paginating `list_all_*` with bounded safety cap
- **Performance**: 16x faster than TypeScript, SIMD-accelerated parsing
- **Quality**: Zero `unwrap()`, comprehensive error handling

**Latest Version:** `pmcp = "2.9"`

**Documentation:**
- [API Reference](https://docs.rs/pmcp)
- [pmcp-book](https://paiml.github.io/rust-mcp-sdk/)
- [Examples](examples/)

---

### ⚡ cargo-pmcp (CLI Toolkit)

Full-lifecycle development toolkit — from scaffolding to production deployment.

```bash
cargo install cargo-pmcp
```

```bash
cargo pmcp new my-workspace             # Scaffold a new workspace
cargo pmcp add server my-server         # Add a server with best-practice template
cargo pmcp dev --server my-server       # Dev server with hot-reload
cargo pmcp test --server my-server      # Auto-generated scenario tests
cargo pmcp loadtest run                 # Load test with latency percentiles
cargo pmcp pentest run                  # Security audit (32 checks, SARIF output)
cargo pmcp preview --open               # Browser-based widget preview
cargo pmcp workbook explain wb.xlsx     # Preview an Excel workbook's MCP tool surface
cargo pmcp deploy --target aws-lambda   # Deploy to AWS Lambda, GCR, or Cloudflare
cargo pmcp deploy logs --tail           # Stream production logs
```

Covers the full development lifecycle: scaffolding, dev mode, testing, load testing, security pentesting, MCP Apps preview, schema management, multi-target deployment, secrets, and OAuth setup.

**Full command reference**: [cargo-pmcp Guide](cargo-pmcp/README.md)

---

### 🧩 Config-Driven Servers (No Rust)

Build production MCP servers over SQL and HTTP backends from a `config.toml` alone — the toolkit synthesizes curated tools + a Code Mode long tail, so exposing organizational data over MCP no longer needs a Rust programmer.

| Crate | What it is |
| ----- | ---------- |
| [`pmcp-server-toolkit`](crates/pmcp-server-toolkit) | The backend-agnostic library: config types, the `[[tools]]` synthesizer, Code Mode wiring, and the connector/auth seams that the binaries below build on. |
| [`pmcp-sql-server`](crates/pmcp-sql-server) | Shape-A binary serving a SQL database (SQLite / Postgres / MySQL / Athena) from `config.toml` + a schema file. Ships a runnable [`sqlite-explorer`](crates/pmcp-sql-server/examples/sqlite-explorer.toml) example. |
| [`pmcp-openapi-server`](crates/pmcp-openapi-server) | Shape-A binary serving any OpenAPI / HTTP backend, with six outgoing-auth models (incl. OAuth passthrough). Ships [`london-tube`](crates/pmcp-openapi-server/examples/london-tube.toml) (api_key) and [`contoso-m365`](crates/pmcp-openapi-server/examples/contoso-m365.toml) (oauth_passthrough, Microsoft Graph + Excel) examples. |
| [`pmcp-workbook-server`](crates/pmcp-workbook-server) | Shape-A binary serving a governed Excel workbook as one named MCP tool per output table (e.g. `calculate_tax`, `estimate_refund`) plus infrastructure tools (`explain` / `get_manifest` / `diff_version` / `render_workbook`) from a compiled `bundle@version` directory alone — no `config.toml`, no schema. The Excel reader and JS code-mode are compile-time only and absent from the served binary (purity gate). |
| [`pmcp-toolkit-postgres`](crates/pmcp-toolkit-postgres) / [`-mysql`](crates/pmcp-toolkit-mysql) / [`-athena`](crates/pmcp-toolkit-athena) | Per-backend SQL connectors for the toolkit. |

These binaries have `cargo pmcp new --kind {sql-server,openapi-server,workbook-server}` scaffold siblings that generate the same config-driven server as a small, deployable crate. See **Path 1** above and the *Config-Driven SQL Servers* / *OpenAPI* / *Config-Driven Workbook Servers* chapters in the [pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/).

---

### 📚 pmcp-book (Reference Guide)

27-chapter comprehensive reference guide to building MCP servers with pmcp.

**📖 [Read Online](https://paiml.github.io/rust-mcp-sdk/book/)**

**Coverage:**
- **Getting Started**: Installation, first server, quick start tutorial
- **Core Concepts**: Tools, resources, prompts, error handling
- **Advanced Features**: Auth, transports, middleware, progress tracking
- **Real-World**: Production servers, testing, deployment, performance
- **Examples & Patterns**: Complete examples and design patterns
- **TypeScript Migration**: Complete compatibility guide
- **Advanced Topics**: Custom transports, AI-assisted development

**Local development:**
```bash
make book-serve    # Serve at http://localhost:3000
make book-open     # Build and open in browser
```

---

### 🎓 pmcp-course (Hands-On Learning)

Interactive course with quizzes, exercises, and real-world projects for mastering MCP development.

**🎓 [Start the Course](https://paiml.github.io/rust-mcp-sdk/course/)**

**Course Structure:**
- **Part I: Foundations** - MCP concepts, first server, typed tools
- **Part II: Core Concepts** - Tools, resources, prompts, validation
- **Part III: Deployment** - AWS Lambda, Cloudflare Workers, Google Cloud Run
- **Part IV: Testing** - Local testing, CI/CD, regression testing
- **Part V: Security** - OAuth 2.0, identity providers, multi-tenant
- **Part VI: AI-Assisted Dev** - Claude Code, feedback loops, collaboration
- **Part VII: Observability** - Middleware, logging, metrics
- **Part VIII: Advanced** - Server composition, MCP Apps (experimental)

**Features:**
- Interactive quizzes after each chapter
- Hands-on exercises with solutions
- Real-world project examples
- Best practices from production servers

**Local development:**
```bash
cd pmcp-course && mdbook serve    # Serve at http://localhost:3000
```

---

### 🤖 ai-agents (AI-Assisted Development)

AI agent configurations that teach Kiro and Claude Code how to build MCP servers.

**Supported AI Assistants:**

**Kiro (Steering Files)** - 10,876 lines of persistent MCP expertise
- Always-active knowledge in every conversation
- Comprehensive testing and observability guidance
- [Installation Guide](ai-agents/kiro/mcp-developer-power/)

**Claude Code (Subagent)** - ~750 lines of focused MCP knowledge
- On-demand invocation for MCP tasks
- Quick scaffolding and implementation
- [Installation Guide](ai-agents/claude-code/)

**What AI agents know:**
- MCP protocol concepts and patterns
- cargo-pmcp workflow (never creates files manually)
- Type-safe tool implementation
- Testing strategies (unit, integration, property, fuzz)
- Production observability (logging, metrics)
- Toyota Way quality standards

**Community implementations welcome:**
- GitHub Copilot, Cursor, Cline, and others
- [Contribution Guide](ai-agents/README.md)

**Learn more**: [AI-Assisted Development Course](https://paiml.github.io/rust-mcp-sdk/course/part6-ai-dev/ch15-ai-assisted.html) | [ai-agents/](ai-agents/)

---

### 🎨 MCP Apps (Rich UI Widgets)

Build rich HTML UI widgets served from MCP servers — works with ChatGPT, Claude, and other MCP clients.

**What it does:**
- **Preview**: Live widget preview with dual proxy/WASM bridge modes
- **Author**: File-based widgets in `widgets/` directory with hot-reload
- **Scaffold**: `cargo pmcp app new` generates a complete MCP Apps project
- **Publish**: ChatGPT-compatible manifest and standalone demo landing pages
- **Test**: 20 E2E browser tests via chromiumoxide CDP

**Quick start:**
```bash
# Scaffold a new MCP Apps project
cargo pmcp app new my-widget-app
cd my-widget-app

# Run the server
cargo run

# Preview in browser (separate terminal)
cargo pmcp preview --url http://localhost:3000 --open

# Generate deployment artifacts
cargo pmcp app build --url https://my-server.example.com
```

**Examples:**
- [Chess App](examples/mcp-apps-chess/) — Interactive chess board with move validation
- [Map App](examples/mcp-apps-map/) — Leaflet.js geospatial city explorer
- [Data Viz App](examples/mcp-apps-dataviz/) — Chart.js dashboard with SQL queries

**Learn more**: [Widget Runtime](packages/widget-runtime/) | [Preview Server](crates/mcp-preview/) | [E2E Tests](crates/mcp-e2e-tests/)

---

## Agents & Teams

Everything so far builds MCP **servers** — things that wait to be called. An agent is
the other side: it owns a decision loop, so it *is* an MCP client. PMCP ships that loop
as a crate and scaffolds it from the CLI, so you can watch an agent make decisions
before writing any Rust.

```bash
# Scaffold a runnable agent package (AgentPackage manifest + crate)
cargo pmcp agent new research-agent
cd research-agent

# Run the loop offline first — no network, no API key
cargo pmcp agent dev --source fixed

# Then against a local model (defaults to Ollama at http://localhost:11434/v1)
cargo pmcp agent dev --source openai-compat --model llama3.2

# Run a two-member reference team in one process
cargo pmcp team dev
```

The agent's identity — instructions, model slot, token and iteration limits — lives in
an `agent.package.json` manifest, not in code, so the same package moves from your
laptop to a deployment unedited. Only `--source` changes when you swap where completions
come from; the loop does not.

**The crates behind it:**
- **🤖 `pmcp-agent`** - The `AgentEngine` loop over three seams (completion source, tool invoker, store), plus `AgentServer` for exposing an agent as an MCP tool that is sampled by its caller
- **👥 `pmcp-team-servers`** - Four reference team servers and the in-process composition runtime `cargo pmcp team dev` drives
- **📦 `pmcp-package`** - The portable AI-Package format (`AgentPackage`, `TeamPackage`) that makes an agent a description rather than a binary

```bash
# One AgentEngine, run twice: standalone, then hosted-sampled — no network, no key
cargo run -p pmcp-agent --example s50_standalone_vs_sampled
```

**Learn more**: [Chapter 12.15: Agents as MCP Clients](https://paiml.github.io/rust-mcp-sdk/book/ch12-15-agents-as-mcp-clients.html) | [`pmcp-agent`](crates/pmcp-agent/) | [`pmcp-team-servers`](crates/pmcp-team-servers/)

---

## Protocol Versions

PMCP speaks **two MCP protocol eras out of one binary**: **v1** (`2025-11-25`) and **v2**
(`2026-07-28`). The era is negotiated **per request**, not per process — the same server answers a
v1 client with sessions and a v2 client without them, concurrently, on the same port. You do not
run two fleets, and you do not choose an era at build time.

Throughout this project an era is written `v1`/`v2` with its date string, while the crate is always
written with its name attached — "pmcp 2.18". A bare version number never means a protocol era.

**Opting in, by role:**

- **Servers** — one explicit call, `ServerBuilder::with_supported_protocol_versions(...)`, listing your v1 version *alongside* `PROTOCOL_VERSION_2026_07_28`. The default accept-list is v1-only by design, so upgrading the crate alone leaves a server answering v1 only. See `examples/s47_v2_stateless_mrtr.rs`.
- **Clients** — one explicit call, `ClientBuilder::with_protocol_version(...)`. Without it a client stays on v1, exactly as before.
- **Agents** — already done for you. `pmcp-agent`'s invoker prefers v2 and falls back to v1, so an agent scaffolded with `cargo pmcp agent new` speaks v2 wherever the server supports it.

There is also a server-side lever in the *opposite* direction — opting **out** of v1:

```bash
cargo build -p pmcp --no-default-features --features full-v2
```

`--no-default-features` on its own proves nothing (it also strips the HTTP transport, so v1 would
appear severed only because nothing was compiled). `full-v2` is the positive feature list: exactly
what `full` carries, minus `v1-compat`.

**Learn more**: [Chapter 12.17: Migrating to MCP 2026-07-28 (v2)](https://paiml.github.io/rust-mcp-sdk/book/ch12-17-migrating-to-mcp-2026-07-28.html) | [v1 sunset policy](docs/v1-sunset-policy.md)

---

## Latest Release: pmcp 2.18.0

**Published 2026-08-16.** The in-flight line is **pmcp 2.19.0**, still unreleased — its entry at the
top of the changelog is the authoritative list of what is landing next.

- **Two protocol eras, one binary**: pmcp serves both MCP `2025-11-25` (v1) and `2026-07-28` (v2), negotiated per request — see [Protocol Versions](#protocol-versions) above
- **Credential storage (2.18.0)**: one machine, one credential store — `pmcp::shared::credential_store` addressed by `(issuer, account, server)`, with an on-disk `FileCredentialStore` that `cargo pmcp auth` writes through
- **Agents & Teams**: the `pmcp-agent` loop, four reference team servers, and the portable AI-Package format — see [Agents & Teams](#agents--teams) above
- **MCP Apps**: Rich interactive HTML UI widgets served from MCP servers — works with ChatGPT, Claude Desktop, and other MCP clients. Live preview with browser-style DevTools (resizable panel, network/events/protocol/bridge tabs)
- **MCP Tasks**: Shared client/server task lifecycle with pluggable stores (in-memory and DynamoDB) and task variables — the v2 shape is an extension and is still provisional
- **Conformance Test Suite**: 19-scenario conformance engine across 5 domains with `cargo pmcp test conformance` and `mcp-tester conformance` CLI integration
- **Tower Middleware**: DNS rebinding protection, CORS with origin-locked headers, configurable security headers — production-ready HTTP stack
- **PMCP Server**: MCP server exposing SDK developer tools (test, scaffold, schema export) via Streamable HTTP, deployed on AWS Lambda
- **Uniform Constructor DX**: Default impls, builders, and constructors for all protocol types — dramatically improved ergonomics
- **85 examples in [`examples/`](examples/)**: runnable coverage of the SDK surface (119 example targets across the whole workspace)

**Full changelog**: [CHANGELOG.md](CHANGELOG.md)

---

## Core Features

### 🚀 **Transport Layer**
- **stdio**: Standard input/output for CLI integration
- **HTTP/SSE**: Streamable HTTP with Server-Sent Events
- **WebSocket**: Full-duplex with auto-reconnection
- **WASM**: Browser and Cloudflare Workers support

### 🛠️ **Type-Safe Development**
- **Automatic Schema Generation**: From Rust types using `schemars`
- **Compile-Time Validation**: Type-checked tool arguments
- **Runtime Validation**: Against generated JSON schemas
- **Zero Unwraps**: Explicit error handling throughout

### 🔐 **Security & Auth**
- **OAuth 2.0**: Full auth context pass-through
- **OIDC Discovery**: Automatic provider configuration
- **Bearer Tokens**: Standard authentication
- **Path Validation**: Secure file system access

### 🧪 **Testing & Quality**
- **mcp-tester**: Comprehensive server testing tool
- **Scenario Generation**: Auto-generate test cases
- **Property Testing**: Invariant validation
- **Quality Gates**: fmt, clippy, coverage enforcement

### ⚡ **Performance**
- **16x faster** than TypeScript SDK
- **50x lower memory** usage
- **SIMD Parsing**: 10.3x SSE speedup with AVX2/SSE4.2
- **Connection Pooling**: Smart load balancing

### 🏭 **Toyota Way Quality**
- **Zero Technical Debt**: TDG score 0.76
- **Jidoka**: Stop the line on defects
- **Genchi Genbutsu**: Go and see (evidence-based)
- **Kaizen**: Continuous improvement
- **No Unwraps**: Explicit error handling only

---

## Documentation

### 📖 Primary Resources

- **[PMCP Documentation Portal](https://paiml.github.io/rust-mcp-sdk/)** - Landing page for all documentation
- **[pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/)** - Comprehensive reference guide (27 chapters)
- **[pmcp-course](https://paiml.github.io/rust-mcp-sdk/course/)** - Hands-on course with quizzes and exercises
- **[API Reference](https://docs.rs/pmcp)** - Complete API documentation
- **[cargo-pmcp Guide](cargo-pmcp/README.md)** - CLI toolkit documentation

### 📚 Additional Resources

- **[Examples](examples/)** - 85 working examples in `examples/` (119 example targets workspace-wide)
- **[CHANGELOG](CHANGELOG.md)** - Version history
- **[Migration Guides](docs/)** - Upgrade instructions
- **[Contributing](CONTRIBUTING.md)** - How to contribute

### 🎯 Quick Links

- [Quick Start Tutorial](https://paiml.github.io/rust-mcp-sdk/book/ch01_5-quick-start-tutorial.html)
- [Your First Server](https://paiml.github.io/rust-mcp-sdk/book/ch02-first-server.html)
- [Course: Getting Started](https://paiml.github.io/rust-mcp-sdk/course/part1-foundations/ch01-enterprise-case.html)
- [Testing Guide](https://paiml.github.io/rust-mcp-sdk/course/part4-testing/ch11-local-testing.html)
- [Production Deployment](https://paiml.github.io/rust-mcp-sdk/course/part3-deployment/ch07-deployment.html)

---

## Examples

The SDK ships **85 runnable examples** in [`examples/`](examples/) — 119 example targets across the
whole workspace — covering the full SDK surface:

```bash
# Basic examples
cargo run --example c01_client_initialize   # Client setup
cargo run --example s01_basic_server        # Basic server
cargo run --example c02_client_tools        # Tool usage

# Type-safe tools (v1.6.0+)
cargo run --example s16_typed_tools --features schema-generation
cargo run --example s17_advanced_typed_tools --features schema-generation

# Advanced features
cargo run --example s28_authentication      # OAuth/Bearer
cargo run --example t01_websocket_transport # WebSocket
cargo run --example m01_basic_middleware    # Middleware chain

# Agent Skills (SEP-2640) — dual-surface skill + prompt
cargo run --example s44_server_skills --features skills,full
cargo run --example c10_client_skills --features skills,full

# Agents & Teams — an agent loop, and four reference servers cooperating
cargo run --example s49_sampling_host
cargo run -p pmcp-agent --example s50_standalone_vs_sampled
cargo run -p pmcp-team-servers --example doc_review_team --features runtime

# MCP 2026-07-28 (v2) — start the SERVER first, in its own terminal; each client
# takes the server address as its argument and defaults to where the server binds
cargo run --example s47_v2_stateless_mrtr --features full   # serves on 127.0.0.1:8147
cargo run --example s48_v2_mrtr_client --features full      # then, in another terminal
cargo run --example s53_v2_agent_client --features full     # the pmcp-agent connector, v2 with v1 fallback

# MCP 2026-07-28 (v2) Tasks — same rule: server first, then the agent
cargo run --example s50_v2_tasks_server --features full     # serves on 127.0.0.1:8150
cargo run --example s51_v2_tasks_agent --features full

# Testing (mcp-tester is a standalone Cargo project in examples/26-server-tester)
cargo install mcp-tester && mcp-tester test http://localhost:8080

# AI-assisted development
# See ai-agents/README.md for Kiro and Claude Code setup
```

See [examples/README.md](examples/README.md) for complete list.

---

## MCP Server Tester

Comprehensive testing tool for validating MCP server implementations.

**Features:**
- Protocol compliance validation (JSON-RPC 2.0, MCP spec)
- Multi-transport support (HTTP, HTTPS, WebSocket, stdio)
- Tool discovery and testing
- CI/CD ready with JSON output

**Installation:**
```bash
cargo install mcp-server-tester

# Or download pre-built binaries from releases
```

**Usage:**
```bash
# Test a server
mcp-tester test http://localhost:8080

# Protocol compliance check
mcp-tester compliance http://localhost:8080 --strict

# Connection diagnostics
mcp-tester diagnose http://localhost:8080
```

**Learn more**: [examples/26-server-tester/README.md](examples/26-server-tester/README.md)

---

## Quality & Performance

### Toyota Way Principles

PMCP is built following Toyota Production System principles:

- **Jidoka (自働化)**: Automation with human touch
  - Quality gates stop builds on defects
  - Zero tolerance for `unwrap()` in production
  - Comprehensive error handling with context

- **Genchi Genbutsu (現地現物)**: Go and see
  - Evidence-based decisions with metrics
  - PMAT quality analysis (TDG score 0.76)
  - Comprehensive testing (unit, property, fuzz)

- **Kaizen (改善)**: Continuous improvement
  - Regular benchmarking and optimization
  - Performance regression prevention
  - Community-driven enhancements

### Quality Metrics

- **TDG Score**: 0.76 (production-ready)
- **Technical Debt**: Minimal (436h across entire codebase)
- **Complexity**: All functions ≤25 complexity
- **Coverage**: 52% line coverage, 100% function coverage
- **Linting**: Zero clippy warnings in production code

### Performance Benchmarks

```
Metric                  PMCP (Rust)     TypeScript SDK    Improvement
────────────────────────────────────────────────────────────────────
Overall Speed           16x             1x                16x faster
Memory Usage            <10 MB          ~500 MB           50x lower
SSE Parsing             336,921 ev/s    32,691 ev/s       10.3x faster
JSON-RPC Parsing        195,181 docs/s  N/A               SIMD-optimized
Round-trip Latency      <100 μs         ~1-2 ms           10-20x faster
Base64 Operations       252+ MB/s       N/A               Optimized
```

**Run benchmarks:**
```bash
make bench                           # General benchmarks
cargo run --example t08_simd_parsing_performance  # SIMD-specific
```

---

## WebAssembly Support

Full WASM support for browser and edge deployment:

**Targets:**
- **Cloudflare Workers** (wasm32-unknown-unknown)
- **WASI Runtimes** (wasm32-wasi)
- **Browser** (wasm-bindgen)

**Quick start:**
```bash
# Build for Cloudflare Workers
cargo build --target wasm32-unknown-unknown --no-default-features --features wasm

# Deploy
make cloudflare-sdk-deploy
```

**Learn more**: [WASM Guide](docs/WASM_TARGETS.md) | [WASM Example](examples/wasm-mcp-server/)

---

## Development

### Prerequisites
- Rust 1.83.0 or later
- Git

### Setup
```bash
git clone https://github.com/paiml/rust-mcp-sdk
cd rust-mcp-sdk

# Install development tools
make setup

# Run quality checks
make quality-gate
```

### Testing
```bash
make test-all           # All tests
make test-property      # Property tests
make coverage           # Coverage report
make mutants            # Mutation tests
```

### Contributing

We welcome contributions! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Ensure quality gates pass (`make quality-gate`)
4. Commit with conventional commits
5. Push and open a Pull Request

**See**: [CONTRIBUTING.md](CONTRIBUTING.md)

---

## Compatibility

| Feature | TypeScript SDK v2.0 | PMCP v2.0 (Rust) |
|---------|---------------------|------------------|
| Protocol Version | 2025-11-25 | 2025-11-25 (+ 2024-11-05 compat) |
| Transports | stdio, SSE, WebSocket | stdio, SSE, WebSocket, WASM |
| Authentication | OAuth 2.0, Bearer | OAuth 2.0, Bearer, OIDC |
| Tools | ✓ | ✓ (Type-safe + outputSchema) |
| Prompts | ✓ | ✓ (Workflows) |
| Resources | ✓ | ✓ (Subscriptions) |
| Sampling | ✓ | ✓ |
| MCP Apps | ✓ | ✓ (Preview + DevTools) |
| Agent Skills (SEP-2640) | ✓ | ✓ (dual-surface skill + prompt, byte-equal) |
| Tower Middleware | N/A | ✓ (DNS rebinding, CORS, security headers) |
| Performance | 1x | 16x faster |
| Memory | Baseline | 50x lower |

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- [Model Context Protocol](https://modelcontextprotocol.io) specification
- [TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk) for reference implementation
- [PAIML MCP Agent Toolkit](https://github.com/paiml/paiml-mcp-agent-toolkit) for quality standards
- Community contributors and early adopters

---

## Links

- **Documentation Portal**: https://paiml.github.io/rust-mcp-sdk/
- **Reference Guide**: https://paiml.github.io/rust-mcp-sdk/book/
- **Course**: https://paiml.github.io/rust-mcp-sdk/course/
- **Crates.io**: https://crates.io/crates/pmcp
- **API Docs**: https://docs.rs/pmcp
- **Issues**: https://github.com/paiml/rust-mcp-sdk/issues
- **Discussions**: https://github.com/paiml/rust-mcp-sdk/discussions

---

**Built with 🦀 Rust and ❤️ following Toyota Way principles**
