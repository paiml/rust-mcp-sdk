//! Shared "unmodified scaffold vs. hand-customized `stack.ts`" routing
//! decision — lifted out of the `pmcp-run` target (Task 7, CFN-renderer
//! extraction) so the `aws-lambda` target (Task 9, CFN deploy engine) can
//! reuse the IDENTICAL rule instead of duplicating it.
//!
//! Every function here is generic across deploy targets already: they take
//! `&DeployConfig`/`Option<&McpMetadata>` and consult
//! `config.target.target_type` internally (via
//! [`crate::commands::deploy::init::render_stack_ts_for_deploy`], which
//! branches on the target type itself) rather than assuming one. Nothing in
//! this module is `pmcp-run`-specific.
//!
//! # Routing rule
//!
//! `deploy/lib/stack.ts` on disk must still be the scaffold `cargo pmcp`
//! itself last wrote (recorded in `deploy/.pmcp-scaffold.toml`, see
//! [`crate::deployment::scaffold_provenance`]) for the pure
//! `pmcp-cfn-renderer` path to even be attempted — see
//! [`custom_stack_ts_reason`]. A hand-modified stack.ts always falls back to
//! the target's legacy (CDK-based) deploy path so operator customizations
//! keep working, and is additionally tainted via [`mark_custom_stack`] so
//! the platform can (where a live consumer exists) tell the two shapes
//! apart from the synthesized template's own `mcp:*` metadata.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

use crate::deployment::config::DeployConfig;
use crate::deployment::metadata::McpMetadata;
use pmcp_package::package::DeployDescriptor;

/// `Some(reason)` naming `deploy/lib/stack.ts` when it was hand-modified —
/// `None` when it is still cargo-pmcp's own, unmodified scaffold, or the file
/// is absent (before any synth/deploy has ever run; the caller's own
/// stack.ts-regeneration guard always writes it first in practice, but this
/// stays total).
///
/// "Hand-modified" means the file differs from the stack.ts cargo-pmcp last
/// WROTE (recorded in `deploy/.pmcp-scaffold.toml`), NOT from what the current
/// `.pmcp/deploy.toml` would render. A `[server] name` rename, an `[iam]` or
/// `[metadata]` change, or a newer cargo-pmcp template therefore no longer
/// reclassifies an untouched scaffold as hand-modified (debug session
/// `cargo-pmcp-deploy-targets`, finding #4). See
/// [`crate::deployment::scaffold_provenance::classify_stack_ts`], which also
/// covers projects initialized before the record existed.
pub(crate) fn custom_stack_ts_reason(config: &DeployConfig) -> Result<Option<String>> {
    use crate::deployment::scaffold_provenance::{classify_stack_ts, StackTsState};
    match classify_stack_ts(config)? {
        StackTsState::Missing | StackTsState::Untouched => Ok(None),
        StackTsState::HandModified => Ok(Some(format!(
            "{} was hand-modified (it no longer matches the scaffold cargo-pmcp last wrote)",
            config
                .project_root
                .join("deploy")
                .join("lib")
                .join("stack.ts")
                .display()
        ))),
    }
}

/// Clone `metadata` (if present) with `custom_stack` set.
///
/// This is the same `[metadata]`-derived map that `server_type`/
/// `snapshot_baked` already ride on via `McpMetadata::apply_config_overrides`
/// on its way into synth/render context — see `McpMetadata::custom_stack`'s
/// doc comment. Whether the resulting taint has a live consumer downstream
/// is target-specific (the `pmcp-run` target's `cdk synth` reads it back out
/// of `to_cdk_context()`; the `aws-lambda` target's legacy `cdk deploy`
/// passes no `-c` context args at all today, so the taint is computed for
/// parity/forward-compat but has no current sink there — see the `aws-lambda`
/// `deploy.rs` call site's own doc comment).
pub(crate) fn mark_custom_stack(metadata: Option<&McpMetadata>) -> Option<McpMetadata> {
    metadata.map(|m| {
        let mut m = m.clone();
        m.custom_stack = true;
        m
    })
}

