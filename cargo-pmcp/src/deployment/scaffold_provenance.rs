//! Provenance of the CDK scaffold files cargo-pmcp generates (debug session
//! `cargo-pmcp-deploy-targets`, findings #1 and #4).
//!
//! `cargo pmcp deploy` has to tell cargo-pmcp's own, unmodified
//! `deploy/lib/stack.ts` from one an operator edited. The unmodified scaffold
//! is safe to regenerate and is deployed by the native `CloudFormation` engine;
//! an edited one is preserved and deployed through `npx cdk`. That used to be
//! decided by comparing the file with what the CURRENT `.pmcp/deploy.toml`
//! would render, so a `[server] name` rename, an `[iam]`/`[metadata]` change,
//! or a newer cargo-pmcp template made the untouched scaffold look
//! hand-modified and sent it down the legacy path.
//!
//! Now it is decided by provenance. Every time cargo-pmcp writes stack.ts (at
//! `deploy init`, and whenever a deploy regenerates it) it records the file's
//! SHA-256 in `deploy/.pmcp-scaffold.toml`. "Hand-modified" means "differs
//! from what cargo-pmcp last wrote".
//!
//! Projects initialized before the record existed have none. For them the old
//! comparison runs, widened to the renders `deploy init` itself could have
//! produced (the name the file's own `serverName` literal carries, with the
//! init-time empty `[iam]`/`[metadata]`, and the scaffold's default Lambda
//! function settings that every cargo-pmcp up to 0.28.0 baked). A match is
//! adopted and recorded, so from then on the record decides; a file that
//! matches nothing stays hand-modified, exactly as before.
//!
//! `deploy/bin/app.ts` is recorded the same way since 0.28.1 (#409), under
//! `bin/app.ts`. A record written by 0.28.0 holds `lib/stack.ts` only; for it
//! (and for no record at all) app.ts is unmodified when it is exactly the
//! scaffold `deploy init` renders for some name, and is then adopted. Both
//! scaffolds follow `.pmcp/deploy.toml` on BOTH engines: the `npx cdk deploy`
//! path regenerates them before deploying, and the native engine after a
//! successful deploy ([`sync_scaffolds_after_native_deploy`]).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::commands::deploy::init::{
    app_ts_scaffold_name, render_app_ts, render_stack_ts_with_function,
};
use crate::deployment::config::{DeployConfig, IamConfig, MetadataConfig};
use crate::deployment::lambda_function::ScaffoldFunction;

/// Where the provenance record lives, relative to the project root.
pub const RECORD_RELATIVE: &str = "deploy/.pmcp-scaffold.toml";

/// The record's key for `deploy/lib/stack.ts` (relative to `deploy/`).
const STACK_TS_ENTRY: &str = "lib/stack.ts";

/// The record's key for `deploy/bin/app.ts` (relative to `deploy/`). Records
/// written before 0.28.1 have no such entry; see [`classify_app_ts`].
const APP_TS_ENTRY: &str = "bin/app.ts";

/// The literal before the server name in the `deploy/bin/app.ts` scaffold.
const APP_TS_NAME_LITERAL: &str = "const serverName = '";

/// Comment written at the top of the record.
const RECORD_HEADER: &str = "\
# Written by cargo-pmcp: the SHA-256 of each scaffold file as cargo-pmcp last
# wrote it. `cargo pmcp deploy` treats a file that still matches as its own,
# unmodified scaffold (regenerated from .pmcp/deploy.toml on deploy, and
# deployed by the native CloudFormation engine). A file that differs is
# hand-modified: it is preserved and deployed with `npx cdk deploy`.
# Commit this file with deploy/. Do not edit it.
";

/// The literals the stack.ts scaffolds bake the server name into: the
/// aws-lambda template's `serverName` constant and the pmcp-run template's
/// `serverId` context fallback.
const STACK_TS_NAME_LITERALS: [&str; 2] = [
    "const serverName = '",
    "this.node.tryGetContext('serverId') || '",
];

/// The on-disk shape of the record.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Record {
    #[serde(default)]
    files: BTreeMap<String, String>,
}

fn record_path(project_root: &Path) -> PathBuf {
    project_root.join(RECORD_RELATIVE)
}

fn stack_ts_path(project_root: &Path) -> PathBuf {
    project_root.join("deploy").join("lib").join("stack.ts")
}

fn app_ts_path(project_root: &Path) -> PathBuf {
    project_root.join("deploy").join("bin").join("app.ts")
}

/// `sha256:<hex>` of `content`, with CRLF line endings normalized to LF so a
/// `core.autocrlf` checkout does not read as an edit.
pub fn content_digest(content: &str) -> String {
    pmcp_package::digest::ManifestDigest::from_bytes(content.replace("\r\n", "\n").as_bytes())
        .as_str()
        .to_string()
}

/// The record's entries; empty when there is no record. An unreadable record
/// is ignored with a warning (the pre-record fallback then applies), never a
/// reason to fail a deploy.
fn read_entries(project_root: &Path) -> BTreeMap<String, String> {
    let path = record_path(project_root);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    match toml::from_str::<Record>(&text) {
        Ok(record) => record.files,
        Err(err) => {
            eprintln!(
                "  {} ignoring unreadable {}: {err}",
                console::style("warning:").yellow(),
                path.display()
            );
            BTreeMap::new()
        },
    }
}

/// Write `entries` back; an empty set removes the record file.
fn write_entries(project_root: &Path, entries: BTreeMap<String, String>) -> Result<()> {
    let path = record_path(project_root);
    if entries.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
    }
    let body = toml::to_string(&Record { files: entries })
        .context("failed to serialize the scaffold record")?;
    std::fs::write(&path, format!("{RECORD_HEADER}{body}"))
        .with_context(|| format!("failed to write {}", path.display()))
}

