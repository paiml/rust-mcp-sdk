//! The `aws-lambda` target's Cognito+DCR OAuth stack shape (Task 6).
//!
//! [`render`] is the single entry point for this whole stack shape —
//! analogous to how `resources::http_api::render` is the entry point for
//! the plain (non-OAuth) `aws-lambda` kernel's own HTTP API. It composes
//! `lambda`/`iam`/`logs` primitives (reused, then lightly patched — see
//! below) plus `dynamodb::render_table`, rather than duplicating them, to
//! build the FULL resource graph:
//!
//! - **3 Lambda functions**, each with its own execution role and
//!   CloudWatch log group: the MCP server function (reuses
//!   [`crate::resources::lambda::render_function_aws_lambda`] /
//!   [`crate::resources::iam::render_execution_role_aws_lambda`] /
//!   [`crate::resources::logs::render_log_group_for`] under the FIXED ids
//!   from Task 5 — `logical_ids::for_function`/`for_execution_role`/
//!   `for_execution_policy`/`for_log_group` — then patches in the two
//!   Cognito environment variables and the stack-wide `"component":
//!   "oauth"` tag); the DCR OAuth-proxy function (its own role/policy —
//!   DynamoDB `ClientsTable` CRUD + `cognito-idp` client-management grants,
//!   no X-Ray); and the JWT-validator authorizer function (role only, no
//!   attached policy — it needs no permissions beyond the base
//!   `AWSLambdaBasicExecutionRole` managed policy).
//! - The Cognito **`UserPool`**, **`UserPoolResourceServer`** (the `mcp`
//!   scope set), and **`UserPoolDomain`** (hosted-UI domain).
//! - The DCR **`ClientsTable`** (via [`crate::resources::dynamodb::render_table`]
//!   — hash key `client_id` (S), `PAY_PER_REQUEST`).
//! - The **HTTP API**: one `AWS::ApiGatewayV2::Api`/`Stage`, TWO
//!   integrations (MCP function, OAuth-proxy function), SEVEN routes (2
//!   protected `/mcp*` + 1 public health check, both via the MCP
//!   integration; 4 public `/oauth2/*`+`/.well-known/*` via the OAuth-proxy
//!   integration), the JWT `Authorizer` wired onto the 2 protected routes,
//!   and 3 `AWS::Lambda::Permission`s (one per function). The `Api`/`Stage`/
//!   `Integration`/`Permission` resources themselves are NOT re-typed
//!   here — this module's own `render_api`/`render_stage` delegate to
//!   `crate::resources::http_api`'s `pub(crate)` `render_api_with`/
//!   `render_stage_with`, and its `render_http_api` calls that module's
//!   `render_integration_for`/`render_permission_for` directly — the
//!   plain `aws-lambda` HTTP API kernel (`resources::http_api`) and this
//!   stack's own HTTP API share one definition of each resource shape,
//!   parameterized by the handful of inputs that actually differ
//!   (description, tags, logical id).
//!
//! There is deliberately no static `AWS::Cognito::UserPoolClient` resource:
//! DCR clients register at RUNTIME via the OAuth-proxy Lambda's
//! `cognito-idp:CreateUserPoolClient` call, not at deploy time.
//!
//! Pinned by `tests/goldens/oauth-cognito-dcr.golden.json` (promoted from
//! `pending/` in Task 6).
//!
//! # What `[auth.cognito]`/`[auth.dcr]` fields this module reads
//!
//! Only `cognito.user_pool_name` (`UserPoolName`, default
//! `"{server}-users"`), `cognito.resource_server_id` (the
//! `UserPoolResourceServer`'s `Identifier`/`Name`), and `cognito.domain`
//! (an optional literal override for the hosted-UI domain prefix — see
//! [`domain_prefix`]) drive rendered CFN properties. `mfa`,
//! `access_token_ttl`, `refresh_token_ttl`, and `social_providers` are part
//! of [`pmcp_package::package::CognitoSection`]'s closed set but are NOT
//! wired into any CFN property here: the real `cargo pmcp deploy init
//! --oauth cognito` scaffold generator
//! (`cargo-pmcp/src/commands/deploy/init.rs::create_oauth_stack_ts`, the
//! parity target this module mirrors) doesn't read them either — its
//! `stack.ts` is a static snapshot taken at scaffold time, not re-rendered
//! from `.pmcp/deploy.toml` (see `tests/goldens/README.md`). Wiring them
//! would diverge from the golden. `[auth.dcr]`'s fields (beyond its mere
//! presence) are runtime configuration for the OAuth-proxy Lambda's OWN
//! business logic, not CFN template shape — the scaffold bundles DCR
//! unconditionally into every Cognito OAuth stack (no
//! Cognito-without-DCR/DCR-without-Cognito scaffold variant exists), so
//! this module renders the `ClientsTable`+OAuth-proxy pieces unconditionally
//! too, without branching on `d.auth.dcr`.
//!
//! Because every real descriptor populates those four inert fields (they
//! aren't `Option`s in [`pmcp_package::package::CognitoSection`], except
//! `social_providers` which defaults to empty), silently ignoring them is
//! correct — erroring would reject 100% of real inputs, and this module
//! must stay golden-faithful. But silently dropping them with no signal at
//! all is a footgun, so [`validate`] emits one advisory
//! `"auth.cognito.inert_fields"` [`crate::resources::iam::Warning`]
//! whenever `[auth.cognito]` is present, naming all four fields — wired
//! into [`crate::render`]'s validation step alongside
//! [`crate::resources::iam::validate`].

