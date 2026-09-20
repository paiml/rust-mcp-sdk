//! # pmcp-cfn-renderer
//!
//! A pure, deterministic `DeployDescriptor` -> CloudFormation renderer for
//! PMCP MCP servers.
//!
//! ## Purity discipline
//!
//! This crate has exactly four dependencies: `pmcp-package` (the descriptor
//! types), `serde`, `serde_json`, and `semver`. It contains **no** AWS SDK,
//! no `tokio`, no `reqwest`, no filesystem access, and no network access —
//! [`render`] is a pure function of its two arguments. This is deliberate:
//! the crate is trust-kernel code run by both `cargo-pmcp` (the CLI) and
//! hosting platforms (e.g. `pmcp.run`), and both must derive byte-identical
//! CloudFormation from the same descriptor.
//!
//! ## Identity/environment split
//!
//! [`render`]'s signature enforces the split: everything about *what* server
//! to deploy comes from `pmcp_package::package::DeployDescriptor` (the
//! closed-set, environment-independent identity contract); everything about
//! *where*/*how* it is deployed this time (account, region, artifact
//! location, resolved environment values, synth-time metadata) comes from
//! [`RenderParams`]. The descriptor can never smuggle an environment-specific
//! value into template identity.
//!
//! ## Determinism
//!
//! [`CfnTemplate::to_canonical_json`] is byte-deterministic: identical
//! inputs to [`render`] always produce an identical canonical JSON string,
//! across runs and platforms. See [`template`]'s module documentation for
//! how this is enforced structurally (via `BTreeMap` fields) rather than by
//! an ad hoc sort step.
//!
//! ## Resource surface
//!
//! The v1 resource surface is exactly seven allowlisted families —
//! `lambda`, `iam`, `logs`, `http_api`, `cognito`, `dynamodb`, `outputs` (see
//! [`resources`]). A descriptor requesting anything outside that surface
//! fails loudly via [`RenderError::UnsupportedSection`], never a silent
//! skip. As of Task 4, [`render`] wires `lambda`, `logs`, `outputs`, and the
//! full `iam` module — the BASE execution role/policy PLUS any declared
//! `[[iam.statements]]` expansion, fail-closed validated first (see
//! [`resources::iam`]'s doc comment) — for the `pmcp-run` target. As of
//! Task 5, [`render`] also wires the `aws-lambda` target: the same 4
//! resource families, but rendered by `*_aws_lambda` siblings with that
//! target's own fixed constants (own tags, no composition IAM sugar,
//! different memory size/log retention — see e.g.
//! [`resources::lambda::render_function_aws_lambda`]'s doc comment), PLUS
//! `http_api` (`AWS::ApiGatewayV2::*` + the invoke `Lambda::Permission`).
//! As of Task 6, [`render`] also wires the `aws-lambda` target's
//! Cognito+DCR OAuth stack shape: when `auth.enabled = true` AND
//! `auth.provider = "cognito"`, [`resources::cognito::render`] builds the
//! full resource graph (3 Lambda functions, the Cognito
//! `UserPool`/`ResourceServer`/`Domain`, the DCR `ClientsTable` via
//! `dynamodb::render_table`, and a JWT-authorizer-protected 7-route HTTP
//! API — see that module's doc comment). This completes the v1
//! seven-family resource surface. Any target type other than
//! `pmcp-run`/`aws-lambda`, `auth.enabled = true` on the `pmcp-run` target
//! (its own OAuth stack shape is not yet rendered), and `auth.enabled =
//! true` on `aws-lambda` with a provider other than `"cognito"` still fail
//! loudly — the first two via [`RenderError::UnsupportedSection`], the last
//! via [`RenderError::Invalid`] naming the unsupported provider — rather
//! than silently rendering an incomplete stack. A declared
//! `[[iam.statements]]` entry that fails [`resources::iam::validate`]'s
//! fail-closed rules also fails loudly via [`RenderError::Invalid`].
//!
//! ## Logical IDs
//!
//! Logical IDs are derived directly from descriptor names via a documented,
//! hash-free transform — see [`logical_ids`].