/// The recorded digest of `deploy/lib/stack.ts`, if any.
pub fn recorded_stack_ts_digest(project_root: &Path) -> Option<String> {
    read_entries(project_root).remove(STACK_TS_ENTRY)
}

/// Record `content` as the `deploy/lib/stack.ts` cargo-pmcp just wrote.
pub fn record_stack_ts(project_root: &Path, content: &str) -> Result<()> {
    let mut entries = read_entries(project_root);
    entries.insert(STACK_TS_ENTRY.to_string(), content_digest(content));
    write_entries(project_root, entries)
}

/// Drop the stack.ts entry: what is on disk now is not a scaffold a deploy
/// may regenerate (the Cognito variant), or no longer exists.
pub fn forget_stack_ts(project_root: &Path) -> Result<()> {
    let mut entries = read_entries(project_root);
    entries.remove(STACK_TS_ENTRY);
    write_entries(project_root, entries)
}

/// What `deploy/lib/stack.ts` is, relative to what cargo-pmcp wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackTsState {
    /// No stack.ts on disk.
    Missing,
    /// cargo-pmcp's own output, unmodified.
    Untouched,
    /// Edited (or written) by someone other than cargo-pmcp.
    HandModified,
}

/// Classify `deploy/lib/stack.ts` (see the module docs). A pre-record project
/// whose stack.ts is recognized as an unmodified scaffold is adopted: its
/// digest is recorded, so a later rename still sees it as unmodified.
pub fn classify_stack_ts(config: &DeployConfig) -> Result<StackTsState> {
    let path = stack_ts_path(&config.project_root);
    if !path.exists() {
        return Ok(StackTsState::Missing);
    }
    let on_disk = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if let Some(recorded) = recorded_stack_ts_digest(&config.project_root) {
        return Ok(if content_digest(&on_disk) == recorded {
            StackTsState::Untouched
        } else {
            StackTsState::HandModified
        });
    }
    if !is_a_scaffold_rendering(&on_disk, config) {
        return Ok(StackTsState::HandModified);
    }
    if let Err(err) = record_stack_ts(&config.project_root, &on_disk) {
        eprintln!(
            "  {} could not record {} as cargo-pmcp's unmodified scaffold: {err:#}",
            console::style("warning:").yellow(),
            path.display()
        );
    }
    Ok(StackTsState::Untouched)
}

/// The pre-record fallback: `on_disk` is a render cargo-pmcp produced, either
/// for the current config or at init time (empty `[iam]`/`[metadata]`), for
/// the configured name or the name the file's own literal carries, with the
/// config's Lambda function settings or the scaffold defaults. The defaults
/// are what every cargo-pmcp up to 0.28.0 baked whatever `.pmcp/deploy.toml`
/// declared, so without them a project initialized by 0.27.x that declares
/// `[server]` sizing or `[environment]` would read as hand-modified.
fn is_a_scaffold_rendering(on_disk: &str, config: &DeployConfig) -> bool {
    let on_disk = on_disk.replace("\r\n", "\n");
    let (init_iam, init_meta) = (IamConfig::default(), MetadataConfig::default());
    let inputs = [(&config.iam, &config.metadata), (&init_iam, &init_meta)];
    let functions = scaffold_function_candidates(config);
    scaffold_name_candidates(&on_disk, &config.server.name)
        .iter()
        .any(|name| {
            inputs.iter().any(|(iam, meta)| {
                functions.iter().any(|function| {
                    render_stack_ts_with_function(
                        &config.target.target_type,
                        name,
                        iam,
                        meta,
                        function,
                    ) == on_disk
                })
            })
        })
}

/// Lambda function settings `stack.ts` may have been rendered with: the
/// config's, then the scaffold defaults (when they differ).
fn scaffold_function_candidates(config: &DeployConfig) -> Vec<ScaffoldFunction> {
    let declared = ScaffoldFunction::from_config(
        config,
        &crate::deployment::lambda_function::secret_keys(config),
    );
    let defaults = ScaffoldFunction::default();
    if declared == defaults {
        vec![defaults]
    } else {
        vec![declared, defaults]
    }
}

/// Names `stack_ts` may have been rendered for: the configured one, then any
/// the file's own name literals carry.
fn scaffold_name_candidates(stack_ts: &str, configured: &str) -> Vec<String> {
    let mut names = vec![configured.to_string()];
    for prefix in STACK_TS_NAME_LITERALS {
        let literal = stack_ts.find(prefix).and_then(|at| {
            let rest = &stack_ts[at + prefix.len()..];
            rest.find('\'').map(|end| rest[..end].to_string())
        });
        if let Some(name) = literal.filter(|name| !names.contains(name)) {
            names.push(name);
        }
    }
    names
}

/// Write `rendered` as `deploy/lib/stack.ts` when cargo-pmcp may: the file is
/// missing, it is still cargo-pmcp's unmodified scaffold, or
/// `--regenerate-stack` was passed. A hand-modified file is preserved
/// (DSTK-01). Returns whether the file was written; a written file is recorded.
pub fn write_scaffold_stack_ts(config: &DeployConfig, rendered: &str) -> Result<bool> {
    let state = classify_stack_ts(config)?;
    let overwrite = config.regenerate_stack || state == StackTsState::Untouched;
    let path = stack_ts_path(&config.project_root);
    let changes = state == StackTsState::Untouched
        && !config.regenerate_stack
        && std::fs::read_to_string(&path).is_ok_and(|current| current != rendered);
    let lib_dir = config.project_root.join("deploy").join("lib");
    let wrote = crate::deployment::config::write_stack_ts_guarded(&lib_dir, rendered, overwrite)?;
    if wrote {
        record_stack_ts(&config.project_root, rendered)?;
    }
    if changes {
        println!(
            "   regenerated deploy/lib/stack.ts from .pmcp/deploy.toml (it was cargo-pmcp's \
             unmodified scaffold)"
        );
    }
    Ok(wrote)
}

