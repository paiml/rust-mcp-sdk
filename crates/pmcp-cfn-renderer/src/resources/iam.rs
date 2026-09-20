//! `AWS::IAM::Role` + `AWS::IAM::Policy` rendering for the MCP server's
//! Lambda execution role.
//!
//! Task 3 shipped only the BASE execution role every `pmcp-run` server gets
//! — the Lambda-basic-execution trust policy, plus the fixed
//! composition-support inline policy CDK always attaches (X-Ray tracing +
//! DynamoDB foundation-server discovery + cross-Lambda invoke). This
//! mirrors the `pmcp-run` branch of
//! `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts` verbatim
//! (`mcpFunction.addToRolePolicy(...)` x3, plus the L2 `lambda.Function`
//! construct's own default-role wiring).
//!
//! Task 4 (this module, current state) extends that with the
//! operator-declared `[[iam.statements]]` expansion ported from
//! `cargo-pmcp/src/deployment/iam.rs`: [`validate`] ports that file's
//! fail-closed validation rules (hard errors + advisory [`Warning`]s), and
//! [`render_execution_role`]'s policy appends each validated declared
//! statement onto the SAME default policy document, after the three
//! platform-composition statements, in descriptor order — never a second
//! `AWS::IAM::Policy` resource. This shape is pinned by
//! `tests/goldens/wild-msr-vtt.golden.json` (a real `pmcp-run` fixture with
//! 4 declared `[[iam.statements]]` blocks) and
//! `tests/goldens/iam-statements.golden.json` (a minimal fixture pinning
//! the single-vs-array collapse rule below).
//!
//! # Sugar deferral (`[[iam.tables]]` / `[[iam.buckets]]`)
//!
//! `cargo-pmcp/src/deployment/iam.rs` also translates `TablePermission`/
//! `BucketPermission` "sugar" entries (`read`/`write`/`readwrite` keywords
//! that expand to DynamoDB/S3 action lists + ARNs). That sugar is a
//! CLI-side-only concept: `pmcp_package::package::IamSection` (the closed
//! descriptor type this renderer consumes) is statements-only —
//! `{ statements: Vec<IamStatement> }`, no `tables`/`buckets` fields (see
//! `crates/pmcp-package/src/package/server.rs` around line 241). A survey
//! of every tracked `.pmcp/deploy.toml` in this repo AND the sibling
//! `pmcp-run` repo (`git ls-files '*/.pmcp/deploy.toml'` in both, then
//! grepping for `iam.tables`/`iam.buckets`) found zero real projects using
//! the sugar forms — several DO use raw `[[iam.statements]]` heavily
//! (`agent-eval-harvest`, `agent-lake`, `msr-vtt`, `graphrag-admin`). Per
//! YAGNI + the design spec's closed-set-promotion discipline, this task
//! ports ONLY the statements path. Because the descriptor type cannot parse
//! sugar fields at all (`#[serde(deny_unknown_fields)]` on `IamSection`
//! rejects an unrecognized `[[iam.tables]]`/`[[iam.buckets]]` key before
//! this module ever sees it), no additional "unimplemented sugar" guard is
//! needed here — the loud failure already happens at descriptor-parse time.
//! If a real project needs the sugar later, promote `tables`/`buckets`
//! fields onto `IamSection` first (bumping `pmcp-package`'s published
//! version per the `[auth.cognito]` precedent), then port
//! `render_table`/`render_bucket`/`table_actions`/`bucket_actions` here.
//!
//! CDK renders a Lambda's attached inline permissions as a SEPARATE
//! `AWS::IAM::Policy` resource (`McpFunction/ServiceRole/DefaultPolicy`),
//! never as a `Policies:` block inline on the role — [`render_execution_role`]
//! matches that shape (two resources, not one) because the semantic-golden
//! harness compares against real `cdk synth` output.

use crate::{
    error::RenderError,
    logical_ids,
    params::RenderParams,
    resources::{aws_lambda_tags, standard_tags, MCP_SERVERS_TABLE},
    template::CfnResource,
};
use pmcp_package::package::{DeployDescriptor, IamSection, IamStatement};
use serde_json::{json, Value};

/// The Lambda's default inline policy's `PolicyName`. CDK derives this name
/// from a content hash of the resource's construct path (e.g.
/// `McpFunctionServiceRoleDefaultPolicy29310C43`), but that hash is
/// CDK-synthesis-specific identity, not renderer truth — the design spec
/// (§5) forbids CDK-style content hashes in renderer output, and different
/// fixtures produce different hashes (the `oauth-cognito-dcr` golden's
/// second Lambda policy is
/// `OAuthProxyFunctionServiceRoleDefaultPolicy7EA1E8EC`), so hardcoding one
/// hash per golden doesn't scale. This renderer is hash-free by design (see
/// `logical_ids`); it emits a stable, declared name instead. The
/// semantic-golden harness's normalizer sentinelizes `PolicyName` on both
/// sides (see `tests/support/mod.rs::sentinelize_policy_name`), so this
/// literal never needs to match what a real `cdk synth` produces.
pub(crate) const DEFAULT_POLICY_NAME: &str = "pmcp-declared";

