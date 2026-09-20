# Phase 76 — cargo-pmcp IAM declarations — Context

**Gathered:** 2026-04-22
**Status:** Ready for planning
**Source:** Operator-authored brief, derived 1:1 from the pmcp.run platform change
request. Every requirement in the source CR is a locked decision for this phase.

Source CR: `/Users/guy/Development/mcp/sdk/pmcp-run/docs/CLI_IAM_CHANGE_REQUEST.md`
(filed 2026-04 by pmcp.run platform team after cost-coach prod incident 2026-04-23).

## CR ambiguities — resolved 2026-04-22 (locked decisions)

Research (`76-RESEARCH.md`) surfaced two CR ambiguities. Operator locked both on 2026-04-22:

- **D-01 — Role handle:** use the Lambda's auto-created role via `mcpFunction.role!`. Emit `mcpFunction.addToRolePolicy(...)` calls (matches existing pattern at `init.rs:594, 607`). Export `mcpFunction.role!.roleArn` as the `McpRoleArn` CfnOutput. Do NOT extract a named `const mcpRole = new iam.Role(...)` — the CR's reference to "McpRole" is informal. The two existing inline policies stay as-is; new iam-declared policies append to the same list.

- **D-02 — DynamoDB action set:** follow the CR literally — 4 actions per bucket, not the 3-action TypeScript construct. `read` emits `{GetItem, Query, Scan, BatchGetItem}`, `write` emits `{PutItem, UpdateItem, DeleteItem, BatchWriteItem}`. The platform's TS `TablePermission` construct can be updated later to match; CR is authoritative for this phase.

- **D-03 — aws-lambda branch import:** the aws-lambda template branch (`init.rs:648-747`) currently lacks `import * as iam from 'aws-cdk-lib/aws-iam'`. Part 2 MUST add it. The pmcp-run branch (`init.rs:492-646`) already imports `iam`.

- **D-04 — Validator surface:** new `ValidateCommand::Deploy` variant (extend existing `commands/validate.rs`). Also invoke from `DeployExecutor::execute` after `DeployConfig::load` so `cargo pmcp deploy` rejects invalid IAM before doing any work.

- **D-05 — Backward compatibility invariant:** given a `.pmcp/deploy.toml` with no `[iam]` section, the emitted `stack.ts` must be byte-identical to the pre-phase output (modulo the additive `McpRoleArn` CfnOutput from Part 1). This is an acceptance criterion for the wave that adds Part 2.

## Why this phase exists

Servers deployed via `cargo pmcp deploy` cannot write to their own DynamoDB tables,
S3 buckets, or Secrets Manager secrets. The platform-provisioned Lambda role has no
IAM for author-owned resources and the `.pmcp/deploy.toml` schema has no way to
declare them. Every multi-tenant server currently ships a bolt-on CDK stack that
looks up the role by name — brittle because the CFN-generated suffix changes every
redeploy. Cost-coach is the first to hit this; it will not be the last.

## Scope — both parts land together (operator chose one coherent PR)

### Part 1 — Stable role ARN export (wave 1, cheap, independent)

- Add `CfnOutput` `McpRoleArn` with `Export.Name = pmcp-${ServerName}-McpRoleArn`
  to the generated CDK stack.
- Apply to BOTH template branches in
  `cargo-pmcp/src/commands/deploy/init.rs:485-747` (pmcp-run + aws-lambda).
- Purely additive — zero new config surface, no breaking change.
- Immediately unblocks existing bolt-on stacks: swap brittle
  `iam.Role.fromRoleName` → `iam.Role.fromRoleArn(Fn.importValue(...))`.

### Part 2 — Declarative `[iam]` section in `.pmcp/deploy.toml` (wave 2+)

New optional top-level section (empty default → backward compatible):

```toml
[[iam.tables]]
name = "cost-coach-tenants"
actions = ["readwrite"]       # "read" | "write" | "readwrite"
include_indexes = true        # default false

[[iam.buckets]]
name = "cost-coach-snapshots"
actions = ["readwrite"]       # object-level ARNs only

[[iam.statements]]
effect = "Allow"
actions = ["secretsmanager:GetSecretValue"]
resources = ["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*"]
```

