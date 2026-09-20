use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::deployment::{
    metadata::McpMetadata,
    r#trait::{BuildArtifact, DeploymentOutputs},
    stack_routing::{
        cloudformation_metadata_from, custom_stack_ts_reason, emit_descriptor_warnings,
        extract_metadata_with_log, load_deploy_descriptor, mark_custom_stack,
    },
    DeployConfig,
};

use super::artifact::{detect_shape, ServerShape};
use super::engine::{self, EngineParams};

/// Deploy to AWS Lambda.
///
/// Routes between the pure `pmcp-cfn-renderer` + native CFN engine (Task 9 —
/// no Node.js, no CDK, no `npx`) and the legacy `DeployExecutor` (`npx cdk
/// deploy`), using the SAME "unmodified scaffold vs. hand-customized
/// `stack.ts`" routing rule Task 7 established for the `pmcp-run` target
/// (`crate::deployment::stack_routing::custom_stack_ts_reason` — lifted out
/// of the `pmcp-run` target so both reuse the identical decision instead of
/// duplicating it).
///
/// # Environment/secrets on the renderer path (Task 9 investigation)
///
/// `extra_env` (= [`DeployConfig::deploy_env_vars`], the merged `[environment]`
/// plus resolved `[secrets]`, secrets win) is threaded through ONLY to the
/// legacy fallback branch, unchanged from before this task. It is
/// deliberately NOT used to build the renderer path's
/// `pmcp_cfn_renderer::RenderParams::environment` — investigation traced
/// EXACTLY which vars the `aws-lambda` scaffold's `stack.ts` (both the plain
/// and the Cognito+DCR `create_oauth_stack_ts` variants —
/// `commands/deploy/init.rs`) bakes into the Lambda's `environment: {}`
/// block, and found it is a FULLY HARDCODED literal (`RUST_LOG: 'info'`,
/// plus fixed Cognito vars on the OAuth variant) with NO `process.env.<KEY>`
/// read for `[environment]`/`[secrets]` at all — unlike the sibling
/// `pmcp-run` scaffold, which reads exactly two fixed-named vars
/// (`PMCP_ORGANIZATION_ID`/`MCP_SERVERS_TABLE`) that way.
/// `deploy_env_vars()` DOES reach the `cdk deploy` child process as
/// environment variables (see `DeployExecutor::run_cdk_deploy`), but on the
/// UNMODIFIED scaffold (the only shape this routing rule ever sends down
/// the renderer path — a hand-modified stack.ts always falls back to the
/// legacy branch) that process-env value is simply never read back out by
/// the generated TypeScript, so it never reaches the deployed Lambda's
/// actual environment variables either way. `deployment::config::
/// stack_ts_preserved_inert_warning`'s own text independently documents this
/// exact gap ("[environment] ... on the aws-lambda target it still reaches
/// the Lambda only if stack.ts reads it via process.env.<KEY>"). Matching
/// legacy therefore means: `RenderParams::environment` carries EXACTLY the
/// fixed `RUST_LOG=info` literal (matching every T5/T6 golden fixture
/// byte-for-byte), never `config.environment`/`config.secrets` — this is a
/// pre-existing legacy limitation this task reproduces faithfully rather
/// than silently fixing OR silently worsening (per the controller brief).
/// No secret VALUE is ever read on this path, so none can reach the
/// template.
pub async fn deploy_aws_lambda(
    config: &DeployConfig,
    artifact: BuildArtifact,
    extra_env: HashMap<String, String>,
) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to AWS Lambda...");
    println!();

    if let Some(reason) = custom_stack_ts_reason(config)? {
        warn_falling_back_to_legacy(&reason);
        // The taint (`mark_custom_stack`) is computed for parity with the
        // `pmcp-run` routing precedent, but has no live consumer on this
        // target today: `DeployExecutor::run_cdk_deploy` passes no `-c`
        // context args to `cdk deploy` at all (unlike `pmcp-run`'s
        // `cdk synth`, which reads the taint back out via
        // `to_cdk_context()`), so there is nothing downstream to feed it
        // into yet. Computing it here keeps the two targets' routing
        // structurally identical and ready for a future consumer without
        // adding any behavior change today.
        let metadata = extract_metadata_with_log(&config.project_root);
        let _tainted = mark_custom_stack(metadata.as_ref());
        return deploy_legacy(config, extra_env).await;
    }

    let descriptor = match load_deploy_descriptor(config) {
        Ok(d) => d,
        Err(e) => {
            warn_falling_back_to_legacy(&format!(
                "{} does not parse as pmcp-cfn-renderer's DeployDescriptor: {e:#}",
                ".pmcp/deploy.toml"
            ));
            return deploy_legacy(config, extra_env).await;
        },
    };
    emit_descriptor_warnings(&descriptor);

    match try_render_and_deploy(config, &artifact, &descriptor).await {
        Ok(outputs) => Ok(outputs),
        Err(RenderOrDeployError::Render(reason)) => {
            warn_falling_back_to_legacy(&format!(
                "pmcp-cfn-renderer cannot render this descriptor yet: {reason}"
            ));
            deploy_legacy(config, extra_env).await
        },
        // Once account resolution / AWS deploy calls are underway this is a
        // real failure, never a legacy fallback trigger — falling back to a
        // totally different deploy mechanism (`cdk deploy`) mid-deploy could
        // leave the SAME stack name in a confusing double-managed state.
        Err(RenderOrDeployError::Deploy(e)) => Err(e),
    }
}