/// The Lambda execution role's trust-policy + managed-policy shape, shared
/// by every `AWS::IAM::Role` this crate renders for a Lambda function's
/// execution role — the `pmcp-run` kernel's role, the `aws-lambda` kernel's
/// role, and (Task 6) the `cognito`/DCR OAuth stack's three roles (MCP
/// function, OAuth-proxy, authorizer). Only `Tags` varies by caller — see
/// [`render_role`]/[`render_role_aws_lambda`] and `resources::cognito`'s
/// role builders.
pub(crate) fn lambda_execution_role_properties(tags: Value) -> Value {
    json!({
        "AssumeRolePolicyDocument": {
            "Version": "2012-10-17",
            "Statement": [{
                "Effect": "Allow",
                "Action": "sts:AssumeRole",
                "Principal": { "Service": "lambda.amazonaws.com" },
            }],
        },
        "ManagedPolicyArns": [{
            "Fn::Join": ["", [
                "arn:",
                { "Ref": "AWS::Partition" },
                ":iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
            ]],
        }],
        "Tags": tags,
    })
}

/// Render the MCP server's Lambda execution role and its default inline
/// policy: `[(role_id, role), (policy_id, policy)]`. Any `[[iam.statements]]`
/// declared on `d.iam` are appended onto the policy's `Statement` array, in
/// descriptor order, after the three fixed platform-composition statements.
///
/// Callers MUST run [`validate`] on `d.iam` first (as [`crate::render`]
/// does) — this function does not re-validate; it assumes every declared
/// statement already passed the fail-closed checks (canonical `Allow`/`Deny`
/// effect, non-empty actions/resources).
#[must_use]
pub fn render_execution_role(d: &DeployDescriptor, p: &RenderParams) -> Vec<(String, CfnResource)> {
    vec![render_role(d), render_policy(d, p)]
}

/// The base `AWS::IAM::Role`: Lambda service trust policy +
/// `AWSLambdaBasicExecutionRole`.
fn render_role(d: &DeployDescriptor) -> (String, CfnResource) {
    (
        logical_ids::for_execution_role().to_string(),
        CfnResource {
            type_: "AWS::IAM::Role".to_string(),
            properties: lambda_execution_role_properties(standard_tags(&d.server.name)),
            depends_on: vec![],
        },
    )
}

/// The default `AWS::IAM::Policy`: X-Ray tracing + DynamoDB
/// foundation-server discovery + cross-Lambda invoke (always present,
/// regardless of `[observability]`/`[composition]` field values — those
/// sections are not wired into this stack shape today), PLUS any
/// operator-declared `[[iam.statements]]` appended after them in descriptor
/// order (see the module doc comment's sugar-deferral note).
///
/// `p` is currently unused: the DynamoDB discovery + cross-Lambda invoke
/// ARNs below are CFN pseudo-parameter forms (see
/// [`family_internal_pseudo_param_arn`]'s doc comment), not literal
/// `p.region`/`p.account_id` interpolations — see that function's doc
/// comment for why. Kept as a parameter (rather than dropped from this and
/// [`render_execution_role`]'s public signature) for symmetry with
/// [`render_policy_aws_lambda`] and in case a future family-internal
/// resource here needs it again.
fn render_policy(d: &DeployDescriptor, _p: &RenderParams) -> (String, CfnResource) {
    let base_statements = vec![
        json!({
            "Effect": "Allow",
            "Action": ["xray:PutTraceSegments", "xray:PutTelemetryRecords"],
            "Resource": "*",
        }),
        json!({
            "Effect": "Allow",
            "Action": ["dynamodb:GetItem", "dynamodb:Query"],
            "Resource": [
                family_internal_pseudo_param_arn(format!(
                    "arn:aws:dynamodb:${{AWS::Region}}:${{AWS::AccountId}}:table/{MCP_SERVERS_TABLE}"
                )),
                family_internal_pseudo_param_arn(format!(
                    "arn:aws:dynamodb:${{AWS::Region}}:${{AWS::AccountId}}:table/{MCP_SERVERS_TABLE}/*"
                )),
            ],
        }),
        json!({
            "Effect": "Allow",
            "Action": "lambda:InvokeFunction",
            "Resource": family_internal_pseudo_param_arn(
                "arn:aws:lambda:${AWS::Region}:${AWS::AccountId}:function:*"
            ),
        }),
    ];
    build_policy_resource(d, base_statements)
}

