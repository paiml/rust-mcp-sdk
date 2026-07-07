# cargo pmcp deploy

Deploy MCP server to cloud platforms.

## Usage

```
cargo pmcp deploy [OPTIONS] [SUBCOMMAND]
```

When invoked without a subcommand, builds and deploys to the configured target.

## Description

Deploy to AWS Lambda, Google Cloud Run, Cloudflare Workers, or pmcp.run. Includes full lifecycle management: init, deploy, logs, metrics, test, rollback, and destroy.

## Deploy Options

| Option | Description |
|--------|-------------|
| `--target <TARGET>` | Deployment target: `aws-lambda`, `cloudflare-workers`, `google-cloud-run`, `pmcp-run` |
| `--shared-pool <POOL>` | Use shared OAuth pool for SSO (pmcp-run only) |
| `--no-oauth` | Skip OAuth configuration during deployment |
| `--regenerate-stack` (alias `--force`) | Overwrite an existing `deploy/lib/stack.ts`. By default the file is **preserved** if it already exists (so an operator-curated stack is never silently clobbered); pass this flag to regenerate it from the loaded config. |

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `init` | Initialize deployment configuration |
| `logs` | View deployment logs |
| `metrics` | View deployment metrics |
| `test` | Test the deployment |
| `rollback` | Rollback to previous version |
| `destroy` | Remove the deployment |
| `secrets` | Manage deployment secrets |
| `outputs` | Show deployment outputs |
| `login` | Login to deployment target |
| `logout` | Logout from deployment target |
| `oauth` | Manage OAuth configuration |
| `status` | Check async operation status |

---

## deploy (no subcommand)

Build and deploy to the configured target.

```
cargo pmcp deploy --target aws-lambda
cargo pmcp deploy --target pmcp-run --shared-pool agent-framework
```

### Flow

For AWS Lambda targets, `cargo pmcp deploy` runs:

