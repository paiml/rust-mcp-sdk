# cargo pmcp loadtest

Load test MCP servers over Streamable HTTP with realistic, weighted traffic patterns.

The loadtest subsystem provides three subcommands:

| Command | Purpose |
|---------|---------|
| `init`  | Generate a starter config from a live server |
| `run`   | Execute a load test locally |
| `upload`| Push config to pmcp.run for cloud execution |

## Quick Start

```bash
# 1. Generate config by discovering tools/resources from a running server
cargo pmcp loadtest init http://localhost:3000/mcp

# 2. Edit .pmcp/loadtest.toml — adjust VUs, duration, scenario weights

# 3. Run the load test
cargo pmcp loadtest run http://localhost:3000/mcp
```

A k6-style terminal summary prints when the test completes, and a JSON report is written to `.pmcp/reports/`.

## Commands Reference

### `loadtest init [URL]`

Generates `.pmcp/loadtest.toml` in the current directory.

```bash
# Default template (placeholder tool names)
cargo pmcp loadtest init

# Discover tools/resources/prompts from a live server
cargo pmcp loadtest init http://localhost:3000/mcp

# Overwrite an existing config
cargo pmcp loadtest init http://localhost:3000/mcp --force
```

When a URL is provided, the command connects to the server, calls `tools/list`, `resources/list`, and `prompts/list`, then populates the config with real operation names and balanced weights.

The generated config includes default `[[stage]]` blocks (ramp to 5 VUs over 10s, then to 10 VUs over 10s, then hold for 60s). This avoids cold-start thundering herd problems, which is especially important when uploading configs for cloud execution on pmcp.run. Delete the stage blocks if you want flat load (all VUs start simultaneously).

### `loadtest run <URL>`

Executes a load test against the target MCP server.

```bash
cargo pmcp loadtest run http://localhost:3000/mcp
```

**Options:**

| Flag | Description |
|------|-------------|
| `--config <path>` | Config file path (default: auto-discover `.pmcp/loadtest.toml`) |
| `--vus <n>` | Override virtual user count (ignored when stages are defined) |
| `--duration <secs>` | Override test duration in seconds |
| `--iterations <n>` | Stop after N total iterations instead of using duration |
| `--no-report` | Skip writing the JSON report |
| `--no-color` | Disable colored terminal output |