/// Parse `.pmcp/deploy.toml` as the renderer's closed-set [`DeployDescriptor`]
/// — a NARROWER type than `DeployConfig`: it fails to parse a table the
/// renderer's descriptor doesn't model yet (e.g. `[aws].account_id`), which
/// callers treat as a graceful legacy-deploy fallback, never a hard error.
pub(crate) fn load_deploy_descriptor(config: &DeployConfig) -> Result<DeployDescriptor> {
    let path = config.project_root.join(".pmcp").join("deploy.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))
}

/// The `.pmcp/deploy.toml` keys cargo-pmcp applies itself, outside the
/// [`DeployDescriptor`]: `[build]` drives `cargo lambda build`,
/// `[server] ephemeral_storage_mb` is merged into the rendered template
/// (`deployment::template_merge`), `[server] mcp_path` is the endpoint path
/// the CLI verifies and prints (`deployment::mcp_endpoint`), and
/// `[gcp] repository` is the Cloud Run image repository (a `[gcp]` table
/// survives a re-init for another target). `(table, key)`; a `None` key names
/// the whole table.
pub const CLI_APPLIED_KEYS: [(&str, Option<&str>); 4] = [
    ("build", None),
    ("server", Some("ephemeral_storage_mb")),
    ("server", Some("mcp_path")),
    ("gcp", Some("repository")),
];

/// [`load_deploy_descriptor`] for the deploy RENDERER path: the keys in
/// [`CLI_APPLIED_KEYS`] are removed before parsing, because the closed-set
/// descriptor does not model them and cargo-pmcp applies them itself. Without
/// this, declaring one would silently send an unmodified scaffold to the
/// legacy `npx cdk` path (debug session `cargo-pmcp-deploy-targets`).
///
/// `cargo pmcp package save` keeps the strict [`load_deploy_descriptor`]: a
/// package carries the descriptor alone, so a key it cannot carry must stop
/// the save rather than vanish from the artifact.
pub fn load_render_descriptor(config: &DeployConfig) -> Result<DeployDescriptor> {
    let path = config.project_root.join(".pmcp").join("deploy.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let mut table: toml::Table =
        toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))?;
    for (section, key) in CLI_APPLIED_KEYS {
        match key {
            None => {
                table.remove(section);
            },
            Some(key) => {
                if let Some(section) = table.get_mut(section).and_then(toml::Value::as_table_mut) {
                    section.remove(key);
                }
            },
        }
    }
    table
        .try_into()
        .with_context(|| format!("Failed to parse {}", path.display()))
}

/// Call `pmcp_cfn_renderer::resources::iam::validate`/`cognito::validate`
/// directly on the descriptor and print every advisory finding — the fix
/// for the tracked T4/T6 gap where `pmcp_cfn_renderer::render` discards
/// these warnings (it is a pure function with no I/O to print them
/// through). A hard `iam::validate` error is swallowed here: the caller's
/// subsequent `render()` call re-validates the same `[iam]` section and
/// surfaces the identical failure as its own `Err` (routed to the legacy
/// fallback), so this function only ever needs the `Ok` (warnings) case.
pub(crate) fn emit_descriptor_warnings(descriptor: &DeployDescriptor) {
    let mut warnings = Vec::new();
    if let Some(iam) = &descriptor.iam {
        if let Ok(iam_warnings) = pmcp_cfn_renderer::resources::iam::validate(iam) {
            warnings.extend(iam_warnings);
        }
    }
    warnings.extend(pmcp_cfn_renderer::resources::cognito::validate(
        &descriptor.auth,
    ));
    for w in &warnings {
        eprintln!("  {} {}", console::style("warning:").yellow(), w.message);
    }
}

/// Populate [`pmcp_cfn_renderer::RenderParams::cloudformation_metadata`]
/// from the EXISTING maintained DSTK-03 shape,
/// [`McpMetadata::to_cloudformation_metadata`] — the same `mcp:*`
/// provenance object both the legacy `cdk` path (via `stack.ts`'s
/// `this.node.tryGetContext` reads, fed by `McpMetadata::to_cdk_context`)
/// and any renderer-path caller share.
///
/// `to_cloudformation_metadata` returns a `serde_json::Value::Object`; this
/// flattens it into the `BTreeMap` `RenderParams::cloudformation_metadata`
/// carries. `None` yields an empty map, which `CfnTemplate`'s own "omit
/// `Metadata` when empty" envelope rule already treats as "no metadata
/// block".
pub(crate) fn cloudformation_metadata_from(
    metadata: Option<&McpMetadata>,
) -> BTreeMap<String, serde_json::Value> {
    metadata
        .map(McpMetadata::to_cloudformation_metadata)
        .and_then(|value| value.as_object().cloned())
        .map(|object| object.into_iter().collect())
        .unwrap_or_default()
}