/// Render the MCP server's Lambda execution role and its default inline
/// policy for the `aws-lambda` target's own stack shape:
/// `[(role_id, role), (policy_id, policy)]`. Unlike [`render_execution_role`]
/// (`pmcp-run`'s composition-aware base policy — X-Ray plus DynamoDB
/// discovery plus cross-Lambda invoke), this stack shape's Lambda has no
/// `MCP_SERVERS_TABLE` discovery table to read and never calls other
/// foundation-server Lambdas, so its ONLY fixed base statement is the X-Ray
/// tracing grant CDK auto-attaches for `tracing: lambda.Tracing.ACTIVE`
/// (mirrors the `aws-lambda` branch of
/// `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts`, which has no
/// `addToRolePolicy` calls of its own). Any `[[iam.statements]]` declared on
/// `d.iam` are still appended after that one base statement, in descriptor
/// order — same append contract as [`render_execution_role`]. Tags use
/// [`crate::resources::aws_lambda_tags`] instead of [`standard_tags`] (own
/// `project`, `target = "aws-lambda"`).
///
/// Callers MUST run [`validate`] on `d.iam` first, same caveat as
/// [`render_execution_role`].
#[must_use]
pub fn render_execution_role_aws_lambda(d: &DeployDescriptor) -> Vec<(String, CfnResource)> {
    vec![render_role_aws_lambda(d), render_policy_aws_lambda(d)]
}

/// The base `AWS::IAM::Role` for the `aws-lambda` target — same trust
/// policy/managed-policy shape as [`render_role`], `aws-lambda`-flavored
/// tags.
fn render_role_aws_lambda(d: &DeployDescriptor) -> (String, CfnResource) {
    (
        logical_ids::for_execution_role().to_string(),
        CfnResource {
            type_: "AWS::IAM::Role".to_string(),
            properties: lambda_execution_role_properties(aws_lambda_tags(&d.server.name)),
            depends_on: vec![],
        },
    )
}

/// The default `AWS::IAM::Policy` for the `aws-lambda` target: X-Ray
/// tracing ONLY (no DynamoDB/cross-Lambda composition sugar — see
/// [`render_execution_role_aws_lambda`]'s doc comment), plus any
/// operator-declared `[[iam.statements]]` appended after it in descriptor
/// order.
fn render_policy_aws_lambda(d: &DeployDescriptor) -> (String, CfnResource) {
    let base_statements = vec![json!({
        "Effect": "Allow",
        "Action": ["xray:PutTraceSegments", "xray:PutTelemetryRecords"],
        "Resource": "*",
    })];
    build_policy_resource(d, base_statements)
}

