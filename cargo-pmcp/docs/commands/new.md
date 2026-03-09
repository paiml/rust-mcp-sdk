# cargo pmcp new

Create a new MCP workspace.

## Usage

```
cargo pmcp new <NAME> [OPTIONS]
```

## Description

Creates a workspace with `server-common` template and scaffolding for building multiple MCP servers. The workspace pattern allows sharing common code (like HTTP bootstrap) across all servers.

The generated workspace includes:
- `Cargo.toml` workspace manifest
- `crates/` directory for server crates
- `scenarios/` directory for test scenarios
- `lambda/` directory for Lambda deployment wrappers
- `server-common` shared library crate

## Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | Yes | Name of the workspace to create |

## Options

| Option | Description |
|--------|-------------|
| `--path <DIR>` | Directory to create workspace in (defaults to current directory) |

## Examples

**Create a workspace in the current directory:**
```bash
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace
```

**Create in a specific directory:**
```bash
cargo pmcp new my-workspace --path ~/projects
```

## What's Generated

```
my-mcp-workspace/
  Cargo.toml          # Workspace manifest
  crates/             # Server crates go here
  scenarios/          # Test scenario YAML files
  lambda/             # Lambda deployment wrappers
  server-common/      # Shared HTTP bootstrap library
```

## Related Commands

- [`cargo pmcp add`](add.md) - Add a server to the workspace
- [`cargo pmcp dev`](dev.md) - Start a development server
