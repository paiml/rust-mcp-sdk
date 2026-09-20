//! `cargo pmcp auth refresh <url>` — refresh the stored access token.

use anyhow::Result;
use clap::Args;
use colored::Colorize;

use crate::commands::auth_cmd::cache::{
    load_for_server, normalize_server_key, open_store, refresh_through_sdk, report_migration,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth refresh <url>` — refresh the stored token for one server.
#[derive(Debug, Args)]
pub struct RefreshArgs {
    /// URL of the stored MCP server to refresh.
    pub url: String,
}

/// Execute the `refresh` subcommand.
///
/// Drives the SDK's refresh path, which sends only the GRANTED scope, sources
/// the `client_id` from the stored record (so a dynamically-registered client
/// can refresh at all) and keeps the stored refresh token when the
/// authorization server's response omits one.
///
/// # What is reported
///
/// The SDK serves a token that has not yet expired from the store verbatim
/// rather than spending a refresh on it, so this command reports which of the
/// two happened instead of claiming a refresh unconditionally. Announcing work
/// that did not occur would be worse than the missing force: an operator
/// diagnosing a stale token needs to know whether the authorization server was
/// actually contacted.
pub async fn execute(args: RefreshArgs, global_flags: &GlobalFlags) -> Result<()> {
    let store = open_store();
    let key = normalize_server_key(&args.url)?;

    let found = load_for_server(&store, &key).await?;
    report_migration(&store).await?;

    let Some((credential_key, before)) = found else {
        anyhow::bail!(
            "no cached credentials for {}. Run `cargo pmcp auth login {}` first.",
            key,
            key
        );
    };

    let token = refresh_through_sdk(store.clone(), &args.url, credential_key.issuer()).await?;

    if global_flags.should_output() {
        if token == before.access_token() {
            println!(
                "Token for {} is still valid; no refresh was needed.",
                key.bright_green()
            );
        } else {
            println!("Refreshed token for {}.", key.bright_green());
        }
    }
    Ok(())
}
