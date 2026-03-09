# cargo-pmcp

Production-grade MCP server development toolkit.

## Overview

`cargo-pmcp` is a comprehensive scaffolding and development tool for building Model Context Protocol (MCP) servers using the PMCP SDK. It streamlines the entire lifecycle from project creation to production deployment.

## Features

- **Project Scaffolding** - Create workspaces and add servers with best-practice templates
- **Development Mode** - Build and run servers with HTTP transport and live logs
- **Client Connection** - One-command setup for Claude Code, Cursor, and MCP Inspector
- **Automated Testing** - Generate and run scenario-based tests from server capabilities
- **Load Testing** - Stress-test with concurrent virtual users, latency histograms, and CI/CD reports
- **Schema Management** - Export, validate, and diff schemas from live MCP servers
- **Workflow Validation** - Catch structural errors in workflows before runtime
- **MCP Apps** - Scaffold widget projects, generate ChatGPT manifests, and build landing pages
- **Widget Preview** - Browser-based preview with dual proxy/WASM bridge modes and hot-reload
- **Multi-Target Deployment** - Deploy to AWS Lambda, Google Cloud Run, Cloudflare Workers, or pmcp.run
- **Secrets Management** - Multi-provider secret storage (local, pmcp.run, AWS Secrets Manager)
- **OAuth Authentication** - Production-ready OAuth 2.0 with AWS Cognito, Dynamic Client Registration, and SSO
- **Landing Pages** - Create, develop, and deploy landing pages for server discovery

## Installation

```bash
cargo install cargo-pmcp
```

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

### 6. Deploy

```bash
# Initialize for AWS Lambda with OAuth
cargo pmcp deploy init --target aws-lambda --oauth cognito

# Deploy
cargo pmcp deploy --target aws-lambda
```

### 7. Monitor

```bash
cargo pmcp deploy logs --tail
cargo pmcp deploy metrics --period 24h
cargo pmcp deploy test --verbose
```

## Commands

| Command | Description | Reference |
|---------|-------------|-----------|
| `new` | Create a new MCP workspace | [docs/commands/new.md](docs/commands/new.md) |
| `add` | Add server, tool, or workflow to workspace | [docs/commands/add.md](docs/commands/add.md) |
| `dev` | Start development server with HTTP transport | [docs/commands/dev.md](docs/commands/dev.md) |
| `connect` | Connect server to Claude Code, Cursor, or Inspector | [docs/commands/connect.md](docs/commands/connect.md) |
| `test` | Run, generate, upload, and download test scenarios | [docs/commands/test.md](docs/commands/test.md) |
| `loadtest` | Load test with virtual users and performance reports | [docs/commands/loadtest.md](docs/commands/loadtest.md) |
| `schema` | Export, validate, and diff MCP server schemas | [docs/commands/schema.md](docs/commands/schema.md) |
| `validate` | Validate workflows and server components | [docs/commands/validate.md](docs/commands/validate.md) |
| `deploy` | Deploy to AWS Lambda, Cloud Run, Workers, pmcp.run | [docs/commands/deploy.md](docs/commands/deploy.md) |
| `secret` | Manage secrets across local, pmcp.run, and AWS | [docs/commands/secret.md](docs/commands/secret.md) |
| `app` | Scaffold MCP Apps projects with widgets | [docs/commands/app.md](docs/commands/app.md) |
| `preview` | Browser-based widget preview with hot-reload | [docs/commands/preview.md](docs/commands/preview.md) |
| `landing` | Create and deploy server landing pages | [docs/commands/landing.md](docs/commands/landing.md) |

## Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Enable verbose output for debugging |
| `--no-color` | | Suppress colored output (also respects `NO_COLOR` env and non-TTY) |
| `--quiet` | | Suppress all non-error output (verbose wins if both are set) |

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

## CI/CD Integration

`cargo-pmcp` supports OAuth 2.0 client credentials flow for automated deployments.

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
