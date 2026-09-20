//! `Outputs` rendering — endpoint URL, function identity, and the stable
//! cross-stack execution-role export. Names matter: `cargo-pmcp`'s
//! `deployment/outputs.rs::load_cdk_outputs` and the scaffold's own
//! `CfnOutput` calls (`commands/deploy/init.rs`) already read these exact
//! names, and Task 9's deploy engine depends on the byte-for-byte match.

use crate::{
    logical_ids,
    params::RenderParams,
    resources::cognito,
    template::{CfnExport, CfnOutput},
};
use pmcp_package::package::DeployDescriptor;
use serde_json::json;
use std::collections::BTreeMap;

/// Build the output entries shared by every renderer target:
/// `DashboardUrl`, `LambdaArn`, `McpRoleArn`. Callers add their own
/// `ApiUrl` (and, for the plain-Lambda kernel, `LambdaName`) on top.
fn common_outputs(d: &DeployDescriptor, p: &RenderParams) -> BTreeMap<String, CfnOutput> {
    let mut outputs = BTreeMap::new();
    outputs.insert(
        "DashboardUrl".to_string(),
        CfnOutput {
            description: Some("CloudWatch Console".to_string()),
            value: json!(format!(
                "https://console.aws.amazon.com/cloudwatch/home?region={}",
                p.region
            )),
            export: None,
        },
    );
    outputs.insert(
        "LambdaArn".to_string(),
        CfnOutput {
            description: Some("MCP Server Lambda ARN".to_string()),
            value: json!({ "Fn::GetAtt": [logical_ids::for_function(), "Arn"] }),
            export: None,
        },
    );
    outputs.insert(
        "McpRoleArn".to_string(),
        CfnOutput {
            description: Some(
                "MCP Server Lambda execution role ARN (stable export for downstream stacks)"
                    .to_string(),
            ),
            value: json!({ "Fn::GetAtt": [logical_ids::for_execution_role(), "Arn"] }),
            export: Some(CfnExport {
                name: format!("pmcp-{}-McpRoleArn", d.server.name),
            }),
        },
    );
    outputs
}

/// Render the plain-Lambda kernel's fixed output set: `ApiUrl`,
/// `DashboardUrl`, `LambdaArn`, `LambdaName`, `McpRoleArn`.
#[must_use]
pub fn render_outputs(d: &DeployDescriptor, p: &RenderParams) -> BTreeMap<String, CfnOutput> {
    let mut outputs = common_outputs(d, p);
    outputs.insert(
        "ApiUrl".to_string(),
        CfnOutput {
            description: Some("MCP endpoint (construct from deployment ID)".to_string()),
            value: json!("https://api.pmcp.run/{use-deployment-id}/mcp"),
            export: None,
        },
    );
    outputs.insert(
        "LambdaName".to_string(),
        CfnOutput {
            description: Some("MCP Server Lambda Name".to_string()),
            value: json!({ "Ref": logical_ids::for_function() }),
            export: None,
        },
    );
    outputs
}

/// Render the `aws-lambda` target's fixed output set: `ApiUrl` (this
/// stack's own HTTP API endpoint, via `Fn::GetAtt` on the rendered
/// `AWS::ApiGatewayV2::Api`'s `ApiEndpoint` attribute — unlike
/// [`render_outputs`]'s fixed pmcp.run edge URL), `DashboardUrl`,
/// `LambdaArn`, `McpRoleArn` — everything [`render_outputs`] emits EXCEPT
/// `LambdaName` (the TS `aws-lambda` scaffold branch never emits it — see
/// `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts`'s `aws-lambda`
/// branch) and with a different `ApiUrl`.
#[must_use]
pub fn render_http_api_outputs(
    d: &DeployDescriptor,
    p: &RenderParams,
    http_api_logical_id: &str,
) -> BTreeMap<String, CfnOutput> {
    let mut outputs = common_outputs(d, p);
    outputs.insert(
        "ApiUrl".to_string(),
        CfnOutput {
            description: Some("MCP Server API URL".to_string()),
            value: json!({ "Fn::GetAtt": [http_api_logical_id, "ApiEndpoint"] }),
            export: None,
        },
    );
    outputs
}

