# cargo pmcp schema

Export and manage schemas from MCP servers.

## Usage

```
cargo pmcp schema <SUBCOMMAND>
```

## Description

Connect to a foundation MCP server, export its schema (tools, resources, prompts), and generate typed Rust client code for calling its tools. Supports both MCP HTTP and Lambda invocation.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `export` | Export schema from an MCP server endpoint |
| `validate` | Validate a local schema file |
| `diff` | Compare local schema with a live server |

---

## schema export

Export schema from an MCP server endpoint.

```
cargo pmcp schema export [OPTIONS]
```

Connects to the server, performs an MCP initialize handshake, then fetches tools, resources, and prompts. Writes a JSON schema file.

### Options

| Option | Description |
|--------|-------------|
| `--endpoint <URL>`, `-e` | MCP server endpoint URL |
| `--server <ID>`, `-s` | Server ID on pmcp.run (alternative to `--endpoint`) |
| `--output <PATH>`, `-o` | Output file path (default: `schemas/<server_id>.json`) |

One of `--endpoint` or `--server` is required.

### Examples

```bash
# Export from a URL
cargo pmcp schema export --endpoint https://mcp.example.com

# Export from a pmcp.run server
cargo pmcp schema export --server db-demo

# Export to a custom path
cargo pmcp schema export --endpoint https://mcp.example.com --output my-schema.json
```

---

## schema validate

Validate a local schema file.

```
cargo pmcp schema validate <SCHEMA>
```

Checks that the schema JSON is well-formed and contains required fields (`server_id`, `name`, tool names).

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `SCHEMA` | Yes | Path to the schema file to validate |

### Example

```bash
cargo pmcp schema validate schemas/calculator.json
```

---

## schema diff

Compare a local schema with a live server.

```
cargo pmcp schema diff <SCHEMA> --endpoint <URL>
```

Connects to the live server, fetches its current tools, and reports added/removed tools compared to the local schema file.

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `SCHEMA` | Yes | Path to the local schema file |

### Options

| Option | Required | Description |
|--------|----------|-------------|
| `--endpoint <URL>`, `-e` | Yes | MCP server endpoint to compare against |

### Example

```bash
cargo pmcp schema diff schemas/calculator.json --endpoint https://calc.example.com/mcp
```

## Related Commands

- [`cargo pmcp validate`](validate.md) - Validate workflows
- [`cargo pmcp test`](test.md) - Test against live servers
