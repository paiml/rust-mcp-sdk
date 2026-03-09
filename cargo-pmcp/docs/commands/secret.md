# cargo pmcp secret

Manage secrets for MCP servers.

## Usage

```
cargo pmcp secret <SUBCOMMAND> [OPTIONS]
```

## Description

Store and retrieve secrets across multiple providers (local, pmcp.run, AWS). Secrets are namespaced by server ID to avoid conflicts.

Secret names follow the format `{server-id}/{SECRET_NAME}`, for example: `chess/ANTHROPIC_API_KEY`.

## Global Options

These options apply to all subcommands:

| Option | Default | Description |
|--------|---------|-------------|
| `--target <PROVIDER>` | auto-detected | Target provider: `pmcp`, `aws`, `local` |
| `--profile <NAME>` | - | Profile from `.pmcp/config.toml` |
| `--server <ID>` | - | Server ID for namespacing secrets |
| `--format <FMT>` | `text` | Output format: `text` or `json` |
| `--quiet` | - | Suppress non-essential output |

**Auto-detection priority:** pmcp.run auth > AWS credentials > local filesystem.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `list` | List secrets (names only, never values) |
| `get` | Get a secret value |
| `set` | Set a secret value |
| `delete` | Delete a secret |
| `providers` | Show provider status |
| `sync` | Sync secrets from configuration |

---

## secret list

List secrets (names only, never values).

```
cargo pmcp secret list [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--filter <PATTERN>` | Filter by name pattern (glob syntax) |
| `--metadata` | Include metadata (creation date, version) |

### Example

```bash
cargo pmcp secret list --server myserver
cargo pmcp secret list --server myserver --target pmcp --format json
```

---

## secret get

Get a secret value.

```
cargo pmcp secret get <NAME> [OPTIONS]
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | Yes | Secret name (format: `server-id/SECRET_NAME`) |

### Options

| Option | Description |
|--------|-------------|
| `--output <PATH>` | Write to file instead of stdout |
| `--no-newline` | Omit trailing newline (for piping) |

### Example

```bash
cargo pmcp secret get myserver/API_KEY
cargo pmcp secret get myserver/API_KEY --output ./secret.txt
cargo pmcp secret get myserver/API_KEY --no-newline | pbcopy
```

---

## secret set

Set a secret value.

```
cargo pmcp secret set <NAME> [OPTIONS]
```

Exactly one input source must be specified. If none is given and stdin is a terminal, defaults to `--prompt`.

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | Yes | Secret name (format: `server-id/SECRET_NAME`) |

### Input Sources (mutually exclusive)

| Option | Description |
|--------|-------------|
| `--prompt` | Interactive hidden input (recommended) |
| `--stdin` | Read from stdin |
| `--file <PATH>` | Read from file |
| `--env <VAR>` | Read from environment variable |
| `--value <TEXT>` | Direct value (WARNING: visible in process list) |
| `--generate` | Generate random value |

### Generation Options (used with `--generate`)

| Option | Default | Description |
|--------|---------|-------------|
| `--generate-length <N>` | `32` | Length for generated secrets |
| `--generate-charset <SET>` | `alphanumeric` | Charset: `alphanumeric`, `ascii`, `hex` |

### Other Options

| Option | Description |
|--------|-------------|
| `--description <TEXT>` | Human-readable description |
| `--no-overwrite` | Fail if secret already exists |

### Examples

```bash
# Interactive prompt (recommended)
cargo pmcp secret set myserver/API_KEY --prompt

# From environment variable
cargo pmcp secret set myserver/API_KEY --env MY_API_KEY

# From file
cargo pmcp secret set myserver/TLS_CERT --file ./cert.pem

# Generate a random session secret
cargo pmcp secret set myserver/SESSION_SECRET --generate --generate-length 64

# From stdin (pipe)
echo -n "sk-xxx" | cargo pmcp secret set myserver/API_KEY --stdin
```

---

## secret delete

Delete a secret.

```
cargo pmcp secret delete <NAME> [OPTIONS]
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | Yes | Secret name |

### Options

| Option | Description |
|--------|-------------|
| `--force` | Skip confirmation |

---

## secret providers

Show provider status.

```
cargo pmcp secret providers [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--check` | Check connectivity to each provider |

### Example

```bash
cargo pmcp secret providers --check
```

---

## secret sync

Sync secrets from configuration.

```
cargo pmcp secret sync [OPTIONS]
```

Parses a TOML file for `secret:` references and checks which secrets exist. Optionally prompts to create missing ones.

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--file <PATH>` | `pmcp.toml` | TOML file to analyze |
| `--check` | - | Check only, no changes |
| `--interactive` | - | Prompt for each missing secret |

### Example

```bash
cargo pmcp secret sync --check
cargo pmcp secret sync --interactive
```

## Security

- Secret values use the `secrecy` crate with automatic memory zeroization
- Local secrets stored with file permissions `0600`
- Debug/Display output shows `[REDACTED]` instead of actual values
- Warns when outputting secrets to terminal

## Related Commands

- [`cargo pmcp deploy`](deploy.md) - Deploy with secrets configured
- [`cargo pmcp deploy secrets`](deploy.md#deploy-secrets) - Simpler deployment-scoped secret management