/// Record `content` as the `deploy/bin/app.ts` cargo-pmcp just wrote (#409).
pub fn record_app_ts(project_root: &Path, content: &str) -> Result<()> {
    let mut entries = read_entries(project_root);
    entries.insert(APP_TS_ENTRY.to_string(), content_digest(content));
    write_entries(project_root, entries)
}

/// What `deploy/bin/app.ts` is, relative to what cargo-pmcp wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppTsState {
    /// No app.ts on disk.
    Missing,
    /// cargo-pmcp's own output, unmodified. `declared` is the server name its
    /// `serverName` literal carries, when there is one.
    Untouched {
        /// The name the file declares its stack for.
        declared: Option<String>,
    },
    /// Edited (or written) by someone other than cargo-pmcp.
    HandModified,
}

/// The name an app.ts's `serverName` literal carries, whatever else the file
/// says.
fn declared_name_literal(content: &str) -> Option<String> {
    let start = content.find(APP_TS_NAME_LITERAL)? + APP_TS_NAME_LITERAL.len();
    let rest = &content[start..];
    rest.find('\'').map(|end| rest[..end].to_string())
}

/// Classify `deploy/bin/app.ts` (#409), like [`classify_stack_ts`]: when the
/// record holds an app.ts digest, "unmodified" means "matches it". Records
/// written before 0.28.1 hold stack.ts only, and a project initialized before
/// 0.28.0 has none; then app.ts is unmodified when it is exactly the scaffold
/// `deploy init` renders for some name (the check 0.28.0 used), and such a
/// file is adopted into the record.
pub fn classify_app_ts(project_root: &Path) -> Result<AppTsState> {
    let path = app_ts_path(project_root);
    if !path.exists() {
        return Ok(AppTsState::Missing);
    }
    let on_disk = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if let Some(recorded) = read_entries(project_root).remove(APP_TS_ENTRY) {
        return Ok(if content_digest(&on_disk) == recorded {
            AppTsState::Untouched {
                declared: declared_name_literal(&on_disk),
            }
        } else {
            AppTsState::HandModified
        });
    }
    let Some(declared) = app_ts_scaffold_name(&on_disk.replace("\r\n", "\n")) else {
        return Ok(AppTsState::HandModified);
    };
    if let Err(err) = record_app_ts(project_root, &on_disk) {
        eprintln!(
            "  {} could not record {} as cargo-pmcp's unmodified scaffold: {err:#}",
            console::style("warning:").yellow(),
            path.display()
        );
    }
    Ok(AppTsState::Untouched {
        declared: Some(declared),
    })
}

/// Point an unmodified `deploy/bin/app.ts` scaffold at `server_name`
/// (finding #1). app.ts names the stack `npx cdk` deploys; `deploy init`
/// writes the name into it as a literal, so after a `[server] name` rename it
/// still declared the old stack. A rewritten file is recorded (#409).
///
/// Returns the previous content when the file was rewritten (so a refused
/// deploy can put it back with [`restore_app_ts`]), `None` when there was
/// nothing to do: no app.ts, a hand-modified one (left for the stack guard to
/// judge), or one that is already the scaffold for `server_name`.
pub fn sync_app_ts_with_server_name(
    project_root: &Path,
    server_name: &str,
) -> Result<Option<String>> {
    let AppTsState::Untouched { declared } = classify_app_ts(project_root)? else {
        return Ok(None);
    };
    let path = app_ts_path(project_root);
    let before = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let rendered = render_app_ts(server_name);
    if before.replace("\r\n", "\n") == rendered {
        return Ok(None);
    }
    std::fs::write(&path, &rendered)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    record_app_ts(project_root, &rendered)?;
    match declared.filter(|declared| declared != server_name) {
        Some(declared) => println!(
            "   deploy/bin/app.ts now declares {server_name}-stack (was {declared}-stack): the \
             unmodified scaffold follows [server] name"
        ),
        None => {
            println!("   regenerated deploy/bin/app.ts (it was cargo-pmcp's unmodified scaffold)");
        },
    }
    Ok(Some(before))
}

/// Put back the app.ts [`sync_app_ts_with_server_name`] replaced, and record
/// it, so the restored file is still cargo-pmcp's unmodified scaffold.
pub fn restore_app_ts(project_root: &Path, before: &str) -> Result<()> {
    let path = app_ts_path(project_root);
    std::fs::write(&path, before).with_context(|| {
        format!(
            "failed to restore {} after the refused deploy",
            path.display()
        )
    })?;
    record_app_ts(project_root, before)
}