/// Build the default `AWS::IAM::Policy` resource shared by
/// [`render_policy`]/[`render_policy_aws_lambda`]: append any declared
/// `[[iam.statements]]` (in descriptor order) onto `base_statements`, then
/// wrap the result in the standard `PolicyDocument`/`Roles` envelope. The
/// two callers differ only in which fixed statements they seed
/// `base_statements` with.
fn build_policy_resource(
    d: &DeployDescriptor,
    mut base_statements: Vec<Value>,
) -> (String, CfnResource) {
    if let Some(iam) = &d.iam {
        base_statements.extend(iam.statements.iter().map(render_declared_statement));
    }
    let properties = json!({
        "PolicyName": DEFAULT_POLICY_NAME,
        "PolicyDocument": {
            "Version": "2012-10-17",
            "Statement": base_statements,
        },
        "Roles": [{ "Ref": logical_ids::for_execution_role() }],
    });
    (
        logical_ids::for_execution_policy().to_string(),
        CfnResource {
            type_: "AWS::IAM::Policy".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// Wrap an ARN template string as a CFN `Fn::Sub` intrinsic — `{"Fn::Sub":
/// template}` — for [`render_policy`]'s two family-internal grants (the
/// `MCP_SERVERS_TABLE` discovery reads and the cross-Lambda invoke
/// wildcard).
///
/// These two resources are ALWAYS in the same AWS account/region the stack
/// itself deploys into (this Lambda's own discovery table, and other
/// `pmcp-run` family members' functions) — CloudFormation can resolve that
/// at deploy time via the `AWS::Region`/`AWS::AccountId` pseudo-parameters,
/// exactly what production `cdk synth` emits for an environment-agnostic
/// stack (no explicit `env: {account}` pinned — see
/// `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs`'s
/// `UNRESOLVED_ACCOUNT_ID` doc comment for the investigation that confirmed
/// this). Baking [`RenderParams::account_id`] as a literal here — this
/// module's pre-T7-review behavior — produced DEAD grants whenever that
/// field isn't a real account (e.g. the `pmcp_run` deploy path's
/// documented all-zeros sentinel): a policy statement scoped to
/// `arn:aws:dynamodb:<region>:000000000000:table/...` can never match any
/// real resource. Pseudo-params sidestep that failure mode entirely for
/// resources that are always same-account by construction.
///
/// USER-DECLARED `[[iam.statements]]` ARNs are NEVER routed through this
/// helper — [`render_declared_statement`] passes them through verbatim, as
/// user content the renderer must not rewrite.
///
/// Other literal-account/region ARNs elsewhere in this crate (the
/// `aws-lambda` target's `execute-api` `SourceArn`, `resources::cognito`'s
/// domain-prefix naming suffix) stay literal on purpose — see
/// `resources::http_api::render_permission_for`'s doc comment — because
/// those are golden-pinned to a REAL account resolved via STS, not this
/// same-account family-internal case.
fn family_internal_pseudo_param_arn(template: impl Into<String>) -> Value {
    json!({ "Fn::Sub": template.into() })
}

/// Render one validated, operator-declared `[[iam.statements]]` entry as a
/// CFN policy-statement JSON object. Passthrough after [`validate`] — no
/// sugar expansion (see the module doc comment).
fn render_declared_statement(stmt: &IamStatement) -> Value {
    json!({
        "Effect": stmt.effect,
        "Action": one_or_many(&stmt.actions),
        "Resource": one_or_many(&stmt.resources),
    })
}

/// CFN/CDK collapses a single-element `Action`/`Resource` array to a bare
/// scalar string rather than a 1-element array (confirmed against the
/// `wild-msr-vtt` golden's Athena statement: a single-resource ARN renders
/// as `"Resource": "arn:...:workgroup/msr-vtt"`, not `["arn:...]`, while its
/// multi-item `Action`/`Resource` lists stay arrays). This helper
/// reproduces that collapse for arbitrary declared statements.
fn one_or_many(items: &[String]) -> Value {
    match items {
        [single] => json!(single),
        many => json!(many),
    }
}

// ============================================================================
// Validation — ported from `cargo-pmcp/src/deployment/iam.rs::validate`
// (statements-only; see the module doc comment's sugar-deferral note for
// why the table/bucket sugar rules — cargo-pmcp's rules 5 + 6 — don't apply
// here).
// ============================================================================

/// A non-blocking validation finding produced by [`validate`], carrying a
/// stable machine-readable `code` (cargo-pmcp's own `Warning` type has no
/// `code` field — this crate's `iam::validate` contract adds one, see the
/// module doc comment) plus the human-readable `message`. Hard errors
/// short-circuit via `Result::Err(RenderError::Invalid)`; soft findings land
/// here so a caller (CLI or hosting platform) can surface them without
/// blocking render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Stable, machine-readable code identifying the warning class —
    /// `"iam.unknown_service_prefix"` or `"iam.cross_account_arn"`.
    pub code: String,
    /// Human-readable warning text.
    pub message: String,
}

impl Warning {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

/// Curated list of well-known AWS service prefixes. Unknown prefixes in
/// `[[iam.statements]]` action strings trigger [`Warning::code`]
/// `"iam.unknown_service_prefix"` (not an error). Ported verbatim from
/// `cargo-pmcp/src/deployment/iam.rs::KNOWN_SERVICE_PREFIXES`.
const KNOWN_SERVICE_PREFIXES: &[&str] = &[
    "acm",
    "apigateway",
    "appconfig",
    "athena",
    "autoscaling",
    "batch",
    "cloudformation",
    "cloudfront",
    "cloudwatch",
    "codebuild",
    "codepipeline",
    "cognito-idp",
    "cognito-identity",
    "dynamodb",
    "ec2",
    "ecr",
    "ecs",
    "elasticloadbalancing",
    "events",
    "eventbridge",
    "execute-api",
    "firehose",
    "glue",
    "iam",
    "kinesis",
    "kms",
    "lambda",
    "logs",
    "rds",
    "route53",
    "s3",
    "secretsmanager",
    "sns",
    "sqs",
    "ssm",
    "states",
    "sts",
    "waf",
    "wafv2",
    "xray",
];

/// Validate declared `[[iam.statements]]` per the CR-locked rule catalogue
/// (`cargo-pmcp/src/deployment/iam.rs`'s rules 1-4 + 7-8; rules 5-6 are
/// table/bucket-sugar-only and don't apply — see the module doc comment).
///
/// Hard errors (4 classes) short-circuit via `Err`; soft findings are
/// returned as a `Vec<Warning>` for the caller to surface without blocking
/// render.
///
/// Hard-error rules:
///   1. `Allow` + `actions=["*"]` + `resources=["*"]` in any statement
///      (wildcard escalation footgun)
///   2. `effect` not in `{"Allow", "Deny"}`
///   3. empty `actions` or empty `resources` in any statement
///   4. action not matching `^[a-z0-9-]+:[A-Za-z0-9*]+$`
///
/// Warning rules:
///   7. unknown service prefix (`code: "iam.unknown_service_prefix"`)
///   8. pinned 12-digit AWS account in an ARN (`code: "iam.cross_account_arn"`)
///
/// # Errors
///
/// Returns [`RenderError::Invalid`] on the first hard-error rule violation.
/// Warnings never produce an `Err`.
pub fn validate(iam: &IamSection) -> Result<Vec<Warning>, RenderError> {
    let mut warnings = Vec::new();
    for (idx, stmt) in iam.statements.iter().enumerate() {
        check_effect_and_shape(idx, stmt)?;
        check_wildcard_escalation(idx, stmt)?;
        check_actions(idx, stmt, &mut warnings)?;
        collect_cross_account_warnings(idx, stmt, &mut warnings);
    }
    Ok(warnings)
}

/// A statement-scoped [`RenderError::Invalid`] builder.
fn invalid(idx: usize, field: &str, message: impl Into<String>) -> RenderError {
    RenderError::Invalid {
        section: "iam".to_string(),
        field: format!("statements[{idx}].{field}"),
        message: message.into(),
    }
}

/// Rules 2 + 3: effect must be canonical; actions/resources must be non-empty.
fn check_effect_and_shape(idx: usize, stmt: &IamStatement) -> Result<(), RenderError> {
    if stmt.effect != "Allow" && stmt.effect != "Deny" {
        return Err(invalid(
            idx,
            "effect",
            format!("effect must be 'Allow' or 'Deny', got '{}'", stmt.effect),
        ));
    }
    if stmt.actions.is_empty() {
        return Err(invalid(idx, "actions", "actions must not be empty"));
    }
    if stmt.resources.is_empty() {
        return Err(invalid(idx, "resources", "resources must not be empty"));
    }
    Ok(())
}

/// Rule 1: reject `Allow` with `actions=["*"]` and `resources=["*"]`.
fn check_wildcard_escalation(idx: usize, stmt: &IamStatement) -> Result<(), RenderError> {
    let is_wildcard_allow = stmt.effect == "Allow"
        && stmt.actions.len() == 1
        && stmt.actions[0] == "*"
        && stmt.resources.len() == 1
        && stmt.resources[0] == "*";
    if is_wildcard_allow {
        return Err(invalid(
            idx,
            "actions",
            "Allow + actions=[\"*\"] + resources=[\"*\"] is a wildcard escalation footgun — \
             refuse to deploy. Tighten actions and resources.",
        ));
    }
    Ok(())
}

/// Rule 4: every action matches `^[a-z0-9-]+:[A-Za-z0-9*]+$`. Warning 7:
/// unknown prefix.
fn check_actions(
    idx: usize,
    stmt: &IamStatement,
    warnings: &mut Vec<Warning>,
) -> Result<(), RenderError> {
    for a in &stmt.actions {
        // `*` alone is allowed so `actions = ["*"]` with a tightened
        // `resources` list remains declarable (Rule 1 already rejects `*`+`*`).
        if a == "*" {
            continue;
        }
        if !is_valid_action_shape(a) {
            return Err(invalid(
                idx,
                "actions",
                format!("action '{a}' does not match ^[a-z0-9-]+:[A-Za-z0-9*]+$"),
            ));
        }
        if let Some((prefix, _)) = a.split_once(':') {
            if !KNOWN_SERVICE_PREFIXES.contains(&prefix) {
                warnings.push(Warning::new(
                    "iam.unknown_service_prefix",
                    format!(
                        "[iam.statements][{idx}]: unknown service prefix '{prefix}' in action \
                         '{a}' — verify this is a valid AWS service"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// `^[a-z0-9-]+:[A-Za-z0-9*]+$` reimplemented without the `regex` crate —
/// this crate's purity discipline caps dependencies at exactly four
/// (`pmcp-package`/`serde`/`serde_json`/`semver`, see `lib.rs`'s module doc
/// comment); pulling in `regex` for one shape check isn't worth breaking
/// that invariant.
fn is_valid_action_shape(action: &str) -> bool {
    let Some((prefix, body)) = action.split_once(':') else {
        return false;
    };
    let prefix_ok = !prefix.is_empty()
        && prefix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let body_ok = !body.is_empty() && body.chars().all(|c| c.is_ascii_alphanumeric() || c == '*');
    prefix_ok && body_ok
}

/// Warning 8 (best-effort): flag resources that pin a specific 12-digit AWS
/// account. Advisory only — never produces an `Err`.
fn collect_cross_account_warnings(idx: usize, stmt: &IamStatement, warnings: &mut Vec<Warning>) {
    for r in &stmt.resources {
        if let Some(acct) = extract_account_from_arn(r) {
            if acct.len() == 12 && acct.chars().all(|c| c.is_ascii_digit()) {
                warnings.push(Warning::new(
                    "iam.cross_account_arn",
                    format!(
                        "[iam.statements][{idx}]: resource '{r}' pins a specific AWS account \
                         '{acct}' — verify this matches your deploy target (use '*' or omit the \
                         account segment for account-agnostic ARNs)"
                    ),
                ));
            }
        }
    }
}

/// Extract the `account` segment (index 4) of an ARN
/// `arn:partition:service:region:account:resource`. Returns `None` if the
/// input is not shaped like an ARN.
fn extract_account_from_arn(arn: &str) -> Option<&str> {
    let mut parts = arn.splitn(6, ':');
    let head = parts.next()?;
    if head != "arn" {
        return None;
    }
    let _partition = parts.next()?;
    let _service = parts.next()?;
    let _region = parts.next()?;
    let account = parts.next()?;
    let _resource = parts.next()?;
    Some(account)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> DeployDescriptor {
        descriptor_with_iam("")
    }

    fn descriptor_with_iam(iam_block: &str) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "pmcp-run"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "iam-test"
            timeout_seconds = 30
            [auth]
            enabled = false
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = true
            create_dashboard = true
            {iam_block}
            "#
        ))
        .expect("fixture descriptor parses")
    }

    fn params() -> RenderParams {
        RenderParams::for_test("iam-test-stack")
    }

    #[test]
    fn render_execution_role_returns_role_then_policy() {
        let resources = render_execution_role(&descriptor(), &params());
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].0, logical_ids::for_execution_role());
        assert_eq!(resources[0].1.type_, "AWS::IAM::Role");
        assert_eq!(resources[1].0, logical_ids::for_execution_policy());
        assert_eq!(resources[1].1.type_, "AWS::IAM::Policy");
    }

    #[test]
    fn policy_uses_cfn_pseudo_params_for_the_family_internal_dynamodb_arns() {
        // T7 review fix: family-internal ARNs (always same-account/region as
        // the stack itself) use CFN pseudo-parameters, not a literal
        // `RenderParams::account_id`/`region` — see
        // `family_internal_pseudo_param_arn`'s doc comment. `params()` sets
        // `account_id`/`region` to non-default values on purpose here (this
        // test would have caught a regression back to literal interpolation
        // if it still baked them in).
        let resources = render_execution_role(&descriptor(), &params());
        let policy_doc = &resources[1].1.properties["PolicyDocument"]["Statement"][1];
        assert_eq!(
            policy_doc["Resource"][0],
            json!({"Fn::Sub": "arn:aws:dynamodb:${AWS::Region}:${AWS::AccountId}:table/McpServer"})
        );
        assert_eq!(
            policy_doc["Resource"][1],
            json!({"Fn::Sub": "arn:aws:dynamodb:${AWS::Region}:${AWS::AccountId}:table/McpServer/*"})
        );
    }

    #[test]
    fn policy_uses_cfn_pseudo_params_for_the_family_internal_lambda_invoke_arn() {
        let resources = render_execution_role(&descriptor(), &params());
        let policy_doc = &resources[1].1.properties["PolicyDocument"]["Statement"][2];
        assert_eq!(
            policy_doc["Resource"],
            json!({"Fn::Sub": "arn:aws:lambda:${AWS::Region}:${AWS::AccountId}:function:*"})
        );
    }

    #[test]
    fn role_tags_carry_the_server_name() {
        let resources = render_execution_role(&descriptor(), &params());
        assert_eq!(
            resources[0].1.properties["Tags"],
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "hosting" },
                { "Key": "service", "Value": "iam-test" },
                { "Key": "target", "Value": "pmcp-run" },
            ])
        );
    }

    #[test]
    fn no_declared_iam_leaves_exactly_the_three_base_statements() {
        let resources = render_execution_role(&descriptor(), &params());
        let statements = resources[1].1.properties["PolicyDocument"]["Statement"]
            .as_array()
            .unwrap();
        assert_eq!(statements.len(), 3);
    }

    #[test]
    fn declared_statements_append_after_the_base_three_in_order() {
        let d = descriptor_with_iam(
            r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["secretsmanager:GetSecretValue"]
            resources = ["arn:aws:secretsmanager:us-east-1:*:secret:foo-*"]

            [[iam.statements]]
            effect = "Deny"
            actions = ["s3:DeleteObject"]
            resources = ["arn:aws:s3:::protected/*"]
            "#,
        );
        let resources = render_execution_role(&d, &params());
        let statements = resources[1].1.properties["PolicyDocument"]["Statement"]
            .as_array()
            .unwrap();
        assert_eq!(statements.len(), 5);
        assert_eq!(statements[3]["Effect"], "Allow");
        assert_eq!(statements[3]["Action"], "secretsmanager:GetSecretValue");
        assert_eq!(
            statements[3]["Resource"],
            "arn:aws:secretsmanager:us-east-1:*:secret:foo-*"
        );
        assert_eq!(statements[4]["Effect"], "Deny");
        assert_eq!(statements[4]["Action"], "s3:DeleteObject");
    }

    #[test]
    fn single_item_actions_and_resources_collapse_to_a_scalar() {
        let d = descriptor_with_iam(
            r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["dynamodb:GetItem"]
            resources = ["arn:aws:dynamodb:us-east-1:*:table/foo"]
            "#,
        );
        let resources = render_execution_role(&d, &params());
        let stmt = &resources[1].1.properties["PolicyDocument"]["Statement"][3];
        assert!(stmt["Action"].is_string(), "expected scalar, got {stmt}");
        assert!(stmt["Resource"].is_string(), "expected scalar, got {stmt}");
    }

    #[test]
    fn multi_item_actions_and_resources_stay_arrays() {
        let d = descriptor_with_iam(
            r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["dynamodb:GetItem", "dynamodb:Query"]
            resources = ["arn:aws:dynamodb:us-east-1:*:table/a", "arn:aws:dynamodb:us-east-1:*:table/b"]
            "#,
        );
        let resources = render_execution_role(&d, &params());
        let stmt = &resources[1].1.properties["PolicyDocument"]["Statement"][3];
        assert!(stmt["Action"].is_array(), "expected array, got {stmt}");
        assert!(stmt["Resource"].is_array(), "expected array, got {stmt}");
    }

    fn aws_lambda_descriptor() -> DeployDescriptor {
        aws_lambda_descriptor_with_iam("")
    }

    fn aws_lambda_descriptor_with_iam(iam_block: &str) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "aws-lambda"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "http-api-test"
            timeout_seconds = 30
            [auth]
            enabled = false
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = true
            create_dashboard = true
            {iam_block}
            "#
        ))
        .expect("fixture descriptor parses")
    }

    #[test]
    fn aws_lambda_execution_role_returns_role_then_policy() {
        let resources = render_execution_role_aws_lambda(&aws_lambda_descriptor());
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].0, logical_ids::for_execution_role());
        assert_eq!(resources[0].1.type_, "AWS::IAM::Role");
        assert_eq!(resources[1].0, logical_ids::for_execution_policy());
        assert_eq!(resources[1].1.type_, "AWS::IAM::Policy");
    }

    #[test]
    fn aws_lambda_role_tags_carry_the_server_name_as_project_and_aws_lambda_as_target() {
        let resources = render_execution_role_aws_lambda(&aws_lambda_descriptor());
        assert_eq!(
            resources[0].1.properties["Tags"],
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "http-api-test" },
                { "Key": "service", "Value": "http-api-test" },
                { "Key": "target", "Value": "aws-lambda" },
            ])
        );
    }

    #[test]
    fn aws_lambda_no_declared_iam_leaves_exactly_the_one_xray_base_statement() {
        let resources = render_execution_role_aws_lambda(&aws_lambda_descriptor());
        let statements = resources[1].1.properties["PolicyDocument"]["Statement"]
            .as_array()
            .unwrap();
        assert_eq!(
            statements.len(),
            1,
            "expected xray-only, got {statements:?}"
        );
        assert_eq!(statements[0]["Action"][0], "xray:PutTraceSegments");
    }

    #[test]
    fn aws_lambda_declared_statements_append_after_the_single_base_statement() {
        let d = aws_lambda_descriptor_with_iam(
            r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["secretsmanager:GetSecretValue"]
            resources = ["arn:aws:secretsmanager:us-east-1:*:secret:foo-*"]
            "#,
        );
        let resources = render_execution_role_aws_lambda(&d);
        let statements = resources[1].1.properties["PolicyDocument"]["Statement"]
            .as_array()
            .unwrap();
        assert_eq!(statements.len(), 2);
        assert_eq!(statements[1]["Action"], "secretsmanager:GetSecretValue");
    }
}

