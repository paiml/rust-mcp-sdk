//! `AWS::Lambda::Function` rendering for the MCP server's Lambda function.
//!
//! Models the `pmcp-run` deployment target's Lambda-only stack shape — no
//! API Gateway; the shared pmcp.run edge routes
//! `https://api.pmcp.run/{deploymentId}/mcp` to this function directly.
//! Mirrors the `pmcp-run` branch of
//! `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts` verbatim.

use crate::{
    logical_ids,
    params::{RenderParams, RuntimeAdapterConfig},
    resources::{aws_lambda_tags, standard_tags, DEFAULT_ORGANIZATION_ID, MCP_SERVERS_TABLE},
    template::CfnResource,
};
use pmcp_package::package::DeployDescriptor;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Fixed memory size for `pmcp-run`-target Lambdas.
///
/// NOT driven by the descriptor's `[server].memory_mb`. The only recorded
/// reason is byte-parity with the TypeScript scaffold, which hardcodes
/// `memorySize: 256` for this target — there is no cost guard, quota or
/// platform limit behind the number (the introducing commit, `6ab0bb1d`, is
/// an OAuth-integration commit whose message never mentions sizing).
///
/// The previous wording here claimed `memory_mb` "is meaningful only for the
/// google-cloud-run target's separate `[server].memory` string field", which
/// was incoherent — it named a DIFFERENT field as the reason this one is
/// ignored — and contradicted `cargo-pmcp`'s own `ServerConfig::memory_mb`
/// doc, which asserted the field was used by AWS targets. Neither was true.
/// The truth, and the current contract: this renderer still pins memory, and
/// `cargo-pmcp`'s pmcp-run deploy path rewrites `Properties.MemorySize`
/// AFTER rendering when `[server].memory_mb` is declared, so the declared
/// value is what actually deploys. See debug session
/// `deploy-server-memory-timeout`. Moving the const behind the descriptor
/// (making the renderer itself honor `memory_mb`) is a tracked follow-up,
/// deliberately not taken yet because it would require regenerating the
/// `cdk synth`-derived golden corpus.
pub const MEMORY_SIZE_MB: i64 = 256;

/// Fixed memory size for `aws-lambda`-target Lambdas — the TS scaffold
/// hardcodes `memorySize: 512` for this branch (vs. `pmcp-run`'s
/// [`MEMORY_SIZE_MB`] = 256). Also NOT driven by `[server].memory_mb`; same
/// rationale and same follow-up as [`MEMORY_SIZE_MB`].
///
/// Unlike the pmcp-run path, `cargo-pmcp`'s `aws-lambda` target has no
/// post-render seam wired, so a declared `memory_mb` is NOT honored there —
/// it prints a divergence warning quoting this value instead. `cargo-pmcp`
/// reads this const directly rather than keeping its own copy of `512`.
pub const AWS_LAMBDA_MEMORY_SIZE_MB: i64 = 512;

/// AWS account that publishes the [AWS Lambda Web Adapter](https://github.com/awslabs/aws-lambda-web-adapter)'s
/// public Lambda Layers (T8 review fix — see [`RuntimeAdapterConfig`]'s doc
/// comment for why this bridge exists). A fixed literal by design — the
/// adapter's own publishing account, not this deployment's account (which is
/// why the ARN below uses `Fn::Sub` only for `${AWS::Region}`, never for
/// this).
const LAMBDA_WEB_ADAPTER_ACCOUNT_ID: &str = "753240598075";

/// Pinned [AWS Lambda Web Adapter](https://github.com/awslabs/aws-lambda-web-adapter)
/// ARM64 zip-package layer version. Verified 2026-07-21 against the
/// adapter's published README — `LambdaAdapterLayerArm64:28` (ARM64 to
/// match `Architectures: ["arm64"]` below; the adapter also publishes an
/// X86 sibling layer, unused here since this renderer never targets x86_64
/// Lambdas). Bump this constant, with a fresh verification, when a newer
/// adapter release ships — it is NOT descriptor-driven; the version lives in
/// the renderer, not in `DeployDescriptor`.
const LAMBDA_WEB_ADAPTER_LAYER_VERSION: u32 = 28;