pub mod error;
pub mod logical_ids;
pub mod params;
pub mod resources;
pub mod template;

// ---------------------------------------------------------------------
// Crate-root re-exports (so callers can write `pmcp_cfn_renderer::RenderParams`
// rather than `pmcp_cfn_renderer::params::RenderParams`). Deep module paths
// still work — this is additive, not a replacement for the module tree.
// ---------------------------------------------------------------------

pub use error::RenderError;
pub use params::{ArtifactRef, RenderParams, RuntimeAdapterConfig};
pub use template::{CfnExport, CfnOutput, CfnResource, CfnTemplate};

use pmcp_package::package::DeployDescriptor;
use std::collections::BTreeMap;

/// Render a `DeployDescriptor` + [`RenderParams`] into a deterministic
/// CloudFormation template.
///
/// # Errors
///
/// Returns [`RenderError::UnsupportedSection`] when the descriptor requests
/// something the renderer doesn't implement yet: a `[target].type` other
/// than `"pmcp-run"`/`"aws-lambda"` (the only two stack shapes wired so far
/// — `cognito` lands in a later task) or `auth.enabled = true` (needs the
/// `cognito` module). Returns [`RenderError::Invalid`] when a declared
/// `[[iam.statements]]` entry fails [`resources::iam::validate`]'s
/// fail-closed rules (bad effect, empty actions/resources, malformed
/// action, or a wildcard-escalation footgun — see that function's doc
/// comment). Never silently skips descriptor content.
///
/// # Current behavior (Task 5)
///
/// For a `pmcp-run`-target descriptor with `auth.enabled = false`, renders
/// the plain-Lambda kernel: the function, its log group, its base execution
/// role + default inline policy (with any validated `[[iam.statements]]`
/// appended), and the fixed five-output set (see [`resources`]).
///
/// For an `aws-lambda`-target descriptor with `auth.enabled = false`,
/// renders that target's own kernel (same function/log-group/role/policy
/// shape, `aws-lambda`-flavored constants) PLUS the `http_api` resource
/// family (`AWS::ApiGatewayV2::*` + the invoke `Lambda::Permission`) and
/// the corresponding four-output set (no `LambdaName`, and `ApiUrl` points
/// at this stack's own API instead of the fixed pmcp.run edge URL).
pub fn render(
    descriptor: &DeployDescriptor,
    params: &RenderParams,
) -> Result<CfnTemplate, RenderError> {
    let target = resolve_target(descriptor)?;
    guard_auth_support(descriptor, target)?;
    if let Some(iam) = &descriptor.iam {
        // Discard warnings here — `render` is the pure, side-effect-free
        // entry point (no I/O to print them through). Hard errors still
        // fail loudly via `?`. Callers that want the advisory findings
        // (unknown service prefix, cross-account ARN pin) call
        // `resources::iam::validate` directly, same as `render` does —
        // it's a fully public, separately-callable function.
        resources::iam::validate(iam)?;
    }
    // Same discard-here, call-directly-for-warnings pattern as
    // `resources::iam::validate` above — `resources::cognito::validate` is
    // infallible (advisory-only, no hard-error rules), so there is no `?`
    // to propagate.
    let _ = resources::cognito::validate(&descriptor.auth);

    match target {
        RenderTarget::AwsLambda if descriptor.auth.enabled => {
            render_aws_lambda_oauth(descriptor, params)
        },
        RenderTarget::AwsLambda => render_aws_lambda(descriptor, params),
        RenderTarget::PmcpRun => render_pmcp_run(descriptor, params),
    }
}

