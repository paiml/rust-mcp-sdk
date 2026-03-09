# cargo pmcp loadtest

Run load tests against MCP servers.

## Usage

```
cargo pmcp loadtest <SUBCOMMAND>
```

## Description

Execute load tests with configurable virtual users, scenarios, and reports. Measures latency percentiles, throughput, error rates, and can auto-detect breaking points.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `run` | Run a load test against an MCP server |
| `init` | Generate a starter `loadtest.toml` config |
| `upload` | Upload a loadtest config to pmcp.run |

---

## loadtest run

Run a load test against an MCP server.

```
cargo pmcp loadtest run <URL> [OPTIONS]
```

Executes a load test using the scenario defined in `.pmcp/loadtest.toml` (or a custom config path). Reports results to the terminal and writes a JSON report to `.pmcp/reports/`.

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `URL` | Yes | Target MCP server URL |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--config <PATH>` | auto-discover `.pmcp/loadtest.toml` | Path to config file |
| `--vus <N>` | from config | Number of virtual users (overrides config) |
| `--duration <SECS>` | from config | Test duration in seconds (overrides config) |
| `--iterations <N>` | from config | Iteration limit (overrides config) |
| `--no-report` | - | Disable JSON report output |
| `--api-key <KEY>` | `MCP_API_KEY` env | API key for Bearer token authentication |
| `--oauth-client-id <ID>` | `MCP_OAUTH_CLIENT_ID` env | OAuth client ID (triggers OAuth flow) |
| `--oauth-issuer <URL>` | `MCP_OAUTH_ISSUER` env | OAuth issuer URL (auto-discovered if omitted) |
| `--oauth-scopes <SCOPES>` | `MCP_OAUTH_SCOPES` env | OAuth scopes, comma-separated (default: `openid`) |
| `--oauth-no-cache` | - | Disable OAuth token caching |
| `--oauth-redirect-port <PORT>` | `8080` / `MCP_OAUTH_REDIRECT_PORT` env | OAuth redirect port for localhost callback |

### Examples

```bash
# Run with defaults from config
cargo pmcp loadtest run https://my-server.example.com

# Quick test with CLI overrides
cargo pmcp loadtest run https://my-server.example.com --vus 20 --duration 60

# Run with API key authentication
cargo pmcp loadtest run https://my-server.example.com --api-key sk-xxx

# Run with OAuth
cargo pmcp loadtest run https://my-server.example.com --oauth-client-id my-client-id
```

---

## loadtest init

Generate a starter loadtest config file.

```
cargo pmcp loadtest init [URL] [OPTIONS]
```

Creates `.pmcp/loadtest.toml` with sensible defaults. If a server URL is provided, discovers available tools/resources/prompts and populates the scenario with real tool names.

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `URL` | No | Optional server URL for schema discovery |

### Options

| Option | Description |
|--------|-------------|
| `--force` | Overwrite existing config file |

### Example

```bash
# Generate with schema discovery
cargo pmcp loadtest init https://my-server.example.com

# Generate bare config
cargo pmcp loadtest init
```

---

## loadtest upload

Upload a loadtest config to pmcp.run for cloud-based execution.

```
cargo pmcp loadtest upload --server-id <ID> <PATH>
```

Validates the TOML config locally, then uploads it.

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `PATH` | Yes | Path to the loadtest TOML config file |

### Options

| Option | Description |
|--------|-------------|
| `--server-id <ID>` | Server ID (deployment ID) on pmcp.run |
| `--name <NAME>` | Override config name (defaults to filename stem) |
| `--description <TEXT>` | Description for the config |

### Example

```bash
cargo pmcp loadtest upload --server-id my-server .pmcp/loadtest.toml
```

## Config File Format

The `loadtest.toml` config uses TOML with the following structure:

```toml
[test]
vus = 10              # Number of virtual users
duration = 30         # Test duration in seconds
# iterations = 100    # Alternative: fixed iteration count

[[scenarios]]
name = "tool_call"
weight = 80           # 80% of requests
type = "tools/call"
tool = "add"
[scenarios.arguments]
a = 5
b = 3

[[scenarios]]
name = "list_resources"
weight = 20           # 20% of requests
type = "resources/list"
```

### Features

- TOML-based scenario config with weighted MCP operation mix
- HdrHistogram latency percentiles with coordinated omission correction
- k6-style live terminal progress and colorized summary report
- Stage-driven load shaping with ramp-up/hold/ramp-down phases
- Automatic breaking point detection
- Per-tool metrics breakdown
- Schema-versioned JSON reports for CI/CD pipelines

## Related Commands

- [`cargo pmcp test`](test.md) - Functional testing (not load)
- [`cargo pmcp deploy`](deploy.md) - Deploy before load testing