1. Loads `.pmcp/deploy.toml` via `DeployConfig::load`.
2. **Validates the `[iam]` section** — runs the same gate as [`cargo pmcp validate deploy`](validate.md#validate-deploy) and fails fast before any AWS API call if validation errors are present. Warnings print to stderr but don't block.
3. Builds the Lambda binary.
4. **Regenerates `deploy/lib/stack.ts` from the loaded config** — splices the `[iam]` and `[metadata]` declarations into the CDK template at single seams. Changes to `.pmcp/deploy.toml` therefore take effect on the next `cargo pmcp deploy` without manual re-init. **Guard:** if `deploy/lib/stack.ts` already exists, it is **preserved** (the write is skipped and a one-line `preserved existing deploy/lib/stack.ts` notice prints) so an operator-curated stack is never silently overwritten. Pass `--regenerate-stack` (alias `--force`) to overwrite it. A missing file is always scaffolded flag-free.
5. Runs `cdk deploy` with `--require-approval never`.

> Both generated stacks (`pmcp-run` and `aws-lambda`) emit a stable `McpRoleArn` CFN output with `exportName: pmcp-${serverName}-McpRoleArn` — consume it from external stacks via `Fn::ImportValue` instead of looking up the role by its CFN-generated name.

### Declaring IAM

To give the deployed Lambda AWS permissions (DynamoDB, S3, SecretsManager, …), add an `[iam]` section to `.pmcp/deploy.toml`. See:

- [IAM.md](../IAM.md) — how-to guide with recipes, troubleshooting, and migration from hand-written bolt-on stacks
- [DEPLOYMENT.md § IAM Declarations](../../DEPLOYMENT.md#iam-declarations-iam-section) — schema reference and full translation tables
- [`cargo pmcp validate deploy`](validate.md#validate-deploy) — pre-flight the config before deploying

### Config-driven stack metadata (`[metadata]`)

The generated `deploy/lib/stack.ts` advertises two MCP metadata literals into the synthesized CDK stack: `mcp:serverType` and `mcp:snapshotBaked`. By default `mcp:serverType` is `'custom'` for pmcp.toml/custom servers and `mcp:snapshotBaked` is omitted. To make these **reproducible-from-config** — so regenerating the stack (`--regenerate-stack`) reproduces your curated values instead of the defaults — add a `[metadata]` block to `.pmcp/deploy.toml`:

```toml
[metadata]
server_type = "graph-rag"   # overrides the mcp:serverType literal (default 'custom')
snapshot_baked = true        # emits the additive mcp:snapshotBaked:'true' literal
```

| Key | Type | Effect |
|-----|------|--------|
| `server_type` | string | Overrides the `mcp:serverType` template literal. |
| `snapshot_baked` | bool | When `true`, emits the additive `mcp:snapshotBaked` literal; when absent/`false`, the literal is omitted. |

Both keys are optional. **Absent the `[metadata]` block, behavior is unchanged** and the generated stack is byte-identical to prior releases (non-opting servers emit no `mcp:snapshotBaked` and keep the default `mcp:serverType`). Because these are reproducible-from-config, regenerating `stack.ts` with `--regenerate-stack` is safe — your curated metadata is reproduced rather than lost.

> See `cargo run -p cargo-pmcp --example deploy_stack_metadata` for a runnable walkthrough of the guard + `[metadata]` workflow.

### Environment variables (`[environment]`)

Non-sensitive runtime configuration goes in the `[environment]` block of `.pmcp/deploy.toml`:

```toml
[environment]
RUST_LOG = "info"
GRAPHRAG_ENDPOINT = "https://graphrag.internal"
```

How `[environment]` reaches the deployed Lambda depends on the target:

- **`pmcp-run`** — after `cdk synth`, `cargo pmcp deploy` merges every `[environment]` key **directly into each `AWS::Lambda::Function`'s `Environment.Variables`** in the synthesized CloudFormation template before upload. This is **construct-agnostic**: it lands the keys regardless of how the `stack.ts` was authored, including shared/managed constructs (e.g. `OpenApiMcpServerStack`) that hardcode `environment: {}` and read no `process.env`. Secrets are **excluded** from this merge (they keep their server-side injection path) and never enter the template.
- **`aws-lambda`** — deploys via `cdk deploy` (no pre-upload template file), so `[environment]` is passed as env vars onto the CDK child process (the **same transient path** resolved `[secrets]` use) and reaches the Lambda only when `stack.ts` reads it via `process.env.<KEY>` inside its `environment: {}` block.

Either way, `[environment]` values are **never written back to disk** and are immune to the `stack.ts` preserve guard.

**Precedence:**

- **`pmcp-run` (template merge):** a declared `[environment]` entry **OVERRIDES** the construct's hardcoded value on key collision — e.g. `RUST_LOG = "warn"` beats a construct default of `info`. This is a locked product decision so `deploy.toml` is the single source of truth for runtime configuration.
- **`aws-lambda` (process.env pass-through):** a hardcoded literal in the `stack.ts` `environment: {}` block wins over a same-key `[environment]` entry unless the stack reads `process.env.<KEY>` — the mechanism is *additive-fill* on that target.
- **Secrets always win:** if a key appears in **both** `[environment]` and `[secrets]`, the resolved **secret wins** (it is excluded from the `pmcp-run` merge and injected as the authoritative sensitive value).

**Fail-loud (`pmcp-run`).** If `[environment]` is non-empty but the synthesized template contains **no** `AWS::Lambda::Function` resource to inject into, `cargo pmcp deploy` prints a prominent stderr warning naming the affected keys instead of silently dropping them.

> **Preserved-stack warning.** When `deploy/lib/stack.ts` is preserved (an operator-curated stack.ts already exists) and `.pmcp/deploy.toml` declares a non-empty `[iam]` and/or `[environment]` section, `cargo pmcp deploy` prints a prominent stderr warning. `[iam]` is spliced only when `stack.ts` is (re)generated (`--regenerate-stack`). For `[environment]`, the `pmcp-run` target now applies it construct-agnostically via the post-synth template merge (a preserved stack.ts no longer blocks it); the `aws-lambda` target still needs the curated `stack.ts` to read the matching `process.env.<KEY>`. This makes the previously-silent no-op loud at deploy time instead of surfacing as a runtime `500`.

---

## deploy init

Initialize deployment configuration.

```
cargo pmcp deploy init [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--region <REGION>` | `us-east-1` / `AWS_REGION` env | AWS region for deployment |
| `--skip-credentials-check` | - | Skip credentials check |
| `--oauth <PROVIDER>` | - | OAuth provider: `cognito`, `oidc`, `none` |
| `--oauth-shared <NAME>` | - | Use shared OAuth infrastructure |
| `--cognito-user-pool-id <ID>` | - | Existing Cognito User Pool ID |
| `--cognito-pool-name <NAME>` | - | Cognito User Pool name when creating new |
| `--social-providers <LIST>` | - | Social logins, comma-separated: `github,google,apple` |

### Example

```bash
# AWS Lambda with OAuth
cargo pmcp deploy init --target aws-lambda --oauth cognito

# Without OAuth (add later)
cargo pmcp deploy init --target aws-lambda

# Google Cloud Run
cargo pmcp deploy init --target google-cloud-run
```

---

## deploy logs

View deployment logs.

```
cargo pmcp deploy logs [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--tail` | - | Follow logs in real-time |
| `--lines <N>` | `100` | Number of lines to show |

---

## deploy metrics

View deployment metrics.

```
cargo pmcp deploy metrics [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--period <PERIOD>` | `24h` | Time period: `1h`, `24h`, `7d`, `30d` |

---

## deploy test

Test the deployment.

```
cargo pmcp deploy test [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--verbose` | Verbose output |

---

## deploy rollback

Rollback to a previous version.

```
cargo pmcp deploy rollback [VERSION] [OPTIONS]
```

| Argument | Description |
|----------|-------------|
| `VERSION` | Version to rollback to (default: previous) |

| Option | Description |
|--------|-------------|
| `--yes` | Skip confirmation |

---

## deploy destroy

Remove the deployment.

```
cargo pmcp deploy destroy [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--yes` | Skip confirmation prompt |
| `--clean` | Remove all deployment files (CDK project, Lambda wrapper, config) |
| `--no-wait` | Don't wait for async operations (pmcp-run only) |

---

## deploy secrets

Manage secrets within the deployment context. For full multi-provider secret management (local, pmcp.run, AWS), see [`cargo pmcp secret`](secret.md).

```
cargo pmcp deploy secrets <ACTION>
```

| Action | Usage | Description |
|--------|-------|-------------|
| `set` | `secrets set <KEY> --from-env <VAR>` | Set a secret from environment variable |
| `list` | `secrets list` | List all secrets |
| `delete` | `secrets delete <KEY> [--yes]` | Delete a secret |

---

## deploy outputs

Show deployment outputs.

```
cargo pmcp deploy outputs [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--format <FMT>` | `text` | Output format: `text` or `json` |

---

## deploy login / logout

```
cargo pmcp deploy login
cargo pmcp deploy logout
```

Authenticate or deauthenticate with the deployment target.

---

## deploy oauth

Manage OAuth configuration for pmcp.run servers.

```
cargo pmcp deploy oauth <ACTION>
```

### oauth enable

```
cargo pmcp deploy oauth enable --server <ID> [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--server <ID>` | *(required)* | Server ID to enable OAuth for |
| `--copy-from <SERVER>` | - | Copy OAuth config from an existing server (easiest SSO setup) |
| `--scopes <LIST>` | `openid,email,mcp/read` | OAuth scopes, comma-separated |
| `--dcr` | `true` | Enable Dynamic Client Registration (RFC 7591) |
| `--public-clients <LIST>` | `claude,cursor,desktop,mcp-inspector,chatgpt` | Public client patterns |
| `--shared-pool <ID>` | - | Use an existing Cognito User Pool for SSO |

### oauth disable

```
cargo pmcp deploy oauth disable --server <ID>
```

### oauth status

```
cargo pmcp deploy oauth status --server <ID>
```

---

## deploy status

Check status of an async operation.

```
cargo pmcp deploy status <OPERATION_ID>
```

## End-to-End Example

```bash
# 1. Initialize for AWS Lambda with OAuth
cargo pmcp deploy init --target aws-lambda --oauth cognito --region us-east-1

# 2. Deploy
cargo pmcp deploy --target aws-lambda

# 3. Test the deployment
cargo pmcp deploy test --verbose

# 4. View logs
cargo pmcp deploy logs --tail

# 5. View metrics
cargo pmcp deploy metrics --period 24h

# 6. Rollback if needed
cargo pmcp deploy rollback --yes

# 7. Clean up
cargo pmcp deploy destroy --clean --yes
```

## Related Commands

- [`cargo pmcp secret`](secret.md) - Manage secrets outside of deploy context
- [`cargo pmcp loadtest`](loadtest.md) - Load test after deployment
- [`cargo pmcp landing`](landing.md) - Create a landing page for the deployed server