/// Split error type for [`deploy_aws_lambda`]'s two fallible phases: a
/// `Render` failure (the descriptor declares something the renderer's
/// resource-family surface doesn't implement yet — see `pmcp_cfn_renderer`'s
/// crate docs) gracefully falls back to the legacy path, same as Task 7's
/// `try_render`; a `Deploy` failure (AWS credentials, CFN/S3 API errors,
/// stack rollback) is a hard error.
enum RenderOrDeployError {
    Render(String),
    Deploy(anyhow::Error),
}

/// The renderer+engine path: resolve the account, build `RenderParams`,
/// render, and deploy via [`engine::deploy_stack`]. Split out of
/// [`deploy_aws_lambda`] to keep that function within the complexity gate
/// (mirrors the `capture.rs` poller precedent's fetch/classify-helper split).
async fn try_render_and_deploy(
    config: &DeployConfig,
    artifact: &BuildArtifact,
    descriptor: &pmcp_package::package::DeployDescriptor,
) -> Result<DeploymentOutputs, RenderOrDeployError> {
    let zip_path = extract_zip_path(artifact).map_err(RenderOrDeployError::Deploy)?;
    let zip_bytes = std::fs::read(&zip_path)
        .with_context(|| format!("failed to read {}", zip_path.display()))
        .map_err(RenderOrDeployError::Deploy)?;

    let region = config.aws().region.clone();
    let account_id = engine::resolve_account_id(&region)
        .await
        .context(
            "resolving AWS account via STS — required for both the CFN engine and (if it were \
             used) `cdk deploy`, which also needs valid AWS credentials for this region",
        )
        .map_err(RenderOrDeployError::Deploy)?;

    // Deliberate recompute (simplify-wave item 10): `mod.rs`'s `build()` also
    // calls `detect_shape` to acquire the artifact, but `ServerShape` isn't
    // threaded through `BuildArtifact` into this `deploy()` call — that enum
    // is shared across every deploy target (pmcp-run, cloudflare,
    // google-cloud-run, ...), so adding an `aws-lambda`-only shape field to
    // it would ripple into every other target's construction/match sites.
    // Recomputing here is cheap (two file-existence checks over an
    // already-parsed, immutable `config`) and keeps `BuildArtifact` a clean
    // cross-target type.
    let shape = detect_shape(config).map_err(RenderOrDeployError::Deploy)?;
    let runtime_adapter = runtime_adapter_for(&shape);

    let bucket = engine::bucket_name(&account_id, &region);
    // Single hash of `zip_bytes` for this whole deploy — `digest`/`s3_key`
    // feed BOTH the template's `ArtifactRef` (below) and `EngineParams`
    // (further down), so `engine::deploy_stack` never needs to re-read the
    // zip from disk or re-derive this key a second time (simplify-wave
    // item 7).
    let (digest, s3_key) = engine::artifact_s3_key(&config.server.name, &zip_bytes);

    let metadata = extract_metadata_with_log(&config.project_root).map(|mut m| {
        m.apply_config_overrides(&config.metadata);
        m
    });

    let params = build_render_params(
        config,
        metadata.as_ref(),
        &account_id,
        pmcp_cfn_renderer::ArtifactRef {
            s3_bucket: bucket.clone(),
            s3_key: s3_key.clone(),
            digest: Some(format!("sha256:{digest}")),
        },
        runtime_adapter,
    );

    let template = pmcp_cfn_renderer::render(descriptor, &params)
        .map(|t| t.to_canonical_json())
        .map_err(|e| RenderOrDeployError::Render(e.to_string()))?;
    println!("✅ CloudFormation template rendered (pmcp-cfn-renderer)");

    // `[server] memory_mb` divergence (debug session
    // `deploy-server-memory-timeout`). This engine pins `MemorySize` to
    // `AWS_LAMBDA_MEMORY_SIZE_MB` regardless of what the descriptor declares,
    // so a declared `memory_mb` is dropped here — say so instead of dropping
    // it in silence. `timeout_seconds` passes `None`/`None` because this
    // engine DOES thread it from the descriptor (`timeout_seconds:
    // d.server.timeout_seconds`), so there is nothing to warn about.
    if let Some(warning) = crate::deployment::config::sizing_divergence_warning(
        "aws-lambda pmcp-cfn-renderer",
        (config.server.memory_mb, None),
        (
            u32::try_from(pmcp_cfn_renderer::resources::lambda::AWS_LAMBDA_MEMORY_SIZE_MB).ok(),
            None,
        ),
        "This engine pins the MCP function's memory. Deploy to the pmcp-run target, which honors \
         the declared sizing.",
    ) {
        eprintln!("{warning}");
        println!();
    }

    let engine_params = EngineParams {
        stack_name: params.stack_name,
        region,
        artifact_bytes: zip_bytes,
        s3_key,
        bucket,
        project_root: config.project_root.clone(),
    };

    let outputs = engine::deploy_stack(&template, engine_params)
        .await
        .map_err(RenderOrDeployError::Deploy)?;
    outputs.display();
    Ok(outputs)
}