/// Extract MCP metadata for `project_root` and log what was found. Returns
/// `None` when the project has no metadata (defaults apply).
pub(crate) fn extract_metadata_with_log(project_root: &Path) -> Option<McpMetadata> {
    println!("📋 Extracting MCP server metadata...");
    match McpMetadata::extract(project_root) {
        Ok(m) => {
            println!("   Server: {} ({})", m.server_id, m.server_type);
            if !m.resources.secrets.is_empty() {
                println!("   Secrets: {}", m.resources.secrets.len());
            }
            if !m.capabilities.tools.is_empty() {
                println!("   Tools: {}", m.capabilities.tools.len());
            }
            Some(m)
        },
        Err(_) => {
            println!("   No metadata found (using defaults)");
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::config::{IamConfig, TablePermission};
    use std::path::PathBuf;

    fn cfg_with_target_and_iam(
        project_root: PathBuf,
        target_type: &str,
        iam: IamConfig,
    ) -> DeployConfig {
        let mut cfg = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-east-1".to_string(),
            project_root,
        );
        cfg.target.target_type = target_type.to_string();
        cfg.iam = iam;
        cfg
    }

    /// The routing decision is generic across targets — proves it works for
    /// `aws-lambda`, not just the `pmcp-run` target it was lifted from.
    #[test]
    fn custom_stack_ts_reason_none_when_freshly_generated_for_aws_lambda() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = cfg_with_target_and_iam(
            tmp.path().to_path_buf(),
            "aws-lambda",
            IamConfig {
                tables: vec![TablePermission {
                    name: "Users".to_string(),
                    actions: vec!["read".to_string()],
                    include_indexes: false,
                }],
                ..IamConfig::default()
            },
        );

        let lib_dir = tmp.path().join("deploy").join("lib");
        std::fs::create_dir_all(&lib_dir).expect("create deploy/lib");
        let stack_ts = crate::commands::deploy::init::render_stack_ts_for_deploy(
            &config.target.target_type,
            &config.server.name,
            &config.iam,
            &config.metadata,
        );
        std::fs::write(lib_dir.join("stack.ts"), &stack_ts).expect("write stack.ts");

        assert_eq!(
            custom_stack_ts_reason(&config).expect("check succeeds"),
            None
        );
    }

    #[test]
    fn custom_stack_ts_reason_names_the_file_when_hand_modified_for_aws_lambda() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "aws-lambda", IamConfig::default());

        let lib_dir = tmp.path().join("deploy").join("lib");
        std::fs::create_dir_all(&lib_dir).expect("create deploy/lib");
        let path = lib_dir.join("stack.ts");
        std::fs::write(&path, "// hand-curated — DO NOT CLOBBER\n").expect("seed curated");

        let reason = custom_stack_ts_reason(&config)
            .expect("check succeeds")
            .expect("hand-modified stack.ts must be detected");
        assert!(reason.contains(&path.display().to_string()));
    }

    /// Finding #4: renaming `[server] name` after `deploy init` must not send
    /// the untouched scaffold to the legacy path. Before the provenance record,
    /// the byte comparison against a render for the NEW name failed, so every
    /// renamed project deployed through `npx cdk deploy`, built the binary
    /// twice, and dropped `[server] memory_mb`/`timeout_seconds`.
    #[test]
    fn custom_stack_ts_reason_none_after_renaming_an_untouched_scaffold() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib_dir = tmp.path().join("deploy").join("lib");
        std::fs::create_dir_all(&lib_dir).expect("create deploy/lib");
        let init_time = crate::commands::deploy::init::render_stack_ts_for_deploy(
            "aws-lambda",
            "forecast-coach-lambda",
            &IamConfig::default(),
            &crate::deployment::config::MetadataConfig::default(),
        );
        std::fs::write(lib_dir.join("stack.ts"), &init_time).expect("write stack.ts");
        crate::deployment::scaffold_provenance::record_stack_ts(tmp.path(), &init_time)
            .expect("record");

        let mut renamed =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "aws-lambda", IamConfig::default());
        renamed.server.name = "forecast-coach-acme".to_string();

        assert_eq!(
            custom_stack_ts_reason(&renamed).expect("check succeeds"),
            None,
            "a renamed but untouched scaffold must stay on the renderer path"
        );
    }

    #[test]
    fn custom_stack_ts_reason_none_when_stack_ts_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "aws-lambda", IamConfig::default());
        assert_eq!(
            custom_stack_ts_reason(&config).expect("check succeeds"),
            None,
            "absent stack.ts (never synthesized/deployed yet) must not be treated as customized"
        );
    }

    #[test]
    fn mark_custom_stack_sets_the_flag() {
        let metadata = McpMetadata {
            version: "1.0".to_string(),
            server_type: "custom".to_string(),
            server_id: "srv-1".to_string(),
            template_id: None,
            template_version: None,
            resources: crate::deployment::metadata::ResourceRequirements::default(),
            capabilities: crate::deployment::metadata::ServerCapabilities::default(),
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        };
        let tainted = mark_custom_stack(Some(&metadata)).expect("Some in, Some out");
        assert!(tainted.custom_stack);
    }

    #[test]
    fn mark_custom_stack_none_stays_none() {
        assert!(mark_custom_stack(None).is_none());
    }

    #[test]
    fn load_deploy_descriptor_errors_clearly_when_deploy_toml_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "aws-lambda", IamConfig::default());
        let err = load_deploy_descriptor(&config).expect_err("missing file must error");
        assert!(err.to_string().contains("deploy.toml"));
    }

    #[test]
    fn cloudformation_metadata_from_none_is_empty() {
        assert!(cloudformation_metadata_from(None).is_empty());
    }

    /// Write `config` as `.pmcp/deploy.toml` (as `deploy init` would).
    fn write_deploy_toml(config: &DeployConfig) {
        let dir = config.project_root.join(".pmcp");
        std::fs::create_dir_all(&dir).expect("mkdir .pmcp");
        std::fs::write(
            dir.join("deploy.toml"),
            toml::to_string_pretty(config).expect("serialize"),
        )
        .expect("write deploy.toml");
    }

    /// Debug session `cargo-pmcp-deploy-targets`: declaring `[build]` or
    /// `[server] ephemeral_storage_mb` must not make the renderer path's
    /// descriptor parse fail (that silently sends an unmodified scaffold to
    /// `npx cdk deploy`). The keys are removed; everything else is the same
    /// descriptor the file would parse to without them.
    #[test]
    fn render_descriptor_accepts_the_keys_cargo_pmcp_applies_itself() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plain =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "aws-lambda", IamConfig::default());
        write_deploy_toml(&plain);
        let expected = load_deploy_descriptor(&plain).expect("plain file parses");

        let mut extended = plain.clone();
        extended.server.ephemeral_storage_mb = Some(4096);
        extended.build.features = vec!["remote-model".to_string()];
        extended.build.no_default_features = true;
        write_deploy_toml(&extended);

        assert_eq!(
            load_render_descriptor(&extended).expect("render path accepts the new keys"),
            expected
        );
    }

    /// `package save` keeps the strict loader: a package carries the
    /// descriptor alone, so a key it cannot carry stops the save.
    #[test]
    fn strict_descriptor_still_refuses_keys_it_cannot_carry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "aws-lambda", IamConfig::default());
        config.server.ephemeral_storage_mb = Some(4096);
        write_deploy_toml(&config);
        assert!(load_deploy_descriptor(&config).is_err());
    }

    /// Only the listed keys are removed: any other unknown key still fails the
    /// closed-set descriptor (and so still falls back to the legacy path).
    #[test]
    fn render_descriptor_still_refuses_other_unknown_keys() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "aws-lambda", IamConfig::default());
        write_deploy_toml(&config);
        let path = tmp.path().join(".pmcp/deploy.toml");
        let text = std::fs::read_to_string(&path).expect("read");
        std::fs::write(
            &path,
            text.replace("[server]\n", "[server]\nunknown_knob = 1\n"),
        )
        .expect("write");
        assert!(load_render_descriptor(&config).is_err());
    }

    /// An aws-lambda config that also carries a `[gcp]` table, as a project
    /// re-inited from google-cloud-run keeps it.
    fn lambda_with_kept_gcp(root: std::path::PathBuf) -> DeployConfig {
        let mut config = cfg_with_target_and_iam(root, "aws-lambda", IamConfig::default());
        config.gcp = Some(crate::deployment::GcpConfig {
            project_id: "acme".to_string(),
            region: "us-central1".to_string(),
            repository: None,
        });
        config
    }

    /// Debug session `cargo-pmcp-deploy-targets` (PR-D): `[server] mcp_path`
    /// (any target) and `[gcp] repository` (kept across a target switch) are
    /// applied by cargo-pmcp itself; the renderer path must not fall back to
    /// `npx cdk` because of them.
    #[test]
    fn render_descriptor_accepts_mcp_path_and_a_kept_gcp_repository() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plain = lambda_with_kept_gcp(tmp.path().to_path_buf());
        write_deploy_toml(&plain);
        let expected = load_deploy_descriptor(&plain).expect("plain file parses");

        let mut extended = plain.clone();
        extended.server.mcp_path = Some("/mcp".to_string());
        extended.gcp.as_mut().expect("[gcp]").repository = Some("pmcp".to_string());
        write_deploy_toml(&extended);

        assert_eq!(
            load_render_descriptor(&extended).expect("render path accepts the PR-D keys"),
            expected
        );
    }

    /// `package save` stays strict for the PR-D keys too (as for PR-B's): the
    /// descriptor cannot carry `mcp_path`, so the save stops rather than drop it.
    #[test]
    fn strict_descriptor_refuses_mcp_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "aws-lambda", IamConfig::default());
        config.server.mcp_path = Some("/mcp".to_string());
        write_deploy_toml(&config);
        let message = format!("{:#}", load_deploy_descriptor(&config).expect_err("strict"));
        assert!(message.contains("mcp_path"), "{message}");
    }

    mod render_descriptor_proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            /// For any `[server] mcp_path` / `[gcp] repository` declaration,
            /// the render-path descriptor equals the file's without them.
            #[test]
            fn endpoint_keys_never_change_the_descriptor(
                mcp_path in prop::option::of("/[a-z0-9/]{0,10}"),
                repository in prop::option::of("[a-z][a-z0-9-]{0,10}[a-z0-9]"),
            ) {
                let tmp = tempfile::tempdir().expect("tempdir");
                let plain = lambda_with_kept_gcp(tmp.path().to_path_buf());
                write_deploy_toml(&plain);
                let expected = load_deploy_descriptor(&plain).expect("plain file parses");

                let mut extended = plain.clone();
                extended.server.mcp_path = mcp_path;
                extended.gcp.as_mut().expect("[gcp]").repository = repository;
                write_deploy_toml(&extended);

                prop_assert_eq!(load_render_descriptor(&extended).expect("parses"), expected);
            }

            /// For any `[build]` / `ephemeral_storage_mb` declaration, the
            /// render-path descriptor equals the descriptor of the same file
            /// without them.
            #[test]
            fn cli_applied_keys_never_change_the_descriptor(
                ephemeral in prop::option::of(any::<u32>()),
                features in prop::collection::vec("[a-z][a-z0-9_-]{0,10}", 0..4),
                no_default in any::<bool>(),
            ) {
                let tmp = tempfile::tempdir().expect("tempdir");
                let plain = cfg_with_target_and_iam(
                    tmp.path().to_path_buf(),
                    "aws-lambda",
                    IamConfig::default(),
                );
                write_deploy_toml(&plain);
                let expected = load_deploy_descriptor(&plain).expect("plain file parses");

                let mut extended = plain.clone();
                extended.server.ephemeral_storage_mb = ephemeral;
                extended.build.features = features;
                extended.build.no_default_features = no_default;
                write_deploy_toml(&extended);

                prop_assert_eq!(load_render_descriptor(&extended).expect("parses"), expected);
            }
        }
    }

    #[test]
    fn emit_descriptor_warnings_does_not_panic_on_a_clean_descriptor() {
        let descriptor: DeployDescriptor = toml::from_str(
            r#"
            [target]
            type = "aws-lambda"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "scratch"
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
        .expect("fixture descriptor parses");
        emit_descriptor_warnings(&descriptor);
    }
}