/// The `pmcp-run` target's plain-Lambda kernel (Task 3/4) — unchanged by
/// Task 5.
fn render_pmcp_run(
    descriptor: &DeployDescriptor,
    params: &RenderParams,
) -> Result<CfnTemplate, RenderError> {
    let mut resources = BTreeMap::new();
    for (id, resource) in resources::iam::render_execution_role(descriptor, params) {
        resources.insert(id, resource);
    }
    let (function_id, function) = resources::lambda::render_function(descriptor, params);
    resources.insert(function_id, function);
    let (log_group_id, log_group) =
        resources::logs::render_log_group(&descriptor.server.name, PLAIN_LOG_RETENTION_DAYS);
    resources.insert(log_group_id, log_group);

    let outputs = resources::outputs::render_outputs(descriptor, params);

    Ok(CfnTemplate {
        description: format!("MCP Server: {}", descriptor.server.name),
        resources,
        outputs,
        metadata: params.cloudformation_metadata.clone(),
    })
}

/// The `aws-lambda` target's own kernel (Task 5): function/log-group/role/
/// policy via the `*_aws_lambda` siblings, plus the `http_api` resource
/// family.
fn render_aws_lambda(
    descriptor: &DeployDescriptor,
    params: &RenderParams,
) -> Result<CfnTemplate, RenderError> {
    let mut resources = BTreeMap::new();
    for (id, resource) in resources::iam::render_execution_role_aws_lambda(descriptor) {
        resources.insert(id, resource);
    }
    let (function_id, function) = resources::lambda::render_function_aws_lambda(descriptor, params);
    resources.insert(function_id.clone(), function);
    let (log_group_id, log_group) = resources::logs::render_log_group_aws_lambda(
        &descriptor.server.name,
        AWS_LAMBDA_LOG_RETENTION_DAYS,
    );
    resources.insert(log_group_id, log_group);

    for (id, resource) in resources::http_api::render(descriptor, params, &function_id)? {
        resources.insert(id, resource);
    }

    let outputs = resources::outputs::render_http_api_outputs(
        descriptor,
        params,
        logical_ids::for_http_api(),
    );

    Ok(CfnTemplate {
        description: format!("MCP Server: {}", descriptor.server.name),
        resources,
        outputs,
        metadata: params.cloudformation_metadata.clone(),
    })
}

/// The `aws-lambda` target's Cognito+DCR OAuth stack shape (Task 6):
/// [`resources::cognito::render`] builds the full resource graph (3 Lambda
/// functions + roles/policies/log groups, the Cognito
/// `UserPool`/`ResourceServer`/`Domain`, the DCR `ClientsTable`, and the
/// HTTP API + JWT-authorizer wiring) in one call — unlike
/// [`render_aws_lambda`], this shape needs no separate `lambda`/`iam`/
/// `logs`/`http_api` calls here because `cognito::render` already composes
/// them internally.
fn render_aws_lambda_oauth(
    descriptor: &DeployDescriptor,
    params: &RenderParams,
) -> Result<CfnTemplate, RenderError> {
    let mut resources = BTreeMap::new();
    for (id, resource) in resources::cognito::render(descriptor, params)? {
        resources.insert(id, resource);
    }

    let outputs = resources::outputs::render_cognito_outputs(descriptor, params);

    Ok(CfnTemplate {
        description: format!("MCP Server: {}", descriptor.server.name),
        resources,
        outputs,
        metadata: params.cloudformation_metadata.clone(),
    })
}

/// Log-retention days for the `pmcp-run` plain-Lambda kernel. NOT driven by
/// `[observability].log_retention_days` — the TS scaffold's `pmcp-run`
/// branch hardcodes `RetentionDays.ONE_WEEK` unconditionally (as it does
/// for X-Ray tracing and the console-link `DashboardUrl` output); the
/// `[observability]` section is not wired into this stack shape today.
const PLAIN_LOG_RETENTION_DAYS: u32 = 7;

