use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::deployment::{
    lambda_function,
    metadata::McpMetadata,
    r#trait::{BuildArtifact, DeploymentOutputs},
    stack_routing::{
        cloudformation_metadata_from, custom_stack_ts_reason, emit_descriptor_warnings,
        extract_metadata_with_log, load_render_descriptor, mark_custom_stack,
    },
    template_merge, DeployConfig,
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
/// # `[environment]`, `[server]` sizing and secrets
///
/// Both engines deliver the same function settings, from
/// [`lambda_function`] (debug session `cargo-pmcp-deploy-targets`, #13/#16;
/// before that the renderer pinned `RUST_LOG=info` and 512 MB, and the
/// scaffold `stack.ts` hardcoded the same literals, whatever deploy.toml
/// declared):
///
/// - renderer path: `[environment]` (over the `RUST_LOG=info` default) goes in
///   through `RenderParams::environment`, and `[server] memory_mb`/
///   `timeout_seconds`/`ephemeral_storage_mb` are merged into the rendered
///   template ([`render_template`]);
/// - legacy path: the unmodified scaffold is regenerated with the same values
///   (`commands::deploy::init::render_stack_ts_for_config`); a hand-modified
///   `stack.ts` is authoritative, as before.
///
/// `extra_env` (= [`DeployConfig::deploy_env_vars`], the merged
/// `[environment]` plus resolved `[secrets]`, secrets win) still reaches only
/// the legacy `cdk deploy` child process, for a hand-modified `stack.ts` that
/// reads `process.env`. Resolved `[secrets]` are NOT delivered to the
/// function by the renderer path or by the scaffold (out of scope; secret
/// values must never be written into a template or `stack.ts`). Only secret
/// NAMES are read here — to keep a same-named `[environment]` entry out — so
/// no secret value can reach the template.
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

    let descriptor = match load_render_descriptor(config) {
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
        Ok(outputs) => {
            keep_scaffolds_in_line(config);
            Ok(outputs)
        },
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

/// After a native-engine deploy, regenerate cargo-pmcp's own, unmodified
/// `deploy/lib/stack.ts` and `deploy/bin/app.ts` from the config just
/// deployed (#409): the engine never reads them, but a later fallback to `npx
/// cdk deploy` does, and after a rename the stack guard would refuse a stale
/// `app.ts`. The deploy has already succeeded, so a failure here is a
/// warning, never a failed deploy.
fn keep_scaffolds_in_line(config: &DeployConfig) {
    if let Err(err) =
        crate::deployment::scaffold_provenance::sync_scaffolds_after_native_deploy(config)
    {
        eprintln!(
            "  {} could not regenerate the unmodified deploy/ scaffolds for `{}` ({err:#}); a later \
             `npx cdk deploy` fallback may refuse until they name the deployed stack.",
            console::style("warning:").yellow(),
            config.server.name
        );
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

    let template = render_template(
        config,
        descriptor,
        metadata.as_ref(),
        &account_id,
        pmcp_cfn_renderer::ArtifactRef {
            s3_bucket: bucket.clone(),
            s3_key: s3_key.clone(),
            digest: Some(format!("sha256:{digest}")),
        },
        runtime_adapter,
    )?;

    let engine_params = EngineParams {
        stack_name: crate::deployment::cdk_stack_guard::expected_stack_name(&config.server.name),
        region,
        artifact_bytes: zip_bytes,
        s3_key,
        bucket,
        project_root: config.project_root.clone(),
        function_name: config.server.name.clone(),
    };

    // Returned, not printed: `cargo pmcp deploy` prints the outputs exactly
    // once (A2 — this used to print them a second time).
    engine::deploy_stack(&template, engine_params)
        .await
        .map_err(RenderOrDeployError::Deploy)
}

/// Render the `aws-lambda` stack for `config`: build the `RenderParams`,
/// render `descriptor`, and apply the declared `[server]` sizing to the MCP
/// function. Pure (no AWS call), so the whole config-to-template wiring is
/// unit-testable; [`try_render_and_deploy`] only adds the account lookup and
/// the engine.
///
/// `[environment]` goes in through `RenderParams::environment` (see
/// [`build_render_params`]), so the renderer's own infrastructure variables
/// (the Lambda Web Adapter's for a built-in server, the Cognito wiring on an
/// OAuth stack) keep winning over a same-named key; any key that loses is
/// named in a warning. `[server]` sizing goes in after rendering, through the
/// merge the `pmcp-run` target uses (`deployment::template_merge`): the
/// renderer pins `MemorySize` to its own constant and renders no
/// `EphemeralStorage` (debug session `cargo-pmcp-deploy-targets`, #13/#16).
fn render_template(
    config: &DeployConfig,
    descriptor: &pmcp_package::package::DeployDescriptor,
    metadata: Option<&McpMetadata>,
    account_id: &str,
    artifact: pmcp_cfn_renderer::ArtifactRef,
    runtime_adapter: Option<pmcp_cfn_renderer::RuntimeAdapterConfig>,
) -> Result<String, RenderOrDeployError> {
    let secret_keys = lambda_function::secret_keys(config);
    lambda_function::warn_environment_keys_shadowing_secrets(&config.environment, &secret_keys);

    let params = build_render_params(config, metadata, account_id, artifact, runtime_adapter);
    let template = pmcp_cfn_renderer::render(descriptor, &params)
        .map(|t| t.to_canonical_json())
        .map_err(|e| RenderOrDeployError::Render(e.to_string()))?;
    println!("✅ CloudFormation template rendered (pmcp-cfn-renderer)");

    let template = template_merge::apply_sizing_merge(
        template,
        &config.server.name,
        template_merge::LambdaSizing::from_server(&config.server),
    )
    .map_err(RenderOrDeployError::Deploy)?;

    let mut overridden = template_merge::environment_not_carried(
        &template,
        &config.server.name,
        &params.environment,
    )
    .map_err(RenderOrDeployError::Deploy)?;
    overridden.retain(|key| config.environment.contains_key(key));
    if !overridden.is_empty() {
        eprintln!(
            "  {} [environment] {} set by cargo-pmcp's renderer for this stack (it wires the \
             runtime); the declared value is not used.",
            console::style("warning:").yellow(),
            overridden.join(", ")
        );
    }
    Ok(template)
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

/// Build [`pmcp_cfn_renderer::RenderParams`] for the `aws-lambda` target.
///
/// `environment` is the MCP function's environment from
/// [`lambda_function::function_environment`]: `RUST_LOG=info` overridden and
/// extended by `[environment]`, minus every secret name. Only secret NAMES are
/// read (`config.secrets`' keys), never a value, so no secret can reach the
/// template.
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
        stack_name: crate::deployment::cdk_stack_guard::expected_stack_name(&config.server.name),
        artifact,
        environment: lambda_function::function_environment(
            &config.environment,
            &lambda_function::secret_keys(config),
        ),
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
/// and consumed by a HAND-CUSTOMIZED stack.ts via `process.env` (the
/// unmodified scaffold renders `[environment]` itself; see
/// [`deploy_aws_lambda`]'s doc comment). Both are **never** written to
/// `deploy.toml` (per D-05/D-06). The resolved secret NAMES travel too, so the
/// regenerated scaffold never renders a same-named `[environment]` entry.
async fn deploy_legacy(
    config: &DeployConfig,
    extra_env: HashMap<String, String>,
) -> Result<DeploymentOutputs> {
    let executor =
        crate::commands::deploy::deploy::DeployExecutor::new(config.project_root.clone())
            .with_extra_env(extra_env)
            .with_secret_keys(lambda_function::secret_keys(config))
            .with_regenerate_stack(config.regenerate_stack);
    if legacy_rebuilds_binary(&detect_shape(config)?) {
        executor.execute()
    } else {
        executor.execute_prebuilt()
    }
}

/// Whether the legacy `npx cdk deploy` fallback must build the Lambda binary
/// itself (debug session `cargo-pmcp-deploy-targets`, A5).
///
/// A custom-Rust project was already built into `deploy/.build/` by this same
/// `cargo pmcp deploy` (`target.build()` runs before routing), so building it
/// again only doubled the build. A built-in server's artifact is a downloaded
/// zip and `deploy/.build/` holds no `bootstrap` for `cdk deploy` to package,
/// so that shape keeps the executor's own build step, unchanged.
fn legacy_rebuilds_binary(shape: &ServerShape) -> bool {
    matches!(shape, ServerShape::BuiltIn { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A5: `cargo pmcp deploy` already built the custom-Rust binary into
    /// deploy/.build (`target.build()`) before routing; the legacy fallback
    /// must deploy that binary, not build it a second time.
    #[test]
    fn legacy_fallback_reuses_the_custom_rust_binary() {
        assert!(!legacy_rebuilds_binary(&ServerShape::CustomRust));
    }

    /// A built-in server's artifact is a downloaded zip, and deploy/.build
    /// holds no `bootstrap` for `cdk deploy` to package, so the legacy path
    /// keeps its own build step there (unchanged: it fails loudly, rather
    /// than deploying an asset with no bootstrap).
    #[test]
    fn legacy_fallback_still_builds_for_a_builtin_server() {
        assert!(legacy_rebuilds_binary(&ServerShape::BuiltIn {
            server_type: "sql-server".to_string()
        }));
    }

    /// With no `[environment]`, the function environment is exactly the
    /// scaffold default every T5/T6 `pmcp-cfn-renderer` golden fixture uses.
    #[test]
    fn default_function_environment_matches_the_golden_fixture_shape() {
        let env = lambda_function::function_environment(
            &HashMap::new(),
            &std::collections::BTreeSet::new(),
        );
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

    /// #16 (debug session `cargo-pmcp-deploy-targets`): declared
    /// `[environment]` reaches `RenderParams.environment` (it used to be
    /// pinned to `RUST_LOG=info`, and this test locked that in), overriding
    /// the `RUST_LOG` default. A key that is also a secret is left out, and no
    /// secret VALUE ever appears.
    #[test]
    fn build_render_params_environment_carries_declared_environment_but_never_secrets() {
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
            .environment
            .insert("RUST_LOG".to_string(), "debug".to_string());
        config
            .environment
            .insert("API_TOKEN".to_string(), "env-value".to_string());
        config
            .secrets
            .insert("API_TOKEN".to_string(), "shhh-secret-value".to_string());

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
        assert_eq!(
            params.environment.get("CUSTOM_VAR").map(String::as_str),
            Some("declared"),
            "declared [environment] must reach the function"
        );
        assert_eq!(
            params.environment.get("RUST_LOG").map(String::as_str),
            Some("debug"),
            "[environment] overrides the RUST_LOG default"
        );
        assert!(
            !params.environment.contains_key("API_TOKEN"),
            "an [environment] key that is also a secret must be left out"
        );
        assert!(
            !params
                .environment
                .values()
                .any(|v| v.contains("shhh-secret-value")),
            "no secret value may reach RenderParams.environment"
        );
    }

    /// Wiring tests: the whole config -> template path of the native engine,
    /// through [`render_template`], from a `.pmcp/deploy.toml` on disk (the
    /// file the deploy reads). Debug session `cargo-pmcp-deploy-targets`, #13
    /// and #16.
    mod render_wiring {
        use super::*;
        use serde_json::Value;

        const NAME: &str = "forecast-coach";

        fn config(root: &std::path::Path) -> DeployConfig {
            let mut config = DeployConfig::default_for_server(
                NAME.to_string(),
                "us-east-1".to_string(),
                root.to_path_buf(),
            );
            config.target.target_type = "aws-lambda".to_string();
            config
        }

        /// Write `config` as `.pmcp/deploy.toml`.
        fn write_deploy_toml(config: &DeployConfig) {
            let dir = config.project_root.join(".pmcp");
            std::fs::create_dir_all(&dir).expect("mkdir .pmcp");
            std::fs::write(
                dir.join("deploy.toml"),
                toml::to_string_pretty(config).expect("serialize config"),
            )
            .expect("write deploy.toml");
        }

        /// Read `.pmcp/deploy.toml` the way the deploy does and render it for
        /// `config` (the CLI's in-memory config).
        fn render_text(
            config: &DeployConfig,
            runtime_adapter: Option<pmcp_cfn_renderer::RuntimeAdapterConfig>,
        ) -> String {
            let descriptor =
                load_render_descriptor(config).expect("deploy.toml parses as a descriptor");
            render_template(
                config,
                &descriptor,
                None,
                "123456789012",
                pmcp_cfn_renderer::ArtifactRef {
                    s3_bucket: "bucket".to_string(),
                    s3_key: "key.zip".to_string(),
                    digest: None,
                },
                runtime_adapter,
            )
            .unwrap_or_else(|e| match e {
                RenderOrDeployError::Render(reason) => panic!("render failed: {reason}"),
                RenderOrDeployError::Deploy(e) => panic!("render failed: {e:#}"),
            })
        }

        /// Write `config` as `.pmcp/deploy.toml`, then render it.
        fn render(
            config: &DeployConfig,
            runtime_adapter: Option<pmcp_cfn_renderer::RuntimeAdapterConfig>,
        ) -> Value {
            write_deploy_toml(config);
            serde_json::from_str(&render_text(config, runtime_adapter)).expect("template is JSON")
        }

        /// The MCP function's `Properties` (the one named `[server] name`).
        fn mcp_function(template: &Value) -> Value {
            template["Resources"]
                .as_object()
                .expect("Resources")
                .values()
                .find(|r| {
                    r["Type"] == "AWS::Lambda::Function" && r["Properties"]["FunctionName"] == NAME
                })
                .expect("the MCP function is rendered")["Properties"]
                .clone()
        }

        /// #13: the renderer pinned `MemorySize` to 512 and warned. The
        /// reporter's CPU-bound tool ran 6.1 s at 512 MB and 2.1 s at 1769 MB.
        #[test]
        fn declared_memory_and_timeout_reach_the_function() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let mut config = config(tmp.path());
            config.server.memory_mb = Some(1769);
            config.server.timeout_seconds = Some(60);

            let function = mcp_function(&render(&config, None));

            assert_eq!(function["MemorySize"], 1769);
            assert_eq!(function["Timeout"], 60);
        }

        /// #407, the native half: the stack owns the MCP function's
        /// `/aws/lambda/<function>` log group and `CloudFormation` deletes it
        /// with the stack (no `Retain` `DeletionPolicy`; the default is
        /// Delete). What it cannot own is a group Lambda re-creates after
        /// that deletion; `deploy destroy` removes that one (`teardown`).
        #[test]
        fn the_native_stack_owns_the_function_log_group() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let template = render(&config(tmp.path()), None);
            let resources = template["Resources"].as_object().expect("Resources");
            let (function_id, _) = resources
                .iter()
                .find(|(_, r)| {
                    r["Type"] == "AWS::Lambda::Function" && r["Properties"]["FunctionName"] == NAME
                })
                .expect("the MCP function");
            let log_group = resources
                .values()
                .find(|r| {
                    r["Type"] == "AWS::Logs::LogGroup"
                        && r["Properties"]["LogGroupName"]
                            == serde_json::json!({
                                "Fn::Join": ["", ["/aws/lambda/", { "Ref": function_id }]]
                            })
                })
                .expect("the stack declares the function's log group");
            assert!(
                matches!(log_group["DeletionPolicy"].as_str(), None | Some("Delete")),
                "{log_group}"
            );
        }

        /// #16: `[server] ephemeral_storage_mb` sets the function's `/tmp`.
        #[test]
        fn declared_ephemeral_storage_reaches_the_function() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let mut config = config(tmp.path());
            config.server.ephemeral_storage_mb = Some(4096);

            let function = mcp_function(&render(&config, None));

            assert_eq!(function["EphemeralStorage"]["Size"], 4096);
        }

        /// Without `ephemeral_storage_mb` the template carries no
        /// `EphemeralStorage` (Lambda's 512 MB default), as before.
        #[test]
        fn undeclared_ephemeral_storage_renders_nothing() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let function = mcp_function(&render(&config(tmp.path()), None));
            assert!(function.get("EphemeralStorage").is_none(), "{function}");
        }

        /// #16: `[environment]` reaches the function on the native engine, and
        /// overrides the `RUST_LOG` default.
        #[test]
        fn declared_environment_reaches_the_function() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let mut config = config(tmp.path());
            config.environment.insert(
                "MODEL_URL".to_string(),
                "s3://models/forecast.bin".to_string(),
            );
            config
                .environment
                .insert("RUST_LOG".to_string(), "warn".to_string());

            let variables =
                mcp_function(&render(&config, None))["Environment"]["Variables"].clone();

            assert_eq!(variables["MODEL_URL"], "s3://models/forecast.bin");
            assert_eq!(variables["RUST_LOG"], "warn");
        }

        /// Out of scope (secrets need their own design), but guarded here: a
        /// resolved secret's VALUE never reaches the template, and an
        /// `[environment]` entry with a secret's name is not rendered.
        #[test]
        fn no_secret_value_reaches_the_template() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let mut config = config(tmp.path());
            config
                .environment
                .insert("API_TOKEN".to_string(), "from-environment".to_string());
            write_deploy_toml(&config);
            // Injected AFTER deploy.toml is written, as the CLI does: resolved
            // secrets live only in memory.
            config
                .secrets
                .insert("API_TOKEN".to_string(), "s3cr3t-value".to_string());

            let template_text = render_text(&config, None);

            assert!(!template_text.contains("s3cr3t-value"), "{template_text}");
            let template: Value = serde_json::from_str(&template_text).expect("json");
            assert!(
                mcp_function(&template)["Environment"]["Variables"]
                    .get("API_TOKEN")
                    .is_none(),
                "an [environment] key named like a resolved secret must not be rendered"
            );
        }

        /// The renderer's own infrastructure variables win over a same-named
        /// `[environment]` key: a built-in server's Lambda Web Adapter must
        /// keep `PORT=8080`, which its bootstrap script listens on.
        #[test]
        fn renderer_owned_variables_win_over_environment() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let mut config = config(tmp.path());
            config
                .environment
                .insert("PORT".to_string(), "3000".to_string());
            config
                .environment
                .insert("FEATURE".to_string(), "on".to_string());
            let adapter = runtime_adapter_for(&ServerShape::BuiltIn {
                server_type: "sql-server".to_string(),
            });

            let variables =
                mcp_function(&render(&config, adapter))["Environment"]["Variables"].clone();

            assert_eq!(variables["PORT"], "8080");
            assert_eq!(variables["AWS_LAMBDA_EXEC_WRAPPER"], "/opt/bootstrap");
            assert_eq!(variables["FEATURE"], "on");
        }
    }
}
