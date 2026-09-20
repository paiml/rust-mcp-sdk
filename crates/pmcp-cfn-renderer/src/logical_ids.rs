//! Stable, hash-free logical-ID scheme for rendered CloudFormation resources.
//!
//! CDK derives logical IDs from construct-tree content hashes (e.g.
//! `McpFunction1A2B3C4D`), so renaming or reordering constructs can silently
//! rename the logical ID of an unrelated resource. This renderer instead
//! derives logical IDs directly and deterministically from descriptor
//! names via [`pascal`] — no hashes, ever.
//!
//! One function per resource family, each returning a fixed, documented
//! logical ID (or, for families with descriptor-supplied names like
//! DynamoDB tables, a function of that name). A renamed descriptor entity
//! therefore renames its logical ID — this is an accepted tradeoff, not a
//! bug: the design's migration model is fleet recreation, not in-place
//! updates of foreign stacks (see the design spec, "Determinism").
//!
//! Future resource families (`cognito`, `dynamodb`'s multi-table shape,
//! etc.) get their own `for_*` functions as their resource modules land in
//! later tasks — this module is additive.

/// Logical ID for the MCP server's Lambda function. Exactly one per stack
/// (a descriptor names exactly one server function).
#[must_use]
pub fn for_function() -> &'static str {
    "McpFunction"
}

/// Logical ID for the function's CloudWatch log group.
#[must_use]
pub fn for_log_group() -> &'static str {
    "LogGroup"
}

/// Logical ID for the function's Lambda execution IAM role.
#[must_use]
pub fn for_execution_role() -> &'static str {
    "ExecutionRole"
}

/// Logical ID for the execution role's default inline policy — CDK renders
/// a Lambda's attached (`addToRolePolicy`) permissions as a SEPARATE
/// `AWS::IAM::Policy` resource, not inline `Policies:` on the role itself.
#[must_use]
pub fn for_execution_policy() -> &'static str {
    "ExecutionRoleDefaultPolicy"
}

/// Logical ID for the HTTP API (API Gateway v2 `AWS::ApiGatewayV2::Api`).
#[must_use]
pub fn for_http_api() -> &'static str {
    "HttpApi"
}

/// Logical ID for the HTTP API's Lambda-proxy integration
/// (`AWS::ApiGatewayV2::Integration`).
#[must_use]
pub fn for_http_integration() -> &'static str {
    "HttpApiIntegration"
}

/// Logical ID for the HTTP API's catch-all route (`AWS::ApiGatewayV2::Route`).
#[must_use]
pub fn for_http_route() -> &'static str {
    "HttpApiRoute"
}

/// Logical ID for the HTTP API's `$default` auto-deploy stage
/// (`AWS::ApiGatewayV2::Stage`).
#[must_use]
pub fn for_http_stage() -> &'static str {
    "HttpApiDefaultStage"
}

/// Logical ID for the `AWS::Lambda::Permission` granting API Gateway
/// permission to invoke the MCP server's function.
#[must_use]
pub fn for_http_permission() -> &'static str {
    "ApiGatewayInvokePermission"
}

/// Logical ID for a named DynamoDB table: `PascalCase(name)` + `"Table"`.
///
/// e.g. `for_table("audit-log")` -> `"AuditLogTable"`.
#[must_use]
pub fn for_table(name: &str) -> String {
    format!("{}Table", pascal(name))
}

// ---------------------------------------------------------------------
// `cognito` family (Task 6) — the `aws-lambda` target's Cognito+DCR OAuth
// stack shape: the Cognito resources themselves, the OAuth-proxy and
// JWT-authorizer Lambdas (each with their own role/log-group), and the
// 2-integration/7-route HTTP API wiring. The MCP function's own resources
// reuse the fixed ids above (`for_function`, `for_execution_role`,
// `for_execution_policy`, `for_log_group`, `for_http_api`,
// `for_http_stage`, `for_http_permission`) — see `resources::cognito`'s
// module doc comment.
// ---------------------------------------------------------------------

/// Logical ID for the Cognito `AWS::Cognito::UserPool`.
#[must_use]
pub fn for_user_pool() -> &'static str {
    "UserPool"
}

/// Logical ID for the `AWS::Cognito::UserPoolResourceServer` carrying the
/// MCP scopes.
#[must_use]
pub fn for_user_pool_resource_server() -> &'static str {
    "UserPoolResourceServer"
}