#[cfg(test)]
mod validate_tests {
    //! Ported (statements-only) from
    //! `cargo-pmcp/src/deployment/iam.rs`'s `validate_tests` module — same
    //! hard-error/warning rules, adapted to `pmcp_package::package::{IamSection,
    //! IamStatement}` and this crate's `RenderError`/`Warning` types. Rules
    //! about `[[iam.tables]]`/`[[iam.buckets]]` sugar (cargo-pmcp's rules 5-6)
    //! are intentionally NOT ported — see the module doc comment.

    use super::*;

    fn one_stmt(effect: &str, actions: Vec<&str>, resources: Vec<&str>) -> IamSection {
        IamSection {
            statements: vec![IamStatement {
                effect: effect.to_string(),
                actions: actions.into_iter().map(String::from).collect(),
                resources: resources.into_iter().map(String::from).collect(),
            }],
        }
    }

    #[test]
    fn empty_config_is_valid() {
        let w = validate(&IamSection::default()).expect("valid");
        assert!(w.is_empty());
    }

    #[test]
    fn allow_star_star_is_hard_error() {
        let iam = one_stmt("Allow", vec!["*"], vec!["*"]);
        let err = validate(&iam).expect_err("wildcard escalation must fail");
        assert!(
            err.to_string().contains("wildcard escalation"),
            "expected wildcard escalation message, got: {err}"
        );
    }