**Translation rules** (CLI emits `addToRolePolicy` on `McpRole`):

| Input                 | Emitted DynamoDB actions                                   |
|-----------------------|------------------------------------------------------------|
| `tables read`         | `GetItem`, `Query`, `Scan`, `BatchGetItem`                 |
| `tables write`        | `PutItem`, `UpdateItem`, `DeleteItem`, `BatchWriteItem`    |
| `tables readwrite`    | union                                                      |
| `include_indexes=true`| add `arn:...:table/NAME/index/*`                           |

| Input                 | Emitted S3 actions                                         |
|-----------------------|------------------------------------------------------------|
| `buckets read`        | `GetObject`                                                |
| `buckets write`       | `PutObject`, `DeleteObject`                                |
| `buckets readwrite`   | union                                                      |

Resource ARN for buckets: `arn:aws:s3:::NAME/*` (object-level only; bucket-level
ops go through `[[iam.statements]]`).

`[[iam.statements]]` → emitted verbatim as `PolicyStatement` after validation.

**Validation** (reject at `cargo pmcp validate` and `deploy` time):
- Hard error: `effect=Allow` + `actions=["*"]` + `resources=["*"]`.
- Error: `effect` not in `Allow`/`Deny`.
- Error: `actions` or `resources` empty.
- Error: action does not match `^[a-z0-9-]+:[A-Za-z0-9*]+$`.
- Warning: unknown service prefix in action.
- Warning: cross-account ARN in resources (not an error — legitimate cases exist).

## Key files to touch

- `cargo-pmcp/src/deployment/config.rs`
  — add `IamConfig`, `TablePermission`, `BucketPermission`, `IamStatement` structs;
    wire into `DeployConfig` with `#[serde(default)]`.
- `cargo-pmcp/src/commands/deploy/init.rs`
  — `CfnOutput` (Part 1) + iam-statement emission in BOTH pmcp-run and aws-lambda
    template branches (Part 2).
- New validator module (or extend `cargo-pmcp/src/commands/validate.rs`)
  — footgun checks surfaced at validate + deploy entry points.
- New tests + example.

## Reference material for planners

- Platform's existing `TablePermission` construct at
  `built-in/shared/cdk-constructs/src/mcp-server-construct.ts:100-103, 252-280` —
  the CLI's translation should mirror this one-to-one (already in production for
  platform-owned servers).
- Cost-coach's current bolt-on workaround: commit `d376c23` — the operator
  experience this phase replaces.
- CR explicitly **rejected** env-var-name auto-inference and `${serverName}-*`
  prefix auto-grant. Do not re-propose these in planning.

## CLAUDE.md mandates (Toyota Way — every item below is required)

- Fuzz testing for `IamConfig` TOML parser.
- Property tests for translation rules (read/write/readwrite permutations,
  `include_indexes` on/off, arbitrary-but-valid inputs → well-formed CFN).
- Unit tests for each validation rule.
- Example demonstrating real-world usage (cost-coach-shaped `.pmcp/deploy.toml`).
- `cargo run --example` works.
- `make quality-gate` passes (CI parity — fmt --all, clippy pedantic+nursery,
  build, test, audit, doctests).
- Contract-first: update `../provable-contracts/contracts/cargo-pmcp/` if that
  crate has a contract.

## Security posture

No privilege escalation risk. `cargo pmcp deploy` runs with the operator's own
AWS credentials. The CLI is codifying permissions the operator already has — it
automates `addToRolePolicy` calls they would otherwise hand-write in CDK. Platform-
side audit UI surfacing of the declared `[iam]` section is a separate CR and not
blocking this phase.

## Rollout constraint

CR proposed Part 1 in week 1, Part 2 in weeks 2-3. Operator chose to land both in
one phase (single coherent review, one PR). Planner should still structure plans
so Part 1 is an independent first wave (shippable on its own if scope ever needs
to split back out) and Part 2 builds on it.

## Out of scope (explicitly)

- Platform-side admin UI showing declared IAM grants per server (separate CR).
- Auto-inference of IAM from env var names or server-name prefixes (CR rejected).
- Bucket-level S3 operations via the sugar block (use `[[iam.statements]]`).
- Cross-region / cross-account ARN sugar (use `[[iam.statements]]`).