/// Log-retention days for the `aws-lambda` target's kernel (including its
/// Cognito+DCR OAuth stack shape, Task 6) — the TS scaffold's `aws-lambda`
/// branches hardcode `RetentionDays.ONE_MONTH` (vs. `pmcp-run`'s
/// [`PLAIN_LOG_RETENTION_DAYS`] = 7). Also not driven by
/// `[observability].log_retention_days`. `pub(crate)` so
/// `resources::cognito` can reuse the same constant for its 3 log groups.
pub(crate) const AWS_LAMBDA_LOG_RETENTION_DAYS: u32 = 30;

/// The renderer's exactly-two supported deploy targets — the closed set
/// [`resolve_target`] validates `descriptor.target.target_type` against.
/// Resolved once per [`render`] call, then matched exhaustively instead of
/// re-checking the raw string at each dispatch point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderTarget {
    PmcpRun,
    AwsLambda,
}

/// Resolve `d.target.target_type` into the closed [`RenderTarget`] set, or
/// fail loudly on anything else — rather than silently producing an
/// incomplete or wrong stack. Replaces what used to be a pair of ad hoc
/// string comparisons (one in this validation, one in [`render`]'s own
/// dispatch) with a single string check whose result both call sites share.
fn resolve_target(d: &DeployDescriptor) -> Result<RenderTarget, RenderError> {
    match d.target.target_type.as_str() {
        "pmcp-run" => Ok(RenderTarget::PmcpRun),
        "aws-lambda" => Ok(RenderTarget::AwsLambda),
        other => Err(RenderError::UnsupportedSection {
            section: "target".to_string(),
            detail: format!(
                "target type '{other}' is not yet rendered (only 'pmcp-run' plain-Lambda and \
                 'aws-lambda' HTTP API rendering are implemented)"
            ),
        }),
    }
}