    #[test]
    fn unknown_effect_is_error() {
        let iam = one_stmt("Permit", vec!["s3:GetObject"], vec!["*"]);
        let err = validate(&iam).expect_err("bad effect must fail");
        assert!(err.to_string().contains("effect"));
    }

    #[test]
    fn empty_actions_is_error() {
        let iam = one_stmt("Allow", vec![], vec!["*"]);
        validate(&iam).expect_err("empty actions must fail");
    }

    #[test]
    fn empty_resources_is_error() {
        let iam = one_stmt("Allow", vec!["s3:GetObject"], vec![]);
        validate(&iam).expect_err("empty resources must fail");
    }

    #[test]
    fn malformed_action_uppercase_prefix_is_error() {
        let iam = one_stmt("Allow", vec!["DynamoDB:getitem"], vec!["*"]);
        validate(&iam).expect_err("bad action casing must fail");
    }

    #[test]
    fn underscore_in_action_prefix_is_error() {
        let iam = one_stmt("Allow", vec!["foo_bar:GetThing"], vec!["*"]);
        validate(&iam).expect_err("underscore in service prefix must fail");
    }

    #[test]
    fn action_with_extra_colon_is_error() {
        let iam = one_stmt("Allow", vec!["s3:Get:Object"], vec!["*"]);
        validate(&iam).expect_err("extra colon must fail");
    }

