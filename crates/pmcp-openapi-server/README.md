# pmcp-openapi-server

Shape A pure-config OpenAPI MCP server binary — point it at a `config.toml` (and, optionally, an OpenAPI spec) and serve a production MCP server with **no Rust required**.

**Status:** 0.1.0 — early access. The pipeline is fully implemented; the public config/CLI surface may evolve as DX matures across Phase 90 and beyond.

> **Most people don't run this binary directly.** The recommended way to build and
> ship a config-driven OpenAPI MCP server is the `cargo pmcp` CLI:
> `cargo pmcp new my-server --kind openapi-server` scaffolds a project, `cargo run`
> serves it locally, and `cargo pmcp deploy` ships it to AWS Lambda / Cloud Run /
> Cloudflare / pmcp.run. See the user guide in
> [`cargo-pmcp/README.md`](../../cargo-pmcp/README.md) (and the *Config-Driven
> OpenAPI Server* chapters in the PMCP book and course).
>
> **This README is for the other path:** running the prebuilt `pmcp-openapi-server`
> binary as-is, or **extending the toolkit** (adding behavior to the HTTP connector
> or Code Mode surface). The CLI scaffold ("Shape B") generates a small crate that
> uses this crate's `dispatch` + `build_server` library seam with its own `main.rs`;
> this crate ("Shape A") is the standalone, no-Rust-required *binary*. They are
> siblings built on the same [`pmcp-server-toolkit`](../pmcp-server-toolkit) — the
> scaffold does not invoke this binary.

## The improvement (why this exists)

To expose an HTTP/REST API over the Model Context Protocol today, you hand-write a Rust binary against the SDK: wire a `ServerBuilder`, implement a tool handler for every endpoint, construct and manage an HTTP client + outgoing auth, wire up Code Mode policy, stand up the HTTP transport, and **recompile for every endpoint or tool change**.

`pmcp-openapi-server` collapses that into two inputs and one binary:

- A **`config.toml`** declaring `[server]`, `[backend]`, `[code_mode]`, `[[tools]]`, and optional `[[resources]]` / `[[prompts]]`.
- An **optional OpenAPI spec** (`--spec`, JSON or YAML) served as the Code Mode `api_schema` resource.

You run one binary, change tools by editing the config, and never recompile. It is the runnable binary built on top of the [`pmcp-server-toolkit`](../pmcp-server-toolkit) library — everything the binary does is wiring of toolkit primitives plus a `[backend]` → `(HttpConnector, HttpCodeExecutor)` dispatch seam.

It follows the **Pareto model**: roughly 20% of operations are covered by curated `[[tools]]` rows, and the ~80% long tail is handled by **Code Mode** generating scripts against the OpenAPI contract. You curate the common operations; Code Mode handles the rest. Both halves run over the **same HTTP engine** — one `reqwest::Client` and one outgoing-auth provider feed the single-call tools, the script tools, and Code Mode alike.

## The `cargo pmcp` on-ramp (recommended)

If you are building a new server, start with the scaffold rather than this binary:

```bash
cargo pmcp new my-openapi-server --kind openapi-server
cd my-openapi-server
cargo run        # serves locally over streamable HTTP
```

