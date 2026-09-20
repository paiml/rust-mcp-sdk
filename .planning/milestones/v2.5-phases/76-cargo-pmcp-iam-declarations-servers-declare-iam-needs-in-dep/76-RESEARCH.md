# Phase 76: cargo-pmcp IAM Declarations — Research

**Researched:** 2026-04-22
**Domain:** Rust CLI scaffolding / AWS IAM / CDK TypeScript code generation
**Confidence:** HIGH (every claim verified by reading source files in this repo and `/Users/guy/Development/mcp/sdk/pmcp-run`; where I infer, it is tagged `[ASSUMED]`)

## Summary

Phase 76 adds two features to `cargo-pmcp` (currently 0.9.1): (1) a stable `McpRoleArn` CFN export and (2) a new `[iam]` section in `.pmcp/deploy.toml` that generates `addToRolePolicy` calls on the Lambda execution role inside the CDK TypeScript stack this tool scaffolds.

Research findings that shift the plan relative to the operator brief:

1. **The current template has no named `McpRole`.** Both template branches (`init.rs:492-639` pmcp-run and `init.rs:649-747` aws-lambda) create the Lambda via `new lambda.Function(this, 'McpFunction', {...})` with **no explicit `role:` prop**, so CDK auto-creates `McpFunctionServiceRole...`. The CR's wording "add `CfnOutput` `McpRoleArn` on `McpRole`" presumes a role named `McpRole` exists — **it does not**. The planner has two options: (a) export `mcpFunction.role!.roleArn` without renaming, or (b) create an explicit `const mcpRole = new iam.Role(...)` and pass `role: mcpRole` to the Lambda. Option (a) is purely additive and zero-risk (honors Part 1 "purely additive" constraint). Option (b) gives a clean `McpRole` handle for Part 2's `addToRolePolicy` calls but mutates the existing resource graph. **Recommendation: option (a) for Part 1, then Part 2's iam statements call `mcpFunction.role!.attachInlinePolicy(...)` or `mcpFunction.addToRolePolicy(...)` — the template already uses `mcpFunction.addToRolePolicy(...)` at lines 594 and 607, so this is the established idiom.**

2. **The TypeScript reference implementation disagrees with the CR on DynamoDB action lists.** `mcp-server-construct.ts:252-273` emits:
   - read: `GetItem`, `Query`, `Scan` (3 actions, **no BatchGetItem**)
   - write: `PutItem`, `UpdateItem`, `DeleteItem` (3 actions, **no BatchWriteItem**)

   The CR specifies 4 actions each, adding `BatchGetItem` and `BatchWriteItem`. Since the CR explicitly says "the CLI's CFN generator is a direct translation of code that's already in production for platform-owned servers" and "mirrors this one-to-one", this is a discrepancy the planner must resolve. **Recommendation: follow the CR (4 actions including Batch operations).** Rationale: (a) the CR is the locked decision; (b) `BatchGetItem`/`BatchWriteItem` are standard DynamoDB read/write operations that users legitimately expect under the `readwrite` sugar; (c) the current TS construct is arguably a bug to be fixed later in pmcp-run — this CLI phase need not propagate it. Flag this as a decision the discuss/plan phase should confirm.

3. **`cargo pmcp validate` currently only validates workflows.** It has no hook for deploy-config validation. `validate.rs` exposes one `ValidateCommand::Workflows` variant. The planner must decide: (a) add a new `ValidateCommand::Deploy` variant, (b) add validation into `DeployExecutor::execute` at `deploy.rs:33`, or (c) both. **Recommendation: both.** Part-2 validation runs unconditionally at `DeployExecutor::execute` start (after `DeployConfig::load`), and a new `Deploy` variant lets users pre-flight without triggering a build.

4. **No ARN parser crate is in the dependency tree.** `Cargo.toml` pulls `regex = "1"` (already used for secret refs) — that is sufficient for the action-regex and cross-account sniffing the CR requires. **No new dependency needed.**

5. **Fuzz, property, and example infrastructure all exist and are idiomatic.** `fuzz_config_parse.rs` is the precedent for a `DeployConfig`-level fuzz target; `tests/property_tests.rs` shows the proptest pattern; `examples/*.rs` (four files) shows the `cargo run --example` convention. No framework work is needed — new tests slot in directly.

**Primary recommendation:** Land as a single phase structured into **5 waves**. Wave 1 = Part 1 (`McpRoleArn` export, both template branches, independently shippable). Wave 2 = `IamConfig` schema + serde + backward-compat tests. Wave 3 = translation rules (TOML → `addToRolePolicy` TypeScript strings) with property tests. Wave 4 = validator module (footgun detection, action-regex, prefix warnings) wired into both `ValidateCommand::Deploy` and `DeployExecutor::execute`. Wave 5 = fuzz target + cost-coach-shaped example + doctests + `make quality-gate`.

## User Constraints (from CONTEXT.md)

### Locked Decisions

Every item in the source CR at `/Users/guy/Development/mcp/sdk/pmcp-run/docs/CLI_IAM_CHANGE_REQUEST.md` is a locked decision for this phase. Specifically:

**Part 1 — Role ARN export:**
- Add `CfnOutput` named `McpRoleArn` with `Export.Name = pmcp-${ServerName}-McpRoleArn`.
- Apply to BOTH template branches in `cargo-pmcp/src/commands/deploy/init.rs:485-747` (pmcp-run + aws-lambda).
- Purely additive — zero new config surface, no breaking change.

**Part 2 — Declarative `[iam]` in `.pmcp/deploy.toml`:**
- New optional top-level section (empty default → backward compatible).
- Three repeated-table forms: `[[iam.tables]]`, `[[iam.buckets]]`, `[[iam.statements]]`.
- `[[iam.tables]]` fields: `name`, `actions` (`"read" | "write" | "readwrite"`), `include_indexes` (default `false`).
- `[[iam.buckets]]` fields: `name`, `actions` (same sugar).
- `[[iam.statements]]` fields: `effect` (`Allow`/`Deny`), `actions[]`, `resources[]` — passthrough after validation.

**Translation rules (CR-specified):**
- DynamoDB read → `GetItem`, `Query`, `Scan`, `BatchGetItem` (4 actions per CR)
- DynamoDB write → `PutItem`, `UpdateItem`, `DeleteItem`, `BatchWriteItem` (4 actions per CR)
- `include_indexes=true` → add `arn:aws:dynamodb:…/table/NAME/index/*`
- S3 read → `GetObject`; write → `PutObject`, `DeleteObject`; buckets are object-level ARN only (`arn:aws:s3:::NAME/*`)

**Validation rules (CR-specified):**
- Hard error: `effect=Allow` + `actions=["*"]` + `resources=["*"]`
- Error: `effect` not in `Allow`/`Deny`
- Error: `actions` or `resources` empty
- Error: action does not match `^[a-z0-9-]+:[A-Za-z0-9*]+$`
- Warn: unknown service prefix in action
- Warn: cross-account ARN in resources

