# cargo pmcp dev

Start a development server.

## Usage

```
cargo pmcp dev --server <NAME> [OPTIONS]
```

## Description

Builds and runs an MCP server with HTTP transport for local development. The server starts on `http://0.0.0.0:<port>` with live logs in the terminal.

If the server has a configured port in the workspace config, that port is used automatically (unless overridden with `--port`).

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `--server <NAME>` | *(required)* | Name of the server to run |
| `--port <PORT>` | `3000` | Port to run the server on (overrides workspace config) |
| `--connect <CLIENT>` | - | Automatically connect to an MCP client (`claude-code`, `cursor`, `inspector`) |

## Examples

**Start a server:**
```bash
cargo pmcp dev --server calculator
```

**Start on a custom port:**
```bash
cargo pmcp dev --server calculator --port 8080
```

**Start and connect to Claude Code in one step:**
```bash
cargo pmcp dev --server calculator --connect claude-code
```

## What It Does

1. Builds the server binary (`cargo build --bin {name}-server`)
2. Optionally runs `connect` to register with an MCP client
3. Starts the server in the foreground with `MCP_HTTP_PORT` and `RUST_LOG=info`

Press **Ctrl+C** to stop the server.

## Related Commands

- [`cargo pmcp connect`](connect.md) - Connect to a client separately
- [`cargo pmcp test`](test.md) - Run tests against the running server
