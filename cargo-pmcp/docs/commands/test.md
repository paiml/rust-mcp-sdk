# cargo pmcp test

Test MCP servers with mcp-tester.

## Usage

```
cargo pmcp test <SUBCOMMAND>
```

## Description

Run tests locally, generate scenarios from server capabilities, and manage scenarios on pmcp.run for cloud-based testing.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `check` | Quick sanity check of MCP server connectivity |
| `run` | Run test scenarios against an MCP server |
| `generate` | Generate test scenarios from server capabilities |
| `upload` | Upload scenarios to pmcp.run |
| `download` | Download scenarios from pmcp.run |
| `list` | List scenarios on pmcp.run |

---

## test check

Quick sanity check of an MCP server.

```
cargo pmcp test check --url <URL> [OPTIONS]
```

Verifies the server is reachable, responds to the initialize handshake, and reports its capabilities. Fastest way to verify a server is working.

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--url <URL>` | *(required)* | URL of the MCP server to check |
| `--transport <TYPE>` | auto-detected | Transport type: `http`, `jsonrpc`, or `stdio` |
| `--verbose`, `-v` | - | Show raw JSON-RPC messages |
| `--timeout <SECS>` | `30` | Connection timeout in seconds |

### Example

```bash
cargo pmcp test check --url http://localhost:3000
cargo pmcp test check --url https://my-server.example.com --verbose
```

---

## test run

Run test scenarios against an MCP server.

```
cargo pmcp test run [OPTIONS]
```

Runs scenarios from the local filesystem against a local or remote server.

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--server <NAME>` | - | Name of the local server to test (uses localhost) |
| `--url <URL>` | - | URL of the server (for remote testing) |
| `--port <PORT>` | `3000` | Port to connect to |
| `--scenarios <PATH>` | auto-discovered | Path to scenario files or directory |
| `--transport <TYPE>` | auto-detected | Transport type: `http`, `jsonrpc`, or `stdio` |
| `--detailed` | - | Show detailed test output |

### Examples

```bash
# Test a local server
cargo pmcp test run --server calculator

# Test a remote server
cargo pmcp test run --url https://my-server.example.com --detailed

# Run specific scenarios
cargo pmcp test run --server calculator --scenarios scenarios/calculator/smoke.yaml
```

---

## test generate

Generate test scenarios from server capabilities.

```
cargo pmcp test generate [OPTIONS]
```

Connects to a running server and generates YAML test scenarios based on its tools, resources, and prompts.

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--server <NAME>` | - | Name of the local server |
| `--url <URL>` | - | URL of the server |
| `--port <PORT>` | `3000` | Port to connect to |
| `--output <PATH>` | auto | Output file path |
| `--transport <TYPE>` | auto-detected | Transport type |
| `--all-tools <BOOL>` | `true` | Include all tools (`true` or `false`) |
| `--with-resources <BOOL>` | `true` | Include resource operations (`true` or `false`) |
| `--with-prompts <BOOL>` | `true` | Include prompt operations (`true` or `false`) |

### Example

```bash
cargo pmcp test generate --server calculator
cargo pmcp test generate --url https://my-server.example.com --output tests/custom.yaml
```

---

## test upload

Upload test scenarios to pmcp.run.

```
cargo pmcp test upload --server-id <ID> <PATHS>...
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `PATHS` | Yes | Path(s) to scenario files or directories |

### Options

| Option | Description |
|--------|-------------|
| `--server-id <ID>` | Server ID (deployment ID) on pmcp.run |
| `--name <NAME>` | Override scenario name (single file uploads only) |
| `--description <TEXT>` | Description for the scenario |

### Example

```bash
cargo pmcp test upload --server-id my-server scenarios/calculator/
```

---

## test download

Download test scenarios from pmcp.run.

```
cargo pmcp test download --scenario-id <ID> [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--scenario-id <ID>` | *(required)* | Scenario ID to download |
| `--output <PATH>`, `-o` | stdout | Output file path |
| `--format <FMT>` | `yaml` | Output format (`yaml` or `json`) |

---

## test list

List test scenarios on pmcp.run.

```
cargo pmcp test list --server-id <ID> [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--server-id <ID>` | Server ID (deployment ID) on pmcp.run |
| `--all` | Show all scenarios including disabled ones |

## Related Commands

- [`cargo pmcp dev`](dev.md) - Start a server before testing
- [`cargo pmcp loadtest`](loadtest.md) - Load test with concurrent virtual users
- [`cargo pmcp validate`](validate.md) - Validate workflows structurally
