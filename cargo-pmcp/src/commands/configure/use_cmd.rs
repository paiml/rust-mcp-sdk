//! `cargo pmcp configure use <name>` — set the active target for the current workspace.
//!
//! Writes `<name>\n` to `<workspace_root>/.pmcp/active-target` (single-line plain text).
//! Errors if `<name>` is not present in `~/.pmcp/config.toml` (so a typo is caught now,
//! not at deploy time).
//!
//! Implements REQ-77-03 (workspace marker file, permissive read / strict write).
//! See also `read_active_marker` (consumed by Plan 77-05 resolver).

use anyhow::{bail, Context, Result};
use clap::Args;

use crate::commands::configure::config::{default_user_config_path, TargetConfigV1};
use crate::commands::configure::name_validation::validate_target_name;
use crate::commands::configure::workspace::find_workspace_root;
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp configure use`.
#[derive(Debug, Args)]
pub struct UseArgs {
    /// Name of the target to activate (must exist in ~/.pmcp/config.toml).
    pub name: String,
}

/// Activate a named target by writing `<name>\n` to `<workspace_root>/.pmcp/active-target`.
///
/// Errors if the name is not defined in `~/.pmcp/config.toml`. Idempotent —
/// running twice with the same name is a no-op (the file is rewritten with the
/// same contents). When overwriting a different target, emits a stderr "switching"
/// note unless `--quiet` (GEM-2 fix per 77-REVIEWS.md).
pub fn execute(args: UseArgs, gf: &GlobalFlags) -> Result<()> {
    validate_target_name(&args.name)?;

    let cfg_path = default_user_config_path();
    let cfg = TargetConfigV1::read(&cfg_path)?;
    if !cfg.targets.contains_key(&args.name) {
        bail!(
            "target '{}' not found in {} — run `cargo pmcp configure add {}`",
            args.name,
            cfg_path.display(),
            args.name
        );
    }

    let workspace_root = find_workspace_root()?;
    let pmcp_dir = workspace_root.join(".pmcp");
    std::fs::create_dir_all(&pmcp_dir)
        .with_context(|| format!("failed to create {}", pmcp_dir.display()))?;
    let marker = pmcp_dir.join("active-target");

    // GEM-2: when overwriting an existing marker that names a DIFFERENT target,
    // emit an informational stderr note. No interactive prompt — just a one-line
    // note so the operator sees the switch happened. Suppressible with --quiet.
    if let Ok(existing) = std::fs::read_to_string(&marker) {
        let prev = existing.trim();
        if !prev.is_empty() && prev != args.name && !gf.quiet {
            eprintln!(
                "note: switching active target from '{}' to '{}' in {}",
                prev,
                args.name,
                marker.display()
            );
        }
    }

    std::fs::write(&marker, format!("{}\n", args.name))
        .with_context(|| format!("failed to write {}", marker.display()))?;

    // A new/!changed target can point at a DIFFERENT deployment, and the discovery
    // cache (~/.pmcp/pmcp-run-config.json) is keyed by api_url, so a stale entry for
    // the previous target would otherwise survive until its 1-hour TTL. Best-effort:
    // failing to clear a cache must not fail the command — the worst case is one
    // stale lookup, which the retry in pmcp_run::graphql also covers.
    let _ = crate::deployment::targets::pmcp_run::auth::clear_config_cache();

    eprintln!(
        "✓ active target for {} is now '{}'",
        workspace_root.display(),
        args.name
    );
    Ok(())
}

/// Reads the marker file at `<workspace_root>/.pmcp/active-target`. Permissive on read:
/// trims whitespace, ignores BOM, returns `None` for empty/missing.
///
/// Used by Plan 77-05's resolver — single source of truth for marker reading.
pub fn read_active_marker(workspace_root: &std::path::Path) -> Result<Option<String>> {
    let marker = workspace_root.join(".pmcp").join("active-target");
    match std::fs::read_to_string(&marker) {
        Ok(s) => {
            let trimmed = s.trim_start_matches('\u{feff}').trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::anyhow!("failed to read {}: {e}", marker.display())),
    }
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::configure::config::{PmcpRunEntry, TargetEntry};
    use serial_test::serial;

    /// Run `f` with HOME and CWD overridden to fresh tempdirs (with a Cargo.toml in CWD
    /// so `find_workspace_root` succeeds). Restores both after.
    /// Use `#[serial]` on tests using this helper.
    fn run_in_isolated_home_and_workspace<F, R>(f: F) -> R
    where
        F: FnOnce(&std::path::Path, &std::path::Path) -> R,
    {
        let home_tmp = tempfile::tempdir().unwrap();
        let ws_tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            ws_tmp.path().join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.0.0\"\n",
        )
        .unwrap();

        let saved_home = std::env::var_os("HOME");
        // Fall back to std::env::temp_dir() when current_dir() is invalid (a prior test
        // may have set CWD into a now-deleted tempdir). The actual restore target only
        // needs to be a valid existing directory; tests don't depend on the *exact* path.
        let saved_cwd = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        std::env::set_var("HOME", home_tmp.path());
        std::env::set_current_dir(ws_tmp.path()).unwrap();

        let r = f(home_tmp.path(), ws_tmp.path());

        // Restore cwd before tempdir is dropped. Ignore errors here too — the saved_cwd
        // may have been deleted while the test ran.
        let _ = std::env::set_current_dir(&saved_cwd);
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        r
    }

    /// Adds a target named `name` to `<home>/.pmcp/config.toml`, preserving any
    /// targets already present (read-modify-write). Tests that call this twice
    /// rely on the second call NOT erasing the first target.
    fn write_config_with_target(home: &std::path::Path, name: &str) {
        let path = home.join(".pmcp").join("config.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut cfg = TargetConfigV1::read(&path).unwrap_or_else(|_| TargetConfigV1::empty());
        cfg.targets.insert(
            name.to_string(),
            TargetEntry::PmcpRun(PmcpRunEntry {
                api_url: None,
                aws_profile: None,
                region: None,
            }),
        );
        cfg.write_atomic(&path).unwrap();
    }

    #[test]
    #[serial]
    fn use_writes_marker() {
        run_in_isolated_home_and_workspace(|home, ws| {
            write_config_with_target(home, "dev");
            let gf = GlobalFlags::default();
            execute(UseArgs { name: "dev".into() }, &gf).unwrap();
            let body = std::fs::read_to_string(ws.join(".pmcp").join("active-target")).unwrap();
            assert_eq!(body, "dev\n");
        });
    }

    #[test]
    #[serial]
    fn use_idempotent() {
        run_in_isolated_home_and_workspace(|home, ws| {
            write_config_with_target(home, "dev");
            let gf = GlobalFlags::default();
            execute(UseArgs { name: "dev".into() }, &gf).unwrap();
            execute(UseArgs { name: "dev".into() }, &gf).unwrap();
            let body = std::fs::read_to_string(ws.join(".pmcp").join("active-target")).unwrap();
            assert_eq!(body, "dev\n");
        });
    }

    #[test]
    #[serial]
    fn use_errors_on_unknown_target() {
        run_in_isolated_home_and_workspace(|home, _ws| {
            write_config_with_target(home, "dev");
            let gf = GlobalFlags::default();
            let err = execute(
                UseArgs {
                    name: "prod".into(),
                },
                &gf,
            )
            .unwrap_err();
            assert!(err.to_string().contains("not found"), "got: {err}");
        });
    }

    #[test]
    #[serial]
    fn use_rejects_invalid_name() {
        run_in_isolated_home_and_workspace(|_home, _ws| {
            let gf = GlobalFlags::default();
            let err = execute(
                UseArgs {
                    name: "../etc".into(),
                },
                &gf,
            )
            .unwrap_err();
            assert!(
                err.to_string().contains("invalid character")
                    || err.to_string().contains("must match"),
                "got: {err}"
            );
        });
    }

    #[test]
    #[serial]
    fn gem2_use_overwrite_emits_switching_note() {
        // GEM-2: when `use prod` is invoked while marker = "dev", emit a stderr note.
        // Stderr capture in unit tests is platform-finicky; the integration tests
        // (Plan 77-08) use subprocess invocation and grep stderr for the literal note.
        // Here we exercise the overwrite code path and verify the marker file is
        // rewritten correctly without panic.
        run_in_isolated_home_and_workspace(|home, ws| {
            write_config_with_target(home, "dev");
            write_config_with_target(home, "prod");
            let gf = GlobalFlags::default();
            execute(UseArgs { name: "dev".into() }, &gf).unwrap();
            // Second call hits the overwrite branch and emits the note.
            execute(
                UseArgs {
                    name: "prod".into(),
                },
                &gf,
            )
            .unwrap();
            let body = std::fs::read_to_string(ws.join(".pmcp").join("active-target")).unwrap();
            assert_eq!(body, "prod\n");
        });
    }

    #[test]
    #[serial]
    fn use_overwrites_existing_marker() {
        run_in_isolated_home_and_workspace(|home, ws| {
            write_config_with_target(home, "dev");
            write_config_with_target(home, "prod");
            let gf = GlobalFlags::default();
            execute(UseArgs { name: "dev".into() }, &gf).unwrap();
            execute(
                UseArgs {
                    name: "prod".into(),
                },
                &gf,
            )
            .unwrap();
            let body = std::fs::read_to_string(ws.join(".pmcp").join("active-target")).unwrap();
            assert_eq!(body, "prod\n");
        });
    }

    #[test]
    fn read_active_marker_handles_bom_and_whitespace() {
        let tmp = tempfile::tempdir().unwrap();
        let pmcp = tmp.path().join(".pmcp");
        std::fs::create_dir_all(&pmcp).unwrap();
        std::fs::write(pmcp.join("active-target"), "\u{feff}  dev  \n").unwrap();
        let r = read_active_marker(tmp.path()).unwrap();
        assert_eq!(r.as_deref(), Some("dev"));
    }

    #[test]
    fn read_active_marker_missing_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.0.0\"\n",
        )
        .unwrap();
        assert!(read_active_marker(tmp.path()).unwrap().is_none());
    }
}
