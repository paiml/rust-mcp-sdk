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
//! `deploy/lib/stack.ts` on disk must byte-match what `cargo pmcp` itself
//! would (re)generate for the current `.pmcp/deploy.toml` for the pure
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

/// `Some(reason)` naming `deploy/lib/stack.ts` when it no longer matches
/// what [`crate::commands::deploy::init::render_stack_ts_for_deploy`] would
/// generate for the current `.pmcp/deploy.toml` — `None` when it still
/// matches the scaffold, or the file is absent (before any synth/deploy has
/// ever run; the caller's own stack.ts-regeneration guard always writes it
/// first in practice, but this stays total).
///
/// Reuses `render_stack_ts_for_deploy` — the SAME function every target's
/// stack.ts-regeneration guard already calls to (re)write the file — rather
/// than re-deriving stack.ts content; this only adds the byte-equality
/// comparison on top.
pub(crate) fn custom_stack_ts_reason(config: &DeployConfig) -> Result<Option<String>> {
    let stack_ts_path = config
        .project_root
        .join("deploy")
        .join("lib")
        .join("stack.ts");
    if !stack_ts_path.exists() {
        return Ok(None);
    }
    let on_disk = std::fs::read_to_string(&stack_ts_path)
        .with_context(|| format!("Failed to read {}", stack_ts_path.display()))?;
    let expected = crate::commands::deploy::init::render_stack_ts_for_deploy(
        &config.target.target_type,
        &config.server.name,
        &config.iam,
        &config.metadata,
    );
    if on_disk == expected {
        Ok(None)
    } else {
        Ok(Some(format!(
            "{} was hand-modified (no longer matches the regenerated scaffold)",
            stack_ts_path.display()
        )))
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
