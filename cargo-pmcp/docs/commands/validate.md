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