/// Fail loudly when `d.auth.enabled` is set on a target that doesn't
/// implement OAuth rendering yet.
///
/// `auth.enabled = true` is implemented only for the `aws-lambda` target's
/// Cognito OAuth+DCR stack shape (Task 6) — the `pmcp-run` target's own
/// OAuth rendering isn't proven by any golden yet, so it stays guarded
/// regardless of `auth.provider`. Provider validation for the `aws-lambda`
/// case happens inside `resources::cognito::render` (a bad provider fails
/// via `RenderError::Invalid`, not this guard).
fn guard_auth_support(d: &DeployDescriptor, target: RenderTarget) -> Result<(), RenderError> {
    if d.auth.enabled && target != RenderTarget::AwsLambda {
        return Err(RenderError::UnsupportedSection {
            section: "auth".to_string(),
            detail: "auth.enabled = true is only implemented for the aws-lambda target's \
                     cognito OAuth+DCR stack shape; the pmcp-run target's OAuth rendering \
                     is not yet implemented"
                .to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal, valid `pmcp-run` descriptor: `auth.enabled = false`, no
    /// `[iam]` section — the shape [`render`] fully supports today.
    fn descriptor(name: &str) -> DeployDescriptor {
        descriptor_with(name, "pmcp-run", false, "")
    }

    fn descriptor_with(
        name: &str,
        target_type: &str,
        auth_enabled: bool,
        iam_block: &str,
    ) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "{target_type}"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "{name}"
            timeout_seconds = 30
            [auth]
            enabled = {auth_enabled}
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = false
            create_dashboard = false
            {iam_block}
            "#
        ))
        .expect("fixture descriptor parses")
    }

    fn params() -> RenderParams {
        RenderParams::for_test("unit-test-stack")
    }

    #[test]
    fn render_description_names_the_server() {
        let template = render(&descriptor("my-server"), &params()).unwrap();
        assert_eq!(template.description, "MCP Server: my-server");
    }

    #[test]
    fn render_produces_the_plain_lambda_resource_kernel() {
        let template = render(&descriptor("my-server"), &params()).unwrap();
        let mut resource_ids: Vec<&String> = template.resources.keys().collect();
        resource_ids.sort();
        assert_eq!(
            resource_ids,
            vec![
                "ExecutionRole",
                "ExecutionRoleDefaultPolicy",
                "LogGroup",
                "McpFunction"
            ]
        );
        assert_eq!(template.outputs.len(), 5);
        assert!(template.metadata.is_empty());
    }

    /// T7 review fix: `render`'s `pmcp-run` path must carry
    /// `RenderParams::cloudformation_metadata` into `CfnTemplate::metadata`
    /// byte-for-byte — no parsing/re-deriving, since `render` treats it as
    /// opaque passthrough content (see `RenderParams::cloudformation_metadata`'s
    /// doc comment). Before this fix, EVERY render path always emitted an
    /// empty `BTreeMap`, discarding this field entirely.
    #[test]
    fn render_pmcp_run_emits_cloudformation_metadata_verbatim() {
        let mut params = params();
        params.cloudformation_metadata = BTreeMap::from([
            ("mcp:version".to_string(), serde_json::json!("1.0.0")),
            ("mcp:serverType".to_string(), serde_json::json!("custom")),
            (
                "mcp:resources".to_string(),
                serde_json::json!({"secrets": ["API_TOKEN"]}),
            ),
        ]);
        let template = render(&descriptor("my-server"), &params).unwrap();
        assert_eq!(template.metadata, params.cloudformation_metadata);
    }

    /// Same verbatim-carriage contract as
    /// [`render_pmcp_run_emits_cloudformation_metadata_verbatim`], for the
    /// `aws-lambda` target's own kernel (`render_aws_lambda`) — a separate
    /// `Ok(CfnTemplate { .. })` construction site, so it needs its own
    /// coverage rather than relying on the `pmcp-run` case alone.
    #[test]
    fn render_aws_lambda_emits_cloudformation_metadata_verbatim() {
        let mut params = params();
        params.cloudformation_metadata =
            BTreeMap::from([("mcp:serverId".to_string(), serde_json::json!("srv-1"))]);
        let template = render(
            &descriptor_with("my-server", "aws-lambda", false, ""),
            &params,
        )
        .unwrap();
        assert_eq!(template.metadata, params.cloudformation_metadata);
    }

    /// Same verbatim-carriage contract, for the `aws-lambda` target's
    /// Cognito+DCR OAuth stack shape (`render_aws_lambda_oauth`) — the third
    /// and last `Ok(CfnTemplate { .. })` construction site in this module.
    #[test]
    fn render_cognito_oauth_emits_cloudformation_metadata_verbatim() {
        let mut params = params();
        params.cloudformation_metadata = BTreeMap::from([(
            "mcp:capabilities".to_string(),
            serde_json::json!({"tools": ["search"]}),
        )]);
        let template = render(&cognito_oauth_descriptor("oauth-test"), &params).unwrap();
        assert_eq!(template.metadata, params.cloudformation_metadata);
    }

    /// The "omit `Metadata` when empty" envelope rule ([`CfnTemplate::metadata`]'s
    /// `skip_serializing_if`) still applies end-to-end through `render` —
    /// an empty `RenderParams::cloudformation_metadata` (the default, and
    /// every pre-T7-review caller's behavior) produces no `"Metadata"` key
    /// in the canonical JSON at all, not an empty `{}` block.
    #[test]
    fn render_omits_the_metadata_key_when_cloudformation_metadata_is_empty() {
        let template = render(&descriptor("my-server"), &params()).unwrap();
        assert!(template.metadata.is_empty());
        assert!(!template.to_canonical_json().contains("\"Metadata\""));
    }

    #[test]
    fn render_rejects_a_target_type_other_than_pmcp_run_or_aws_lambda() {
        let err = render(
            &descriptor_with("my-server", "google-cloud-run", false, ""),
            &params(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RenderError::UnsupportedSection {
                section: "target".to_string(),
                detail: "target type 'google-cloud-run' is not yet rendered (only \
                          'pmcp-run' plain-Lambda and 'aws-lambda' HTTP API rendering \
                          are implemented)"
                    .to_string(),
            }
        );
    }

    #[test]
    fn render_rejects_auth_enabled_on_pmcp_run() {
        // The `aws-lambda` target's Cognito OAuth+DCR stack shape landed in
        // Task 6 (see `render_accepts_cognito_oauth_on_aws_lambda` below) —
        // the `pmcp-run` target's own OAuth rendering is not proven by any
        // golden yet, so it stays guarded.
        let err = render(
            &descriptor_with("my-server", "pmcp-run", true, ""),
            &params(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RenderError::UnsupportedSection {
                section: "auth".to_string(),
                detail: "auth.enabled = true is only implemented for the aws-lambda target's \
                          cognito OAuth+DCR stack shape; the pmcp-run target's OAuth rendering \
                          is not yet implemented"
                    .to_string(),
            }
        );
    }

    #[test]
    fn render_rejects_unsupported_auth_provider_on_aws_lambda() {
        // `descriptor_with`'s `[auth]` block always sets `provider = "none"`
        // — Task 6 wires ONLY the `"cognito"` flavor on `aws-lambda`; any
        // other provider must fail loudly naming itself, not silently
        // render an incomplete/wrong OAuth stack.
        let err = render(
            &descriptor_with("my-server", "aws-lambda", true, ""),
            &params(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RenderError::Invalid {
                section: "auth".to_string(),
                field: "provider".to_string(),
                message: "auth.enabled = true on the aws-lambda target requires provider = \
                          \"cognito\" (got \"none\") — no other OAuth provider is implemented yet"
                    .to_string(),
            }
        );
    }

    #[test]
    fn render_accepts_cognito_oauth_on_aws_lambda() {
        let template = render(&cognito_oauth_descriptor("oauth-test"), &params()).unwrap();
        assert!(
            template.resources.contains_key("UserPool"),
            "expected a UserPool resource, got {:?}",
            template.resources.keys().collect::<Vec<_>>()
        );
        assert!(template.outputs.contains_key("UserPoolId"));
        assert!(!template.outputs.contains_key("LambdaArn"));
    }

    /// A minimal `aws-lambda` + Cognito-OAuth descriptor — the shape
    /// [`render_accepts_cognito_oauth_on_aws_lambda`] exercises. Full
    /// resource-graph coverage lives in `resources::cognito`'s own tests
    /// and the `oauth-cognito-dcr` golden.
    fn cognito_oauth_descriptor(name: &str) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "aws-lambda"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "{name}"
            timeout_seconds = 30
            [auth]
            enabled = true
            provider = "cognito"
            callback_urls = []
            [auth.cognito]
            user_pool_name = "{name}-users"
            resource_server_id = "mcp"
            social_providers = []
            mfa = "optional"
            access_token_ttl = "1h"
            refresh_token_ttl = "30d"
            [auth.dcr]
            enabled = true
            [observability]
            log_retention_days = 30
            enable_xray = true
            create_dashboard = true
            "#
        ))
        .expect("fixture descriptor parses")
    }

    #[test]
    fn render_produces_the_http_api_resource_kernel_for_aws_lambda_target() {
        let template = render(
            &descriptor_with("my-server", "aws-lambda", false, ""),
            &params(),
        )
        .unwrap();
        let mut resource_ids: Vec<&String> = template.resources.keys().collect();
        resource_ids.sort();
        assert_eq!(
            resource_ids,
            vec![
                "ApiGatewayInvokePermission",
                "ExecutionRole",
                "ExecutionRoleDefaultPolicy",
                "HttpApi",
                "HttpApiDefaultStage",
                "HttpApiIntegration",
                "HttpApiRoute",
                "LogGroup",
                "McpFunction",
            ]
        );
        assert_eq!(template.outputs.len(), 4);
        assert!(template.metadata.is_empty());
    }

    #[test]
    fn render_aws_lambda_outputs_have_no_lambda_name_and_a_getatt_api_url() {
        let template = render(
            &descriptor_with("my-server", "aws-lambda", false, ""),
            &params(),
        )
        .unwrap();
        let mut names: Vec<&String> = template.outputs.keys().collect();
        names.sort();
        assert_eq!(
            names,
            vec!["ApiUrl", "DashboardUrl", "LambdaArn", "McpRoleArn"]
        );
        assert_eq!(
            template.outputs["ApiUrl"].value,
            serde_json::json!({ "Fn::GetAtt": ["HttpApi", "ApiEndpoint"] })
        );
    }

    #[test]
    fn render_aws_lambda_accepts_declared_iam_statements_after_the_single_xray_statement() {
        let iam_block = r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["s3:GetObject"]
            resources = ["arn:aws:s3:::example-bucket/*"]
            "#;
        let template = render(
            &descriptor_with("my-server", "aws-lambda", false, iam_block),
            &params(),
        )
        .expect("declared iam.statements render on the aws-lambda target too");
        let statements = template.resources["ExecutionRoleDefaultPolicy"].properties
            ["PolicyDocument"]["Statement"]
            .as_array()
            .unwrap();
        assert_eq!(statements.len(), 2, "1 xray base + 1 declared statement");
        assert_eq!(statements[1]["Action"], "s3:GetObject");
    }

    #[test]
    fn render_accepts_declared_iam_statements_and_appends_them_to_the_policy() {
        let iam_block = r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["s3:GetObject"]
            resources = ["arn:aws:s3:::example-bucket/*"]
            "#;
        let template = render(
            &descriptor_with("my-server", "pmcp-run", false, iam_block),
            &params(),
        )
        .expect("declared iam.statements now render, Task 4");
        // Still exactly the same 4-resource plain-Lambda kernel — the
        // declared statement lands inside ExecutionRoleDefaultPolicy's
        // existing Statement array, not a new resource.
        let mut resource_ids: Vec<&String> = template.resources.keys().collect();
        resource_ids.sort();
        assert_eq!(
            resource_ids,
            vec![
                "ExecutionRole",
                "ExecutionRoleDefaultPolicy",
                "LogGroup",
                "McpFunction"
            ]
        );
        let statements = template.resources["ExecutionRoleDefaultPolicy"].properties
            ["PolicyDocument"]["Statement"]
            .as_array()
            .unwrap();
        assert_eq!(statements.len(), 4, "3 base + 1 declared statement");
        assert_eq!(statements[3]["Action"], "s3:GetObject");
    }

    #[test]
    fn render_rejects_an_invalid_declared_iam_statement() {
        // Fail-closed: a wildcard-escalation statement must reject the
        // whole render, not silently drop the offending statement.
        let iam_block = r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["*"]
            resources = ["*"]
            "#;
        let err = render(
            &descriptor_with("my-server", "pmcp-run", false, iam_block),
            &params(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RenderError::Invalid {
                section: "iam".to_string(),
                field: "statements[0].actions".to_string(),
                message: "Allow + actions=[\"*\"] + resources=[\"*\"] is a wildcard escalation \
                          footgun — refuse to deploy. Tighten actions and resources."
                    .to_string(),
            }
        );
    }

    #[test]
    fn render_accepts_an_empty_declared_iam_section() {
        // `[iam]` present but with no `[[iam.statements]]` entries renders
        // identically to no `[iam]` section at all.
        let template = render(
            &descriptor_with("my-server", "pmcp-run", false, "[iam]"),
            &params(),
        )
        .unwrap();
        assert_eq!(template.resources.len(), 4);
    }
}