/// Logical ID for the `AWS::Cognito::UserPoolDomain` (hosted UI domain).
#[must_use]
pub fn for_user_pool_domain() -> &'static str {
    "UserPoolDomain"
}

/// Logical ID for the JWT `AWS::ApiGatewayV2::Authorizer` protecting the
/// `/mcp` routes.
#[must_use]
pub fn for_authorizer() -> &'static str {
    "Authorizer"
}

/// Logical ID for the token-validator Lambda backing [`for_authorizer`].
#[must_use]
pub fn for_authorizer_function() -> &'static str {
    "AuthorizerFunction"
}

/// Logical ID for [`for_authorizer_function`]'s execution role. Unlike the
/// MCP/OAuth-proxy functions, this role never gets an attached
/// `AWS::IAM::Policy` — it needs no permissions beyond the base
/// `AWSLambdaBasicExecutionRole` managed policy.
#[must_use]
pub fn for_authorizer_role() -> &'static str {
    "AuthorizerRole"
}

/// Logical ID for [`for_authorizer_function`]'s CloudWatch log group.
#[must_use]
pub fn for_authorizer_log_group() -> &'static str {
    "AuthorizerLogGroup"
}

/// Logical ID for the Dynamic-Client-Registration proxy Lambda (handles
/// `/oauth2/*` and `/.well-known/*`).
#[must_use]
pub fn for_oauth_proxy_function() -> &'static str {
    "OAuthProxyFunction"
}

/// Logical ID for [`for_oauth_proxy_function`]'s execution role.
#[must_use]
pub fn for_oauth_proxy_role() -> &'static str {
    "OAuthProxyRole"
}

/// Logical ID for [`for_oauth_proxy_role`]'s default inline policy (DynamoDB
/// `ClientsTable` CRUD + `cognito-idp` client-management grants).
#[must_use]
pub fn for_oauth_proxy_policy() -> &'static str {
    "OAuthProxyRoleDefaultPolicy"
}

/// Logical ID for [`for_oauth_proxy_function`]'s CloudWatch log group.
#[must_use]
pub fn for_oauth_proxy_log_group() -> &'static str {
    "OAuthProxyLogGroup"
}

/// Logical ID for the HTTP API integration targeting the MCP function.
#[must_use]
pub fn for_mcp_integration() -> &'static str {
    "McpIntegration"
}

/// Logical ID for the HTTP API integration targeting
/// [`for_oauth_proxy_function`].
#[must_use]
pub fn for_oauth_integration() -> &'static str {
    "OAuthIntegration"
}

/// Logical ID for the protected `POST /mcp` route.
#[must_use]
pub fn for_mcp_route() -> &'static str {
    "McpRoute"
}

/// Logical ID for the protected `POST /mcp/{proxy+}` route.
#[must_use]
pub fn for_mcp_proxy_route() -> &'static str {
    "McpProxyRoute"
}

/// Logical ID for the public `GET /` health-check route.
#[must_use]
pub fn for_health_route() -> &'static str {
    "HealthRoute"
}

/// Logical ID for the public `GET /.well-known/{proxy+}` OAuth-discovery
/// route.
#[must_use]
pub fn for_oauth_discovery_route() -> &'static str {
    "OAuthDiscoveryRoute"
}

/// Logical ID for the public `GET /oauth2/authorize` route.
#[must_use]
pub fn for_oauth_authorize_route() -> &'static str {
    "OAuthAuthorizeRoute"
}

/// Logical ID for the public `POST /oauth2/register` (DCR) route.
#[must_use]
pub fn for_oauth_register_route() -> &'static str {
    "OAuthRegisterRoute"
}

/// Logical ID for the public `POST /oauth2/token` route.
#[must_use]
pub fn for_oauth_token_route() -> &'static str {
    "OAuthTokenRoute"
}

/// Logical ID for the `AWS::Lambda::Permission` letting API Gateway invoke
/// [`for_oauth_proxy_function`].
#[must_use]
pub fn for_oauth_permission() -> &'static str {
    "ApiGatewayInvokeOAuthPermission"
}

/// Logical ID for the `AWS::Lambda::Permission` letting API Gateway invoke
/// [`for_authorizer_function`].
#[must_use]
pub fn for_authorizer_permission() -> &'static str {
    "ApiGatewayInvokeAuthorizerPermission"
}

