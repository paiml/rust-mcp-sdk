//! `cargo pmcp auth status [<url>]` — tabular inspection of stored credentials.

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp::shared::credential_store::{
    CredentialKey, CredentialStore, CredentialStoreAdmin, StoredCredentials,
};
use pmcp::FileCredentialStore;

use crate::commands::auth_cmd::cache::{
    current_unix_secs, normalize_server_key, open_store, report_migration,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth status [<url>]` — print a 5-column credential table.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// URL to inspect. If absent, prints a table of every stored server.
    pub url: Option<String>,
}

/// Execute the `status` subcommand.
///
/// Renders a tabular view sourced from [`CredentialStoreAdmin::list_keys`]:
/// URL | ISSUER | SCOPES | EXPIRES | REFRESHABLE. Never prints an access token.
pub async fn execute(args: StatusArgs, global_flags: &GlobalFlags) -> Result<()> {
    let store = open_store();
    let keys = store
        .list_keys()
        .await
        .context("listing stored credentials")?;
    report_migration(&store).await?;

    if keys.is_empty() {
        println!("No cached credentials. Run `cargo pmcp auth login <url>` to authenticate.");
        return Ok(());
    }

    let Some(selected) = select_keys(&keys, args.url.as_deref())? else {
        return Ok(());
    };

    if global_flags.no_color {
        colored::control::set_override(false);
    }

    print_header_row();
    let now = current_unix_secs();
    for key in selected {
        let Some(credentials) = load_row(&store, &key).await? else {
            continue;
        };
        print_status_row(&key, &credentials, now);
    }
    Ok(())
}

/// Read one row's details. A key that vanished between the listing and the read
/// is skipped rather than reported as an error — another process logging out
/// concurrently is not a failure of this command.
async fn load_row(
    store: &FileCredentialStore,
    key: &CredentialKey,
) -> Result<Option<StoredCredentials>> {
    store
        .load(key)
        .await
        .with_context(|| format!("reading stored credentials for {}", key.server()))
}

/// Resolve the keys to display. Returns:
/// - `Some(keys)` to print one or more rows.
/// - `None` if a specific URL was requested but nothing is stored for it (the
///   caller should return `Ok(())` after the "no cached credentials" line).
fn select_keys(keys: &[CredentialKey], url: Option<&str>) -> Result<Option<Vec<CredentialKey>>> {
    let Some(raw) = url else {
        return Ok(Some(keys.to_vec()));
    };
    let server_key = normalize_server_key(raw)?;
    let matching: Vec<CredentialKey> = keys
        .iter()
        .filter(|key| key.server() == server_key)
        .cloned()
        .collect();
    if matching.is_empty() {
        println!("No cached credentials for {}.", server_key.bright_yellow());
        return Ok(None);
    }
    Ok(Some(matching))
}

/// Print the bright-cyan bold header row with column titles.
fn print_header_row() {
    let header = format!(
        "{:<40}  {:<30}  {:<25}  {:<14}  {}",
        "URL", "ISSUER", "SCOPES", "EXPIRES", "REFRESHABLE"
    );
    println!("{}", header.bright_cyan().bold());
}

/// Format the EXPIRES column (pure helper).
///
/// Returns `(formatted_text, is_expired)` so the caller can colorize the row red
/// when expired without the ANSI escape inflating the column width.
fn format_expires_column(expires_at: Option<u64>, now: u64) -> (String, bool) {
    match expires_at {
        Some(exp) if exp > now => (format!("in {}s", exp - now), false),
        Some(_) => ("EXPIRED".to_string(), true),
        None => ("<unknown>".to_string(), false),
    }
}

/// Print one row of the status table, in red when the credentials are expired.
fn print_status_row(key: &CredentialKey, credentials: &StoredCredentials, now: u64) {
    let scopes = if credentials.granted_scopes().is_empty() {
        "<none>".to_string()
    } else {
        credentials.granted_scopes().join(",")
    };
    let (expires_plain, expires_is_expired) = format_expires_column(credentials.expires_at(), now);
    let refreshable_plain = if credentials.refresh_token().is_some() {
        "yes"
    } else {
        "no"
    };

    let row_plain = format!(
        "{:<40}  {:<30}  {:<25}  {:<14}  {}",
        key.server(),
        key.issuer(),
        scopes,
        expires_plain,
        refreshable_plain
    );
    if expires_is_expired {
        println!("{}", row_plain.bright_red());
    } else {
        println!("{}", row_plain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth_cmd::cache::CLI_ACCOUNT_SCOPE;

    fn key(server: &str) -> CredentialKey {
        CredentialKey::new("https://as.example", CLI_ACCOUNT_SCOPE, server)
    }

    #[test]
    fn auth_cmd_status_selects_every_key_when_no_url_is_given() {
        let keys = vec![key("https://a.example"), key("https://b.example")];
        let selected = select_keys(&keys, None).unwrap().unwrap();
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn auth_cmd_status_selects_one_server_and_normalizes_the_url() {
        let keys = vec![key("https://a.example"), key("https://b.example")];
        let selected = select_keys(&keys, Some("HTTPS://A.Example/v1/"))
            .unwrap()
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].server(), "https://a.example");
    }

    #[test]
    fn auth_cmd_status_reports_nothing_to_show_for_an_unknown_server() {
        let keys = vec![key("https://a.example")];
        assert!(select_keys(&keys, Some("https://never-seen.example"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn auth_cmd_status_expires_column_distinguishes_the_three_states() {
        assert_eq!(
            format_expires_column(Some(100), 40),
            ("in 60s".into(), false)
        );
        assert_eq!(
            format_expires_column(Some(10), 40),
            ("EXPIRED".into(), true)
        );
        assert_eq!(
            format_expires_column(None, 40),
            ("<unknown>".to_string(), false)
        );
    }
}
