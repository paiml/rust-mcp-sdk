# cargo pmcp landing

Manage landing pages for MCP servers.

## Usage

```
cargo pmcp landing <SUBCOMMAND>
```

## Description

Create, develop, and deploy landing pages that showcase your MCP server. Landing pages help users discover and install MCP servers.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `init` | Initialize a new landing page |
| `dev` | Run the landing page locally for development |
| `build` | Build the landing page for production |
| `deploy` | Deploy the landing page to a target |

---

## landing init

Initialize a new landing page for your MCP server.

```
cargo pmcp landing init [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--template <TEMPLATE>` | `nextjs` | Template to use |
| `--output <DIR>` | `./landing` | Output directory for landing page |
| `--server-name <NAME>` | auto-detected | MCP server name (reads from `pmcp.toml` if not provided) |

### Example

```bash
cargo pmcp landing init
cargo pmcp landing init --template nextjs --output ./my-landing
```

---

## landing dev

Run the landing page locally for development.

```
cargo pmcp landing dev [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--dir <DIR>` | `./landing` | Landing page directory |
| `--port <PORT>` | `3001` | Port to run on |
| `--watch` | - | Watch for changes in `pmcp-landing.toml` |

### Example

```bash
cargo pmcp landing dev --watch
cargo pmcp landing dev --port 4000
```

---

## landing build

Build the landing page for production.

> **Note:** This subcommand is not yet implemented. Use `cd ./landing && npm run build` as a workaround.

```
cargo pmcp landing build [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--dir <DIR>` | `./landing` | Landing page directory |
| `--output <DIR>` | `./landing/.next` | Build output directory |

---

## landing deploy

Deploy the landing page to a target.

```
cargo pmcp landing deploy [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--dir <DIR>` | `./landing` | Landing page directory |
| `--target <TARGET>` | `pmcp-run` | Deployment target |
| `--server-id <ID>` | auto-detected | MCP server ID to link to |

### Example

```bash
cargo pmcp landing deploy
cargo pmcp landing deploy --target pmcp-run --server-id my-server
```

## Related Commands

- [`cargo pmcp app`](app.md) - MCP Apps project management (widget-specific landing pages)
- [`cargo pmcp deploy`](deploy.md) - Deploy the MCP server itself