/// The env var the adapter's own `/opt/bootstrap` requires to be activated
/// as the function's Runtime API client for a zip-packaged custom runtime
/// (`provided.al2023`) — see [`RuntimeAdapterConfig`]'s doc comment.
const LWA_EXEC_WRAPPER_ENV_VAR: &str = "AWS_LAMBDA_EXEC_WRAPPER";
const LWA_EXEC_WRAPPER_PATH: &str = "/opt/bootstrap";
const LWA_PORT_ENV_VAR: &str = "PORT";
const LWA_READINESS_CHECK_PATH_ENV_VAR: &str = "AWS_LWA_READINESS_CHECK_PATH";

/// The values that vary across the FOUR `AWS::Lambda::Function` resources
/// this crate renders: [`render_function`]/[`render_function_aws_lambda`]
/// (this module's `pmcp-run`/`aws-lambda` kernel functions) and
/// `resources::cognito`'s `render_oauth_proxy_function`/
/// `render_authorizer_function` (the Cognito+DCR OAuth stack's other two
/// functions). Every other part of the shape — `Runtime`, `Handler`,
/// `Architectures`, `Code`, and `LoggingConfig` — is identical across all
/// four and lives exactly once, in [`build_function_resource`].
pub(crate) struct LambdaFunctionSpec {
    pub function_name: String,
    pub memory_mb: i64,
    pub timeout_seconds: i64,
    /// Logical id of the `AWS::IAM::Role` this function's `Role` property
    /// references via `Fn::GetAtt`.
    pub role_logical_id: &'static str,
    /// The function's `Environment.Variables` object.
    pub environment_variables: Value,
    pub tags: Value,
    /// `TracingConfig: {"Mode": "Active"}` — present for the `pmcp-run` and
    /// `aws-lambda` kernel functions (CDK's `tracing: lambda.Tracing.ACTIVE`),
    /// absent for the Cognito+DCR stack's OAuth-proxy/authorizer functions
    /// (no such construct option on their TS scaffold branch).
    pub tracing_active: bool,
    /// `Layers` — `Some` only for [`render_function_aws_lambda`] when
    /// `RenderParams::runtime_adapter` is populated (T8 review fix).
    pub layers: Option<Value>,
    pub depends_on: Vec<String>,
}

/// Build one `AWS::Lambda::Function` resource from `spec` — the single
/// definition of the property-block shape all four call sites share. See
/// [`LambdaFunctionSpec`]'s doc comment for which fields vary between them.
/// `pub(crate)` so `resources::cognito` can call it directly.
#[must_use]
pub(crate) fn build_function_resource(p: &RenderParams, spec: LambdaFunctionSpec) -> CfnResource {
    let mut properties = json!({
        "FunctionName": spec.function_name,
        "Runtime": "provided.al2023",
        "Handler": "bootstrap",
        "Architectures": ["arm64"],
        "MemorySize": spec.memory_mb,
        "Timeout": spec.timeout_seconds,
        "Code": { "S3Bucket": p.artifact.s3_bucket, "S3Key": p.artifact.s3_key },
        "Role": { "Fn::GetAtt": [spec.role_logical_id, "Arn"] },
        "Environment": { "Variables": spec.environment_variables },
        "LoggingConfig": { "LogFormat": "JSON" },
        "Tags": spec.tags,
    });
    if spec.tracing_active {
        properties["TracingConfig"] = json!({ "Mode": "Active" });
    }
    if let Some(layers) = spec.layers {
        properties["Layers"] = layers;
    }
    CfnResource {
        type_: "AWS::Lambda::Function".to_string(),
        properties,
        depends_on: spec.depends_on,
    }
}

