//! `cargo pmcp configure add <name>` — define a new named target in `~/.pmcp/config.toml`.
//!
//! Modes: interactive prompts (when no flags supplied) OR flag-driven (CI-friendly).
//! Mixed mode: skip prompts for fields already passed via flag.
//!
//! Implements REQ-77-01 (add subcommand), REQ-77-03 (single-line marker file via use_cmd),
//! REQ-77-07 (raw-credential validator), REQ-77-08 (atomic write inherited from config.rs).

use anyhow::{bail, Context, Result};
use clap::Args;
use std::io::{self, Write};

use crate::commands::configure::config::{
    default_user_config_path, AwsLambdaEntry, CloudflareWorkersEntry, GoogleCloudRunEntry,
    PmcpRunEntry, TargetConfigV1, TargetEntry,
};
use crate::commands::configure::name_validation::validate_target_name;
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp configure add`.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// Target name (must match `[A-Za-z0-9_-]+`, e.g. "dev", "prod-east-1").
    pub name: String,

    /// Target type: `pmcp-run`, `aws-lambda`, `google-cloud-run`, `cloudflare-workers`.
    #[arg(long)]
    pub r#type: Option<String>,

    /// pmcp.run API URL (pmcp-run targets only).
    #[arg(long)]
    pub api_url: Option<String>,

    /// AWS CLI profile name (resolved by AWS SDK at use time).
    #[arg(long)]
    pub aws_profile: Option<String>,

    /// AWS region (e.g. us-west-2).
    #[arg(long)]
    pub region: Option<String>,

    /// AWS account ID (aws-lambda targets, optional).
    #[arg(long)]
    pub account_id: Option<String>,

    /// GCP project ID (google-cloud-run targets only).
    #[arg(long)]
    pub gcp_project: Option<String>,

    /// Env-var NAME (not value) holding the API token (cloudflare-workers).
    #[arg(long)]
    pub api_token_env: Option<String>,

    /// Bypass raw-credential pattern detection (for legitimate values that match a heuristic).
    #[arg(long)]
    pub allow_credential_pattern: bool,
}

/// Top-level handler — resolves user config path, reads, mutates, atomic-writes.
pub fn execute(args: AddArgs, _global_flags: &GlobalFlags) -> Result<()> {
    validate_target_name(&args.name)?;

    let path = default_user_config_path();
    let mut cfg = TargetConfigV1::read(&path)?;

    if cfg.targets.contains_key(&args.name) {
        bail!(
            "target '{}' already exists in {} — edit the file directly to update or use a new name",
            args.name,
            path.display()
        );
    }

    let entry = build_entry_from_args_or_prompts(&args)?;
    validate_no_raw_credentials(&entry, args.allow_credential_pattern)?;

    cfg.targets.insert(args.name.clone(), entry);
    cfg.write_atomic(&path)?;

    // A new/!changed target can point at a DIFFERENT deployment, and the discovery
    // cache (~/.pmcp/pmcp-run-config.json) is keyed by api_url, so a stale entry for
    // the previous target would otherwise survive until its 1-hour TTL. Best-effort:
    // failing to clear a cache must not fail the command — the worst case is one
    // stale lookup, which the retry in pmcp_run::graphql also covers.
    let _ = crate::deployment::targets::pmcp_run::auth::clear_config_cache();

    eprintln!("✓ target '{}' added to {}", args.name, path.display());
    eprintln!(
        "  run `cargo pmcp configure use {}` to make it active in this workspace",
        args.name
    );
    Ok(())
}

/// Builds a `TargetEntry` from CLI flags, falling back to interactive prompts for any unset
/// optional fields. The `--type` selector picks the variant; per-variant builders below
/// keep this dispatch shallow (P4 — Phase 75 refactor catalog).
fn build_entry_from_args_or_prompts(args: &AddArgs) -> Result<TargetEntry> {
    let target_type = match &args.r#type {
        Some(t) => t.clone(),
        None => {
            prompt("Target type [pmcp-run / aws-lambda / google-cloud-run / cloudflare-workers]: ")?
        },
    };

    match target_type.as_str() {
        "pmcp-run" => build_pmcp_run_entry(args),
        "aws-lambda" => build_aws_lambda_entry(args),
        "google-cloud-run" => build_google_cloud_run_entry(args),
        "cloudflare-workers" => build_cloudflare_workers_entry(args),
        other => bail!(
            "unknown target type '{}' — must be one of: pmcp-run, aws-lambda, google-cloud-run, cloudflare-workers",
            other
        ),
    }
}

/// Builds a `pmcp-run` entry from flags / prompts.
fn build_pmcp_run_entry(args: &AddArgs) -> Result<TargetEntry> {
    Ok(TargetEntry::PmcpRun(PmcpRunEntry {
        api_url: optional_field(&args.api_url, "api_url (e.g. https://api.pmcp.run): ")?,
        aws_profile: optional_field(&args.aws_profile, "aws_profile (AWS CLI profile name): ")?,
        region: optional_field(&args.region, "region (e.g. us-west-2): ")?,
    }))
}

/// Builds an `aws-lambda` entry from flags / prompts.
fn build_aws_lambda_entry(args: &AddArgs) -> Result<TargetEntry> {
    Ok(TargetEntry::AwsLambda(AwsLambdaEntry {
        aws_profile: optional_field(&args.aws_profile, "aws_profile: ")?,
        region: optional_field(&args.region, "region: ")?,
        account_id: optional_field(&args.account_id, "account_id (12-digit, optional): ")?,
    }))
}

/// Builds a `google-cloud-run` entry from flags / prompts.
fn build_google_cloud_run_entry(args: &AddArgs) -> Result<TargetEntry> {
    Ok(TargetEntry::GoogleCloudRun(GoogleCloudRunEntry {
        gcp_project: optional_field(&args.gcp_project, "gcp_project: ")?,
        region: optional_field(&args.region, "region (e.g. us-central1): ")?,
    }))
}

/// Builds a `cloudflare-workers` entry from flags / prompts.
fn build_cloudflare_workers_entry(args: &AddArgs) -> Result<TargetEntry> {
    Ok(TargetEntry::CloudflareWorkers(CloudflareWorkersEntry {
        account_id: required_field(&args.account_id, "account_id: ")?,
        api_token_env: required_field(
            &args.api_token_env,
            "api_token_env (env var NAME, e.g. MY_CF_TOKEN): ",
        )?,
    }))
}

/// Returns the flag value if set, otherwise prompts; empty input yields `None`.
fn optional_field(flag: &Option<String>, prompt_text: &str) -> Result<Option<String>> {
    if let Some(v) = flag {
        return Ok(Some(v.clone()));
    }
    let s = prompt(prompt_text)?;
    Ok(if s.trim().is_empty() {
        None
    } else {
        Some(s.trim().to_string())
    })
}

/// Returns the flag value if set, otherwise prompts; empty input is rejected.
fn required_field(flag: &Option<String>, prompt_text: &str) -> Result<String> {
    if let Some(v) = flag {
        return Ok(v.clone());
    }
    let s = prompt(prompt_text)?;
    let s = s.trim().to_string();
    if s.is_empty() {
        bail!("required field cannot be empty");
    }
    Ok(s)
}

/// Hand-rolled prompt: write text to stderr, read one line from stdin (no `dialoguer` dep).
fn prompt(text: &str) -> Result<String> {
    eprint!("{text}");
    io::stderr().flush().ok();
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read stdin")?;
    Ok(input.trim_end_matches(['\r', '\n']).to_string())
}

/// Rejects six well-known raw-credential patterns at insertion time (REQ-77-07 / T-77-01).
///
/// Patterns checked are anchored (`^...$`) to avoid false positives on substrings.
/// The error message names the credential KIND but never echoes the matched value
/// (T-77-01-A) and always mentions `--allow-credential-pattern` so users can find
/// the escape hatch from the error itself (GEM-1 from 77-REVIEWS.md).
fn validate_no_raw_credentials(entry: &TargetEntry, allow_override: bool) -> Result<()> {
    if allow_override {
        return Ok(());
    }

    // Per RESEARCH Q3 — six concrete patterns covering AWS/GitHub/Stripe/Google.
    let patterns: &[(&str, &str)] = &[
        (r"^AKIA[0-9A-Z]{16}$", "AWS access key ID"),
        (r"^ASIA[0-9A-Z]{16}$", "AWS temporary session access key"),
        (r"^ghp_[A-Za-z0-9]{36}$", "GitHub personal access token"),
        (r"^github_pat_[A-Za-z0-9_]{82}$", "GitHub fine-grained PAT"),
        (r"^sk_live_[A-Za-z0-9]{24,}$", "Stripe live secret key"),
        (r"^AIza[0-9A-Za-z_-]{35}$", "Google API key"),
    ];

    let scalars = collect_scalar_field_values(entry);
    for value in &scalars {
        for (pat, kind) in patterns {
            let re = regex::Regex::new(pat).expect("validator regex must compile");
            if re.is_match(value) {
                bail!(
                    "value matches a {} pattern — `cargo pmcp configure` stores REFERENCES only.\n\
                     Use one of:\n  - AWS profile name (set AWS_PROFILE or `aws configure --profile <name>`)\n  \
                     - env-var NAME (e.g. `--api-token-env MY_TOKEN`)\n  \
                     - AWS Secrets Manager ARN\n\
                     If this is a legitimate non-secret value, retry with `--allow-credential-pattern`.",
                    kind
                );
            }
        }
    }
    Ok(())
}

/// Collects all scalar string fields from an entry for credential-pattern scanning.
fn collect_scalar_field_values(entry: &TargetEntry) -> Vec<String> {
    let mut v = Vec::new();
    let push_opt = |v: &mut Vec<String>, s: &Option<String>| {
        if let Some(x) = s {
            v.push(x.clone());
        }
    };
    match entry {
        TargetEntry::PmcpRun(e) => {
            push_opt(&mut v, &e.api_url);
            push_opt(&mut v, &e.aws_profile);
            push_opt(&mut v, &e.region);
        },
        TargetEntry::AwsLambda(e) => {
            push_opt(&mut v, &e.aws_profile);
            push_opt(&mut v, &e.region);
            push_opt(&mut v, &e.account_id);
        },
        TargetEntry::GoogleCloudRun(e) => {
            push_opt(&mut v, &e.gcp_project);
            push_opt(&mut v, &e.region);
        },
        TargetEntry::CloudflareWorkers(e) => {
            v.push(e.account_id.clone());
            v.push(e.api_token_env.clone());
        },
    }
    v
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::configure::config::{
        CloudflareWorkersEntry, PmcpRunEntry, TargetConfigV1, TargetEntry,
    };
    use serial_test::serial;

    /// Run `f` with `HOME` overridden to a fresh tempdir; restore HOME after.
    /// Use `#[serial]` on tests using this helper — process-env-mutation is racy.
    fn run_in_isolated_home<F, R>(f: F) -> R
    where
        F: FnOnce(&std::path::Path) -> R,
    {
        let tmp = tempfile::tempdir().expect("tempdir");
        let saved = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        let r = f(tmp.path());
        match saved {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        r
    }

    fn args(name: &str) -> AddArgs {
        AddArgs {
            name: name.to_string(),
            r#type: Some("pmcp-run".into()),
            api_url: Some("https://x.example.com".into()),
            aws_profile: Some("dev".into()),
            region: Some("us-west-2".into()),
            account_id: None,
            gcp_project: None,
            api_token_env: None,
            allow_credential_pattern: false,
        }
    }

    #[test]
    #[serial]
    fn add_creates_target() {
        run_in_isolated_home(|home| {
            let gf = GlobalFlags::default();
            execute(args("dev"), &gf).expect("add must succeed");
            let cfg = TargetConfigV1::read(&home.join(".pmcp").join("config.toml")).unwrap();
            assert!(cfg.targets.contains_key("dev"));
        });
    }

    #[test]
    #[serial]
    fn add_errors_on_duplicate() {
        run_in_isolated_home(|_home| {
            let gf = GlobalFlags::default();
            execute(args("dev"), &gf).unwrap();
            let err = execute(args("dev"), &gf).unwrap_err();
            assert!(err.to_string().contains("already exists"), "got: {err}");
        });
    }

    #[test]
    fn name_validation_rejects_path_traversal() {
        assert!(validate_target_name("../foo").is_err());
        assert!(validate_target_name("foo/bar").is_err());
        assert!(validate_target_name("-foo").is_err());
        assert!(validate_target_name("").is_err());
        assert!(validate_target_name("dev").is_ok());
        assert!(validate_target_name("prod-east-1").is_ok());
        assert!(validate_target_name("staging_v2").is_ok());
    }

    #[test]
    fn reject_aws_access_key_pattern() {
        let entry = TargetEntry::PmcpRun(PmcpRunEntry {
            api_url: Some("AKIAIOSFODNN7EXAMPLE".into()),
            aws_profile: None,
            region: None,
        });
        let err = validate_no_raw_credentials(&entry, false).unwrap_err();
        assert!(err.to_string().contains("AWS access key"), "got: {err}");
        // GEM-1: error message must mention --allow-credential-pattern
        assert!(
            err.to_string().contains("--allow-credential-pattern"),
            "GEM-1: error must mention escape hatch; got: {err}"
        );
    }

    #[test]
    fn reject_github_pat_pattern() {
        let entry = TargetEntry::CloudflareWorkers(CloudflareWorkersEntry {
            account_id: "x".into(),
            api_token_env: "ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789".into(),
        });
        let err = validate_no_raw_credentials(&entry, false).unwrap_err();
        assert!(err.to_string().contains("GitHub"), "got: {err}");
    }

    #[test]
    fn reject_stripe_live_pattern() {
        let entry = TargetEntry::PmcpRun(PmcpRunEntry {
            api_url: Some("sk_live_abcdefghijklmnopqrstuvwx".into()),
            aws_profile: None,
            region: None,
        });
        assert!(validate_no_raw_credentials(&entry, false).is_err());
    }

    #[test]
    fn allow_credential_pattern_bypasses_check() {
        let entry = TargetEntry::PmcpRun(PmcpRunEntry {
            api_url: Some("AKIAIOSFODNN7EXAMPLE".into()),
            aws_profile: None,
            region: None,
        });
        assert!(validate_no_raw_credentials(&entry, true).is_ok());
    }

    #[test]
    #[serial]
    fn aws_lambda_variant_persists_account_id() {
        run_in_isolated_home(|home| {
            let gf = GlobalFlags::default();
            let mut a = args("prod");
            a.r#type = Some("aws-lambda".into());
            a.api_url = None;
            a.account_id = Some("123456789012".into());
            a.region = Some("us-east-1".into());
            execute(a, &gf).unwrap();
            let cfg = TargetConfigV1::read(&home.join(".pmcp").join("config.toml")).unwrap();
            match cfg.targets.get("prod") {
                Some(TargetEntry::AwsLambda(e)) => {
                    assert_eq!(e.account_id.as_deref(), Some("123456789012"));
                    assert_eq!(e.region.as_deref(), Some("us-east-1"));
                },
                other => panic!("expected AwsLambda, got: {:?}", other),
            }
        });
    }
}