/// `RenderParams::runtime_adapter` for `shape` (T8 review fix wiring, Task
/// 9's missing link): `ServerShape::BuiltIn` artifacts speak plain HTTP and
/// need the AWS Lambda Web Adapter bridge (see
/// `pmcp_cfn_renderer::RuntimeAdapterConfig`'s doc comment); `CustomRust`
/// artifacts link `lambda_runtime` directly and need none. Port 8080 matches
/// both the adapter's own default AND what `aws_lambda::artifact`'s
/// bootstrap wrapper script passes to the wrapped binary's `--http` flag.
/// `readiness_check_path: None` for every built-in server_type — T8's own
/// investigation already established that none of the three Shape A
/// binaries (`pmcp-sql-server`/`pmcp-openapi-server`/`pmcp-workbook-server`)
/// expose a dedicated health route, so the adapter's own default (`GET /`,
/// healthy on any 100-499 status) already works for all of them; there is no
/// per-server-type default to differentiate.
fn runtime_adapter_for(shape: &ServerShape) -> Option<pmcp_cfn_renderer::RuntimeAdapterConfig> {
    const DEFAULT_ADAPTER_PORT: u16 = 8080;
    match shape {
        ServerShape::BuiltIn { .. } => Some(pmcp_cfn_renderer::RuntimeAdapterConfig {
            port: DEFAULT_ADAPTER_PORT,
            readiness_check_path: None,
        }),
        ServerShape::CustomRust => None,
    }
}