/// Render the MCP server's `AWS::Lambda::Function`: `(logical_id, resource)`.
#[must_use]
pub fn render_function(d: &DeployDescriptor, p: &RenderParams) -> (String, CfnResource) {
    let resource = build_function_resource(
        p,
        LambdaFunctionSpec {
            function_name: d.server.name.clone(),
            memory_mb: MEMORY_SIZE_MB,
            timeout_seconds: d.server.timeout_seconds,
            role_logical_id: logical_ids::for_execution_role(),
            environment_variables: json!(environment_variables(d, p)),
            tags: standard_tags(&d.server.name),
            tracing_active: true,
            layers: None,
            // CDK adds an explicit dependency from the function onto its
            // execution role's default policy (IAM eventual-consistency
            // safety) as well as the role itself.
            depends_on: vec![
                logical_ids::for_execution_policy().to_string(),
                logical_ids::for_execution_role().to_string(),
            ],
        },
    );
    (logical_ids::for_function().to_string(), resource)
}

/// Render the MCP server's `AWS::Lambda::Function` for the `aws-lambda`
/// target's own (non-pmcp.run) stack shape: `(logical_id, resource)`.
/// Mirrors the `aws-lambda` branch of
/// `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts` — same
/// `Runtime`/`Handler`/`Architectures`/`Role` wiring as [`render_function`],
/// but a fixed 512 MB memory size, `p.environment` passed through UNCHANGED
/// (no `pmcp-run`-only composition vars — this stack shape has no
/// `MCP_SERVERS_TABLE` discovery table to read, and never calls other
/// foundation-server Lambdas), and `aws-lambda`-flavored tags (own
/// `project`, `target = "aws-lambda"`) via
/// [`crate::resources::aws_lambda_tags`].
///
/// When `p.runtime_adapter` is `Some` (T8 review fix — Critical finding:
/// `ServerShape::BuiltIn` artifacts need the AWS Lambda Web Adapter to
/// bridge plain-HTTP Shape A binaries onto the Lambda Runtime API), this
/// also attaches the adapter's `Layers` entry and its required env vars —
/// see [`RuntimeAdapterConfig`]'s doc comment. `None` (every existing
/// custom-Rust deployment today) renders NEITHER, byte-identical to this
/// function's pre-T8 output — this is a UNIT-TEST-OWNED property: the
/// checked-in golden fixtures' `params` JSON predates this field and
/// deserializes it to `None` via `#[serde(default)]`, so none of them
/// exercise or need to change for this branch (mirrors the T7
/// `cloudformation_metadata` field's precedent).
#[must_use]
pub fn render_function_aws_lambda(d: &DeployDescriptor, p: &RenderParams) -> (String, CfnResource) {
    let layers = p
        .runtime_adapter
        .is_some()
        .then(|| json!([runtime_adapter_layer_arn()]));
    let resource = build_function_resource(
        p,
        LambdaFunctionSpec {
            function_name: d.server.name.clone(),
            memory_mb: AWS_LAMBDA_MEMORY_SIZE_MB,
            timeout_seconds: d.server.timeout_seconds,
            role_logical_id: logical_ids::for_execution_role(),
            environment_variables: json!(environment_variables_aws_lambda(p)),
            tags: aws_lambda_tags(&d.server.name),
            tracing_active: true,
            layers,
            // Same eventual-consistency dependency ordering as
            // `render_function` — see its own doc comment.
            depends_on: vec![
                logical_ids::for_execution_policy().to_string(),
                logical_ids::for_execution_role().to_string(),
            ],
        },
    );
    (logical_ids::for_function().to_string(), resource)
}

/// `p.environment.clone()` with `extra` layered on top, `extra` winning on
/// any same-named entry — the shared "clone the resolved env, then
/// overlay fixed overrides" mechanic behind both
/// [`environment_variables`]'s three pmcp-run composition vars and
/// [`environment_variables_aws_lambda`]'s adapter vars.
fn merged_environment(
    p: &RenderParams,
    extra: impl IntoIterator<Item = (String, String)>,
) -> BTreeMap<String, String> {
    let mut vars = p.environment.clone();
    vars.extend(extra);
    vars
}