/// Split `name` on `-`/`_`, uppercase each segment's first character, and
/// concatenate (PascalCase) — the transform every `for_*` function that
/// takes a descriptor-supplied name is built on.
///
/// Non-alphanumeric separators (`-`, `_`) are dropped; everything else in a
/// segment is left untouched (so an already-cased segment like `"API"`
/// stays `"API"` rather than becoming `"Api"`). Empty segments (from
/// leading/trailing/repeated separators) are skipped.
#[must_use]
pub fn pascal(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_function_is_stable_and_hash_free() {
        assert_eq!(for_function(), "McpFunction");
    }

    #[test]
    fn for_log_group_is_stable() {
        assert_eq!(for_log_group(), "LogGroup");
    }

    #[test]
    fn for_http_api_is_stable() {
        assert_eq!(for_http_api(), "HttpApi");
    }

    #[test]
    fn http_api_family_ids_are_stable_and_distinct() {
        let ids = [
            for_http_api(),
            for_http_integration(),
            for_http_route(),
            for_http_stage(),
            for_http_permission(),
        ];
        assert_eq!(
            ids,
            [
                "HttpApi",
                "HttpApiIntegration",
                "HttpApiRoute",
                "HttpApiDefaultStage",
                "ApiGatewayInvokePermission",
            ]
        );
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ids.len(),
            "expected all distinct, got {ids:?}"
        );
    }

    #[test]
    fn for_execution_role_is_stable() {
        assert_eq!(for_execution_role(), "ExecutionRole");
    }

    #[test]
    fn for_execution_policy_is_stable_and_distinct_from_the_role() {
        assert_eq!(for_execution_policy(), "ExecutionRoleDefaultPolicy");
        assert_ne!(for_execution_policy(), for_execution_role());
    }

    #[test]
    fn for_table_pascal_cases_and_suffixes() {
        assert_eq!(for_table("audit-log"), "AuditLogTable");
        assert_eq!(for_table("session_store"), "SessionStoreTable");
        assert_eq!(for_table("orders"), "OrdersTable");
    }

    #[test]
    fn pascal_splits_on_dash_and_underscore() {
        assert_eq!(pascal("audit-log"), "AuditLog");
        assert_eq!(pascal("session_store"), "SessionStore");
        assert_eq!(pascal("mixed-case_name"), "MixedCaseName");
    }

    #[test]
    fn pascal_preserves_existing_casing_within_a_segment() {
        assert_eq!(pascal("API-gateway"), "APIGateway");
    }

    #[test]
    fn pascal_skips_empty_segments() {
        assert_eq!(pascal("--leading"), "Leading");
        assert_eq!(pascal("trailing--"), "Trailing");
        assert_eq!(pascal("a--b"), "AB");
    }

    #[test]
    fn pascal_is_idempotent_on_a_single_word() {
        assert_eq!(pascal("orders"), "Orders");
    }

    #[test]
    fn for_table_never_produces_the_same_id_for_distinct_names() {
        // Not a full injectivity proof — a fast, cheap regression guard that
        // two distinct realistic names don't collide.
        assert_ne!(for_table("orders"), for_table("order"));
    }

    #[test]
    fn cognito_family_ids_are_stable_and_distinct() {
        let ids = [
            for_user_pool(),
            for_user_pool_resource_server(),
            for_user_pool_domain(),
            for_authorizer(),
            for_authorizer_function(),
            for_authorizer_role(),
            for_authorizer_log_group(),
            for_oauth_proxy_function(),
            for_oauth_proxy_role(),
            for_oauth_proxy_policy(),
            for_oauth_proxy_log_group(),
            for_mcp_integration(),
            for_oauth_integration(),
            for_mcp_route(),
            for_mcp_proxy_route(),
            for_health_route(),
            for_oauth_discovery_route(),
            for_oauth_authorize_route(),
            for_oauth_register_route(),
            for_oauth_token_route(),
            for_oauth_permission(),
            for_authorizer_permission(),
        ];
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ids.len(),
            "expected all distinct, got {ids:?}"
        );
        // Also distinct from the fixed ids the cognito stack shape reuses
        // for the MCP function's own resources (see `resources::cognito`'s
        // module doc comment).
        assert!(!ids.contains(&for_function()));
        assert!(!ids.contains(&for_execution_role()));
        assert!(!ids.contains(&for_http_api()));
    }
}