    #[test]
    fn action_missing_colon_is_error() {
        let iam = one_stmt("Allow", vec!["s3GetObject"], vec!["*"]);
        validate(&iam).expect_err("missing colon must fail");
    }

    #[test]
    fn unknown_service_prefix_is_warning_not_error() {
        let iam = one_stmt("Allow", vec!["totallyfake:DoThing"], vec!["*"]);
        let warnings = validate(&iam).expect("warnings only, no hard error");
        assert!(
            warnings.iter().any(
                |w| w.code == "iam.unknown_service_prefix" && w.message.contains("totallyfake")
            ),
            "expected warning about 'totallyfake' prefix, got: {warnings:?}"
        );
    }

    #[test]
    fn cross_account_arn_does_not_hard_error() {
        // Cross-account detection is advisory, not a gate. Best-effort parser
        // may or may not emit a warning for a given shape — test documents
        // behaviour without over-specifying the warning class.
        let iam = one_stmt(
            "Allow",
            vec!["s3:GetObject"],
            vec!["arn:aws:s3:::bucket/object:999999999999:foo"],
        );
        let warnings = validate(&iam).expect("not a hard error");
        assert!(
            warnings.iter().all(|w| w.code != "iam.wildcard_escalation"),
            "no wildcard spam expected"
        );
    }