use crate::{
    error::RenderError,
    logical_ids,
    params::RenderParams,
    resources::{
        aws_lambda_map_tags_with_component, aws_lambda_tags_with_component, dynamodb, http_api,
        iam, lambda, logs,
    },
    template::CfnResource,
};
use pmcp_package::package::{AuthSection, CognitoSection, DeployDescriptor};
use serde_json::{json, Map, Value};

/// The stack-wide marker tag every taggable resource in this stack shape
/// carries on top of the standard `aws-lambda` four — mirrors
/// `cdk.Tags.of(this).add('component', 'oauth')` in
/// `cargo-pmcp/src/commands/deploy/init.rs::create_oauth_stack_ts`.
const COMPONENT_TAG: &str = "oauth";

/// Fixed literal CDK emits for both `EmailVerificationMessage`/
/// `SmsVerificationMessage` and `VerificationMessageTemplate`'s
/// `EmailMessage`/`SmsMessage` — not descriptor-driven (see the module doc
/// comment's "what fields this module reads" section).
const VERIFICATION_CODE_MESSAGE: &str = "The verification code to your new account is {####}";

/// Fixed MCP scopes the `UserPoolResourceServer` declares — not
/// descriptor-driven today (see the module doc comment).
const RESOURCE_SERVER_SCOPES: [(&str, &str); 2] = [
    ("read", "Read access to MCP tools and resources"),
    ("write", "Write access to MCP tools"),
];

/// Fixed size/timeout for the OAuth-proxy Lambda — the TS scaffold
/// hardcodes these (no descriptor field customizes them).
const OAUTH_PROXY_MEMORY_MB: i64 = 256;
const OAUTH_PROXY_TIMEOUT_SECONDS: i64 = 30;

/// Fixed size/timeout for the authorizer Lambda.
const AUTHORIZER_MEMORY_MB: i64 = 256;
const AUTHORIZER_TIMEOUT_SECONDS: i64 = 10;

/// Render the whole Cognito+DCR OAuth stack shape: `(logical_id, resource)`
/// pairs in no particular order — the caller (`crate::render_aws_lambda_oauth`)
/// collects them into a `BTreeMap`. See the module doc comment for the full
/// resource inventory.
///
/// # Errors
///
/// Returns [`RenderError::Invalid`] when `d.auth.provider` is not
/// `"cognito"` (naming the offending provider) and
/// [`RenderError::MissingField`] when `d.auth.provider == "cognito"` but
/// `d.auth.cognito` is absent. Callers must only invoke this after
/// confirming `d.target.target_type == "aws-lambda"` and `d.auth.enabled`
/// (as `crate::render`'s guard does) — this function does not re-check
/// either.
pub fn render(
    d: &DeployDescriptor,
    p: &RenderParams,
) -> Result<Vec<(String, CfnResource)>, RenderError> {
    let cognito = require_cognito(d)?;

    let mut resources = Vec::new();
    resources.extend(render_mcp_lambda(d, p));
    resources.extend(render_oauth_proxy_lambda(d, p));
    resources.extend(render_authorizer_lambda(d, p));
    resources.push(render_user_pool(d, cognito));
    resources.push(render_user_pool_resource_server(cognito));
    resources.push(render_user_pool_domain(d, p, cognito));
    resources.push(render_clients_table(d));
    resources.extend(render_http_api(d, p));
    Ok(resources)
}

/// Validate `d.auth`'s provider/section shape and return the (now known
/// present) `[auth.cognito]` config.
fn require_cognito(d: &DeployDescriptor) -> Result<&CognitoSection, RenderError> {
    if d.auth.provider != "cognito" {
        return Err(RenderError::Invalid {
            section: "auth".to_string(),
            field: "provider".to_string(),
            message: format!(
                "auth.enabled = true on the aws-lambda target requires provider = \"cognito\" \
                 (got \"{}\") — no other OAuth provider is implemented yet",
                d.auth.provider
            ),
        });
    }
    d.auth
        .cognito
        .as_ref()
        .ok_or_else(|| RenderError::MissingField {
            section: "auth".to_string(),
            field: "cognito".to_string(),
        })
}