The scaffold emits a single runnable crate (a `config.toml`, a minimal `api.yaml`, a tiny `src/main.rs` that calls this crate's `dispatch` + `build_server`, and a `deploy.toml`). Edit the config, re-run, deploy with `cargo pmcp deploy`. This README documents the **Shape A binary** for the cases where you want a prebuilt, no-Rust binary instead.

## What this crate is NOT

- **Not the library.** The reusable building blocks (config types, the HTTP connector, outgoing auth, the `[[tools]]` synthesizer, Code Mode wiring) live in `pmcp-server-toolkit`. This crate is the runnable binary plus a `dispatch`/`build_server` seam on top of it.
- **Not a SQL toolkit.** For config-driven SQL servers, see the sibling [`pmcp-sql-server`](../pmcp-sql-server).
- **Not an API of its own.** You supply the backend `base_url`, the outgoing auth, and your declared operations; the binary calls your real HTTP backend.

## Two kinds of tools (D-01)

A `[[tools]]` entry is one of two kinds. Both are declared in the same config and both run over the same HTTP engine:

| Kind | How you declare it | What it does |
| ---- | ------------------ | ------------ |
| **single-call** | `path = "..."` + `method = "..."` | Maps 1:1 onto one backend endpoint, with typed `[[tools.parameters]]`. |
| **script** | `script = """ ... """` | A multi-call orchestration in an engine-accurate JS subset — fetch, filter, fan-out to more endpoints, shape the result. |

Worked example (from the `london-tube` reference fixture — a single-call tool and a script tool side by side):

```toml
# A single-call tool: one endpoint, no parameters.
[[tools]]
name = "get-tube-status"
description = "Get the current status of all London Underground lines."
path = "/Line/Mode/tube/Status"
method = "GET"

# A script tool: fetch statuses, filter to disrupted lines, fan out for detail.
[[tools]]
name = "disrupted-lines-with-detail"
description = "List currently-disrupted tube lines with per-line disruption detail."
script = """
const statuses = await api.get('/Line/Mode/tube/Status');
const disrupted = statuses.filter(line => line.lineStatuses.some(s => s.statusSeverity < 10));
const out = [];
for (const line of disrupted) {
  const detail = await api.get(`/Line/${line.id}/Disruption`);
  out.push({ line: line.name, detail: detail, max: args.maxLines });
}
return { count: out.length, lines: out };
"""

[[tools.parameters]]
name = "maxLines"
type = "integer"
required = false
default = 5
```

The ~80% you don't curate is Code Mode: with `[code_mode] enabled = true`, the server exposes `validate_code` / `execute_code`, and the agent writes scripts against the `api_schema` resource (your `--spec`).

## Outgoing authentication (D-05)

The `[backend.auth] type` value selects how the binary authenticates to your backend. There are six variants — `none` plus five authenticated ones — traced to `pmcp_server_toolkit::http::auth::AuthConfig`:

| `type`                       | Mechanism                                                        | Key `[backend.auth]` fields |
| ---------------------------- | --------------------------------------------------------------- | --------------------------- |
| `none`                       | No authentication (the default).                                | —                           |
| `api_key`                    | API key as query parameter(s) and/or header(s).                 | `query_params`, `headers`, `required` |
| `bearer`                     | `Authorization: Bearer <token>`.                                | `token`, `required` |
| `basic`                      | `Authorization: Basic <base64(user:pass)>`.                     | `username`, `password`, `required` |
| `oauth2_client_credentials`  | OAuth2 client-credentials grant (fetches + caches a token).     | `token_url`, `client_id`, `client_secret`, `scopes`, `required` |
| `oauth_passthrough`          | Forwards the **incoming** MCP client token to the backend (SSO). | `target_header`, `required` |

Secrets use `${ENV_VAR}` (or `env:VAR`) references resolved at provider-construction time — never inline a real token in a committed config. An `api_key` value of `"${TFL_APP_KEY}"`, for example, is resolved from the process env; an unset `required = false` reference is omitted rather than sent as the literal placeholder.

```toml
[backend]
base_url = "https://api.tfl.gov.uk"

[backend.auth]
type = "api_key"
query_params = { app_key = "${TFL_APP_KEY}" }
required = false
```

The static variants (`none` / `api_key` / `bearer` / `basic` / `oauth2_client_credentials`) ignore any inbound MCP token. Only `oauth_passthrough` forwards the per-request inbound token to the backend.

### `oauth_passthrough` trust boundary (WR-04)

`oauth_passthrough` relays a **client-controlled** credential into an **operator-controlled** destination:

- The **MCP client controls the forwarded token VALUE** — it is the raw inbound `Authorization` header the client sent, captured at the binary boundary and passed through verbatim (the binary prefixes a bare token with `Bearer ` when no scheme is present).
- The **operator controls the destination header NAME** — `target_header` is fixed in your committed config; the client cannot redirect the token to a different header.

Relaying the client's own credential to the backend is the **intended** SSO-passthrough behavior: use it only when the backend should receive the MCP client's own identity. The control-character rejection in `HeaderValue::try_from` is the protection against header-injection; a malformed token value is rejected rather than relayed. Do **not** enable `oauth_passthrough` when the backend should be called with a server-side service credential — use `bearer` / `oauth2_client_credentials` for that instead.

## The OpenAPI spec is optional (D-03)

`--spec` is **optional**. A curated-only server — only single-call and/or script `[[tools]]`, no Code Mode contract resource — boots with no spec at all. When you do supply `--spec`, it is served as the `api_schema` Code Mode resource so the agent can author scripts against your real OpenAPI contract.

If `[code_mode] enabled = true` but no `--spec` is supplied, the server emits a warning and proceeds: Code Mode runs **without** the `api_schema` resource (the agent generates scripts without the contract) rather than failing.

## Quickstart

### 1. Build / install

```bash
# Build the binary.
cargo build -p pmcp-openapi-server --release

# Or install it on your PATH.
cargo install --path crates/pmcp-openapi-server
```

### 2. A minimal `config.toml`

```toml
[server]
name = "OpenAPI Server Min Demo"
version = "0.1.0"

[backend]
base_url = "https://api.example.com"

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
# Use a "${ENV_VAR}" reference — resolved at wiring time. Never inline a real secret.
token_secret = "${CODE_MODE_SECRET}"

# ~20% curated tools; Code Mode covers the long tail against the api_schema resource.
[[tools]]
name = "get_widget"
description = "Fetch a widget by id"
path = "/widgets/{id}"
method = "GET"

[[tools.parameters]]
name = "id"
type = "string"
description = "Widget id"
required = true
```

The full `london-tube` reference configuration — a TfL demo with `api_key` query-param auth, a single-call tool, and a script tool — lives at [`tests/fixtures/london-tube.toml`](tests/fixtures/london-tube.toml).

### 3. Run it

```bash
# Required: --config. --spec is OPTIONAL (curated-only boots without it).
pmcp-openapi-server --config config.toml

# Serve the OpenAPI spec as the Code Mode api_schema resource.
pmcp-openapi-server --config config.toml --spec openapi.yaml

# Override the bind address (default 127.0.0.1:8080).
pmcp-openapi-server --config config.toml --http 0.0.0.0:9000

# Control log verbosity via RUST_LOG (the binary inits a tracing EnvFilter).
RUST_LOG=info pmcp-openapi-server --config config.toml --spec openapi.yaml
```

The server is served over the streamable-HTTP transport. By default it binds loopback (`127.0.0.1:8080`) and restricts origins to localhost, so the out-of-the-box binary does not expose a public listener.

### CLI flags

| Flag       | Required | Default            | Purpose |
| ---------- | -------- | ------------------ | ------- |
| `--config` | yes      | —                  | Server `config.toml` (`[server]` + `[backend]` + `[[tools]]` + `[code_mode]`). |
| `--spec`   | no       | (none)             | OpenAPI document (JSON or YAML) served as the `api_schema` Code Mode resource (D-03). |
| `--http`   | no       | `127.0.0.1:8080`   | Bind address (`host:port`) for the streamable-HTTP transport. |

## Design context

For the architectural responsibility map, the `[backend]` → `(HttpConnector, HttpCodeExecutor)` dispatch seam, the one-engine decision (D-02), and the locked decisions behind the optional-spec model, see the `.planning/phases/90-openapi-built-in-server/` design log.