/// The function's environment variables: `p.environment`'s resolved values
/// (e.g. `RUST_LOG`, already resolved from the descriptor by the caller —
/// see `RenderParams`'s identity/environment split) plus the three
/// composition-support vars every `pmcp-run` Lambda gets
/// (`MCP_SERVERS_TABLE`, `PMCP_ORGANIZATION_ID`, `PMCP_SERVER_ID`),
/// mirroring the TS scaffold's `environment: {...}` object. The three fixed
/// vars win over any same-named entry in `p.environment`.
fn environment_variables(d: &DeployDescriptor, p: &RenderParams) -> BTreeMap<String, String> {
    merged_environment(
        p,
        [
            (
                "MCP_SERVERS_TABLE".to_string(),
                MCP_SERVERS_TABLE.to_string(),
            ),
            (
                "PMCP_ORGANIZATION_ID".to_string(),
                DEFAULT_ORGANIZATION_ID.to_string(),
            ),
            ("PMCP_SERVER_ID".to_string(), d.server.name.clone()),
        ],
    )
}

/// The `aws-lambda`-target function's environment variables: `p.environment`
/// passed through (same as before T8; no `pmcp-run`-only composition vars —
/// see [`render_function_aws_lambda`]'s doc comment), plus — only when
/// `p.runtime_adapter` is `Some` — the [AWS Lambda Web Adapter](https://github.com/awslabs/aws-lambda-web-adapter)'s
/// required env vars: `AWS_LAMBDA_EXEC_WRAPPER=/opt/bootstrap` (activates
/// the layer's own Runtime API client), `PORT` (the local port the wrapped
/// Shape A binary listens on — must match what `cargo-pmcp`'s
/// `aws_lambda::artifact` bootstrap wrapper script passes to `--http`), and
/// `AWS_LWA_READINESS_CHECK_PATH` when [`RuntimeAdapterConfig::readiness_check_path`]
/// is configured. The adapter's fixed vars win over any same-named entry in
/// `p.environment`, same precedence rule as [`environment_variables`]'s
/// fixed vars.
fn environment_variables_aws_lambda(p: &RenderParams) -> BTreeMap<String, String> {
    match &p.runtime_adapter {
        Some(adapter) => merged_environment(p, adapter_env_vars(adapter)),
        None => p.environment.clone(),
    }
}

/// The adapter's fixed env vars (see [`environment_variables_aws_lambda`]'s
/// doc comment), as `(key, value)` pairs ready for [`merged_environment`].
fn adapter_env_vars(adapter: &RuntimeAdapterConfig) -> Vec<(String, String)> {
    let mut vars = vec![
        (
            LWA_EXEC_WRAPPER_ENV_VAR.to_string(),
            LWA_EXEC_WRAPPER_PATH.to_string(),
        ),
        (LWA_PORT_ENV_VAR.to_string(), adapter.port.to_string()),
    ];
    if let Some(path) = &adapter.readiness_check_path {
        vars.push((LWA_READINESS_CHECK_PATH_ENV_VAR.to_string(), path.clone()));
    }
    vars
}