/// The four [`CognitoSection`] fields this module accepts but never wires
/// into a rendered CFN property — see the module doc comment's "what
/// fields this module reads" section for why silently ignoring them
/// (rather than erroring) is the correct, golden-faithful behavior.
const INERT_COGNITO_FIELDS: &str = "mfa, access_token_ttl, refresh_token_ttl, social_providers";

/// Advisory-only validation for `[auth.cognito]`: whenever it is present,
/// emits exactly one [`iam::Warning`] (code `"auth.cognito.inert_fields"`)
/// naming the four fields (`INERT_COGNITO_FIELDS`) that are part of
/// [`CognitoSection`]'s closed set but are silently dropped by this
/// module's renderer rather than being wired into any CloudFormation
/// property. Returns an empty `Vec` when `auth.cognito` is `None` — this is
/// advisory-only and never blocks a render (mirrors [`iam::validate`]'s
/// warning half; unlike that function this one is infallible, since there
/// is no ill-formed shape to reject here).
#[must_use]
pub fn validate(auth: &AuthSection) -> Vec<iam::Warning> {
    if auth.cognito.is_none() {
        return Vec::new();
    }
    vec![iam::Warning {
        code: "auth.cognito.inert_fields".to_string(),
        message: format!(
            "[auth.cognito] fields ({INERT_COGNITO_FIELDS}) are accepted but do not affect \
             the rendered CloudFormation stack — the aws-lambda Cognito+DCR scaffold this \
             renderer mirrors doesn't wire them into the template either."
        ),
    }]
}

// ---------------------------------------------------------------------
// The 3 Lambda functions + their roles/policies/log groups
// ---------------------------------------------------------------------

/// The MCP server's own function/role/log group: reuses the `aws-lambda`
/// kernel's builders (Task 5) under their FIXED logical ids, then patches
/// in the two Cognito environment variables and the oauth component tag.
fn render_mcp_lambda(d: &DeployDescriptor, p: &RenderParams) -> Vec<(String, CfnResource)> {
    let (function_id, mut function) = lambda::render_function_aws_lambda(d, p);
    function.properties["Tags"] = aws_lambda_tags_with_component(&d.server.name, COMPONENT_TAG);
    inject_cognito_env(&mut function, p);

    let mut role_and_policy = iam::render_execution_role_aws_lambda(d);
    role_and_policy[0].1.properties["Tags"] =
        aws_lambda_tags_with_component(&d.server.name, COMPONENT_TAG);

    let log_group = logs::render_log_group_for(
        logical_ids::for_function(),
        logical_ids::for_log_group(),
        aws_lambda_tags_with_component(&d.server.name, COMPONENT_TAG),
        crate::AWS_LAMBDA_LOG_RETENTION_DAYS,
    );

    let mut out = vec![(function_id, function)];
    out.extend(role_and_policy);
    out.push(log_group);
    out
}

/// Insert `COGNITO_REGION`/`COGNITO_USER_POOL_ID` into a function's
/// `Environment.Variables`, alongside whatever it already carries.
fn inject_cognito_env(function: &mut CfnResource, p: &RenderParams) {
    function.properties["Environment"]["Variables"]["COGNITO_REGION"] = json!(p.region);
    function.properties["Environment"]["Variables"]["COGNITO_USER_POOL_ID"] =
        json!({ "Ref": logical_ids::for_user_pool() });
}

/// `p.environment` plus the two Cognito env vars every function in this
/// stack shape carries — as a raw JSON object (not `BTreeMap<String,
/// String>`, since `COGNITO_USER_POOL_ID`'s value is a `Ref`, not a
/// literal string).
fn base_environment(p: &RenderParams) -> Map<String, Value> {
    let mut vars: Map<String, Value> = p
        .environment
        .iter()
        .map(|(k, v)| (k.clone(), json!(v)))
        .collect();
    vars.insert("COGNITO_REGION".to_string(), json!(p.region));
    vars.insert(
        "COGNITO_USER_POOL_ID".to_string(),
        json!({ "Ref": logical_ids::for_user_pool() }),
    );
    vars
}

/// The DCR OAuth-proxy function + its own role/policy/log group.
fn render_oauth_proxy_lambda(d: &DeployDescriptor, p: &RenderParams) -> Vec<(String, CfnResource)> {
    vec![
        render_oauth_proxy_function(d, p),
        render_oauth_proxy_role(d),
        render_oauth_proxy_policy(d),
        logs::render_log_group_for(
            logical_ids::for_oauth_proxy_function(),
            logical_ids::for_oauth_proxy_log_group(),
            aws_lambda_tags_with_component(&d.server.name, COMPONENT_TAG),
            crate::AWS_LAMBDA_LOG_RETENTION_DAYS,
        ),
    ]
}

