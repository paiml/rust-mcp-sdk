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
