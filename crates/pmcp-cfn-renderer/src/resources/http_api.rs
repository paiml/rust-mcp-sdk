//! `AWS::ApiGatewayV2::*` + `AWS::Lambda::Permission` rendering for the
//! `aws-lambda` target's own HTTP API — a single-route `AWS_PROXY`
//! integration in front of the MCP server's Lambda function.
//!
//! Mirrors the `aws-lambda` branch of
//! `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts` verbatim: an
//! `apigatewayv2.HttpApi` L2 construct (which implicitly creates its own
//! `$default` auto-deploy stage — there is no explicit `CfnStage` call in
//! the TS source, yet CDK still emits one), a single `CfnIntegration`
//! (`AWS_PROXY`, payload format `"2.0"`), a single catch-all `CfnRoute`
//! (`POST /{proxy+}` — every MCP server speaks JSON-RPC over one POST
//! endpoint, so there is no descriptor field selecting alternate routes),
//! and the `AWS::Lambda::Permission` API Gateway needs to invoke the
//! function (`mcpFunction.addPermission('ApiGatewayInvoke', ...)`).
//!
//! Pinned by `tests/goldens/http-api.golden.json` (promoted from
//! `pending/` in Task 5).
//!
//! `resources::cognito`'s own (larger) HTTP API shares this same
//! `Api`/`Stage`/`Integration`/`Lambda::Permission` resource shape — its
//! `render_http_api` calls this module's `pub(crate)` `render_api_with`/
//! `render_stage_with`/`render_integration_for`/`render_permission_for`
//! builders rather than duplicating them, varying only the few inputs that
//! actually differ (description string, tags, and — since that stack has
//! two integrations and three permissions — the logical id).

use crate::{
    error::RenderError, logical_ids, params::RenderParams, resources::aws_lambda_map_tags,
    template::CfnResource,
};
use pmcp_package::package::DeployDescriptor;
use serde_json::{json, Value};

/// The MCP server's single catch-all HTTP route. Fixed — see the module
/// doc comment.
const ROUTE_KEY: &str = "POST /{proxy+}";

/// Render the `aws-lambda` target's HTTP API surface: the
/// `AWS::ApiGatewayV2::Api`, its `Integration`, catch-all `Route`, implicit
/// `$default` `Stage`, and the `AWS::Lambda::Permission` letting API
/// Gateway invoke `function_logical_id`. Returns `(logical_id, resource)`
/// pairs in no particular order — the caller (`crate::render`) collects
/// them into a `BTreeMap`.
///
/// # Errors
///
/// Currently infallible (always `Ok`) — the `Result` return type matches
/// this crate's other resource-family entry points
/// ([`crate::resources::iam::validate`]) and leaves room for descriptor
/// validation a later task may need without another signature break.
#[allow(clippy::unnecessary_wraps)]
pub fn render(
    d: &DeployDescriptor,
    p: &RenderParams,
    function_logical_id: &str,
) -> Result<Vec<(String, CfnResource)>, RenderError> {
    Ok(vec![
        render_api(d),
        render_integration(function_logical_id),
        render_route(),
        render_stage(d),
        render_permission(function_logical_id, p),
    ])
}

/// The `AWS::ApiGatewayV2::Api`: `ProtocolType = "HTTP"`, a fixed
/// `[GET, POST, OPTIONS]`/`*` CORS preflight configuration (mirrors the TS
/// scaffold's `corsPreflight` block — no descriptor field customizes this
/// today), and the `aws-lambda`-target's own `Tags` (the flat `Map` shape
/// this resource type's `Tags` property requires — see
/// [`crate::resources::aws_lambda_map_tags`]'s doc comment).
fn render_api(d: &DeployDescriptor) -> (String, CfnResource) {
    render_api_with(
        d,
        "MCP Server HTTP API",
        aws_lambda_map_tags(&d.server.name),
    )
}