/// The fixed environment the `aws-lambda` scaffold's `stack.ts` hardcodes —
/// see [`deploy_aws_lambda`]'s doc comment for the investigation this
/// reproduces. Matches every T5/T6 `pmcp-cfn-renderer` golden fixture's
/// `params.environment` byte-for-byte.
fn scaffold_fixed_environment() -> BTreeMap<String, String> {
    BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())])
}

/// Build [`pmcp_cfn_renderer::RenderParams`] for the `aws-lambda` target.
fn build_render_params(
    config: &DeployConfig,
    metadata: Option<&McpMetadata>,
    account_id: &str,
    artifact: pmcp_cfn_renderer::ArtifactRef,
    runtime_adapter: Option<pmcp_cfn_renderer::RuntimeAdapterConfig>,
) -> pmcp_cfn_renderer::RenderParams {
    pmcp_cfn_renderer::RenderParams {
        account_id: account_id.to_string(),
        region: config.aws().region.clone(),
        stack_name: format!("{}-stack", config.server.name),
        artifact,
        environment: scaffold_fixed_environment(),
        cloudformation_metadata: cloudformation_metadata_from(metadata),
        runtime_adapter,
    }
}

/// Extract the deployable zip path from a [`BuildArtifact`] — prefers the
/// deployment package (always present for `aws-lambda`, since Task 8's
/// `artifact::acquire_artifact` always wraps its output in a zip), falling
/// back to the bare path defensively.
fn extract_zip_path(artifact: &BuildArtifact) -> Result<PathBuf> {
    match artifact {
        BuildArtifact::Binary {
            deployment_package: Some(p),
            ..
        }
        | BuildArtifact::Wasm {
            deployment_package: Some(p),
            ..
        }
        | BuildArtifact::Custom {
            deployment_package: Some(p),
            ..
        } => Ok(p.clone()),
        BuildArtifact::Binary { path, .. }
        | BuildArtifact::Wasm { path, .. }
        | BuildArtifact::Custom { path, .. } => Ok(path.clone()),
    }
}

/// Print the standard "falling back to the legacy deploy path" advisory, in
/// the same yellow `warning:` style as `crate::deployment::iam::emit_warnings`
/// and the `pmcp-run` target's own `warn_falling_back_to_cdk`.
fn warn_falling_back_to_legacy(reason: &str) {
    eprintln!(
        "  {} {reason} — falling back to the legacy `cdk deploy` path for this deploy.",
        console::style("warning:").yellow()
    );
}

