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
| `--regenerate-stack` (alias `--force`) | Overwrite a **hand-modified** `deploy/lib/stack.ts`. By default a hand-modified file is **preserved** (so an operator-curated stack is never silently clobbered); an unmodified scaffold is regenerated from the loaded config without this flag. See [Unmodified vs hand-modified `stack.ts`](#unmodified-vs-hand-modified-stackts). |

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
3. Builds the Lambda binary (once: the `npx cdk deploy` fallback deploys the binary this step built).
4. **Routes the deploy.** An unmodified `deploy/lib/stack.ts` scaffold is deployed by the native CloudFormation engine (no Node.js), as `{[server] name}-stack`, straight from `.pmcp/deploy.toml`. A hand-modified one goes through `npx cdk deploy` (steps 5-7). See [Unmodified vs hand-modified `stack.ts`](#unmodified-vs-hand-modified-stackts).
5. On the `npx cdk deploy` path, when both scaffold files are still cargo-pmcp's own (`stack.ts` unmodified or missing, `deploy/bin/app.ts` unmodified), **points `app.ts` at the current `[server] name`**. Then it **checks the stack identity** — see [Stack-identity guard](#stack-identity-guard). This runs before step 6 can overwrite a hand-modified `stack.ts`. A missing `stack.ts` is scaffolded first, because the CDK app cannot be listed without it, and is removed again if the guard refuses (as is an `app.ts` change).
6. **Regenerates `deploy/lib/stack.ts` from the loaded config** — splices the `[iam]` and `[metadata]` declarations into the CDK template at single seams. Changes to `.pmcp/deploy.toml` therefore take effect on the next `cargo pmcp deploy` without manual re-init. **Guard:** a **hand-modified** `stack.ts` is **preserved** (the write is skipped and a one-line `preserved existing deploy/lib/stack.ts` notice prints) so an operator-curated stack is never silently overwritten. Pass `--regenerate-stack` (alias `--force`) to overwrite it. An unmodified scaffold, or a missing file, is written flag-free.
7. Runs `cdk deploy {[server] name}-stack --require-approval never`, naming the stack explicitly.

The deploy prints the stack outputs once, at the end.

### Unmodified vs hand-modified `stack.ts`

Every time cargo-pmcp writes `deploy/lib/stack.ts` (at `deploy init`, and whenever a deploy regenerates it) it records the file's SHA-256 in `deploy/.pmcp-scaffold.toml`. **Hand-modified** means "differs from the `stack.ts` cargo-pmcp last wrote". Commit `deploy/.pmcp-scaffold.toml` with `deploy/`; do not edit it.

Before cargo-pmcp 0.28.0, "hand-modified" meant "differs from what the *current* `.pmcp/deploy.toml` would render". A `[server] name` rename, an `[iam]` or `[metadata]` change, or a newer cargo-pmcp template therefore made an untouched scaffold look hand-modified: the deploy fell back to `npx cdk deploy`, built the binary twice, and ignored `[server] memory_mb`/`timeout_seconds`. With the record, those changes keep the scaffold unmodified; the scaffold is regenerated to match the config.

**Projects initialized before 0.28.0** have no record. On their first deploy, the old comparison runs once, widened to the renders `deploy init` itself could have produced: the current config, or the init-time one (the name in the file's own `serverName` literal, with empty `[iam]`/`[metadata]`). A match is recorded and from then on the record decides. A file that matches nothing stays hand-modified, as before.

A `stack.ts` scaffolded with `deploy init --oauth cognito` is never recorded: `cargo pmcp deploy` regenerates only the plain scaffold, so recording the Cognito one would let a deploy replace it. It keeps deploying through `npx cdk deploy`, as before.

### Stack-identity guard

cargo-pmcp names, reports, reads outputs for, and destroys an `aws-lambda` deployment as `{[server] name}-stack`. The stack `npx cdk` acts on is whatever `deploy/bin/app.ts` declares, and `deploy init` writes the server name into `app.ts` as a literal (`const serverName = '...'`). An **unmodified** `app.ts` follows `[server] name` on its own (when `stack.ts` is unmodified too): the deploy rewrites the literal, and a rename deploys a **new** stack. A hand-edited `app.ts`, or one next to a hand-modified `stack.ts` (whose own `serverName` still names the old function), is left alone, and after a rename the two disagree. Before the guard, `cdk deploy` then silently updated the **old** stack in place.

Before `cdk deploy`, cargo-pmcp runs `npx cdk list` in `deploy/`, with the same environment the deploy uses, and refuses unless the app declares `{[server] name}-stack`. When `cdk list` also prints a CloudFormation name (`id (stackName)`), that name must match too. If `cdk list` itself fails, the operation is refused (the stack cannot be verified). A refusal happens before any CloudFormation change and before `--regenerate-stack` rewrites `stack.ts`, and it names both stacks:

```
refusing to run `cdk deploy`: deploy/bin/app.ts does not declare the stack cargo-pmcp targets.
  expected: acme-stack  (from `[server] name = "acme"` in .pmcp/deploy.toml)
  declared: old-server-stack
Nothing was deployed; CloudFormation is unchanged.
```

To recover, choose one:

- **Keep deploying the existing stack:** set `[server] name` back to the name the declared stack was created with (the error suggests it, e.g. `[server] name = "old-server"`).
- **Deploy under the new name:** set `const serverName = '<new name>';` in `deploy/bin/app.ts`, and the same `serverName` in `deploy/lib/stack.ts` if you customized it (an unmodified `stack.ts` is regenerated for you), then re-run. This creates a **new** stack; the old one keeps running until you delete it (`aws cloudformation delete-stack --stack-name <old>-stack --region <region>`).

The guard applies only to the `npx cdk deploy` path. The native CloudFormation engine (used for an unmodified scaffold) deploys `{[server] name}-stack` directly from `.pmcp/deploy.toml` and never reads `app.ts`. `destroy` deletes the stack through CloudFormation and does not use `npx cdk` (see [deploy destroy](#deploy-destroy)). The `pmcp-run` target runs `cdk synth`, never `cdk deploy`, and keys the deployment on `[server] name`, so it is not affected.

### Renaming a deployment

Set `[server] name` in `.pmcp/deploy.toml` (or run `cargo pmcp deploy init --name <new>`) and deploy. With unmodified scaffolds, the deploy creates a **second** stack, `<new>-stack`, with a function named `<new>`; the old stack keeps running until you destroy it (set the old name back and run `cargo pmcp deploy destroy`, or `aws cloudformation delete-stack --stack-name <old>-stack --region <region>`).

**A rename deployed by cargo-pmcp before 0.28.0 with `--regenerate-stack`** renamed the function *inside the old stack*: the old stack, `<old>-stack`, now owns a function named `<new>`. Creating `<new>-stack` would fail with "already exists" and leave it in `ROLLBACK_COMPLETE`, so the native engine checks first and refuses, naming the owning stack. Choose one:

- **Keep the existing deployment:** set `[server] name` back to the old name and deploy. The function gets its old name back.
- **Replace it:** delete the old stack (`aws cloudformation delete-stack --stack-name <old>-stack --region <region>`), wait for the deletion to finish, then deploy under the new name.

If a deploy already left `<new>-stack` in `ROLLBACK_COMPLETE`, delete that stack first (`cargo pmcp deploy destroy` with the new name does it).

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

> **Preserved-stack warning.** When `deploy/lib/stack.ts` is preserved (an operator-curated stack.ts already exists) and `.pmcp/deploy.toml` declares a non-empty `[iam]` and/or `[environment]` section, `cargo pmcp deploy` prints a prominent stderr warning. `[iam]` is spliced only when `stack.ts` is (re)generated (`--regenerate-stack`). For `[environment]`, the `pmcp-run` target now applies it construct-agnostically via the post-synth template merge (a preserved stack.ts no longer blocks it); the `aws-lambda` target still needs the curated `stack.ts` to read the matching `process.env.<KEY>`. On `aws-lambda` the warning also covers declared `[server] memory_mb`/`timeout_seconds` (see [Lambda sizing](#lambda-sizing-server-memory_mb--timeout_seconds)), because that target's `cdk deploy` path has no template to inject them into; `pmcp-run` does **not** warn about sizing, since its post-synth merge honors it there. This makes the previously-silent no-op loud at deploy time instead of surfacing as a runtime `500`.

### Lambda sizing (`[server] memory_mb` / `timeout_seconds`)

```toml
[server]
name = "okf-demo"
memory_mb = 1024        # Lambda MemorySize
timeout_seconds = 60    # Lambda Timeout
```

Both keys are **optional**. Omitting one means "leave whatever the deploy path's
own default is" — it does **not** mean "512"/"30". That distinction is
deliberate: with a parse-time default there is no way to tell an omitted key
from an explicit one, and materializing 512 over the `pmcp-run` engine's
built-in 256 would silently resize every function that never asked.

Which paths honor them:

| Deploy path | `memory_mb` | `timeout_seconds` |
|---|---|---|
| **`pmcp-run`** (both the `pmcp-cfn-renderer` and `npx cdk synth` engines) | ✅ applied | ✅ applied |
| **`aws-lambda`** via the native CloudFormation engine | ⚠️ warns — memory is pinned by the renderer | ✅ applied |
| **`aws-lambda`** via `npx cdk deploy` | ⚠️ warns — the `stack.ts` literal is authoritative | ⚠️ warns |
| **`google-cloud-run`** | n/a — uses `[server] memory` | n/a |

On `pmcp-run`, `cargo pmcp deploy` rewrites `Properties.MemorySize` /
`Properties.Timeout` **in the synthesized CloudFormation template**, after
engine routing and before upload — the same post-synth seam `[environment]`
uses. Because it runs after routing it works for a **hand-edited, preserved
`stack.ts`** too (those always route through `npx cdk synth`), which is the
case that matters: the sizing literal a human once patched into `stack.ts` no
longer has to be kept in sync by hand.

Unlike the `[environment]` merge, this one targets **only the MCP function** —
the `AWS::Lambda::Function` whose `FunctionName` equals `[server] name`. An
OAuth-enabled stack renders three Lambdas at three sizings
(`<name>-oauth-proxy` 256/30, `<name>` 512/30, `<name>-authorizer` 256/**10**);
resizing the 10-second authorizer would be a regression, not a fix.

The merge is **loud**. It prints what it changed, per property:

```
   ✅ Applied [server] sizing — McpFunction: MemorySize 256 -> 1024
   ✅ Applied [server] sizing — McpFunction: Timeout 30 -> 60
```

**Precedence — `deploy.toml` WINS over the `stack.ts` literal.** This is a
deliberate divergence from the `[environment]` rule two sections above, where
the literal wins. The two are not analogous: `[environment]` is a **map**, so
"the literal wins" is a coherent *additive fill* — `deploy.toml` contributes
keys the construct never set. `memorySize` is a **scalar the construct always
sets**, so "the literal wins" would degenerate to "the config is inert", which
is the defect this behavior exists to fix. If you want a different size on
`pmcp-run`, change `deploy.toml` — it is the source of truth.

**Fail-loud.** If sizing is declared but the synthesized template contains no
`AWS::Lambda::Function` matching `[server] name`, `cargo pmcp deploy` prints a
prominent stderr warning naming the expected function and the declared values
instead of dropping them.

**Divergence warnings on `aws-lambda`.** Neither `aws-lambda` engine has a seam
to inject sizing through, so `cargo pmcp deploy` says so rather than staying
silent — but only when the declared value actually differs from what will be
deployed, so a pristine scaffold (which declares exactly its own `stack.ts`
literals) stays quiet:

```
⚠️  [server] sizing in .pmcp/deploy.toml is NOT applied on the aws-lambda `npx cdk deploy` path:
   • memory_mb = 1024 declared, but 512 MB will be deployed
   Edit deploy/lib/stack.ts's memorySize/timeout literals, or deploy to the pmcp-run target, which honors the declared sizing.
```

> **History.** Before this behavior existed, `memory_mb` was parsed and read by
> **zero** production code paths on every target, while its own schema doc
> claimed "used by AWS targets" — so operators set it in good faith, got the
> hardcoded 256, and found out when a snapshot-baked server OOMed on its first
> request with nothing in the logs. `timeout_seconds` had a subtler form of the
> same problem: the CFN renderer honored it while the TypeScript scaffold did
> not, so the same `deploy.toml` produced a different Timeout depending on
> whether a human had ever touched `stack.ts`.

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
| `--name <NAME>` | package name, minus one trailing `-lambda` | Server name: `[server] name` in `.pmcp/deploy.toml`. It names the deployment (on `aws-lambda` the stack is `<NAME>-stack` and the function `<NAME>`). ASCII letters, digits and hyphens; starts with a letter; at most 64 characters. On an existing `deploy.toml` it renames the server and keeps every other setting. |

### Re-running `deploy init`

An existing `.pmcp/deploy.toml` is **kept**. Re-running `deploy init` never resets `[server] name`, `[environment]`, `[iam]`, `[secrets]`, `[metadata]`, sizing, or comments. It changes only what you ask for, each as a one-line, verified edit: `--name` renames the server, and an explicit `--target-type` that differs from the file's updates `[target] type`. On `aws-lambda` and `pmcp-run` it appends an `[aws]` section (from `--region`) when the file has none. The kept `[aws] region` wins over `--region`/`AWS_REGION`; init says so when they differ. A `deploy.toml` that does not parse stops init; fix it or move it aside.

On `aws-lambda`, OAuth flags (`--oauth cognito`, `--oauth-shared`) that would change a kept `[auth]` are refused: set `[auth]` in the file instead. Without OAuth flags, a kept Cognito `[auth]` drives the scaffold, so re-running `deploy init` on an OAuth project scaffolds the OAuth stack again.

The default name for a **new** `deploy.toml` is the Cargo package name with one trailing `-lambda` removed (`forecast-coach-lambda` deploys as `forecast-coach`); `-lambda` is a packaging convention, not a service name. Before 0.28.0 the package name was used as is, and `aws-lambda`/`azure-container-apps` init rewrote the whole file from defaults.

### Example

```bash
# AWS Lambda with OAuth
cargo pmcp deploy init --target aws-lambda --oauth cognito

# Without OAuth (add later)
cargo pmcp deploy init --target aws-lambda

# Google Cloud Run
cargo pmcp deploy init --target google-cloud-run

# Rename the server of an existing deployment config (everything else is kept)
cargo pmcp deploy init --target aws-lambda --name acme-forecast
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

On `aws-lambda`, `destroy` deletes the CloudFormation stack `{[server] name}-stack` directly (`DeleteStack`, in `[aws] region`) and waits until the deletion finishes. It removes the stack whichever engine created it, native CloudFormation or `npx cdk deploy`, and needs no Node.js. A `DELETE_FAILED` stack fails the command with the stack's recent failure events.

If no stack of that name exists, `destroy` says so ("nothing to delete") — unless `deploy/outputs.json` records another stack that still exists. Then the deployment lives under another name, and `destroy` refuses instead of reporting success (so `--clean` cannot delete the local files of a running deployment), naming both stacks and the two ways to remove it: set `[server] name` to the recorded one and re-run, or `aws cloudformation delete-stack --stack-name <stack> --region <region>`.

Before 0.28.0, `destroy` ran `npx cdk destroy` in the shell's region, which reached only a stack the CDK app in `deploy/bin/app.ts` declared. The deploy-artifact bucket the native engine uses (`pmcp-deploy-<account>-<region>`) is shared by every server in the account and region and is not deleted.

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

Show deployment outputs. On `aws-lambda` they are read from `deploy/outputs.json` for `{[server] name}-stack` by name; if the file records only other stacks (for example after a rename, before the first deploy under the new name), the command says which ones instead of showing another stack's outputs under this name.

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