fn render_oauth_proxy_function(d: &DeployDescriptor, p: &RenderParams) -> (String, CfnResource) {
    let mut variables = base_environment(p);
    variables.insert(
        "DCR_TABLE_NAME".to_string(),
        json!({ "Ref": logical_ids::for_table(&clients_table_name(d)) }),
    );
    let resource = lambda::build_function_resource(
        p,
        lambda::LambdaFunctionSpec {
            function_name: format!("{}-oauth-proxy", d.server.name),
            memory_mb: OAUTH_PROXY_MEMORY_MB,
            timeout_seconds: OAUTH_PROXY_TIMEOUT_SECONDS,
            role_logical_id: logical_ids::for_oauth_proxy_role(),
            environment_variables: Value::Object(variables),
            tags: aws_lambda_tags_with_component(&d.server.name, COMPONENT_TAG),
            // No `tracing: lambda.Tracing.ACTIVE` in the TS scaffold's
            // OAuth-proxy function construct.
            tracing_active: false,
            layers: None,
            depends_on: vec![
                logical_ids::for_oauth_proxy_policy().to_string(),
                logical_ids::for_oauth_proxy_role().to_string(),
            ],
        },
    );
    (
        logical_ids::for_oauth_proxy_function().to_string(),
        resource,
    )
}

fn render_oauth_proxy_role(d: &DeployDescriptor) -> (String, CfnResource) {
    (
        logical_ids::for_oauth_proxy_role().to_string(),
        CfnResource {
            type_: "AWS::IAM::Role".to_string(),
            properties: iam::lambda_execution_role_properties(aws_lambda_tags_with_component(
                &d.server.name,
                COMPONENT_TAG,
            )),
            depends_on: vec![],
        },
    )
}