/// Deploy via the original `DeployExecutor` (`npx cdk deploy`) — unchanged
/// from before Task 9.
///
/// `extra_env` carries the merged transient env-var map from
/// [`DeployConfig::deploy_env_vars`] — developer-declared `[environment]`
/// values plus deploy-time-resolved `[secrets]` (secrets win on collision).
/// Both are forwarded as transient process env vars to the CDK child process
/// and consumed by the stack.ts via `process.env` (when it reads it — see
/// [`deploy_aws_lambda`]'s doc comment for the unmodified-scaffold gap this
/// only matters for a HAND-CUSTOMIZED stack.ts). Both are **never** written
/// to `deploy.toml` (per D-05/D-06).
async fn deploy_legacy(
    config: &DeployConfig,
    extra_env: HashMap<String, String>,
) -> Result<DeploymentOutputs> {
    let executor =
        crate::commands::deploy::deploy::DeployExecutor::new(config.project_root.clone())
            .with_extra_env(extra_env)
            .with_regenerate_stack(config.regenerate_stack);
    executor.execute()?;

    let stack_name = format!("{}-stack", config.server.name);
    crate::deployment::load_cdk_outputs(&config.project_root, &config.aws().region, &stack_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_fixed_environment_matches_the_golden_fixture_shape() {
        let env = scaffold_fixed_environment();
        assert_eq!(env.len(), 1);
        assert_eq!(env.get("RUST_LOG"), Some(&"info".to_string()));
    }

    #[test]
    fn runtime_adapter_for_builtin_is_some_with_port_8080_and_no_readiness_path() {
        let shape = ServerShape::BuiltIn {
            server_type: "sql-server".to_string(),
        };
        let adapter = runtime_adapter_for(&shape).expect("BuiltIn must wire the adapter");
        assert_eq!(adapter.port, 8080);
        assert_eq!(adapter.readiness_check_path, None);
    }

    #[test]
    fn runtime_adapter_for_builtin_is_some_for_every_server_type() {
        for server_type in ["sql-server", "openapi-server", "workbook-server"] {
            let shape = ServerShape::BuiltIn {
                server_type: server_type.to_string(),
            };
            assert!(
                runtime_adapter_for(&shape).is_some(),
                "{server_type} must wire the adapter"
            );
        }
    }

    #[test]
    fn runtime_adapter_for_custom_rust_is_none() {
        assert_eq!(runtime_adapter_for(&ServerShape::CustomRust), None);
    }

    #[test]
    fn extract_zip_path_prefers_deployment_package() {
        let artifact = BuildArtifact::Binary {
            path: PathBuf::from("/tmp/bootstrap"),
            size: 10,
            deployment_package: Some(PathBuf::from("/tmp/artifact.zip")),
        };
        assert_eq!(
            extract_zip_path(&artifact).unwrap(),
            PathBuf::from("/tmp/artifact.zip")
        );
    }

    #[test]
    fn extract_zip_path_falls_back_to_bare_path_when_no_package() {
        let artifact = BuildArtifact::Binary {
            path: PathBuf::from("/tmp/bootstrap"),
            size: 10,
            deployment_package: None,
        };
        assert_eq!(
            extract_zip_path(&artifact).unwrap(),
            PathBuf::from("/tmp/bootstrap")
        );
    }

    #[test]
    fn build_render_params_stack_name_matches_convention() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-east-1".to_string(),
            tmp.path().to_path_buf(),
        );
        config.target.target_type = "aws-lambda".to_string();

        let params = build_render_params(
            &config,
            None,
            "123456789012",
            pmcp_cfn_renderer::ArtifactRef {
                s3_bucket: "bucket".to_string(),
                s3_key: "key.zip".to_string(),
                digest: None,
            },
            None,
        );
        assert_eq!(params.stack_name, "demo-server-stack");
        assert_eq!(params.account_id, "123456789012");
        assert_eq!(
            params.environment.get("RUST_LOG"),
            Some(&"info".to_string())
        );
    }

    /// Interfaces §5 parity with the `pmcp-run` target's own
    /// `build_render_params`: `RenderParams.environment` must never carry
    /// `config.secrets` regardless of what `config.environment`/
    /// `config.secrets` declare — for `aws-lambda` this holds trivially
    /// because the fixed scaffold literal never reads either map at all
    /// (see this module's own doc comment for the investigation).
    #[test]
    fn build_render_params_environment_never_reflects_declared_secrets() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-east-1".to_string(),
            tmp.path().to_path_buf(),
        );
        config.target.target_type = "aws-lambda".to_string();
        config
            .environment
            .insert("CUSTOM_VAR".to_string(), "declared".to_string());
        config
            .secrets
            .insert("API_TOKEN".to_string(), "shhh".to_string());

        let params = build_render_params(
            &config,
            None,
            "123456789012",
            pmcp_cfn_renderer::ArtifactRef {
                s3_bucket: "bucket".to_string(),
                s3_key: "key.zip".to_string(),
                digest: None,
            },
            None,
        );
        assert!(
            !params.environment.contains_key("API_TOKEN"),
            "secret keys/values must never reach RenderParams.environment"
        );
        assert!(
            !params.environment.contains_key("CUSTOM_VAR"),
            "declared [environment] is inert on the unmodified aws-lambda scaffold today \
             (matches legacy — see this module's doc comment)"
        );
    }
}