    #[test]
    fn cross_account_pinned_account_warns() {
        let iam = one_stmt(
            "Allow",
            vec!["dynamodb:GetItem"],
            vec!["arn:aws:dynamodb:us-east-1:999999999999:table/foo"],
        );
        let warnings = validate(&iam).expect("valid, warning only");
        assert!(
            warnings
                .iter()
                .any(|w| w.code == "iam.cross_account_arn" && w.message.contains("999999999999")),
            "expected cross-account warning, got: {warnings:?}"
        );
    }

    #[test]
    fn typical_multi_statement_config_is_valid_without_warnings() {
        let iam = IamSection {
            statements: vec![
                IamStatement {
                    effect: "Allow".to_string(),
                    actions: vec!["dynamodb:GetItem".to_string(), "dynamodb:Query".to_string()],
                    resources: vec!["arn:aws:dynamodb:us-east-1:*:table/foo".to_string()],
                },
                IamStatement {
                    effect: "Allow".to_string(),
                    actions: vec!["secretsmanager:GetSecretValue".to_string()],
                    resources: vec![
                        "arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*".to_string()
                    ],
                },
            ],
        };
        let warnings = validate(&iam).expect("config must validate");
        assert!(
            warnings.is_empty(),
            "config emitted unexpected warnings: {warnings:?}"
        );
    }

    #[test]
    fn public_api_warning_is_constructable_and_clonable() {
        // Compile-time + runtime sanity: Warning is Clone + Debug + Eq.
        let iam = one_stmt("Allow", vec!["notaservice:Foo"], vec!["*"]);
        let warnings = validate(&iam).expect("ok");
        let first = warnings.first().cloned();
        let _debug_repr = format!("{first:?}");
        assert!(first.is_some());
        assert_eq!(first.clone(), first);
    }

    #[test]
    fn wildcard_action_alone_is_allowed_without_error_or_warning() {
        // `actions = ["*"]` with a tightened, non-"*" resources list is
        // declarable — only Allow+*+* (both wildcarded) is the hard-error
        // escalation footgun (rule 1).
        let iam = one_stmt("Allow", vec!["*"], vec!["arn:aws:s3:::specific-bucket/*"]);
        let warnings = validate(&iam).expect("single wildcard action must not error");
        assert!(
            warnings.is_empty(),
            "wildcard action alone should not warn, got: {warnings:?}"
        );
    }
}