/// After the native `CloudFormation` engine deployed `config`, bring
/// cargo-pmcp's own, unmodified scaffolds in line with it (#409).
///
/// The engine deploys `{[server] name}-stack` from `.pmcp/deploy.toml` and
/// never reads deploy/lib/stack.ts or deploy/bin/app.ts, so before 0.28.1 a
/// rename left both naming the old server: a later fallback to `npx cdk
/// deploy` (a hand edit, or a descriptor the renderer cannot render yet)
/// would have targeted the old stack, and the stack guard refused it. Now an
/// unmodified stack.ts is regenerated from the config, exactly as the `npx
/// cdk deploy` path regenerates it, and an unmodified app.ts follows `[server]
/// name`; both are recorded. A hand-modified stack.ts (and the app.ts beside
/// it) is never touched, and a project with no scaffold gets none.
pub fn sync_scaffolds_after_native_deploy(config: &DeployConfig) -> Result<()> {
    let state = classify_stack_ts(config)?;
    if state == StackTsState::HandModified {
        return Ok(());
    }
    if state == StackTsState::Untouched {
        let rendered = crate::commands::deploy::init::render_stack_ts_for_config(
            config,
            &crate::deployment::lambda_function::secret_keys(config),
        );
        let current = std::fs::read_to_string(stack_ts_path(&config.project_root))
            .unwrap_or_default()
            .replace("\r\n", "\n");
        if current != rendered {
            write_scaffold_stack_ts(config, &rendered)?;
        }
    }
    sync_app_ts_with_server_name(&config.project_root, &config.server.name)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::deploy::init::{render_app_ts, render_stack_ts_for_deploy};
    use crate::deployment::config::{IamConfig, MetadataConfig, TablePermission};

    fn config(root: &Path, target: &str, name: &str) -> DeployConfig {
        let mut cfg = DeployConfig::default_for_server(
            name.to_string(),
            "us-east-1".to_string(),
            root.to_path_buf(),
        );
        cfg.target.target_type = target.to_string();
        cfg
    }

    fn stack_ts_path(root: &Path) -> std::path::PathBuf {
        root.join("deploy").join("lib").join("stack.ts")
    }

    /// What `deploy init` writes: the scaffold for `name` with init-time
    /// (empty) `[iam]`/`[metadata]`, plus its provenance record.
    fn init_scaffold(root: &Path, target: &str, name: &str) -> String {
        let content = render_stack_ts_for_deploy(
            target,
            name,
            &IamConfig::default(),
            &MetadataConfig::default(),
        );
        std::fs::create_dir_all(root.join("deploy").join("lib")).expect("mkdir");
        std::fs::write(stack_ts_path(root), &content).expect("write stack.ts");
        record_stack_ts(root, &content).expect("record");
        content
    }

    fn users_table() -> IamConfig {
        IamConfig {
            tables: vec![TablePermission {
                name: "Users".to_string(),
                actions: vec!["read".to_string()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        }
    }

    /// Finding #4, the reported case: after a `[server] name` rename the
    /// untouched init scaffold is still cargo-pmcp's own output.
    #[test]
    fn renamed_untouched_scaffold_is_still_untouched() {
        for target in ["aws-lambda", "pmcp-run"] {
            let tmp = tempfile::tempdir().expect("tempdir");
            init_scaffold(tmp.path(), target, "old-server");
            let cfg = config(tmp.path(), target, "acme");
            assert_eq!(
                classify_stack_ts(&cfg).expect("classify"),
                StackTsState::Untouched,
                "{target}"
            );
        }
    }

    /// The same class: declaring `[iam]` after init.
    #[test]
    fn iam_declared_after_init_keeps_the_scaffold_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        init_scaffold(tmp.path(), "aws-lambda", "acme");
        let mut cfg = config(tmp.path(), "aws-lambda", "acme");
        cfg.iam = users_table();
        assert_eq!(
            classify_stack_ts(&cfg).expect("classify"),
            StackTsState::Untouched
        );
    }

    #[test]
    fn an_edited_scaffold_is_hand_modified() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let content = init_scaffold(tmp.path(), "aws-lambda", "acme");
        std::fs::write(stack_ts_path(tmp.path()), content.replace("512", "1024")).expect("edit");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        assert_eq!(
            classify_stack_ts(&cfg).expect("classify"),
            StackTsState::HandModified
        );
    }

    #[test]
    fn a_missing_stack_ts_is_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        assert_eq!(
            classify_stack_ts(&cfg).expect("classify"),
            StackTsState::Missing
        );
    }

    /// Migration: a project initialized before the record existed. A
    /// stack.ts that matches today's render is adopted, and recorded, so a
    /// LATER rename still sees it as untouched.
    #[test]
    fn migration_adopts_a_matching_scaffold_and_records_it() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        let content = render_stack_ts_for_deploy("aws-lambda", "acme", &cfg.iam, &cfg.metadata);
        std::fs::create_dir_all(tmp.path().join("deploy/lib")).expect("mkdir");
        std::fs::write(stack_ts_path(tmp.path()), &content).expect("write");

        assert_eq!(
            classify_stack_ts(&cfg).expect("classify"),
            StackTsState::Untouched
        );
        assert_eq!(
            recorded_stack_ts_digest(tmp.path()),
            Some(content_digest(&content)),
            "an adopted scaffold must be recorded"
        );
        let renamed = config(tmp.path(), "aws-lambda", "renamed");
        assert_eq!(
            classify_stack_ts(&renamed).expect("classify"),
            StackTsState::Untouched
        );
    }

    /// Migration also recognizes a pre-record scaffold that was ALREADY
    /// renamed (or had `[iam]` added) before upgrading: it matches the
    /// init-time render for the name its own `serverName` literal carries.
    #[test]
    fn migration_recognizes_an_already_renamed_scaffold() {
        for target in ["aws-lambda", "pmcp-run"] {
            let tmp = tempfile::tempdir().expect("tempdir");
            let content = render_stack_ts_for_deploy(
                target,
                "old-server",
                &IamConfig::default(),
                &MetadataConfig::default(),
            );
            std::fs::create_dir_all(tmp.path().join("deploy/lib")).expect("mkdir");
            std::fs::write(stack_ts_path(tmp.path()), &content).expect("write");
            let mut cfg = config(tmp.path(), target, "acme");
            cfg.iam = users_table();
            assert_eq!(
                classify_stack_ts(&cfg).expect("classify"),
                StackTsState::Untouched,
                "{target}"
            );
        }
    }

    #[test]
    fn migration_never_adopts_a_hand_modified_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy/lib")).expect("mkdir");
        std::fs::write(stack_ts_path(tmp.path()), "// curated\n").expect("write");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        assert_eq!(
            classify_stack_ts(&cfg).expect("classify"),
            StackTsState::HandModified
        );
        assert_eq!(recorded_stack_ts_digest(tmp.path()), None);
    }

    /// A Windows checkout with `core.autocrlf` must not turn the untouched
    /// scaffold into a hand-modified one.
    #[test]
    fn crlf_checkout_of_the_scaffold_is_still_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let content = init_scaffold(tmp.path(), "aws-lambda", "acme");
        std::fs::write(stack_ts_path(tmp.path()), content.replace('\n', "\r\n")).expect("crlf");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        assert_eq!(
            classify_stack_ts(&cfg).expect("classify"),
            StackTsState::Untouched
        );
    }

    /// An untouched scaffold is regenerated by a deploy without
    /// `--regenerate-stack` (it is cargo-pmcp's own output), and the new
    /// content is recorded.
    #[test]
    fn deploy_regenerates_an_untouched_scaffold_without_the_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        init_scaffold(tmp.path(), "aws-lambda", "old-server");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        let rendered = render_stack_ts_for_deploy("aws-lambda", "acme", &cfg.iam, &cfg.metadata);

        assert!(write_scaffold_stack_ts(&cfg, &rendered).expect("write"));
        assert_eq!(
            std::fs::read_to_string(stack_ts_path(tmp.path())).expect("read"),
            rendered
        );
        assert_eq!(
            recorded_stack_ts_digest(tmp.path()),
            Some(content_digest(&rendered))
        );
    }

    #[test]
    fn deploy_preserves_a_hand_modified_stack_ts_without_the_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy/lib")).expect("mkdir");
        std::fs::write(stack_ts_path(tmp.path()), "// curated\n").expect("write");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        assert!(!write_scaffold_stack_ts(&cfg, "// rendered\n").expect("write"));
        assert_eq!(
            std::fs::read_to_string(stack_ts_path(tmp.path())).expect("read"),
            "// curated\n"
        );
    }

    #[test]
    fn regenerate_flag_overwrites_and_records_a_hand_modified_stack_ts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy/lib")).expect("mkdir");
        std::fs::write(stack_ts_path(tmp.path()), "// curated\n").expect("write");
        let mut cfg = config(tmp.path(), "aws-lambda", "acme");
        cfg.regenerate_stack = true;
        assert!(write_scaffold_stack_ts(&cfg, "// rendered\n").expect("write"));
        assert_eq!(
            recorded_stack_ts_digest(tmp.path()),
            Some(content_digest("// rendered\n"))
        );
    }

    #[test]
    fn forget_drops_the_entry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        init_scaffold(tmp.path(), "aws-lambda", "acme");
        forget_stack_ts(tmp.path()).expect("forget");
        assert_eq!(recorded_stack_ts_digest(tmp.path()), None);
        assert!(!tmp.path().join(RECORD_RELATIVE).exists());
    }

    /// A corrupt record is ignored (with the pre-record fallback), never an
    /// error that blocks the deploy.
    #[test]
    fn a_corrupt_record_falls_back_instead_of_failing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let content = init_scaffold(tmp.path(), "aws-lambda", "acme");
        std::fs::write(tmp.path().join(RECORD_RELATIVE), "not = [valid").expect("corrupt");
        std::fs::write(stack_ts_path(tmp.path()), &content).expect("write");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        assert_eq!(
            classify_stack_ts(&cfg).expect("classify"),
            StackTsState::Untouched
        );
    }

    /// The aws-lambda `stack.ts` that cargo-pmcp 0.27.x and 0.28.0 wrote for
    /// `demo-server` with empty `[iam]`/`[metadata]`, frozen as a fixture (a
    /// byte copy of `tests/golden/aws-lambda-empty.ts` at the 0.28.0 release,
    /// unchanged since 0.27.0). It hardcodes `memorySize: 512`, a 30 s timeout
    /// and `RUST_LOG: 'info'`, whatever deploy.toml said.
    const STACK_TS_0_28_0: &str =
        include_str!("../../tests/fixtures/scaffold/aws-lambda-stack-ts-0.28.0-demo-server.ts");

    /// 0.28.1 leaves the scaffold's default render byte-identical to what
    /// 0.27.x/0.28.0 wrote (#407 needed no template change: every scaffold
    /// log group already carries `removalPolicy: DESTROY`), so an untouched
    /// 0.28.0 scaffold without a record is still recognised by the pre-record
    /// comparison. A future template change must keep this fixture
    /// recognised some other way (see `is_a_scaffold_rendering`).
    #[test]
    fn the_default_render_is_still_the_frozen_0_28_0_scaffold() {
        assert_eq!(
            render_stack_ts_for_deploy(
                "aws-lambda",
                "demo-server",
                &IamConfig::default(),
                &MetadataConfig::default()
            ),
            STACK_TS_0_28_0
        );
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_stack_ts(tmp.path());
        assert_eq!(
            classify_stack_ts(&config(tmp.path(), "aws-lambda", "renamed")).expect("classify"),
            StackTsState::Untouched
        );
    }

    /// A config whose `[server]` sizing and `[environment]` differ from what a
    /// 0.28.0 scaffold baked (debug session `cargo-pmcp-deploy-targets`, PR-B).
    fn config_with_function_settings(root: &Path, name: &str) -> DeployConfig {
        let mut cfg = config(root, "aws-lambda", name);
        cfg.server.memory_mb = Some(1769);
        cfg.server.timeout_seconds = Some(60);
        cfg.server.ephemeral_storage_mb = Some(4096);
        cfg.environment.insert(
            "MODEL_URL".to_string(),
            "s3://models/forecast.bin".to_string(),
        );
        cfg
    }

    fn seed_0_28_0_stack_ts(root: &Path) {
        std::fs::create_dir_all(root.join("deploy/lib")).expect("mkdir");
        std::fs::write(stack_ts_path(root), STACK_TS_0_28_0).expect("write stack.ts");
    }

    /// An untouched scaffold written AND recorded by 0.28.0 stays cargo-pmcp's
    /// own after the scaffold started rendering `[server]` sizing and
    /// `[environment]`: the record holds what was written, not a render.
    #[test]
    fn a_recorded_0_28_0_scaffold_stays_untouched_with_declared_function_settings() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_stack_ts(tmp.path());
        record_stack_ts(tmp.path(), STACK_TS_0_28_0).expect("record");
        for name in ["demo-server", "renamed"] {
            let cfg = config_with_function_settings(tmp.path(), name);
            assert_eq!(
                classify_stack_ts(&cfg).expect("classify"),
                StackTsState::Untouched,
                "{name}"
            );
        }
    }

    /// The pre-record migration (projects initialized by 0.27.x) still
    /// recognizes the 0.27.x/0.28.0 scaffold when deploy.toml declares sizing
    /// and `[environment]` the old template never rendered — also after a
    /// rename and with `[iam]` declared — and records it. Without this the
    /// change would push every such project to the hand-modified path.
    #[test]
    fn a_pre_record_0_27_scaffold_is_recognized_with_declared_function_settings() {
        for (name, iam) in [
            ("demo-server", IamConfig::default()),
            ("renamed", IamConfig::default()),
            ("renamed", users_table()),
        ] {
            let tmp = tempfile::tempdir().expect("tempdir");
            seed_0_28_0_stack_ts(tmp.path());
            let mut cfg = config_with_function_settings(tmp.path(), name);
            cfg.iam = iam;

            assert_eq!(
                classify_stack_ts(&cfg).expect("classify"),
                StackTsState::Untouched,
                "{name}"
            );
            assert_eq!(
                recorded_stack_ts_digest(tmp.path()),
                Some(content_digest(STACK_TS_0_28_0)),
                "an adopted scaffold must be recorded"
            );
        }
    }

    /// ... and the next deploy regenerates it with the declared settings, and
    /// records the new content.
    #[test]
    fn a_0_28_0_scaffold_is_regenerated_with_the_declared_function_settings() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_stack_ts(tmp.path());
        let cfg = config_with_function_settings(tmp.path(), "demo-server");
        let rendered = crate::commands::deploy::init::render_stack_ts_for_config(
            &cfg,
            &std::collections::BTreeSet::new(),
        );

        assert!(write_scaffold_stack_ts(&cfg, &rendered).expect("write"));

        let on_disk = std::fs::read_to_string(stack_ts_path(tmp.path())).expect("read");
        assert!(on_disk.contains("memorySize: 1769,"), "{on_disk}");
        assert!(
            on_disk.contains("ephemeralStorageSize: cdk.Size.mebibytes(4096),"),
            "{on_disk}"
        );
        assert!(
            on_disk.contains("MODEL_URL: 's3://models/forecast.bin',"),
            "{on_disk}"
        );
        assert_eq!(
            recorded_stack_ts_digest(tmp.path()),
            Some(content_digest(&rendered))
        );
    }

    // ---------- #409: the native engine keeps the scaffolds in line ----------

    fn app_ts_path(root: &Path) -> std::path::PathBuf {
        root.join("deploy").join("bin").join("app.ts")
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).expect("read")
    }

    /// What `deploy init` of cargo-pmcp 0.28.0 left: app.ts and stack.ts for
    /// `name`, and a record holding stack.ts only.
    fn seed_0_28_0_init(root: &Path, name: &str) {
        init_scaffold(root, "aws-lambda", name);
        std::fs::create_dir_all(root.join("deploy/bin")).expect("mkdir");
        std::fs::write(app_ts_path(root), render_app_ts(name)).expect("write app.ts");
    }

    /// #409, the reported case (forecast-coach spike 021 step 4): a rename
    /// deployed by the native engine left deploy/lib/stack.ts and
    /// deploy/bin/app.ts naming the OLD server, so a later `npx cdk deploy`
    /// fallback would target the old stack (and the stack guard refuse it).
    /// Both untouched files now follow the deployed config, and app.ts is
    /// recorded beside stack.ts — reading a 0.28.0 record that has only
    /// stack.ts.
    #[test]
    fn a_native_deploy_after_a_rename_regenerates_both_untouched_scaffolds() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_init(tmp.path(), "fc-cp028-a");
        let cfg = config(tmp.path(), "aws-lambda", "fc-cp028-b");

        sync_scaffolds_after_native_deploy(&cfg).expect("sync");

        let stack_ts = crate::commands::deploy::init::render_stack_ts_for_config(
            &cfg,
            &crate::deployment::lambda_function::secret_keys(&cfg),
        );
        assert_eq!(read(&stack_ts_path(tmp.path())), stack_ts);
        assert_eq!(read(&app_ts_path(tmp.path())), render_app_ts("fc-cp028-b"));
        let entries = read_entries(tmp.path());
        assert_eq!(
            entries.get(STACK_TS_ENTRY),
            Some(&content_digest(&stack_ts))
        );
        assert_eq!(
            entries.get("bin/app.ts"),
            Some(&content_digest(&render_app_ts("fc-cp028-b"))),
            "app.ts must be recorded beside stack.ts"
        );
    }

    /// The regenerated stack.ts carries what deploy.toml declares, as the
    /// legacy path's regeneration does, so a later fallback deploys the same
    /// function settings.
    #[test]
    fn a_native_deploy_regenerates_stack_ts_with_the_declared_settings() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_init(tmp.path(), "demo-server");
        let cfg = config_with_function_settings(tmp.path(), "demo-server");

        sync_scaffolds_after_native_deploy(&cfg).expect("sync");

        let on_disk = read(&stack_ts_path(tmp.path()));
        assert!(on_disk.contains("memorySize: 1769,"), "{on_disk}");
        assert!(
            on_disk.contains("MODEL_URL: 's3://models/forecast.bin',"),
            "{on_disk}"
        );
        assert_eq!(read(&app_ts_path(tmp.path())), render_app_ts("demo-server"));
    }

    /// A hand-modified app.ts is preserved (the stack guard judges it on the
    /// legacy path); the untouched stack.ts is still regenerated.
    #[test]
    fn a_native_deploy_keeps_a_hand_modified_app_ts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_init(tmp.path(), "old-server");
        let curated = render_app_ts("old-server").replace("app.synth();", "// x\napp.synth();");
        std::fs::write(app_ts_path(tmp.path()), &curated).expect("edit app.ts");
        let cfg = config(tmp.path(), "aws-lambda", "acme");

        sync_scaffolds_after_native_deploy(&cfg).expect("sync");

        assert_eq!(read(&app_ts_path(tmp.path())), curated);
        assert!(read(&stack_ts_path(tmp.path())).contains("const serverName = 'acme';"));
    }

    /// A hand-modified stack.ts is never rewritten, and its app.ts is left
    /// alone with it (its own `serverName` still names the old function).
    #[test]
    fn a_native_deploy_keeps_a_hand_modified_stack_ts_and_its_app_ts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_init(tmp.path(), "old-server");
        std::fs::write(stack_ts_path(tmp.path()), "// curated\n").expect("edit stack.ts");
        let cfg = config(tmp.path(), "aws-lambda", "acme");

        sync_scaffolds_after_native_deploy(&cfg).expect("sync");

        assert_eq!(read(&stack_ts_path(tmp.path())), "// curated\n");
        assert_eq!(read(&app_ts_path(tmp.path())), render_app_ts("old-server"));
    }

    /// The deploy/bin/app.ts cargo-pmcp 0.28.0 wrote for `demo-server`, frozen
    /// (a byte copy of that release's `render_app_ts("demo-server")`).
    const APP_TS_0_28_0: &str =
        include_str!("../../tests/fixtures/scaffold/aws-lambda-app-ts-0.28.0-demo-server.ts");

    /// Untouched scaffolds written by 0.28.0 — whether or not that release
    /// recorded the stack.ts, and with a record that never holds app.ts —
    /// are recognised as cargo-pmcp's own and follow a rename on a native
    /// deploy.
    #[test]
    fn a_native_deploy_regenerates_the_frozen_0_28_0_scaffolds() {
        for recorded in [true, false] {
            let tmp = tempfile::tempdir().expect("tempdir");
            seed_0_28_0_stack_ts(tmp.path());
            if recorded {
                record_stack_ts(tmp.path(), STACK_TS_0_28_0).expect("record");
            }
            std::fs::create_dir_all(tmp.path().join("deploy/bin")).expect("mkdir");
            std::fs::write(app_ts_path(tmp.path()), APP_TS_0_28_0).expect("write app.ts");
            let cfg = config_with_function_settings(tmp.path(), "renamed");

            sync_scaffolds_after_native_deploy(&cfg).expect("sync");

            assert_eq!(
                read(&app_ts_path(tmp.path())),
                render_app_ts("renamed"),
                "recorded = {recorded}"
            );
            let stack_ts = read(&stack_ts_path(tmp.path()));
            assert!(
                stack_ts.contains("const serverName = 'renamed';"),
                "{stack_ts}"
            );
            assert!(stack_ts.contains("memorySize: 1769,"), "{stack_ts}");
        }
    }

    /// A project with no CDK scaffold (a built-in server, say) gets none.
    #[test]
    fn a_native_deploy_creates_no_scaffolds() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = config(tmp.path(), "aws-lambda", "acme");
        sync_scaffolds_after_native_deploy(&cfg).expect("sync");
        assert!(!tmp.path().join("deploy").exists());
    }

    // ---------- #409: app.ts provenance ----------

    #[test]
    fn a_missing_app_ts_is_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            classify_app_ts(tmp.path()).expect("classify"),
            AppTsState::Missing
        );
    }

    /// A 0.28.0 record (stack.ts only) is read as before: an app.ts that is
    /// exactly a scaffold render is cargo-pmcp's own, and is adopted.
    #[test]
    fn a_0_28_0_record_without_app_ts_adopts_an_unmodified_app_ts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_init(tmp.path(), "acme");
        assert_eq!(read_entries(tmp.path()).get(APP_TS_ENTRY), None);

        assert_eq!(
            classify_app_ts(tmp.path()).expect("classify"),
            AppTsState::Untouched {
                declared: Some("acme".to_string())
            }
        );
        assert_eq!(
            read_entries(tmp.path()).get(APP_TS_ENTRY),
            Some(&content_digest(&render_app_ts("acme"))),
            "an adopted app.ts must be recorded"
        );
        assert!(
            read_entries(tmp.path()).contains_key(STACK_TS_ENTRY),
            "adopting app.ts must keep the stack.ts entry"
        );
    }

    /// The frozen 0.28.0 app.ts is recognised (and not as hand-modified).
    #[test]
    fn the_frozen_0_28_0_app_ts_is_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy/bin")).expect("mkdir");
        std::fs::write(app_ts_path(tmp.path()), APP_TS_0_28_0).expect("write");
        assert_eq!(
            classify_app_ts(tmp.path()).expect("classify"),
            AppTsState::Untouched {
                declared: Some("demo-server".to_string())
            }
        );
    }

    /// Without a record, an edited app.ts is hand-modified and NOT adopted.
    #[test]
    fn an_unrecorded_edited_app_ts_is_hand_modified() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy/bin")).expect("mkdir");
        std::fs::write(app_ts_path(tmp.path()), "// curated app\n").expect("write");
        assert_eq!(
            classify_app_ts(tmp.path()).expect("classify"),
            AppTsState::HandModified
        );
        assert_eq!(read_entries(tmp.path()).get(APP_TS_ENTRY), None);
    }

    /// Once recorded, the record decides: any edit is hand-modified — even
    /// one that leaves a scaffold render for ANOTHER name (an operator who
    /// changed the name literal by hand meant it).
    #[test]
    fn a_recorded_app_ts_is_hand_modified_after_any_edit() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy/bin")).expect("mkdir");
        std::fs::write(app_ts_path(tmp.path()), render_app_ts("acme")).expect("write");
        record_app_ts(tmp.path(), &render_app_ts("acme")).expect("record");
        std::fs::write(app_ts_path(tmp.path()), render_app_ts("other")).expect("edit");
        assert_eq!(
            classify_app_ts(tmp.path()).expect("classify"),
            AppTsState::HandModified
        );
        assert_eq!(
            sync_app_ts_with_server_name(tmp.path(), "acme").expect("sync"),
            None
        );
        assert_eq!(read(&app_ts_path(tmp.path())), render_app_ts("other"));
    }

    /// A CRLF checkout of a recorded app.ts is still unmodified.
    #[test]
    fn a_crlf_checkout_of_a_recorded_app_ts_is_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy/bin")).expect("mkdir");
        record_app_ts(tmp.path(), &render_app_ts("acme")).expect("record");
        std::fs::write(
            app_ts_path(tmp.path()),
            render_app_ts("acme").replace('\n', "\r\n"),
        )
        .expect("write");
        assert!(matches!(
            classify_app_ts(tmp.path()).expect("classify"),
            AppTsState::Untouched { .. }
        ));
    }

    /// A synced app.ts is recorded, and restoring it (a refused deploy)
    /// records the restored content, so it stays cargo-pmcp's own.
    #[test]
    fn sync_records_and_restore_keeps_the_app_ts_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_0_28_0_init(tmp.path(), "old-server");
        let before = sync_app_ts_with_server_name(tmp.path(), "acme")
            .expect("sync")
            .expect("rewritten");
        assert_eq!(
            read_entries(tmp.path()).get(APP_TS_ENTRY),
            Some(&content_digest(&render_app_ts("acme")))
        );
        restore_app_ts(tmp.path(), &before).expect("restore");
        assert_eq!(read(&app_ts_path(tmp.path())), render_app_ts("old-server"));
        assert_eq!(
            classify_app_ts(tmp.path()).expect("classify"),
            AppTsState::Untouched {
                declared: Some("old-server".to_string())
            }
        );
    }

    /// Finding #1: an untouched app.ts follows `[server] name`.
    #[test]
    fn sync_points_an_untouched_app_ts_at_the_new_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bin = tmp.path().join("deploy/bin");
        std::fs::create_dir_all(&bin).expect("mkdir");
        std::fs::write(bin.join("app.ts"), render_app_ts("old-server")).expect("write");

        let before = sync_app_ts_with_server_name(tmp.path(), "acme").expect("sync");

        assert_eq!(before, Some(render_app_ts("old-server")));
        assert_eq!(
            std::fs::read_to_string(bin.join("app.ts")).expect("read"),
            render_app_ts("acme")
        );
    }

    #[test]
    fn sync_leaves_a_hand_modified_or_current_app_ts_alone() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bin = tmp.path().join("deploy/bin");
        std::fs::create_dir_all(&bin).expect("mkdir");
        let curated = render_app_ts("old-server").replace("app.synth();", "// x\napp.synth();");
        std::fs::write(bin.join("app.ts"), &curated).expect("write");
        assert_eq!(
            sync_app_ts_with_server_name(tmp.path(), "acme").expect("sync"),
            None
        );
        assert_eq!(
            std::fs::read_to_string(bin.join("app.ts")).expect("read"),
            curated
        );

        std::fs::write(bin.join("app.ts"), render_app_ts("acme")).expect("write");
        assert_eq!(
            sync_app_ts_with_server_name(tmp.path(), "acme").expect("sync"),
            None
        );

        std::fs::remove_file(bin.join("app.ts")).expect("rm");
        assert_eq!(
            sync_app_ts_with_server_name(tmp.path(), "acme").expect("sync"),
            None
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn config(root: &Path) -> DeployConfig {
        let mut cfg = DeployConfig::default_for_server(
            "acme".to_string(),
            "us-east-1".to_string(),
            root.to_path_buf(),
        );
        cfg.target.target_type = "aws-lambda".to_string();
        cfg
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Whatever cargo-pmcp records as written is untouched; changing any
        /// one character of it makes it hand-modified.
        #[test]
        fn recorded_content_is_untouched_and_any_edit_is_not(
            content in "[ -~\\n]{1,200}",
            at in any::<prop::sample::Index>(),
            replacement in "[ -~]",
        ) {
            let tmp = tempfile::tempdir().expect("tempdir");
            std::fs::create_dir_all(tmp.path().join("deploy/lib")).expect("mkdir");
            let path = tmp.path().join("deploy/lib/stack.ts");
            std::fs::write(&path, &content).expect("write");
            record_stack_ts(tmp.path(), &content).expect("record");
            prop_assert_eq!(classify_stack_ts(&config(tmp.path())).expect("classify"), StackTsState::Untouched);

            let mut chars: Vec<char> = content.chars().collect();
            let i = at.index(chars.len());
            let new_char = replacement.chars().next().expect("one char");
            prop_assume!(chars[i] != new_char);
            chars[i] = new_char;
            let edited: String = chars.into_iter().collect();
            std::fs::write(&path, &edited).expect("write");
            prop_assert_eq!(classify_stack_ts(&config(tmp.path())).expect("classify"), StackTsState::HandModified);
        }
    }
}