/// The OAuth-proxy role's default inline policy: DynamoDB `ClientsTable`
/// CRUD (mirrors CDK's `Table.grantReadWriteData()`, which — unlike a
/// manually-constructed `PolicyStatement` — always keeps `Resource` as a
/// one-element ARRAY even for a single ARN) plus `cognito-idp`
/// client-management grants (a manually-constructed statement, whose
/// single-element `Resource` DOES collapse to a bare value — matches
/// `iam::render_declared_statement`'s `one_or_many` collapse rule, verified
/// against this exact golden). No X-Ray statement — this function has no
/// `tracing: lambda.Tracing.ACTIVE` in the TS scaffold.
fn render_oauth_proxy_policy(d: &DeployDescriptor) -> (String, CfnResource) {
    let table_arn =
        json!({ "Fn::GetAtt": [logical_ids::for_table(&clients_table_name(d)), "Arn"] });
    let user_pool_arn = json!({ "Fn::GetAtt": [logical_ids::for_user_pool(), "Arn"] });
    let properties = json!({
        "PolicyName": iam::DEFAULT_POLICY_NAME,
        "PolicyDocument": {
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Action": [
                        "dynamodb:BatchGetItem", "dynamodb:Query", "dynamodb:GetItem", "dynamodb:Scan",
                        "dynamodb:ConditionCheckItem", "dynamodb:BatchWriteItem", "dynamodb:PutItem",
                        "dynamodb:UpdateItem", "dynamodb:DeleteItem", "dynamodb:DescribeTable",
                    ],
                    "Resource": [table_arn.clone()],
                },
                {
                    "Effect": "Allow",
                    "Action": ["dynamodb:GetRecords", "dynamodb:GetShardIterator"],
                    "Resource": [table_arn],
                },
                {
                    "Effect": "Allow",
                    "Action": [
                        "cognito-idp:CreateUserPoolClient", "cognito-idp:DescribeUserPoolClient",
                        "cognito-idp:DeleteUserPoolClient", "cognito-idp:ListUserPoolClients",
                    ],
                    "Resource": user_pool_arn,
                },
            ],
        },
        "Roles": [{ "Ref": logical_ids::for_oauth_proxy_role() }],
    });
    (
        logical_ids::for_oauth_proxy_policy().to_string(),
        CfnResource {
            type_: "AWS::IAM::Policy".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// The JWT-validator authorizer function + its role (no policy) + log group.
fn render_authorizer_lambda(d: &DeployDescriptor, p: &RenderParams) -> Vec<(String, CfnResource)> {
    vec![
        render_authorizer_function(d, p),
        render_authorizer_role(d),
        logs::render_log_group_for(
            logical_ids::for_authorizer_function(),
            logical_ids::for_authorizer_log_group(),
            aws_lambda_tags_with_component(&d.server.name, COMPONENT_TAG),
            crate::AWS_LAMBDA_LOG_RETENTION_DAYS,
        ),
    ]
}

fn render_authorizer_function(d: &DeployDescriptor, p: &RenderParams) -> (String, CfnResource) {
    let variables = base_environment(p);
    let resource = lambda::build_function_resource(
        p,
        lambda::LambdaFunctionSpec {
            function_name: format!("{}-authorizer", d.server.name),
            memory_mb: AUTHORIZER_MEMORY_MB,
            timeout_seconds: AUTHORIZER_TIMEOUT_SECONDS,
            role_logical_id: logical_ids::for_authorizer_role(),
            environment_variables: Value::Object(variables),
            tags: aws_lambda_tags_with_component(&d.server.name, COMPONENT_TAG),
            // No `tracing: lambda.Tracing.ACTIVE` in the TS scaffold's
            // authorizer function construct.
            tracing_active: false,
            layers: None,
            // No attached policy to depend on — unlike the MCP/OAuth-proxy
            // functions, this role gets no `addToRolePolicy` calls.
            depends_on: vec![logical_ids::for_authorizer_role().to_string()],
        },
    );
    (logical_ids::for_authorizer_function().to_string(), resource)
}

fn render_authorizer_role(d: &DeployDescriptor) -> (String, CfnResource) {
    (
        logical_ids::for_authorizer_role().to_string(),
        CfnResource {
            type_: "AWS::IAM::Role".to_string(),
            properties: iam::lambda_execution_role_properties(aws_lambda_tags_with_component(
                &d.server.name,
                COMPONENT_TAG,
            )),
            depends_on: vec![],
        },
    )
}

// ---------------------------------------------------------------------
// Cognito resources
// ---------------------------------------------------------------------

fn render_user_pool(d: &DeployDescriptor, cognito: &CognitoSection) -> (String, CfnResource) {
    let pool_name = cognito
        .user_pool_name
        .clone()
        .unwrap_or_else(|| format!("{}-users", d.server.name));
    let properties = json!({
        "UserPoolName": pool_name,
        "AccountRecoverySetting": {
            "RecoveryMechanisms": [{ "Name": "verified_email", "Priority": 1 }],
        },
        "AdminCreateUserConfig": { "AllowAdminCreateUserOnly": false },
        "AutoVerifiedAttributes": ["email"],
        "EmailVerificationMessage": VERIFICATION_CODE_MESSAGE,
        "EmailVerificationSubject": "Verify your new account",
        "Policies": {
            "PasswordPolicy": {
                "MinimumLength": 8,
                "RequireLowercase": true,
                "RequireNumbers": true,
                "RequireSymbols": false,
                "RequireUppercase": false,
            },
        },
        "SmsVerificationMessage": VERIFICATION_CODE_MESSAGE,
        "UserPoolTags": aws_lambda_map_tags_with_component(&d.server.name, COMPONENT_TAG),
        "UsernameAttributes": ["email"],
        "VerificationMessageTemplate": {
            "DefaultEmailOption": "CONFIRM_WITH_CODE",
            "EmailMessage": VERIFICATION_CODE_MESSAGE,
            "EmailSubject": "Verify your new account",
            "SmsMessage": VERIFICATION_CODE_MESSAGE,
        },
    });
    (
        logical_ids::for_user_pool().to_string(),
        CfnResource {
            type_: "AWS::Cognito::UserPool".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

fn render_user_pool_resource_server(cognito: &CognitoSection) -> (String, CfnResource) {
    let scopes: Vec<Value> = RESOURCE_SERVER_SCOPES
        .iter()
        .map(|(name, description)| json!({ "ScopeName": name, "ScopeDescription": description }))
        .collect();
    let properties = json!({
        "Identifier": cognito.resource_server_id,
        "Name": cognito.resource_server_id,
        "Scopes": scopes,
        "UserPoolId": { "Ref": logical_ids::for_user_pool() },
    });
    (
        logical_ids::for_user_pool_resource_server().to_string(),
        CfnResource {
            type_: "AWS::Cognito::UserPoolResourceServer".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

fn render_user_pool_domain(
    d: &DeployDescriptor,
    p: &RenderParams,
    cognito: &CognitoSection,
) -> (String, CfnResource) {
    let properties = json!({
        "Domain": domain_prefix(d, p, cognito),
        "UserPoolId": { "Ref": logical_ids::for_user_pool() },
    });
    (
        logical_ids::for_user_pool_domain().to_string(),
        CfnResource {
            type_: "AWS::Cognito::UserPoolDomain".to_string(),
            properties,
            // Mirrors `userPoolDomain.node.addDependency(userPool)` in the
            // TS scaffold.
            depends_on: vec![logical_ids::for_user_pool().to_string()],
        },
    )
}

/// The hosted-UI domain prefix: `cognito.domain` verbatim when the
/// descriptor sets an override, else `{server_name}-{last 8 chars of the
/// account id}` — mirrors the TS scaffold's
/// `` `${serverName}-${this.account.slice(-8)}` `` (a Cognito domain must be
/// globally unique across all AWS accounts, hence the account-derived
/// suffix). `pub(crate)` — also used by `resources::outputs::render_cognito_outputs`
/// to build the matching `UserPoolDomainUrl` output.
#[must_use]
pub(crate) fn domain_prefix(
    d: &DeployDescriptor,
    p: &RenderParams,
    cognito: &CognitoSection,
) -> String {
    if let Some(domain) = &cognito.domain {
        return domain.clone();
    }
    let suffix_start = p.account_id.len().saturating_sub(8);
    format!("{}-{}", d.server.name, &p.account_id[suffix_start..])
}

// ---------------------------------------------------------------------
// DCR ClientsTable
// ---------------------------------------------------------------------

fn render_clients_table(d: &DeployDescriptor) -> (String, CfnResource) {
    dynamodb::render_table(
        &clients_table_name(d),
        ("client_id", "S"),
        aws_lambda_tags_with_component(&d.server.name, COMPONENT_TAG),
    )
}

/// The DCR clients table's name: `{server_name}-oauth-clients`. `pub(crate)`
/// — also used by `resources::outputs::render_cognito_outputs` to build the
/// matching `ClientsTableName` output's `Ref`.
#[must_use]
pub(crate) fn clients_table_name(d: &DeployDescriptor) -> String {
    format!("{}-oauth-clients", d.server.name)
}

// ---------------------------------------------------------------------
// HTTP API: Api/Stage/Integrations/Routes/Authorizer/Permissions
// ---------------------------------------------------------------------

const API_DESCRIPTION: &str = "MCP Server HTTP API with OAuth";

fn render_http_api(d: &DeployDescriptor, p: &RenderParams) -> Vec<(String, CfnResource)> {
    let mcp_integration = logical_ids::for_mcp_integration();
    let oauth_integration = logical_ids::for_oauth_integration();

    let mut resources = vec![
        render_api(d),
        render_stage(d),
        http_api::render_integration_for(mcp_integration, logical_ids::for_function()),
        http_api::render_integration_for(
            oauth_integration,
            logical_ids::for_oauth_proxy_function(),
        ),
    ];
    resources.extend(render_routes(mcp_integration, oauth_integration));
    resources.push(render_authorizer_resource(d, p));
    resources.push(http_api::render_permission_for(
        logical_ids::for_http_permission(),
        logical_ids::for_function(),
        p,
    ));
    resources.push(http_api::render_permission_for(
        logical_ids::for_oauth_permission(),
        logical_ids::for_oauth_proxy_function(),
        p,
    ));
    resources.push(http_api::render_permission_for(
        logical_ids::for_authorizer_permission(),
        logical_ids::for_authorizer_function(),
        p,
    ));
    resources
}

/// Delegates to [`http_api::render_api_with`] — see this module's Finding-1
/// dedup note in the module doc comment. Only the description/tags differ
/// from the plain `aws-lambda` HTTP API.
fn render_api(d: &DeployDescriptor) -> (String, CfnResource) {
    http_api::render_api_with(
        d,
        API_DESCRIPTION,
        aws_lambda_map_tags_with_component(&d.server.name, COMPONENT_TAG),
    )
}

/// Delegates to [`http_api::render_stage_with`] — see this module's
/// Finding-1 dedup note in the module doc comment. Only the tags differ
/// from the plain `aws-lambda` HTTP API's `$default` stage.
fn render_stage(d: &DeployDescriptor) -> (String, CfnResource) {
    http_api::render_stage_with(aws_lambda_map_tags_with_component(
        &d.server.name,
        COMPONENT_TAG,
    ))
}

/// One `AWS::ApiGatewayV2::Route`. `authorizer_id = Some(_)` marks it
/// `CUSTOM`-protected via the JWT authorizer; `None` leaves it public.
fn render_route(
    logical_id: &str,
    route_key: &str,
    integration_id: &str,
    authorizer_id: Option<&str>,
) -> (String, CfnResource) {
    let mut properties = json!({
        "ApiId": { "Ref": logical_ids::for_http_api() },
        "RouteKey": route_key,
        "Target": {
            "Fn::Join": ["", ["integrations/", { "Ref": integration_id }]],
        },
    });
    if let Some(authorizer_id) = authorizer_id {
        properties["AuthorizationType"] = json!("CUSTOM");
        properties["AuthorizerId"] = json!({ "Ref": authorizer_id });
    }
    (
        logical_id.to_string(),
        CfnResource {
            type_: "AWS::ApiGatewayV2::Route".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// The fixed 7-route set: 2 protected `/mcp*` + 1 public health check (all
/// via `mcp_integration`), plus 4 public `/oauth2/*`+`/.well-known/*` (via
/// `oauth_integration`) — mirrors the TS scaffold's `CfnRoute` calls
/// exactly.
fn render_routes(mcp_integration: &str, oauth_integration: &str) -> Vec<(String, CfnResource)> {
    let authorizer = logical_ids::for_authorizer();
    vec![
        render_route(
            logical_ids::for_mcp_route(),
            "POST /mcp",
            mcp_integration,
            Some(authorizer),
        ),
        render_route(
            logical_ids::for_mcp_proxy_route(),
            "POST /mcp/{proxy+}",
            mcp_integration,
            Some(authorizer),
        ),
        render_route(
            logical_ids::for_health_route(),
            "GET /",
            mcp_integration,
            None,
        ),
        render_route(
            logical_ids::for_oauth_discovery_route(),
            "GET /.well-known/{proxy+}",
            oauth_integration,
            None,
        ),
        render_route(
            logical_ids::for_oauth_authorize_route(),
            "GET /oauth2/authorize",
            oauth_integration,
            None,
        ),
        render_route(
            logical_ids::for_oauth_register_route(),
            "POST /oauth2/register",
            oauth_integration,
            None,
        ),
        render_route(
            logical_ids::for_oauth_token_route(),
            "POST /oauth2/token",
            oauth_integration,
            None,
        ),
    ]
}

fn render_authorizer_resource(d: &DeployDescriptor, p: &RenderParams) -> (String, CfnResource) {
    let properties = json!({
        "ApiId": { "Ref": logical_ids::for_http_api() },
        "AuthorizerType": "REQUEST",
        "AuthorizerPayloadFormatVersion": "2.0",
        "AuthorizerResultTtlInSeconds": 300,
        "AuthorizerUri": {
            "Fn::Join": ["", [
                format!("arn:aws:apigateway:{}:lambda:path/2015-03-31/functions/", p.region),
                { "Fn::GetAtt": [logical_ids::for_authorizer_function(), "Arn"] },
                "/invocations",
            ]],
        },
        "EnableSimpleResponses": true,
        "IdentitySource": ["$request.header.Authorization"],
        "Name": format!("{}-authorizer", d.server.name),
    });
    (
        logical_ids::for_authorizer().to_string(),
        CfnResource {
            type_: "AWS::ApiGatewayV2::Authorizer".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn descriptor(provider: &str) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "aws-lambda"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "cognito-test"
            timeout_seconds = 30
            [auth]
            enabled = true
            provider = "{provider}"
            callback_urls = []
            [auth.cognito]
            user_pool_name = "cognito-test-users"
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

    fn params() -> RenderParams {
        RenderParams {
            environment: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
            ..RenderParams::for_test("cognito-test-stack")
        }
    }

    #[test]
    fn require_cognito_rejects_a_non_cognito_provider() {
        let err = require_cognito(&descriptor("auth0")).unwrap_err();
        assert_eq!(
            err,
            RenderError::Invalid {
                section: "auth".to_string(),
                field: "provider".to_string(),
                message: "auth.enabled = true on the aws-lambda target requires provider = \
                          \"cognito\" (got \"auth0\") — no other OAuth provider is implemented yet"
                    .to_string(),
            }
        );
    }

    #[test]
    fn render_produces_every_expected_resource_type() {
        let resources = render(&descriptor("cognito"), &params()).unwrap();
        let mut types: Vec<&str> = resources.iter().map(|(_, r)| r.type_.as_str()).collect();
        types.sort_unstable();
        types.dedup();
        assert_eq!(
            types,
            vec![
                "AWS::ApiGatewayV2::Api",
                "AWS::ApiGatewayV2::Authorizer",
                "AWS::ApiGatewayV2::Integration",
                "AWS::ApiGatewayV2::Route",
                "AWS::ApiGatewayV2::Stage",
                "AWS::Cognito::UserPool",
                "AWS::Cognito::UserPoolDomain",
                "AWS::Cognito::UserPoolResourceServer",
                "AWS::DynamoDB::Table",
                "AWS::IAM::Policy",
                "AWS::IAM::Role",
                "AWS::Lambda::Function",
                "AWS::Lambda::Permission",
                "AWS::Logs::LogGroup",
            ]
        );
    }

    #[test]
    fn render_produces_exactly_three_functions_two_policies_three_roles() {
        let resources = render(&descriptor("cognito"), &params()).unwrap();
        let count = |t: &str| resources.iter().filter(|(_, r)| r.type_ == t).count();
        assert_eq!(count("AWS::Lambda::Function"), 3);
        assert_eq!(count("AWS::IAM::Role"), 3);
        assert_eq!(
            count("AWS::IAM::Policy"),
            2,
            "authorizer role has no policy"
        );
        assert_eq!(count("AWS::Logs::LogGroup"), 3);
        assert_eq!(count("AWS::ApiGatewayV2::Integration"), 2);
        assert_eq!(count("AWS::ApiGatewayV2::Route"), 7);
        assert_eq!(count("AWS::Lambda::Permission"), 3);
    }

    #[test]
    fn mcp_function_carries_cognito_env_vars_and_the_oauth_tag() {
        let resources = render(&descriptor("cognito"), &params()).unwrap();
        let (_, function) = resources
            .iter()
            .find(|(id, _)| id == logical_ids::for_function())
            .unwrap();
        assert_eq!(
            function.properties["Environment"]["Variables"]["COGNITO_REGION"],
            "us-east-1"
        );
        assert_eq!(
            function.properties["Environment"]["Variables"]["COGNITO_USER_POOL_ID"],
            json!({ "Ref": "UserPool" })
        );
        assert!(function.properties["Tags"]
            .as_array()
            .unwrap()
            .contains(&json!({ "Key": "component", "Value": "oauth" })));
    }

    #[test]
    fn oauth_proxy_function_carries_the_dcr_table_env_var() {
        let resources = render(&descriptor("cognito"), &params()).unwrap();
        let (_, function) = resources
            .iter()
            .find(|(id, _)| id == logical_ids::for_oauth_proxy_function())
            .unwrap();
        assert_eq!(
            function.properties["Environment"]["Variables"]["DCR_TABLE_NAME"],
            json!({ "Ref": "CognitoTestOauthClientsTable" })
        );
    }

    #[test]
    fn authorizer_function_depends_only_on_its_role_no_policy() {
        let resources = render(&descriptor("cognito"), &params()).unwrap();
        let (_, function) = resources
            .iter()
            .find(|(id, _)| id == logical_ids::for_authorizer_function())
            .unwrap();
        assert_eq!(
            function.depends_on,
            vec![logical_ids::for_authorizer_role().to_string()]
        );
    }

    #[test]
    fn domain_prefix_defaults_to_server_name_and_account_suffix() {
        let cognito = CognitoSection {
            user_pool_id: None,
            user_pool_name: None,
            resource_server_id: "mcp".to_string(),
            social_providers: vec![],
            mfa: "optional".to_string(),
            access_token_ttl: "1h".to_string(),
            refresh_token_ttl: "30d".to_string(),
            domain: None,
        };
        assert_eq!(
            domain_prefix(&descriptor("cognito"), &params(), &cognito),
            "cognito-test-56789012"
        );
    }

    #[test]
    fn domain_prefix_uses_the_explicit_override_when_present() {
        let cognito = CognitoSection {
            user_pool_id: None,
            user_pool_name: None,
            resource_server_id: "mcp".to_string(),
            social_providers: vec![],
            mfa: "optional".to_string(),
            access_token_ttl: "1h".to_string(),
            refresh_token_ttl: "30d".to_string(),
            domain: Some("custom-domain".to_string()),
        };
        assert_eq!(
            domain_prefix(&descriptor("cognito"), &params(), &cognito),
            "custom-domain"
        );
    }

    #[test]
    fn clients_table_name_is_server_name_suffixed() {
        assert_eq!(
            clients_table_name(&descriptor("cognito")),
            "cognito-test-oauth-clients"
        );
    }

    fn auth_section_without_cognito() -> AuthSection {
        AuthSection {
            enabled: false,
            provider: "none".to_string(),
            callback_urls: vec![],
            cognito: None,
            dcr: None,
            groups: None,
            scopes: None,
        }
    }

    #[test]
    fn validate_fires_one_advisory_warning_when_cognito_is_present() {
        let warnings = validate(&descriptor("cognito").auth);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "auth.cognito.inert_fields");
    }

    #[test]
    fn validate_is_silent_when_auth_cognito_is_absent() {
        let warnings = validate(&auth_section_without_cognito());
        assert!(warnings.is_empty());
    }

    #[test]
    fn validate_warning_names_all_four_inert_fields() {
        let warnings = validate(&descriptor("cognito").auth);
        let message = &warnings[0].message;
        for field in [
            "mfa",
            "access_token_ttl",
            "refresh_token_ttl",
            "social_providers",
        ] {
            assert!(
                message.contains(field),
                "warning message should name `{field}`, got: {message}"
            );
        }
    }
}