/// Shared `AWS::ApiGatewayV2::Api` builder — reused by this module's own
/// plain `aws-lambda` HTTP API kernel ([`render_api`]) and by
/// `resources::cognito`'s Cognito+DCR OAuth stack shape, whose HTTP API is
/// otherwise identical (same fixed logical id, `ProtocolType`, and CORS
/// preflight configuration). The two callers differ only in the
/// `Description` string and the `Tags` value (this target's plain four-tag
/// map vs. the OAuth stack's `component: oauth`-tagged map) — both are
/// taken as parameters so the resource shape itself is defined exactly
/// once. `pub(crate)` so `resources::cognito::render_api` can call it.
pub(crate) fn render_api_with(
    d: &DeployDescriptor,
    description: &str,
    tags: Value,
) -> (String, CfnResource) {
    let properties = json!({
        "Name": d.server.name,
        "ProtocolType": "HTTP",
        "Description": description,
        "CorsConfiguration": {
            "AllowOrigins": ["*"],
            "AllowMethods": ["GET", "POST", "OPTIONS"],
            "AllowHeaders": ["*"],
        },
        "Tags": tags,
    });
    (
        logical_ids::for_http_api().to_string(),
        CfnResource {
            type_: "AWS::ApiGatewayV2::Api".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// The `AWS::ApiGatewayV2::Integration`: `AWS_PROXY` to
/// `function_logical_id`'s ARN, payload format `"2.0"`.
fn render_integration(function_logical_id: &str) -> (String, CfnResource) {
    render_integration_for(logical_ids::for_http_integration(), function_logical_id)
}

/// Shared `AWS::ApiGatewayV2::Integration` builder — reused by this
/// module's single fixed integration ([`render_integration`]) and by
/// `resources::cognito`'s HTTP API, which needs TWO integrations (one per
/// backing Lambda function) and therefore already needs the logical id as
/// a parameter. `pub(crate)` so `resources::cognito::render_http_api` can
/// call it directly for both.
pub(crate) fn render_integration_for(
    logical_id: &str,
    function_logical_id: &str,
) -> (String, CfnResource) {
    let properties = json!({
        "ApiId": { "Ref": logical_ids::for_http_api() },
        "IntegrationType": "AWS_PROXY",
        "IntegrationUri": { "Fn::GetAtt": [function_logical_id, "Arn"] },
        "PayloadFormatVersion": "2.0",
    });
    (
        logical_id.to_string(),
        CfnResource {
            type_: "AWS::ApiGatewayV2::Integration".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// The catch-all `AWS::ApiGatewayV2::Route`: [`ROUTE_KEY`] targeting the
/// rendered `Integration`.
fn render_route() -> (String, CfnResource) {
    let properties = json!({
        "ApiId": { "Ref": logical_ids::for_http_api() },
        "RouteKey": ROUTE_KEY,
        "Target": {
            "Fn::Join": ["", ["integrations/", { "Ref": logical_ids::for_http_integration() }]],
        },
    });
    (
        logical_ids::for_http_route().to_string(),
        CfnResource {
            type_: "AWS::ApiGatewayV2::Route".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// The implicit `$default` auto-deploy `AWS::ApiGatewayV2::Stage`. CDK's
/// `HttpApi` L2 construct creates this stage itself (no explicit `CfnStage`
/// call in the TS scaffold), but it is still a real, separately-tagged
/// resource in the synthesized template.
fn render_stage(d: &DeployDescriptor) -> (String, CfnResource) {
    render_stage_with(aws_lambda_map_tags(&d.server.name))
}

/// Shared `AWS::ApiGatewayV2::Stage` builder — reused by [`render_stage`]
/// and by `resources::cognito`'s HTTP API, whose `$default` stage is
/// identical except for its `Tags` value (this target's plain four-tag map
/// vs. the OAuth stack's `component: oauth`-tagged map). `pub(crate)` so
/// `resources::cognito::render_stage` can call it.
pub(crate) fn render_stage_with(tags: Value) -> (String, CfnResource) {
    let properties = json!({
        "ApiId": { "Ref": logical_ids::for_http_api() },
        "StageName": "$default",
        "AutoDeploy": true,
        "Tags": tags,
    });
    (
        logical_ids::for_http_stage().to_string(),
        CfnResource {
            type_: "AWS::ApiGatewayV2::Stage".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// The `AWS::Lambda::Permission` granting `apigateway.amazonaws.com`
/// permission to invoke `function_logical_id`, scoped to this stack's own
/// HTTP API via an `execute-api` ARN built from `p.region`/`p.account_id`
/// (mirrors `mcpFunction.addPermission('ApiGatewayInvoke', ...)`'s
/// `sourceArn` template literal — CDK bakes the region/account as LITERAL
/// strings, not CFN pseudo-parameters, because the TS stack is synthesized
/// with an explicit `env: {account, region}`).
fn render_permission(function_logical_id: &str, p: &RenderParams) -> (String, CfnResource) {
    render_permission_for(logical_ids::for_http_permission(), function_logical_id, p)
}

/// Shared `AWS::Lambda::Permission` builder — reused by this module's
/// single fixed permission ([`render_permission`]) and by
/// `resources::cognito`'s HTTP API, which needs THREE such permissions (one
/// per Lambda function: MCP, OAuth-proxy, authorizer) and therefore already
/// needs the logical id as a parameter. `pub(crate)` so
/// `resources::cognito::render_http_api` can call it directly for all
/// three.
pub(crate) fn render_permission_for(
    logical_id: &str,
    function_logical_id: &str,
    p: &RenderParams,
) -> (String, CfnResource) {
    let properties = json!({
        "Action": "lambda:InvokeFunction",
        "FunctionName": { "Fn::GetAtt": [function_logical_id, "Arn"] },
        "Principal": "apigateway.amazonaws.com",
        "SourceArn": {
            "Fn::Join": ["", [
                format!("arn:aws:execute-api:{}:{}:", p.region, p.account_id),
                { "Ref": logical_ids::for_http_api() },
                "/*/*",
            ]],
        },
    });
    (
        logical_id.to_string(),
        CfnResource {
            type_: "AWS::Lambda::Permission".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(name: &str) -> DeployDescriptor {
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
        RenderParams::for_test("http-api-test-stack")
    }

    #[test]
    fn render_returns_the_five_expected_resources() {
        let resources = render(&descriptor("my-server"), &params(), "McpFunction").unwrap();
        let mut ids: Vec<&String> = resources.iter().map(|(id, _)| id).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "ApiGatewayInvokePermission",
                "HttpApi",
                "HttpApiDefaultStage",
                "HttpApiIntegration",
                "HttpApiRoute",
            ]
        );
    }

    #[test]
    fn api_uses_the_server_name_and_protocol_type_http() {
        let resources = render(&descriptor("my-server"), &params(), "McpFunction").unwrap();
        let (_, api) = resources
            .iter()
            .find(|(id, _)| id == logical_ids::for_http_api())
            .unwrap();
        assert_eq!(api.type_, "AWS::ApiGatewayV2::Api");
        assert_eq!(api.properties["Name"], "my-server");
        assert_eq!(api.properties["ProtocolType"], "HTTP");
    }

    #[test]
    fn integration_uri_references_the_given_function_logical_id() {
        let resources = render(&descriptor("my-server"), &params(), "SomeOtherFunction").unwrap();
        let (_, integration) = resources
            .iter()
            .find(|(id, _)| id == logical_ids::for_http_integration())
            .unwrap();
        assert_eq!(
            integration.properties["IntegrationUri"],
            json!({ "Fn::GetAtt": ["SomeOtherFunction", "Arn"] })
        );
    }

    #[test]
    fn route_key_is_the_fixed_catch_all_proxy_route() {
        let resources = render(&descriptor("my-server"), &params(), "McpFunction").unwrap();
        let (_, route) = resources
            .iter()
            .find(|(id, _)| id == logical_ids::for_http_route())
            .unwrap();
        assert_eq!(route.properties["RouteKey"], "POST /{proxy+}");
    }

    #[test]
    fn stage_is_the_default_auto_deploy_stage() {
        let resources = render(&descriptor("my-server"), &params(), "McpFunction").unwrap();
        let (_, stage) = resources
            .iter()
            .find(|(id, _)| id == logical_ids::for_http_stage())
            .unwrap();
        assert_eq!(stage.properties["StageName"], "$default");
        assert_eq!(stage.properties["AutoDeploy"], true);
    }

    #[test]
    fn permission_source_arn_uses_the_params_region_and_account() {
        let resources = render(&descriptor("my-server"), &params(), "McpFunction").unwrap();
        let (_, permission) = resources
            .iter()
            .find(|(id, _)| id == logical_ids::for_http_permission())
            .unwrap();
        assert_eq!(permission.type_, "AWS::Lambda::Permission");
        assert_eq!(
            permission.properties["SourceArn"],
            json!({
                "Fn::Join": ["", [
                    "arn:aws:execute-api:us-east-1:123456789012:",
                    { "Ref": "HttpApi" },
                    "/*/*",
                ]]
            })
        );
    }

    #[test]
    fn api_and_stage_tags_use_the_flat_map_shape() {
        let resources = render(&descriptor("my-server"), &params(), "McpFunction").unwrap();
        for id in [logical_ids::for_http_api(), logical_ids::for_http_stage()] {
            let (_, resource) = resources.iter().find(|(rid, _)| rid == id).unwrap();
            assert!(
                resource.properties["Tags"].is_object(),
                "{id} Tags should be a Map, got {:?}",
                resource.properties["Tags"]
            );
        }
    }
}
