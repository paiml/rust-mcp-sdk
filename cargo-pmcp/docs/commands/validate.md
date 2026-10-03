# cargo pmcp validate

Validate MCP server components.

## Usage

```
cargo pmcp validate <SUBCOMMAND>
```

## Description

Runs validation checks on workflows, tools, and other server components. Helps catch structural errors before runtime.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `workflows` | Validate all workflows in the project |
| `deploy` | Validate `.pmcp/deploy.toml` — dry-run for IAM declarations before `cargo pmcp deploy` |

---

## validate deploy

Dry-run validator for `.pmcp/deploy.toml`. Runs the same gate that `cargo pmcp deploy` runs immediately before `cdk deploy`, but makes no AWS API calls — use it as a pre-flight check.

```
cargo pmcp validate deploy [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--verbose`, `-v` | Print the resolved project root |
| `<server>` | Positional: path to the server directory (defaults to current directory) |

### What it checks

Full rule catalog lives in [DEPLOYMENT.md § IAM Declarations → Validation rules](../../DEPLOYMENT.md#validation-rules). Summary:

- **Hard error** — Wildcard escalation (`Allow` + `actions=["*"]` + `resources=["*"]`) in any `[[iam.statements]]` entry.
- **Hard error** — `effect` not in `{"Allow", "Deny"}` (case-sensitive).
- **Hard error** — Empty `actions` or `resources` in any `[[iam.statements]]`.
- **Hard error** — Action does not match `^[a-z0-9-]+:[A-Za-z0-9*]+$`.
- **Hard error** — Sugar keyword not in `{"read", "write", "readwrite"}`.
- **Hard error** — Empty `name` in any `[[iam.tables]]` or `[[iam.buckets]]`.
- **Warning** — Unknown service prefix (not in the curated 40-service list).
- **Warning** — Cross-account ARN pins (12-digit account segment).

It also runs the Lambda-settings check `cargo pmcp deploy` runs before building (see [deploy § Lambda sizing](deploy.md#lambda-sizing-server-memory_mb--timeout_seconds--ephemeral_storage_mb) and [deploy § Build options](deploy.md#build-options-build)):

- **Hard error** — `[server] ephemeral_storage_mb` outside Lambda's 512-10240 MB range.
- **Hard error** — A `[build] features` entry that is empty, starts with `-`, or contains a comma or whitespace.

And the endpoint checks `cargo pmcp deploy` runs before building (see [deploy § Post-deploy verification](deploy.md#post-deploy-verification-and-exit-codes)):

- **Hard error** — `[server] mcp_path` that does not start with `/`, or contains `?`, `#`, whitespace or a control character.
- **Hard error** — On `google-cloud-run`, a `[gcp] repository` that is not an Artifact Registry repository id (lowercase letters, digits and `-`, starting with a letter, ending with a letter or digit, at most 63 characters).

Hard errors return non-zero; warnings print to stderr but return zero.

### Examples

```bash
# Validate the project in the current directory
cargo pmcp validate deploy

# Validate a specific server directory, with verbose output
cargo pmcp validate deploy --verbose ./servers/cost-coach

# CI gate — fails on any hard-error rule
cargo pmcp validate deploy || exit 1
```

### Related

- [`cargo pmcp deploy`](deploy.md) — runs the same validator immediately before deploying (fail-closed)
- [IAM.md](../IAM.md) — task-oriented guide for declaring IAM in `.pmcp/deploy.toml`
- [DEPLOYMENT.md § IAM Declarations](../../DEPLOYMENT.md#iam-declarations-iam-section) — schema reference

---

## validate workflows

Validate all workflows in the project.

```
cargo pmcp validate workflows [OPTIONS]
```

Runs `cargo check` to ensure compilation, then discovers and runs workflow validation tests (functions matching `test_workflow*`).

### Options

| Option | Description |
|--------|-------------|
| `--generate` | Generate validation test scaffolding if none exists |
| `--verbose`, `-v` | Show all test output |
| `--server <DIR>` | Server directory to validate (defaults to current directory) |

### Examples

**Validate workflows:**
```bash
cargo pmcp validate workflows
```

**Generate test scaffolding first:**
```bash
cargo pmcp validate workflows --generate
```

**Validate a specific server directory:**
```bash
cargo pmcp validate workflows --server crates/mcp-calculator-core
```

### Validation Steps

1. **Compilation check** - Runs `cargo check`
2. **Test discovery** - Finds tests matching `workflow*`, `test_workflow*`, etc.
3. **Test execution** - Runs discovered workflow tests and reports results

If no workflow tests exist, use `--generate` to create a test scaffolding file at `tests/workflow_validation.rs`.

## Related Commands

- [`cargo pmcp test`](test.md) - Run scenario-based MCP tests
- [`cargo pmcp schema`](schema.md) - Validate schema files