Authentication options are covered in the [Authentication](#authentication) section below.

Config auto-discovery walks parent directories looking for `.pmcp/loadtest.toml`, similar to how git finds `.git/`.

### `loadtest upload`

Uploads a validated config to [pmcp.run](https://pmcp.run) for cloud-based load test execution.

```bash
cargo pmcp loadtest upload \
  --server your-deployment-id \
  .pmcp/loadtest.toml \
  --name "baseline" \
  --description "10 VU steady-state baseline"
```

**Options:**

| Flag | Description |
|------|-------------|
| `--server <id>` | **(required)** Deployment ID on pmcp.run |
| `<path>` | **(required)** Path to the TOML config file |
| `--name <name>` | Config name (defaults to filename stem) |
| `--description <text>` | Description for the config |

Requires pmcp.run authentication. Log in first with `cargo pmcp auth login`.

The config is validated locally (parsed and checked for valid scenarios) before uploading.

## Authentication

Three authentication modes, ordered from simplest to most flexible.

### No auth (default)

For unprotected MCP servers, no flags are needed:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp
```

### API Key

The simplest auth path. Pass a single flag (or environment variable) and the key is injected as a `Bearer` token on every virtual user request:

```bash
# Via flag
cargo pmcp loadtest run http://localhost:3000/mcp --api-key sk-your-key

# Via environment variable
MCP_API_KEY=sk-your-key cargo pmcp loadtest run http://localhost:3000/mcp
```

### OAuth 2.0 (PKCE)

For MCP servers protected by an OAuth 2.0 / OpenID Connect provider.

**Simplest path — auto-discovery:**

Just provide your OAuth client ID. The issuer URL is auto-discovered from the MCP server's `/.well-known/openid-configuration` endpoint:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp \
  --oauth-client-id your-client-id
```

What happens:
1. The tool fetches `http://localhost:3000/.well-known/openid-configuration` to find the authorization and token endpoints
2. Your browser opens to the provider's login page (PKCE authorization code flow)
3. After you authenticate, the callback is captured on `localhost:8080`
4. The access token is cached at `~/.pmcp/oauth-tokens.json`
5. On subsequent runs, the cached token is reused (no browser prompt)

**Explicit issuer:**

When the MCP server doesn't expose an OIDC discovery document, provide the issuer URL directly:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp \
  --oauth-client-id your-client-id \
  --oauth-issuer https://your-pool.auth.us-east-1.amazoncognito.com
```

**All OAuth options:**

| Flag | Env Variable | Default | Description |
|------|-------------|---------|-------------|
| `--oauth-client-id` | `MCP_OAUTH_CLIENT_ID` | — | Client ID (triggers OAuth flow) |
| `--oauth-issuer` | `MCP_OAUTH_ISSUER` | auto-discovered | Issuer URL |
| `--oauth-scopes` | `MCP_OAUTH_SCOPES` | `openid` | Comma-separated scopes |
| `--oauth-no-cache` | — | `false` | Disable token caching |
| `--oauth-redirect-port` | `MCP_OAUTH_REDIRECT_PORT` | `8080` | Localhost callback port |

**CI/CD usage:**

All flags have environment variable equivalents, making it easy to configure auth in CI pipelines without exposing secrets in command arguments:

```bash
export MCP_OAUTH_CLIENT_ID=abc123
export MCP_OAUTH_ISSUER=https://auth.example.com
export MCP_OAUTH_SCOPES=openid,profile

cargo pmcp loadtest run https://my-mcp-server.example.com/mcp
```

## Finding Your OAuth Credentials

### AWS Cognito

1. Open **AWS Console** > **Cognito** > **User Pools** > select your pool
2. Go to the **App integration** tab > **App client list**
3. Find your app client and copy the **Client ID** — this is your `--oauth-client-id`
4. Your issuer URL is the Cognito domain: `https://<domain>.auth.<region>.amazoncognito.com`
5. Verify the app client settings:
   - **Authorization code grant** is enabled
   - **Callback URL** includes `http://localhost:8080/callback`
   - **Allowed OAuth scopes** include `openid`

```bash
cargo pmcp loadtest run https://my-server.example.com/mcp \
  --oauth-client-id 1abc2def3ghi4jkl5mno \
  --oauth-issuer https://mypool.auth.us-east-1.amazoncognito.com
```

### Other providers (Auth0, Okta, Keycloak, etc.)

Any OIDC-compliant provider works. You need:

- **Client ID** — from your provider's app/client registration
- **Issuer URL** — the base URL of the OIDC provider (must serve `/.well-known/openid-configuration`)

If the MCP server itself exposes a discovery document, you can skip `--oauth-issuer` entirely and rely on auto-discovery.

## How Authentication Works

The authentication flow is designed for load testing ergonomics:

1. **Token acquired once at startup** — before any virtual users are spawned. If authentication fails, the test aborts immediately (fail-fast)
2. **PKCE flow** — a browser window opens, the user authenticates, and the callback is captured on `localhost:<port>`
3. **Device code fallback** — if the browser flow fails, falls back to device code authorization (RFC 8628) when the provider supports it
4. **Token caching** — the access token is cached at `~/.pmcp/oauth-tokens.json` and reused on subsequent runs. Use `--oauth-no-cache` to force a fresh token
5. **Shared across VUs** — a single `HttpMiddlewareChain` with the `Bearer` token is injected into all virtual user HTTP requests

API key mode skips the OAuth flow entirely — the key is wrapped in a `Bearer` token and injected directly.

## Config File Reference

The config file is TOML with three sections: `[settings]`, `[[scenario]]`, and `[[stage]]`.

Configs generated by `loadtest init` include default ramp-up stages. Stages are optional — delete them for flat load (all VUs launch simultaneously). However, stages are strongly recommended for cloud load testing (`loadtest upload`) to avoid cold-start thundering herd effects across distributed workers.

### Minimal config

```toml
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
arguments = { text = "hello" }
```

### Mixed scenario with all operation types

```toml
[settings]
virtual_users = 20
duration_secs = 120
timeout_ms = 5000
# request_interval_ms = 15000  # optional pacing delay per VU

[[scenario]]
type = "tools/call"
weight = 60
tool = "calculate"
arguments = { expression = "2+2" }

[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"

[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
```

### Staged load profile (ramp-up / hold / ramp-down)

```toml
[settings]
virtual_users = 1          # ignored when stages are present
duration_secs = 120        # safety ceiling
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"

[[stage]]
target_vus = 10
duration_secs = 30         # ramp up to 10 VUs over 30s

[[stage]]
target_vus = 50
duration_secs = 60         # ramp up to 50 VUs over 60s

[[stage]]
target_vus = 0
duration_secs = 30         # ramp down to 0 over 30s
```

### Settings reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `virtual_users` | `u32` | — | Concurrent VUs (ignored when stages are defined) |
| `duration_secs` | `u64` | — | Total test duration in seconds |
| `timeout_ms` | `u64` | — | Per-request timeout in milliseconds |
| `expected_interval_ms` | `u64` | `100` | Expected request interval for coordinated omission correction |
| `request_interval_ms` | `u64` | none | Pacing delay between requests per VU (omit for closed-loop) |

### Scenario step types

| Type | Required Fields | Optional |
|------|----------------|----------|
| `tools/call` | `weight`, `tool` | `arguments` (JSON object) |
| `resources/read` | `weight`, `uri` | — |
| `prompts/get` | `weight`, `prompt` | `arguments` (string map) |

## Reports

By default, each run writes a JSON report to `.pmcp/reports/loadtest-<timestamp>.json`. Use `--no-report` to disable.
