# cargo pmcp add

Add a component to the workspace.

## Usage

```
cargo pmcp add <SUBCOMMAND>
```

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `server` | Add a new MCP server to the workspace |
| `tool` | Add a tool to an existing server |
| `workflow` | Add a workflow to an existing server |

---

## add server

Add a new MCP server to the workspace.

```
cargo pmcp add server <NAME> [OPTIONS]
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | Yes | Name of the server (creates `mcp-{name}-core` and `{name}-server` crates) |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--template <TEMPLATE>` | `minimal` | Server template to use (`minimal`, `complete`, `sqlite-explorer`) |
| `--port <PORT>` | auto | Port to assign (auto-increments if not specified) |
| `--replace` | - | Replace existing server with same name (requires confirmation) |

### Examples

```bash
# Add a server with the default minimal template
cargo pmcp add server calculator

# Add with the complete template (tools, prompts, resources included)
cargo pmcp add server calculator --template complete

# Add on a specific port
cargo pmcp add server api --port 3001

# Replace an existing server
cargo pmcp add server calculator --template complete --replace
```

---

## add tool

Add a tool to an existing server.

```
cargo pmcp add tool <NAME> --server <SERVER>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | Yes | Name of the tool |

| Option | Required | Description |
|--------|----------|-------------|
| `--server <SERVER>` | Yes | Server to add the tool to |

---

## add workflow

Add a workflow to an existing server.

```
cargo pmcp add workflow <NAME> --server <SERVER>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | Yes | Name of the workflow |

| Option | Required | Description |
|--------|----------|-------------|
| `--server <SERVER>` | Yes | Server to add the workflow to |

## Related Commands

- [`cargo pmcp new`](new.md) - Create the workspace first
- [`cargo pmcp dev`](dev.md) - Start the server after adding it
