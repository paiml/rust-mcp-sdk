//! `AWS::Logs::LogGroup` rendering for the MCP server's Lambda function.

use crate::{
    logical_ids,
    resources::{aws_lambda_tags, standard_tags},
    template::CfnResource,
};
use serde_json::{json, Value};

/// Render a Lambda function's CloudWatch log group: `(logical_id,
/// resource)`. The log group's NAME references `function_logical_id` via
/// `Fn::Join`/`Ref`, mirroring what `cdk synth` emits for `logGroupName:
/// \`/aws/lambda/${fn.functionName}\`` (a token resolved at synth time, not
/// a literal string interpolation of the server name).
///
/// Shared primitive behind [`render_log_group`]/[`render_log_group_aws_lambda`]
/// (both fixed to [`logical_ids::for_function`]/[`logical_ids::for_log_group`])
/// and — Task 6 — `resources::cognito`'s per-function log groups (the
/// OAuth-proxy and authorizer Lambdas, whose function/log-group logical ids
/// differ from the MCP function's).
#[must_use]
pub(crate) fn render_log_group_for(
    function_logical_id: &str,
    log_group_logical_id: &str,
    tags: Value,
    retention_days: u32,
) -> (String, CfnResource) {
    let properties = json!({
        "LogGroupName": {
            "Fn::Join": ["", ["/aws/lambda/", { "Ref": function_logical_id }]],
        },
        "RetentionInDays": retention_days,
        "Tags": tags,
    });
    (
        log_group_logical_id.to_string(),
        CfnResource {
            type_: "AWS::Logs::LogGroup".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// Render the MCP function's CloudWatch log group: `(logical_id, resource)`.
#[must_use]
pub fn render_log_group(service_name: &str, retention_days: u32) -> (String, CfnResource) {
    render_log_group_for(
        logical_ids::for_function(),
        logical_ids::for_log_group(),
        standard_tags(service_name),
        retention_days,
    )
}

/// Render the MCP function's CloudWatch log group for the `aws-lambda`
/// target's own stack shape: `(logical_id, resource)`. Same
/// `LogGroupName`/`Ref` wiring as [`render_log_group`], but
/// `aws-lambda`-flavored tags (own `project`, `target = "aws-lambda"`) via
/// [`crate::resources::aws_lambda_tags`] instead of [`standard_tags`].
#[must_use]
pub fn render_log_group_aws_lambda(
    service_name: &str,
    retention_days: u32,
) -> (String, CfnResource) {
    render_log_group_for(
        logical_ids::for_function(),
        logical_ids::for_log_group(),
        aws_lambda_tags(service_name),
        retention_days,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_group_name_references_the_function_by_ref_not_a_literal_string() {
        let (_, resource) = render_log_group("my-server", 7);
        assert_eq!(
            resource.properties["LogGroupName"],
            json!({
                "Fn::Join": ["", ["/aws/lambda/", { "Ref": "McpFunction" }]],
            })
        );
    }

    #[test]
    fn retention_days_passes_through() {
        let (_, resource) = render_log_group("my-server", 30);
        assert_eq!(resource.properties["RetentionInDays"], 30);
    }

    #[test]
    fn tags_use_the_service_name_argument() {
        let (_, resource) = render_log_group("my-server", 7);
        assert_eq!(
            resource.properties["Tags"],
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "hosting" },
                { "Key": "service", "Value": "my-server" },
                { "Key": "target", "Value": "pmcp-run" },
            ])
        );
    }

    #[test]
    fn logical_id_is_stable() {
        let (id, _) = render_log_group("my-server", 7);
        assert_eq!(id, "LogGroup");
    }

    #[test]
    fn aws_lambda_tags_carry_the_server_name_as_project_and_aws_lambda_as_target() {
        let (_, resource) = render_log_group_aws_lambda("my-server", 30);
        assert_eq!(
            resource.properties["Tags"],
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "my-server" },
                { "Key": "service", "Value": "my-server" },
                { "Key": "target", "Value": "aws-lambda" },
            ])
        );
    }

    #[test]
    fn aws_lambda_retention_days_passes_through() {
        let (_, resource) = render_log_group_aws_lambda("my-server", 30);
        assert_eq!(resource.properties["RetentionInDays"], 30);
    }

    #[test]
    fn aws_lambda_logical_id_is_stable() {
        let (id, _) = render_log_group_aws_lambda("my-server", 30);
        assert_eq!(id, "LogGroup");
    }
}