/// Render the `aws-lambda` target's Cognito+DCR OAuth stack shape's
/// (Task 6) fixed output set: `ApiUrl`, `ClientsTableName`, `DashboardUrl`,
/// `OAuthDiscoveryUrl`, `UserPoolDomainUrl`, `UserPoolId`. Unlike
/// [`render_http_api_outputs`], this variant has no `LambdaArn`/`McpRoleArn`
/// (the stack renders THREE Lambda functions/roles, not one — see
/// `resources::cognito`'s module doc comment for why "the" execution role
/// isn't a well-defined concept here) — mirrors the `CfnOutput` calls in the
/// OAuth branch of `cargo-pmcp/src/commands/deploy/init.rs::create_oauth_stack_ts`
/// exactly (6 outputs, no Lambda-identity ones).
///
/// # Panics
///
/// Panics if `d.auth.cognito` is `None`. Callers MUST only reach this
/// function after [`crate::resources::cognito::render`] has already
/// succeeded for the same descriptor (as `crate::render_aws_lambda_oauth`
/// does) — that call already validated `d.auth.cognito.is_some()`.
#[must_use]
pub fn render_cognito_outputs(
    d: &DeployDescriptor,
    p: &RenderParams,
) -> BTreeMap<String, CfnOutput> {
    let cognito_cfg = d
        .auth
        .cognito
        .as_ref()
        .expect("cognito section present — validated by cognito::render before outputs run");

    let mut outputs = common_outputs(d, p);
    outputs.remove("LambdaArn");
    outputs.remove("McpRoleArn");
    outputs.insert(
        "ApiUrl".to_string(),
        CfnOutput {
            description: Some("MCP Server API URL".to_string()),
            value: json!({ "Fn::GetAtt": [logical_ids::for_http_api(), "ApiEndpoint"] }),
            export: None,
        },
    );
    outputs.insert(
        "OAuthDiscoveryUrl".to_string(),
        CfnOutput {
            description: Some("OAuth Discovery URL".to_string()),
            value: json!({
                "Fn::Join": ["", [
                    { "Fn::GetAtt": [logical_ids::for_http_api(), "ApiEndpoint"] },
                    "/.well-known/openid-configuration",
                ]],
            }),
            export: None,
        },
    );
    outputs.insert(
        "UserPoolId".to_string(),
        CfnOutput {
            description: Some("Cognito User Pool ID".to_string()),
            value: json!({ "Ref": logical_ids::for_user_pool() }),
            export: None,
        },
    );
    outputs.insert(
        "UserPoolDomainUrl".to_string(),
        CfnOutput {
            description: Some("Cognito Hosted UI Domain".to_string()),
            value: json!(format!(
                "https://{}.auth.{}.amazoncognito.com",
                cognito::domain_prefix(d, p, cognito_cfg),
                p.region,
            )),
            export: None,
        },
    );
    outputs.insert(
        "ClientsTableName".to_string(),
        CfnOutput {
            description: Some("DynamoDB table for registered OAuth clients".to_string()),
            value: json!({ "Ref": logical_ids::for_table(&cognito::clients_table_name(d)) }),
            export: None,
        },
    );
    outputs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(name: &str) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "pmcp-run"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "{name}"
            timeout_seconds = 30
            [auth]
            enabled = false
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = true
            create_dashboard = true
            "#
        ))
        .expect("fixture descriptor parses")
    }

    fn params() -> RenderParams {
        RenderParams {
            region: "us-west-2".to_string(),
            ..RenderParams::for_test("outputs-test-stack")
        }
    }

    #[test]
    fn renders_exactly_the_five_expected_output_names() {
        let outputs = render_outputs(&descriptor("out-test"), &params());
        let mut names: Vec<&String> = outputs.keys().collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "ApiUrl",
                "DashboardUrl",
                "LambdaArn",
                "LambdaName",
                "McpRoleArn"
            ]
        );
    }

    #[test]
    fn dashboard_url_uses_the_params_region() {
        let outputs = render_outputs(&descriptor("out-test"), &params());
        assert_eq!(
            outputs["DashboardUrl"].value,
            json!("https://console.aws.amazon.com/cloudwatch/home?region=us-west-2")
        );
    }

    #[test]
    fn mcp_role_arn_carries_a_stable_export_name() {
        let outputs = render_outputs(&descriptor("out-test"), &params());
        let export = outputs["McpRoleArn"]
            .export
            .as_ref()
            .expect("McpRoleArn has an Export");
        assert_eq!(export.name, "pmcp-out-test-McpRoleArn");
    }

    #[test]
    fn only_mcp_role_arn_has_an_export() {
        let outputs = render_outputs(&descriptor("out-test"), &params());
        for (name, output) in &outputs {
            if name == "McpRoleArn" {
                assert!(output.export.is_some());
            } else {
                assert!(output.export.is_none(), "{name} should have no Export");
            }
        }
    }

    #[test]
    fn http_api_outputs_have_no_lambda_name() {
        let outputs = render_http_api_outputs(&descriptor("out-test"), &params(), "HttpApi");
        let mut names: Vec<&String> = outputs.keys().collect();
        names.sort();
        assert_eq!(
            names,
            vec!["ApiUrl", "DashboardUrl", "LambdaArn", "McpRoleArn"]
        );
    }

    #[test]
    fn http_api_outputs_api_url_is_a_getatt_on_the_given_logical_id() {
        let outputs = render_http_api_outputs(&descriptor("out-test"), &params(), "HttpApi");
        assert_eq!(
            outputs["ApiUrl"].value,
            json!({ "Fn::GetAtt": ["HttpApi", "ApiEndpoint"] })
        );
    }

    #[test]
    fn http_api_outputs_mcp_role_arn_carries_a_stable_export_name() {
        let outputs = render_http_api_outputs(&descriptor("out-test"), &params(), "HttpApi");
        let export = outputs["McpRoleArn"]
            .export
            .as_ref()
            .expect("McpRoleArn has an Export");
        assert_eq!(export.name, "pmcp-out-test-McpRoleArn");
    }
}
