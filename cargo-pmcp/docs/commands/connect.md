# cargo pmcp connect

Connect a server to an MCP client.

## Usage

```
cargo pmcp connect --server <NAME> --client <CLIENT> [OPTIONS]
```

## Description

Configures the connection between your running MCP server and an MCP client application. Supports Claude Code (automatic), Cursor (manual config), and MCP Inspector (launches npx).

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `--server <NAME>` | *(required)* | Name of the server |
| `--client <CLIENT>` | *(required)* | MCP client to connect to (`claude-code`, `cursor`, `inspector`) |
| `--url <URL>` | `http://localhost:3000` | Server URL |

## Supported Clients

| Client | Alias(es) | Method |
|--------|-----------|--------|
| `claude-code` | `claudecode`, `claude` | Runs `claude mcp add -t http <server> <url>` |
| `cursor` | - | Prints manual config for `~/.cursor/mcp.json` |
| `inspector` | - | Starts `npx @modelcontextprotocol/inspector` |

## Examples

**Connect to Claude Code:**
```bash
cargo pmcp connect --server calculator --client claude-code
```

**Connect to Cursor with a custom URL:**
```bash
cargo pmcp connect --server calculator --client cursor --url http://localhost:8080
```

**Open MCP Inspector:**
```bash
cargo pmcp connect --server calculator --client inspector
```

## Related Commands

- [`cargo pmcp dev`](dev.md) - Start the server first (or use `--connect` flag)
