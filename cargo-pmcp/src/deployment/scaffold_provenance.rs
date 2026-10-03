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
//! init-time empty `[iam]`/`[metadata]`). A match is adopted and recorded, so
//! from then on the record decides; a file that matches nothing stays
//! hand-modified, exactly as before.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::commands::deploy::init::{
    app_ts_scaffold_name, render_app_ts, render_stack_ts_for_deploy,
};
use crate::deployment::config::{DeployConfig, IamConfig, MetadataConfig};

/// Where the provenance record lives, relative to the project root.
pub const RECORD_RELATIVE: &str = "deploy/.pmcp-scaffold.toml";

/// The record's key for `deploy/lib/stack.ts` (relative to `deploy/`).
const STACK_TS_ENTRY: &str = "lib/stack.ts";

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
/// the configured name or the name the file's own literal carries.
fn is_a_scaffold_rendering(on_disk: &str, config: &DeployConfig) -> bool {
    let on_disk = on_disk.replace("\r\n", "\n");
    let (init_iam, init_meta) = (IamConfig::default(), MetadataConfig::default());
    let inputs = [(&config.iam, &config.metadata), (&init_iam, &init_meta)];
    scaffold_name_candidates(&on_disk, &config.server.name)
        .iter()
        .any(|name| {
            inputs.iter().any(|(iam, meta)| {
                render_stack_ts_for_deploy(&config.target.target_type, name, iam, meta) == on_disk
            })
        })
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

/// Point an unmodified `deploy/bin/app.ts` scaffold at `server_name`
/// (finding #1). app.ts names the stack `npx cdk` deploys; `deploy init`
/// writes the name into it as a literal, so after a `[server] name` rename it
/// still declared the old stack.
///
/// Returns the previous content when the file was rewritten (so a refused
/// deploy can put it back), `None` when there was nothing to do: no app.ts,
/// a hand-modified one (left for the stack guard to judge), or one that
/// already declares `{server_name}-stack`.
pub fn sync_app_ts_with_server_name(
    project_root: &Path,
    server_name: &str,
) -> Result<Option<String>> {
    let path = app_ts_path(project_root);
    if !path.exists() {
        return Ok(None);
    }
    let before = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let Some(declared) = app_ts_scaffold_name(&before.replace("\r\n", "\n")) else {
        return Ok(None);
    };
    if declared == server_name {
        return Ok(None);
    }
    std::fs::write(&path, render_app_ts(server_name))
        .with_context(|| format!("Failed to write {}", path.display()))?;
    println!(
        "   deploy/bin/app.ts now declares {server_name}-stack (was {declared}-stack): the \
         unmodified scaffold follows [server] name"
    );
    Ok(Some(before))
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