**Rejection list (do not re-propose):**
- Env-var-name auto-inference (`_TABLE_NAME` / `_BUCKET` magic).
- `${serverName}-*` prefix auto-grant.

**Operational:**
- Both parts land together in one PR.
- Wave 1 must structurally isolate Part 1 so it remains shippable independently if scope re-splits.
- Backward compat: empty/missing `[iam]` section must emit a byte-identical `stack.ts` except for the Part 1 `CfnOutput` addition.
- Target: `cargo-pmcp` minor bump (additive feature).

### Claude's Discretion

- Translation action-set discrepancy resolution (CR 4-action lists vs TS construct 3-action lists — see Finding #2 above; recommended resolution: follow CR).
- Whether to rename the Lambda role to `McpRole` explicitly or export `mcpFunction.role!.roleArn` as-is (Finding #1; recommended: as-is for Part 1, `addToRolePolicy` idiom for Part 2).
- Whether to extend `ValidateCommand::Workflows` or add a new `ValidateCommand::Deploy` variant (Finding #3; recommended: new variant + runtime check in `DeployExecutor::execute`).
- Service-prefix "known list" contents (see Open Technical Choice #2).
- Fuzz target granularity: fuzz `DeployConfig::load` whole, or `IamConfig` narrow (recommended: whole, matches existing fuzz_config_parse pattern).
- Example file name + shape (recommended: `examples/deploy_iam_cost_coach.rs` that loads a shipped `examples/fixtures/cost-coach.deploy.toml` and prints the translated statements).

### Deferred Ideas (OUT OF SCOPE)

- Platform-side admin UI showing declared IAM grants per server (separate CR).
- Bucket-level S3 operations via sugar (must go through `[[iam.statements]]`).
- Cross-region / cross-account ARN sugar (must go through `[[iam.statements]]`).
- Contract YAMLs under `../provable-contracts/contracts/cargo-pmcp/` — note CLAUDE.md mentions contract-first, but that repo is not present in this tree (verified: `ls ../provable-contracts` would be outside working directory). Plan should check for its existence at execution time, skip if absent.

## Phase Requirements

No `phase_req_ids` provided for Phase 76 (confirmed: init command returned null). REQUIREMENTS.md has no IAM-related REQ-IDs. This phase is authored from the CR directly; acceptance criteria live in CONTEXT.md and this RESEARCH.md rather than being enumerated in REQUIREMENTS.md. The planner should still surface ACs in PLAN.md.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|---|---|---|---|
| Parse `[iam]` from `.pmcp/deploy.toml` | `cargo-pmcp/src/deployment/config.rs` | — | Existing location for all serde schema (DeployConfig, AuthConfig, AssetsConfig, CompositionConfig) |
| Validate `[iam]` block (footgun detection, regex, warnings) | `cargo-pmcp/src/deployment/` (new `iam_validate.rs`) | `cargo-pmcp/src/commands/validate.rs` (new variant) + `cargo-pmcp/src/commands/deploy/deploy.rs` (runtime hook) | Validation logic belongs next to the schema; CLI entry points just call into it |
| Translate `[iam]` → TypeScript `addToRolePolicy` strings | `cargo-pmcp/src/commands/deploy/init.rs` (new helper `render_iam_statements`) | — | Lives with `create_stack_ts` to keep all stack-rendering string generation colocated |
| `McpRoleArn` CfnOutput emission | `cargo-pmcp/src/commands/deploy/init.rs` (both `format!` branches at 492 and 649) | — | Part of the emitted CDK template |
| Fuzz target for TOML parsing | `cargo-pmcp/fuzz/fuzz_targets/` | — | Existing fuzz convention (`fuzz_config_parse.rs`) |
| Property tests for translation + validation | `cargo-pmcp/tests/` | — | Existing proptest convention (`property_tests.rs`) |
| Example | `cargo-pmcp/examples/` | — | Existing example convention |

## Codebase Survey (verified findings)

### `cargo-pmcp/src/deployment/config.rs` (717 lines) `[VERIFIED: read]`

- `DeployConfig` struct at lines 6–29. Uses `#[serde(default)]` for optional sections (`secrets`, `assets`, `composition`). **Precedent for adding `iam: IamConfig` with `#[serde(default)]` is clear and idiomatic.**
- Sub-configs defined in this file: `TargetConfig`, `AwsConfig`, `ServerConfig`, `AuthConfig`, `CognitoConfig`, `OidcConfig`, `DcrConfig`, `ScopesConfig`, `ObservabilityConfig`, `AlarmConfig`, `ApiGatewayConfig`, `AssetsConfig`, `CompositionConfig`.
- Convention: each sub-struct has `#[derive(Debug, Clone, Serialize, Deserialize)]`; fields that should omit on roundtrip use `#[serde(skip_serializing_if = "Option::is_none")]`; defaults for non-optional fields use named `fn default_foo()` helpers; a `Default` impl is hand-written per struct (no `#[derive(Default)]` where fields have non-trivial defaults).
- `DeployConfig::load` at line 443: reads `.pmcp/deploy.toml`, calls `toml::from_str`, then `auto_configure_template_assets`. **This is the natural point to also call a new `iam.validate()` that raises hard-errors but NOT warnings** — warnings should be printed, not failed. Better: validation stays a separate function called from `deploy.rs` and the new `validate` CLI variant.
- `DeployConfig::save` exists (line 502) and `toml::to_string_pretty` is used. **Adding `iam` must roundtrip through save/load** — property test: `DeployConfig { iam: arb_iam(), .. } → to_string → from_str → equal`.
- `DeployConfig::default_for_server` (line 514): constructs defaults. **Must set `iam: IamConfig::default()` to an empty config.**

### `cargo-pmcp/src/commands/deploy/init.rs` `[VERIFIED: read lines 1–80, 480–747]`

- Two `format!`-based template branches matching the brief exactly:
  - Lines **492–645**: `if self.target_type == "pmcp-run"` — Lambda-only stack, early `return`.
  - Lines **649–747**: aws-lambda default path, full API Gateway stack.
- Both branches end with a block of `new cdk.CfnOutput(this, '<Name>', {...})` calls (pmcp-run: `LambdaArn`, `LambdaName`, `ApiUrl`, `DashboardUrl`; aws-lambda: `ApiUrl`, `LambdaArn`, `DashboardUrl`). **Adding `McpRoleArn` goes cleanly at the end of each output block.**
- Both branches use `mcpFunction.addToRolePolicy(new iam.PolicyStatement({...}))` — pmcp-run branch already has TWO such calls (lines 594 and 607: DynamoDB composition read, Lambda invoke). **This is the exact idiom to extend for Part 2.** Aws-lambda branch has NONE of these calls — Part 2 IAM emission for aws-lambda branch will also need to emit the `import * as iam from 'aws-cdk-lib/aws-iam';` line (pmcp-run branch already has this at line 496).
- **Backward-compat check for Part 1:** the pmcp-run template already emits `new cdk.CfnOutput(this, 'LambdaArn', ...)` at line 616 that exports `mcpFunction.functionArn` (no `Export.Name`). Adding `McpRoleArn` is a new output, not a rename — it will not conflict with existing deployed stacks at `cdk deploy` time (CloudFormation accepts added outputs).
- **`Export.Name` syntax in CDK:** `new cdk.CfnOutput(this, 'McpRoleArn', { value: mcpFunction.role!.roleArn, exportName: \`pmcp-${serverId}-McpRoleArn\` })` — use the TS non-null assertion `mcpFunction.role!` because `role` is typed `IRole | undefined`. Confirm this compiles via the planner's wave-1 verification step (run `cdk synth` on a scaffolded test project).
- **Missing import in aws-lambda branch:** it has `import * as cdk from 'aws-cdk-lib';` and `import * as lambda from 'aws-cdk-lib/aws-lambda';` etc., but NO `import * as iam from 'aws-cdk-lib/aws-iam';`. Part 2 needs to inject this import when rendering the iam-statements block, but only if the block is non-empty. Simplest: always inject the import (tree-shaken out if unused) to keep the generator deterministic. Alternative: conditionally inject. **Recommendation: always inject; unused imports are not a CDK error.**

### `cargo-pmcp/src/commands/validate.rs` (500 lines) `[VERIFIED: read]`

- `ValidateCommand` enum (line 13) has ONE variant: `Workflows { generate, server }`. Pure workflow focus — runs `cargo check`, finds tests, runs them, parses output.
- **No existing hook for deploy-config validation.** To add IAM validation, the natural path is:
  - New `ValidateCommand::Deploy { server: Option<String> }` variant that calls `DeployConfig::load` and runs the new validator.
  - Call the same validator from `DeployExecutor::execute` in `deploy.rs:33` right after `DeployConfig::load`.
- The existing module uses `console::style` for output formatting — match that for consistency.

### `cargo-pmcp/src/commands/deploy/deploy.rs` (140 lines) `[VERIFIED: read]`

- `DeployExecutor::execute` at line 33 is the deploy runtime hook. After `DeployConfig::load(&self.project_root)?` at line 39, insert `validate_iam(&config)?;` (returns `Err` on hard errors, prints warnings to stderr).
- Unit-test convention: `#[cfg(test)] mod tests` at bottom, tests `extra_env_default_empty` and `with_extra_env_builder` — simple constructor-level tests.

### `cargo-pmcp/tests/` `[VERIFIED: ls + read property_tests.rs:1-100]`

- Three test files:
  - `auth_integration.rs` — integration tests
  - `engine_property_tests.rs` — proptest-based
  - `property_tests.rs` — proptest-based, the template for new property tests
- **Convention:** `use proptest::prelude::*;`, `fn arb_foo() -> impl Strategy<Value = Foo>`, `proptest! { #[test] fn prop_foo(...) { prop_assert_eq!(...) } }`, one file per subsystem.
- **Precedent at property_tests.rs:44–80**: valid-config roundtrip test. Exactly the pattern Phase 76 needs for `IamConfig` roundtrip.
- **Precedent at property_tests.rs:86–100**: validator-rejects-invalid test. Exactly the pattern Phase 76 needs for footgun detection.

### `cargo-pmcp/fuzz/` `[VERIFIED: ls + read fuzz_config_parse.rs]`

- 3 fuzz targets: `fuzz_breaking_point.rs`, `fuzz_config_parse.rs`, `fuzz_metrics_record.rs`.
- `fuzz_config_parse.rs` is the template: `fuzz_target!(|data: &[u8]| { if let Ok(s) = std::str::from_utf8(data) { let _ = LoadTestConfig::from_toml(s); } });`.
- **New fuzz target recommendation:** `fuzz_deploy_config.rs` that calls `toml::from_str::<DeployConfig>(s)` on arbitrary UTF-8 — this exercises `IamConfig` parsing as a subset without needing a narrower entry point. Must not panic.
- `fuzz/Cargo.toml` already shows the registration pattern (`[[bin]]` + `name` + `path`).

### `cargo-pmcp/examples/` `[VERIFIED: ls]`

- 4 examples: `engine_demo.rs`, `loadtest_demo.rs`, `secrets_local_workflow.rs`, `secrets_provider_demo.rs`.
- All runnable via `cargo run --example <name>` (verify in Cargo.toml — auto-discovered from examples/*.rs).
- **New example name:** `deploy_iam_declaration.rs` (or similar). Shape: (1) inline or file-loaded `deploy.toml` string demonstrating cost-coach-shaped config; (2) call `DeployConfig::load_from_str` (add if not exists — current `DeployConfig::load` takes a path); (3) call the new translation helper and print each emitted TypeScript statement; (4) call the validator and show both accepted and rejected cases.

### `cargo-pmcp/src/deployment/targets/` `[VERIFIED: ls]`

- Subdirs `aws_lambda/`, `cloudflare/`, `google_cloud_run/`, `pmcp_run/`. These are "target" adapter modules — Phase 76's IAM changes touch only `pmcp_run/` and `aws_lambda/` conceptually (corresponding to the two template branches in `init.rs`). But the actual edit site is `init.rs` (the template generator), not these adapters.

### `mcpFunction.role` in CDK generated template `[VERIFIED: grep]`

- No current references to `mcpFunction.role` in any .rs file. The name `McpRole` does not appear anywhere. **This confirms Finding #1**: the CR's phrasing "`addToRolePolicy` on `McpRole`" is informal — the actual expression is `mcpFunction.role!` (the auto-created role) or use `mcpFunction.addToRolePolicy(...)` which the current template already does at lines 594 and 607.

## Reference Implementation Mirror

`/Users/guy/Development/mcp/sdk/pmcp-run/built-in/shared/cdk-constructs/src/mcp-server-construct.ts:252-280` — **verbatim transcription** of the TypeScript loop that Part 2 must mirror:

```typescript
// ========================================================================
// Additional Table Permissions
// ========================================================================
for (const tablePerm of tablePermissions) {
  const actions: string[] = [];
  if (tablePerm.actions.includes('read') || tablePerm.actions.includes('readwrite')) {
    actions.push('dynamodb:GetItem', 'dynamodb:Query', 'dynamodb:Scan');
  }
  if (tablePerm.actions.includes('write') || tablePerm.actions.includes('readwrite')) {
    actions.push('dynamodb:PutItem', 'dynamodb:UpdateItem', 'dynamodb:DeleteItem');
  }

  const resources = [
    `arn:aws:dynamodb:${stack.region}:${stack.account}:table/${tablePerm.tableName}`,
  ];
  if (tablePerm.includeIndexes) {
    resources.push(`arn:aws:dynamodb:${stack.region}:${stack.account}:table/${tablePerm.tableName}/index/*`);
  }

  this.function.addToRolePolicy(new iam.PolicyStatement({
    effect: iam.Effect.ALLOW,
    actions,
    resources,
  }));
}

// ========================================================================
// Additional Policies
// ========================================================================
for (const policy of additionalPolicies) {
  this.function.addToRolePolicy(policy);
}
```

TypeScript `TablePermission` interface at `types.ts:109–116`:

```typescript
export interface TablePermission {
  tableName: string;
  actions: ('read' | 'write' | 'readwrite')[];
  includeIndexes?: boolean;
}
```

**Key takeaways for Part 2's Rust implementation:**

1. The TS construct emits 3 actions per read and 3 per write — the CR upgrades to 4 each. Decision locked per Finding #2.
2. S3 bucket handling is **absent** in the TS construct — Part 2 is a superset of TS's table handling.
3. `[[iam.statements]]` → the equivalent of TS `additionalPolicies` — emit verbatim. The TS code does `new iam.PolicyStatement(policy)` — for CLI-generated code we render literal TS: `new iam.PolicyStatement({ effect: iam.Effect.ALLOW, actions: [...], resources: [...] })`.
4. In the generated Rust CLI output, each entry becomes ONE `format!` block rendered into the `stack_ts` string, indented to match existing 4-space indentation inside the constructor.

## Translation Rules Reference Table

### DynamoDB tables (sugar)

| Input `actions` | Emitted DynamoDB actions (CR) | TS Construct emits | Decision |
|---|---|---|---|
| `["read"]` | `GetItem`, `Query`, `Scan`, `BatchGetItem` | `GetItem`, `Query`, `Scan` | **Follow CR — add `BatchGetItem`** |
| `["write"]` | `PutItem`, `UpdateItem`, `DeleteItem`, `BatchWriteItem` | `PutItem`, `UpdateItem`, `DeleteItem` | **Follow CR — add `BatchWriteItem`** |
| `["readwrite"]` | union of read + write (8 actions) | union (6 actions) | Follow CR (8 actions) |
| `["read", "write"]` (explicit both) | union — same as `["readwrite"]` | union | Follow CR; validator must accept this form |

**Resource ARNs:**
| Config | Emitted resources |
|---|---|
| base | `arn:aws:dynamodb:${this.region}:${this.account}:table/NAME` |
| `include_indexes = true` | Above + `arn:aws:dynamodb:${this.region}:${this.account}:table/NAME/index/*` |

**Example TypeScript emitted:**
```typescript
mcpFunction.addToRolePolicy(new iam.PolicyStatement({
  effect: iam.Effect.ALLOW,
  actions: ['dynamodb:GetItem', 'dynamodb:Query', 'dynamodb:Scan', 'dynamodb:BatchGetItem',
            'dynamodb:PutItem', 'dynamodb:UpdateItem', 'dynamodb:DeleteItem', 'dynamodb:BatchWriteItem'],
  resources: [
    `arn:aws:dynamodb:${this.region}:${this.account}:table/cost-coach-tenants`,
    `arn:aws:dynamodb:${this.region}:${this.account}:table/cost-coach-tenants/index/*`,
  ],
}));
```

### S3 buckets (sugar)

| Input `actions` | Emitted S3 actions |
|---|---|
| `["read"]` | `GetObject` |
| `["write"]` | `PutObject`, `DeleteObject` |
| `["readwrite"]` | `GetObject`, `PutObject`, `DeleteObject` |

**Resource ARN:** `arn:aws:s3:::NAME/*` (object-level only — the bucket ARN `arn:aws:s3:::NAME` is NOT included).

### `[[iam.statements]]` (passthrough)

Emitted verbatim as `new iam.PolicyStatement({ effect: iam.Effect.<EFFECT>, actions: [...], resources: [...] })` after validation passes. `effect: "Allow"` → `iam.Effect.ALLOW`; `effect: "Deny"` → `iam.Effect.DENY`.

### Tie-breaking rules (not in CR, planner should lock)

- **`actions: []` with no "readwrite" entry:** validator error (empty action list) — locked by CR.
- **`actions: ["read", "readwrite"]`:** dedupe; behave as `["readwrite"]`. Recommend validator-level dedup before emitting.
- **`actions: ["unknown"]`:** validator error (per CR "action does not match `^[a-z0-9-]+:[A-Za-z0-9*]+$`" — but sugar values aren't prefixed by `:`, so a separate validator rule: `actions` in `[[iam.tables]]`/`[[iam.buckets]]` must be a subset of `{"read","write","readwrite"}`).
- **Duplicate table names across multiple `[[iam.tables]]` entries:** allow (emit separate PolicyStatements). CDK IAM aggregator dedupes anyway.

## Validation Rules Reference Table

| Rule | Severity | Check | Recommended check location |
|---|---|---|---|
| 1. Allow-star-star | **Hard error** | `effect == "Allow" && actions == ["*"] && resources == ["*"]` for any `[[iam.statements]]` entry | `validator::check_allow_star` |
| 2. Bad effect | **Error** | `effect` not in `{"Allow", "Deny"}` | serde custom deserializer or validator |
| 3. Empty actions | **Error** | `statement.actions.is_empty()` (passthrough block) | validator |
| 4. Empty resources | **Error** | `statement.resources.is_empty()` (passthrough block) | validator |
| 5. Bad action format | **Error** | Any action in `statement.actions` fails regex `^[a-z0-9-]+:[A-Za-z0-9*]+$` | validator (compile regex once with `regex::Regex`) |
| 6. Bad sugar action | **Error** | Any value in `tables[*].actions` or `buckets[*].actions` ∉ `{"read","write","readwrite"}` | serde or validator |
| 7. Unknown service prefix | **Warning** | Action prefix (`dynamodb` in `dynamodb:GetItem`) not in known-prefix list | validator |
| 8. Cross-account ARN | **Warning** | ARN's account segment differs from deploy config's account (or is a literal that's not `*` or empty) | validator, best-effort |
| 9. Empty name in tables/buckets | **Error** (recommended, not in CR) | `tables[*].name.is_empty() \|\| buckets[*].name.is_empty()` | validator |
| 10. Table/bucket name regex (recommended, not in CR) | **Warning** | name not matching AWS DynamoDB table regex `^[a-zA-Z0-9_.-]{3,255}$` / S3 bucket regex | validator |

**Severity convention:**
- Hard error / Error → `return Err(anyhow!(...))` halting `validate` and `deploy` commands.
- Warning → print to stderr with `console::style("warning:").yellow()`, do not halt.

**Edge case the planner must decide:** multi-statement user input with ONE Allow-*-* and other valid statements → fail the whole config (recommended), not just drop the offending statement.

## Open Technical Choices (research answers to brief's 10 questions)

### Q1: Is there a canonical Rust AWS ARN parser in this workspace?
**Answer (verified via `Cargo.toml` grep):** No. The workspace does not depend on `arn` or similar crates. **Recommendation: don't add one.** The CR's requirements (action regex + cross-account ARN sniffing) are narrow enough that the existing `regex = "1"` dep suffices. Cross-account sniffing: parse `arn:aws:<svc>:<region>:<account>:...` by splitting on `:`; the account is segment 4 (zero-indexed). `*` or empty = treat as "any account", not a cross-account warning.

### Q2: Is there a known service-prefix list, or do we bundle one?
**Answer:** Bundle a conservative list. `[ASSUMED]` No canonical list is exposed by any crate we depend on — `rusoto`/`aws-sdk-*` don't export this. The CR gives examples: `dynamodb`, `s3`, `secretsmanager`, `ssm`, `kms`, `sqs`, `lambda`, `iam`, `sts`. **Recommendation: ship a hand-curated list of ~40 common prefixes** (dynamodb, s3, secretsmanager, ssm, kms, sqs, sns, lambda, iam, sts, logs, cloudwatch, cloudformation, ec2, rds, events, eventbridge, athena, glue, firehose, kinesis, ecs, ecr, apigateway, execute-api, states, xray, cognito-idp, cognito-identity, appconfig, waf, wafv2, route53, acm, elasticloadbalancing, autoscaling, batch, codebuild, codepipeline, cloudfront). Unknown prefix → warning (never error). Update cadence: annually or on user report.

### Q3: Idiomatic pattern for interpolating variable list of `addToRolePolicy` calls?
**Answer:** Keep `format!` + inner `String` builder. Precedent: `init.rs` uses `format!` extensively (21 call sites verified). A template engine (`handlebars`, `tera`) would be a new dep for limited benefit. **Recommendation:**
- Pre-render the iam block into a `String` via a helper `fn render_iam_block(iam: &IamConfig) -> String` that returns the full indented TypeScript snippet (or empty string if the config is empty — preserving backward compat).
- Inject that string as a `{iam_block}` positional placeholder in the two `format!` blocks in `init.rs`.
- Use 4-space indentation to match the surrounding code style.

### Q4: Platform's TS construct translation — what does `cdk synth` emit?
**Answer (inferred from TS source):** After `cdk synth`, each `addToRolePolicy` call becomes an inline policy statement appended to the `McpFunctionServiceRoleDefaultPolicy` CloudFormation resource's `PolicyDocument.Statement` array. The synthesized JSON looks like:
```json
{
  "Effect": "Allow",
  "Action": ["dynamodb:GetItem", ...],
  "Resource": ["arn:aws:dynamodb:us-west-2:123456789012:table/cost-coach-tenants"]
}
```
**Indistinguishability goal:** The CLI-generated `stack.ts` emits the same `addToRolePolicy` calls with the same effect/actions/resources → after `cdk synth`, the resulting PolicyDocument statement IS indistinguishable. Confirmed pattern. The only non-determinism: CDK's deterministic policy statement ordering is based on insertion order, so if the TS construct synthesizes statements in order [composition-read, lambda-invoke, tablePerms[0], tablePerms[1], ..., additionalPolicies[0], ...], our CLI must preserve that order — i.e., emit in order `tables → buckets → statements`. Lock this ordering decision.

### Q5: Property test strategy shape?
**Answer:** Two `Strategy`s as the brief suggests.
- `arb_valid_iam_config() -> impl Strategy<Value = IamConfig>` — generates only *valid* inputs (sugar keywords drawn from `prop_oneof![Just("read"), Just("write"), Just("readwrite")]`; action strings drawn from `"[a-z]{2,15}:[A-Za-z]{2,20}"`; effect from `prop_oneof![Just("Allow"), Just("Deny")]`; non-empty vectors via `prop::collection::vec(..., 1..5)`).
- `arb_invalid_iam_config() -> impl Strategy<Value = IamConfig>` — generates inputs that violate specific rules (Allow-star-star generated deliberately; empty action vec generated deliberately; effect=`"Permit"` etc.).
- Use `proptest` (already in `[dev-dependencies]`). Do NOT add `arbitrary` — not in deps, unnecessary.
- Precedent: `property_tests.rs:15-42` (arb_settings, arb_scenario_step_zero_weight) shows exact style.

### Q6: Fuzz target entry point — narrow `IamConfig::from_str` or whole `DeployConfig::load`?
**Answer:** Whole `DeployConfig` parse, matching the existing `fuzz_config_parse.rs` pattern. Target: `fuzz_deploy_config.rs`, body:
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _: Result<cargo_pmcp::deployment::config::DeployConfig, _> = toml::from_str(s);
    }
});
```
This exercises `IamConfig` deserialization inside `DeployConfig` and requires no new public API. Must not panic on ANY UTF-8 input.

### Q7: Example surfacing — how does `cargo run --example` work here?
**Answer:** Cargo auto-discovers `examples/*.rs` as runnable binaries. Cargo.toml has no explicit `[[example]]` table so default auto-discovery applies. A new file `examples/deploy_iam_declaration.rs` becomes `cargo run --example deploy_iam_declaration`. Dependencies: `cargo-pmcp` (itself) via path or `use cargo_pmcp::...`. Examples demonstrate real APIs, so expose `IamConfig::parse_from_toml_str` (or similar) as a public helper to make the example readable.

**Recommended example shape:**
```rust
//! Demonstrates declaring IAM needs for a multi-tenant MCP server.
//! Mirrors cost-coach's 2026-04-23 production configuration.

fn main() -> anyhow::Result<()> {
    let toml_str = include_str!("../fixtures/cost-coach.deploy.toml");
    let iam = cargo_pmcp::deployment::config::IamConfig::from_toml_str(toml_str)?;
    cargo_pmcp::deployment::iam_validate::validate(&iam)?;
    let ts = cargo_pmcp::commands::deploy::init::render_iam_block(&iam);
    println!("{}", ts);
    Ok(())
}
```

### Q8: Validation Architecture signals (feeds VALIDATION.md)
Expanded and grouped below under the dedicated **Validation Architecture** section.

### Q9: Wave structure
See **Proposed Wave Structure** below. 5 waves confirmed against actual file coupling:
- Wave 1 edits `init.rs` (2 format! blocks) only — fully independent.
- Wave 2 edits `config.rs` only — independent of Wave 1.
- Wave 3 edits `init.rs` again (adds `{iam_block}` placeholder + helper) — depends on Wave 2 schema.
- Wave 4 adds `deployment/iam_validate.rs`, edits `validate.rs` and `deploy.rs` — depends on Wave 2.
- Wave 5 adds fuzz, example, doctests, README notes — depends on Waves 2–4.

### Q10: Rollout risk (zero-diff when [iam] is absent)
**Answer:** Backward compat holds iff:
- `IamConfig::default()` serializes to nothing (or is skipped on `toml::to_string`). Verify by using `#[serde(default, skip_serializing_if = "IamConfig::is_empty")]` on the `iam` field of `DeployConfig`.
- `render_iam_block(&IamConfig::default())` returns empty string — test this directly.
- Part 1's `McpRoleArn` CfnOutput IS a change to generated `stack.ts` — CDK `cdk diff` against an existing deployed stack shows **one added output, one added export**. CloudFormation accepts added outputs. Existing downstream consumers of OTHER outputs (LambdaArn, LambdaName, ApiUrl, DashboardUrl) are unaffected.
- **Write a golden-file test:** `tests/golden_stack_ts.rs` — run `create_stack_ts` with `iam: IamConfig::default()` and compare to a checked-in golden `.ts` file. Pre-phase baseline golden has the stack without `McpRoleArn`; post-Wave-1 baseline has it. Any reordering regression is caught immediately.
- **cdk.out digest:** CDK's synthesized JSON key order is deterministic, so adding outputs at the END of the Outputs section produces a stable diff. No existing-deployment digest problem.

## Proposed Wave Structure

| Wave | Scope | Files touched | Independently shippable? |
|---|---|---|---|
| **Wave 1** | Part 1: `McpRoleArn` CfnOutput in both template branches. Include import fix for aws-lambda branch (`import * as iam from 'aws-cdk-lib/aws-iam';` — safe addition). Golden-file test for both branches. | `src/commands/deploy/init.rs`, new `tests/golden_stack_ts.rs` | **Yes** — purely additive, zero config surface |
| **Wave 2** | `IamConfig` struct hierarchy (`IamConfig`, `TablePermission`, `BucketPermission`, `IamStatement`), wired into `DeployConfig` with `#[serde(default, skip_serializing_if = "IamConfig::is_empty")]`. Roundtrip unit + property tests. `default_for_server` updated. | `src/deployment/config.rs`, `tests/property_tests.rs` (new `prop_iam_*` functions or a new `tests/iam_property_tests.rs`) | Yes (schema-only addition, no behavior change) |
| **Wave 3** | Translation rules → `render_iam_block(&IamConfig) -> String` helper. Wire into both `format!` blocks in `init.rs` via new `{iam_block}` positional arg. Unit tests for every row in the Translation Rules table. Property test: `arb_valid_iam → render → parse resulting TS string for structural shape → no dropped statements`. | `src/commands/deploy/init.rs`, `tests/iam_property_tests.rs` | Depends on Wave 2 |
| **Wave 4** | `src/deployment/iam_validate.rs` module with `validate(&IamConfig) -> Result<Vec<Warning>>`. Wire into `DeployExecutor::execute` (hard-errors halt, warnings print) and new `ValidateCommand::Deploy` variant. Unit test per rule in Validation Rules table. | `src/deployment/iam_validate.rs` (new), `src/deployment/mod.rs`, `src/commands/validate.rs`, `src/commands/deploy/deploy.rs` | Depends on Wave 2 |
| **Wave 5** | Fuzz target `fuzz_deploy_config.rs` + registration in `fuzz/Cargo.toml`. Example `examples/deploy_iam_declaration.rs` + fixture `examples/fixtures/cost-coach.deploy.toml` (or inline). Doctests on new public APIs. `README.md` + `DEPLOYMENT.md` updates documenting the `[iam]` section. Final `make quality-gate` green. | `fuzz/fuzz_targets/fuzz_deploy_config.rs` (new), `fuzz/Cargo.toml`, `examples/deploy_iam_declaration.rs` (new), docs | Depends on Waves 2–4 |

**Risk-ordered if scope must split:** ship Wave 1 as a patch (`0.9.1 → 0.9.2`) immediately; ship Waves 2–5 as a minor (`0.9.2 → 0.10.0`). CR explicitly supports this split posture.

## Validation Architecture (Nyquist Dimension 8)

### Test Framework
| Property | Value |
|---|---|
| Framework | `cargo test` with `proptest = "1"` for property tests + `libfuzzer-sys = "0.4"` via `cargo +nightly fuzz` |
| Config file | `cargo-pmcp/Cargo.toml` (`[dev-dependencies]` line 77-79), `cargo-pmcp/fuzz/Cargo.toml` |
| Quick run command | `cargo test -p cargo-pmcp iam` (filters to iam-tagged tests) |
| Full suite command | `cd cargo-pmcp && cargo test && cd fuzz && cargo +nightly fuzz run fuzz_deploy_config -- -max_total_time=30` (30s fuzz smoke) |
| Phase gate | `make quality-gate` from repo root — matches CI exactly per CLAUDE.md |

### Observable correctness signals (grouped by test type)

#### Unit tests
| Signal | Test location |
|---|---|
| `IamConfig::default().is_empty() == true` | `src/deployment/config.rs` `#[cfg(test)]` |
| `toml::to_string(DeployConfig { iam: default(), ..default_for_server(...) })` does NOT contain `[iam]` section | `tests/property_tests.rs` |
| `render_iam_block(&IamConfig::default()) == ""` | `src/commands/deploy/init.rs` `#[cfg(test)]` |
| For `[[iam.tables]] actions=["read"]`: emitted TS contains each of 4 actions: `GetItem`, `Query`, `Scan`, `BatchGetItem` | Wave 3 unit tests |
| For `include_indexes=true`: emitted TS contains both `/table/foo` and `/table/foo/index/*` resources | Wave 3 |
| For `[[iam.buckets]] actions=["readwrite"]`: emitted TS contains `GetObject`, `PutObject`, `DeleteObject` and resource `arn:aws:s3:::NAME/*` | Wave 3 |
| For `[[iam.statements]]` `effect=Allow actions=["*"] resources=["*"]`: `validate()` returns `Err` | Wave 4 |
| For effect=`Permit`: `validate()` returns `Err` | Wave 4 |
| For action=`DynamoDB:getitem` (bad case): `validate()` returns `Err` | Wave 4 |
| For unknown service prefix `foo:bar`: `validate()` returns `Ok` with one warning | Wave 4 |
| For cross-account ARN `arn:aws:dynamodb:us-west-2:999999999999:table/t` when config account=`111111111111`: `validate()` returns `Ok` with one warning | Wave 4 |

#### Property tests
| Signal | Test location |
|---|---|
| `arb_valid_iam_config` → `toml::to_string` → `toml::from_str` → equal | `tests/iam_property_tests.rs` |
| `arb_valid_iam_config` → `render_iam_block` → parse resulting string with a rough regex for `addToRolePolicy` → count matches statements emitted | `tests/iam_property_tests.rs` |
| `arb_valid_iam_config` → `validate()` is `Ok` (warnings allowed) | `tests/iam_property_tests.rs` |
| `arb_invalid_iam_config` variant of Allow-star-star → `validate()` is `Err` | `tests/iam_property_tests.rs` |
| `arb_empty_actions` → `validate()` is `Err` | `tests/iam_property_tests.rs` |
| For any `IamConfig`: `render_iam_block` output is valid UTF-8 TypeScript (contains balanced braces) — coarse invariant | `tests/iam_property_tests.rs` |

#### Integration tests (golden file)
| Signal | Test location |
|---|---|
| `create_stack_ts("pmcp-run", "demo-server")` with `iam: default` → matches `tests/golden/pmcp-run-empty.ts` byte-for-byte (except Wave 1 McpRoleArn addition) | `tests/golden_stack_ts.rs` |
| `create_stack_ts("aws-lambda", "demo-server")` with `iam: default` → matches `tests/golden/aws-lambda-empty.ts` byte-for-byte | `tests/golden_stack_ts.rs` |
| `create_stack_ts("pmcp-run", "demo-server")` with cost-coach-shaped `iam` → matches `tests/golden/pmcp-run-cost-coach.ts` byte-for-byte | `tests/golden_stack_ts.rs` |
| The emitted `stack.ts` compiles via `npx tsc --noEmit stack.ts` in a scaffolded test project | Manual OR integration in Wave 5 |

#### Fuzz tests
| Signal | Test location |
|---|---|
| `cargo +nightly fuzz run fuzz_deploy_config -- -max_total_time=60` for 60s → no panics, no ICE, no OOM | `fuzz/fuzz_targets/fuzz_deploy_config.rs` |

#### Manual / example
| Signal | Method |
|---|---|
| `cargo run --example deploy_iam_declaration` prints the cost-coach TS block and exits 0 | Manual |
| `cargo pmcp validate deploy --server <scaffolded>` on a toml with `Allow *:*` exits non-zero with a helpful message | Manual |
| `cargo pmcp deploy` on a toml with warning-tier issues prints warnings to stderr and proceeds | Manual (requires AWS creds) |

#### CI / quality gate
| Signal | Method |
|---|---|
| `make quality-gate` from repo root passes (fmt, clippy pedantic+nursery, build, test, audit, doctests) | CI |
| Doctests on new public APIs (`IamConfig::from_toml_str`, `validate`, `render_iam_block`) pass | `cargo test --doc -p cargo-pmcp` |

### Phase Requirements → Test Map
| Phase output | Observable signal | Automated command |
|---|---|---|
| Part 1: McpRoleArn export | Golden file match + `cdk synth` on scaffolded project contains `McpRoleArn` export | `cargo test -p cargo-pmcp golden_stack` |
| Part 2: schema roundtrip | Property test: arb_iam roundtrip | `cargo test -p cargo-pmcp iam_roundtrip` |
| Part 2: translation rules | Per-row unit tests + property tests | `cargo test -p cargo-pmcp iam_render` |
| Part 2: validator | Per-rule unit tests | `cargo test -p cargo-pmcp iam_validate` |
| Part 2: robustness | 60s fuzz smoke | `cd fuzz && cargo +nightly fuzz run fuzz_deploy_config -- -max_total_time=60` |
| Part 2: example | `cargo run --example deploy_iam_declaration` exits 0 | Manual / CI example-run step |
| Overall | `make quality-gate` | From repo root |

### Sampling Rate
- **Per task commit:** `cargo test -p cargo-pmcp`
- **Per wave merge:** `cargo test -p cargo-pmcp && cargo clippy -p cargo-pmcp --all-targets -- -D warnings`
- **Phase gate:** `make quality-gate` green from repo root.

### Wave 0 Gaps
- [ ] `tests/golden_stack_ts.rs` — new golden-file test scaffold (Wave 1 creates it)
- [ ] `tests/golden/` directory with baseline `.ts` files (Wave 1)
- [ ] `tests/iam_property_tests.rs` — new property-test file (Wave 2/3 extends existing `tests/property_tests.rs` OR creates new file; recommend new file for clarity)
- [ ] `fuzz/fuzz_targets/fuzz_deploy_config.rs` + entry in `fuzz/Cargo.toml` (Wave 5)
- [ ] `examples/deploy_iam_declaration.rs` + fixture (Wave 5)

No framework install needed — proptest and libfuzzer-sys already in the dependency tree.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| `cargo` | build/test | ✓ (assumed) | stable | — |
| `cargo +nightly` | fuzz | varies | — | Wave 5 fuzz target can be CI-only; skip if nightly missing locally |
| `proptest` | property tests | ✓ in `[dev-dependencies]` | `1` | — |
| `libfuzzer-sys` | fuzz | ✓ in `fuzz/Cargo.toml` | `0.4` | — |
| `regex` | validator | ✓ in `[dependencies]` | `1` | — |
| `toml` | config parsing | ✓ in `[dependencies]` | `1.0` | — |
| `npx tsc` | optional stack.ts compile check | varies | — | Skip TS compile check if missing — golden file comparison is authoritative |
| `aws` CLI | manual E2E deploy test | varies | — | Not required — example test covers config path without AWS |
| `cdk` CLI | manual E2E synth test | varies | — | Not required for unit/property/golden tests |

**Missing dependencies with no fallback:** none (all required tooling is in the dev-dependencies tree or has a skip-if-missing strategy).

**Missing dependencies with fallback:** nightly Rust (fuzz), TypeScript compiler (optional stack.ts validity check).

## Considered and Rejected (per CR, do not re-propose)

These options were considered and explicitly rejected in the CR. The planner must not re-propose them and should reference the CR's rationale if reviewers ask:

1. **Auto-infer IAM from env var names ending in `_TABLE_NAME` / `_BUCKET`.** Rejected because env var values can be placeholders, cross-account ARNs, SSM refs, or arbitrary strings. Silently granting IAM based on variable names hides the effective policy and surprises operators.

2. **Auto-grant `dynamodb:*` on `arn:...:table/${serverName}-*` prefix.** Rejected because future tables matching the naming convention would silently inherit RW access, and it doesn't cover tables provisioned in separate stacks (which is cost-coach's actual case).

Both are documented here for completeness; do not revisit in planning.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | 40-entry AWS service-prefix known list is the right conservative scope | Q2 | Low — unknown prefix → warning, never error; false negatives just silence a warning |
| A2 | `mcpFunction.role!` in TypeScript is safe (role IS always defined for a lambda.Function without explicit role prop) | Q1 recommendation + Wave 1 | Low — CDK auto-creates a role; this is documented CDK behavior |
| A3 | CDK preserves insertion order of `addToRolePolicy` calls in the synthesized template | Q4 | Low — empirically true in CDK; matters only for golden-file byte equality, not correctness |
| A4 | TypeScript `import * as iam from 'aws-cdk-lib/aws-iam';` added to aws-lambda branch is harmless if iam isn't used | Codebase survey `init.rs` | Low — TypeScript allows unused imports at module scope |
| A5 | CR's 4-action DynamoDB lists (with Batch) are the intended authoritative spec, superseding the TS construct's 3-action lists | Finding #2 + Translation Rules | Medium — discuss-phase could flip this; Wave 3 is small and can be revised |
| A6 | `../provable-contracts/contracts/cargo-pmcp/` may not exist in this tree and contract update is skippable | Deferred Ideas | Low — CLAUDE.md contract-first is aspirational, not blocking |
| A7 | `DeployConfig::load_from_str` does not exist today and Wave 5 example will need to add it (small additive public helper) | Example recommendation | Low — one-line helper |

## Open Questions for the Planner

1. **Translation action-set resolution.** Confirm Finding #2 / A5 with operator: follow CR's 4-action lists (add Batch operations) or mirror the TS construct's 3-action lists? Planner should lock this in PLAN.md as a D-XX decision.

2. **Role naming.** Keep auto-generated role (`mcpFunction.role!`) for both Part 1 export and Part 2 `addToRolePolicy` calls, or introduce a named `McpRole` construct? Finding #1 recommends the former; confirm in planning.

3. **Validator hook points.** Both `DeployExecutor::execute` runtime check AND new `ValidateCommand::Deploy` variant, or just the former? Recommendation: both.

4. **Golden-file location.** Place golden `.ts` files at `cargo-pmcp/tests/golden/` (convention-following) or `cargo-pmcp/tests/fixtures/` (alternate)? Recommend `tests/golden/` — matches the "golden file" terminology used in test name.

5. **Semver bump.** `0.9.1 → 0.10.0` (minor, additive feature) per workspace convention, OR stay `0.9.x` if team treats pre-1.0 as anything-goes? Recommend `0.10.0` — feature addition per CLAUDE.md §"Version Bump Rules".

6. **README placement.** Add a new `## IAM Declarations` section to `cargo-pmcp/DEPLOYMENT.md` and a pointer in `cargo-pmcp/README.md`, or inline in README? Recommend: detailed docs in `DEPLOYMENT.md`, one-line summary + link in `README.md`.

7. **Fuzz corpus seeding.** Should Wave 5 seed `fuzz/corpus/fuzz_deploy_config/` with a few real `deploy.toml` examples (empty, cost-coach-shaped, invalid-allow-star) to accelerate coverage? Recommend yes — 3-5 seed files is low-effort high-value.

## Sources

### Primary (HIGH confidence — VERIFIED by direct file read)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/config.rs` lines 1–717 (schema conventions)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/deploy/init.rs` lines 1–80, 480–747 (template generator, two branches)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/validate.rs` lines 1–500 (current workflow-only validator)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/deploy/deploy.rs` lines 1–140 (DeployExecutor hook point)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/property_tests.rs` lines 1–100 (proptest convention)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/fuzz/fuzz_targets/fuzz_config_parse.rs` (fuzz convention)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/Cargo.toml` (deps: regex, proptest, toml present; no arn parser)
- `/Users/guy/Development/mcp/sdk/pmcp-run/built-in/shared/cdk-constructs/src/mcp-server-construct.ts` lines 80–280 (TablePermission loop, additionalPolicies loop — the reference implementation)
- `/Users/guy/Development/mcp/sdk/pmcp-run/built-in/shared/cdk-constructs/src/types.ts` lines 100–116 (TablePermission interface)
- `/Users/guy/Development/mcp/sdk/pmcp-run/docs/CLI_IAM_CHANGE_REQUEST.md` (the CR)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/76-.../76-CONTEXT.md` (locked decisions)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/STATE.md` (project history, cargo-pmcp recent phases)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/CLAUDE.md` (Toyota Way mandates)

### Secondary
- AWS CloudFormation `Outputs.Export.Name` semantics — CDK docs behavior inferred from `cdk.CfnOutput` API (well-known CDK pattern).

## Project Constraints (from CLAUDE.md)

All items below are mandatory per `./CLAUDE.md` and apply to this phase:

1. **Zero tolerance for defects.** No clippy warnings; no SATD comments; cognitive complexity ≤ 25 per function.
2. **`make quality-gate` before any commit.** CI runs the same; local shortcuts like `cargo clippy -- -D warnings` are weaker and miss pedantic+nursery lints.
3. **ALWAYS requirements per feature:** fuzz testing, property testing, unit testing (80%+ coverage), runnable `cargo run --example`. Phase 76 must ship all four.
4. **Pre-commit quality gates are blocking** — cannot commit without fmt, clippy, build, doctests passing.
5. **PMAT quality-gate proxy during development** (if enabled in the local environment) — file writes should go through `quality_proxy` MCP tool.
6. **Tests run with `--test-threads=1`** in CI (race condition prevention). Local tests should work serial.
7. **Contract-first** (`../provable-contracts/contracts/cargo-pmcp/`) — update if the repo exists; skip if not (see A6).
8. **Version bump rules.** `cargo-pmcp` is a leaf downstream crate; bump `0.9.1 → 0.10.0` for this minor additive feature. No pmcp core bump required.
9. **Release uses `v` prefix tags.** Tag after PR merges + CI green.
10. **Workspace publish order.** `cargo-pmcp` publishes last (after pmcp-widget-utils, pmcp, mcp-tester, mcp-preview).

## Metadata

**Confidence breakdown:**
- Reference implementation mirror: HIGH — read the TS source directly.
- Standard stack / dep availability: HIGH — read `Cargo.toml` directly; all deps present.
- Architecture / file layout: HIGH — read actual files and grepped usage sites.
- Validation rules: HIGH — CR is explicit; no ambiguity.
- Translation action-set (4-action vs 3-action): MEDIUM — CR and TS source disagree; recommendation is to follow CR but planner should confirm. Flagged A5.
- Wave structure: MEDIUM — proposed from file coupling analysis; checker may adjust.

**Research date:** 2026-04-22
**Valid until:** 2026-05-22 (30 days — stable, schema-focused phase; no fast-moving ecosystem deps)

## RESEARCH COMPLETE

Research complete. Planner can now create PLAN.md files for Waves 1–5. Key items for the planner to lock as D-XX decisions: Finding #2 (action-set resolution), Finding #1 (role naming), and validator hook strategy (Q3 in Open Technical Choices). All other findings have clear recommendations with rationale.