/// The AWS Lambda Web Adapter's pinned ARM64 layer ARN, as a CFN `Fn::Sub`
/// value (`${AWS::Region}` resolves server-side; [`LAMBDA_WEB_ADAPTER_ACCOUNT_ID`]
/// is a fixed literal — the adapter's own publishing account, not this
/// deployment's).
fn runtime_adapter_layer_arn() -> serde_json::Value {
    json!({
        "Fn::Sub": format!(
            "arn:aws:lambda:${{AWS::Region}}:{LAMBDA_WEB_ADAPTER_ACCOUNT_ID}:layer:LambdaAdapterLayerArm64:{LAMBDA_WEB_ADAPTER_LAYER_VERSION}"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> DeployDescriptor {
        toml::from_str(
            r#"
            [target]
            type = "pmcp-run"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "lambda-test"
            memory_mb = 512
            timeout_seconds = 45
            [auth]
            enabled = false
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = true
            create_dashboard = true
            "#,
        )
        .expect("fixture descriptor parses")
    }

    fn params(environment: BTreeMap<String, String>) -> RenderParams {
        RenderParams {
            environment,
            ..RenderParams::for_test("lambda-test-stack")
        }
    }

    /// [`params`] with a populated `runtime_adapter` — T8 review fix tests.
    fn params_with_adapter(adapter: RuntimeAdapterConfig) -> RenderParams {
        RenderParams {
            runtime_adapter: Some(adapter),
            ..params(BTreeMap::new())
        }
    }

    #[test]
    fn memory_size_is_fixed_regardless_of_descriptor_memory_mb() {
        let (_, resource) = render_function(&descriptor(), &params(BTreeMap::new()));
        assert_eq!(resource.properties["MemorySize"], 256);
    }

    #[test]
    fn timeout_comes_from_the_descriptor() {
        let (_, resource) = render_function(&descriptor(), &params(BTreeMap::new()));
        assert_eq!(resource.properties["Timeout"], 45);
    }

    #[test]
    fn environment_merges_params_env_with_fixed_composition_vars() {
        let environment = BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]);
        let (_, resource) = render_function(&descriptor(), &params(environment));
        assert_eq!(
            resource.properties["Environment"]["Variables"],
            json!({
                "RUST_LOG": "info",
                "MCP_SERVERS_TABLE": "McpServer",
                "PMCP_ORGANIZATION_ID": "default-org",
                "PMCP_SERVER_ID": "lambda-test",
            })
        );
    }

    #[test]
    fn function_depends_on_the_execution_role_and_its_policy() {
        let (_, resource) = render_function(&descriptor(), &params(BTreeMap::new()));
        assert_eq!(
            resource.depends_on,
            vec![
                logical_ids::for_execution_policy().to_string(),
                logical_ids::for_execution_role().to_string(),
            ]
        );
    }

    fn aws_lambda_descriptor() -> DeployDescriptor {
        toml::from_str(
            r#"
            [target]
            type = "aws-lambda"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "http-api-test"
            memory_mb = 512
            timeout_seconds = 30
            [auth]
            enabled = false
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = true
            create_dashboard = true
            "#,
        )
        .expect("fixture descriptor parses")
    }

    #[test]
    fn aws_lambda_memory_size_is_fixed_at_512_regardless_of_descriptor_memory_mb() {
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(BTreeMap::new()));
        assert_eq!(resource.properties["MemorySize"], 512);
    }

    #[test]
    fn aws_lambda_environment_is_passed_through_with_no_composition_vars_added() {
        let environment = BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]);
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(environment));
        assert_eq!(
            resource.properties["Environment"]["Variables"],
            json!({ "RUST_LOG": "info" })
        );
    }

    #[test]
    fn aws_lambda_tags_carry_the_server_name_as_project_and_aws_lambda_as_target() {
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(BTreeMap::new()));
        assert_eq!(
            resource.properties["Tags"],
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "http-api-test" },
                { "Key": "service", "Value": "http-api-test" },
                { "Key": "target", "Value": "aws-lambda" },
            ])
        );
    }

    #[test]
    fn aws_lambda_function_depends_on_the_execution_role_and_its_policy() {
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(BTreeMap::new()));
        assert_eq!(
            resource.depends_on,
            vec![
                logical_ids::for_execution_policy().to_string(),
                logical_ids::for_execution_role().to_string(),
            ]
        );
    }

    // -----------------------------------------------------------------
    // runtime_adapter (T8 review fix — Critical finding: AWS Lambda Web
    // Adapter layer for ServerShape::BuiltIn artifacts)
    // -----------------------------------------------------------------

    /// `runtime_adapter: None` (every existing custom-Rust deployment today,
    /// and every checked-in golden fixture) must render NEITHER `Layers` nor
    /// any adapter env var — proves the T8 fix is byte-identical to pre-fix
    /// output on the untouched path, which is why the golden corpus itself
    /// needed no changes.
    #[test]
    fn aws_lambda_function_omits_layers_and_adapter_env_vars_when_runtime_adapter_is_none() {
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(BTreeMap::new()));
        assert!(
            resource.properties.get("Layers").is_none(),
            "Layers must be entirely absent, not an empty array: {:?}",
            resource.properties
        );
        let vars = &resource.properties["Environment"]["Variables"];
        assert!(vars.get("AWS_LAMBDA_EXEC_WRAPPER").is_none());
        assert!(vars.get("PORT").is_none());
        assert!(vars.get("AWS_LWA_READINESS_CHECK_PATH").is_none());
    }

    /// `render_function` (the `pmcp-run` target's sibling) never wires the
    /// adapter at all, even when `runtime_adapter` is populated — the T8 fix
    /// is scoped to the `aws-lambda` target's own function only (see that
    /// function's own doc comment; `pmcp-run` foundation servers are always
    /// compiled Rust `lambda_runtime` binaries and never hit
    /// `ServerShape::BuiltIn`).
    #[test]
    fn pmcp_run_render_function_ignores_runtime_adapter_entirely() {
        let adapter_params = params_with_adapter(RuntimeAdapterConfig {
            port: 8080,
            readiness_check_path: None,
        });
        let (_, resource) = render_function(&descriptor(), &adapter_params);
        assert!(resource.properties.get("Layers").is_none());
        let vars = &resource.properties["Environment"]["Variables"];
        assert!(vars.get("AWS_LAMBDA_EXEC_WRAPPER").is_none());
    }

    #[test]
    fn aws_lambda_function_gains_the_pinned_adapter_layer_when_runtime_adapter_is_some() {
        let adapter_params = params_with_adapter(RuntimeAdapterConfig {
            port: 8080,
            readiness_check_path: None,
        });
        let (_, resource) = render_function_aws_lambda(&aws_lambda_descriptor(), &adapter_params);
        assert_eq!(
            resource.properties["Layers"],
            json!([
                { "Fn::Sub": "arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:28" }
            ])
        );
    }

    #[test]
    fn aws_lambda_function_gains_exec_wrapper_and_port_env_vars_when_runtime_adapter_is_some() {
        let environment = BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]);
        let adapter_params = RenderParams {
            environment,
            runtime_adapter: Some(RuntimeAdapterConfig {
                port: 9000,
                readiness_check_path: None,
            }),
            ..params(BTreeMap::new())
        };
        let (_, resource) = render_function_aws_lambda(&aws_lambda_descriptor(), &adapter_params);
        assert_eq!(
            resource.properties["Environment"]["Variables"],
            json!({
                "RUST_LOG": "info",
                "AWS_LAMBDA_EXEC_WRAPPER": "/opt/bootstrap",
                "PORT": "9000",
            })
        );
    }

    #[test]
    fn aws_lambda_function_omits_readiness_env_var_when_not_configured() {
        let adapter_params = params_with_adapter(RuntimeAdapterConfig {
            port: 8080,
            readiness_check_path: None,
        });
        let (_, resource) = render_function_aws_lambda(&aws_lambda_descriptor(), &adapter_params);
        assert!(resource.properties["Environment"]["Variables"]
            .get("AWS_LWA_READINESS_CHECK_PATH")
            .is_none());
    }

    #[test]
    fn aws_lambda_function_sets_readiness_env_var_when_configured() {
        let adapter_params = params_with_adapter(RuntimeAdapterConfig {
            port: 8080,
            readiness_check_path: Some("/healthz".to_string()),
        });
        let (_, resource) = render_function_aws_lambda(&aws_lambda_descriptor(), &adapter_params);
        assert_eq!(
            resource.properties["Environment"]["Variables"]["AWS_LWA_READINESS_CHECK_PATH"],
            json!("/healthz")
        );
    }

    #[test]
    fn aws_lambda_function_adapter_env_vars_win_over_same_named_environment_entry() {
        let environment = BTreeMap::from([("PORT".to_string(), "1234".to_string())]);
        let adapter_params = RenderParams {
            environment,
            runtime_adapter: Some(RuntimeAdapterConfig {
                port: 8080,
                readiness_check_path: None,
            }),
            ..params(BTreeMap::new())
        };
        let (_, resource) = render_function_aws_lambda(&aws_lambda_descriptor(), &adapter_params);
        assert_eq!(
            resource.properties["Environment"]["Variables"]["PORT"],
            json!("8080"),
            "the adapter's own PORT must win over a same-named [environment] entry"
        );
    }
}
