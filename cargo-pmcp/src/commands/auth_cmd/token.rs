//! `cargo pmcp auth token <url>` — raw access token to stdout (gh-style).

use anyhow::Result;
use clap::Args;

use crate::commands::auth_cmd::cache::{
    is_near_expiry, load_for_server, normalize_server_key, open_store, refresh_through_sdk,
    report_migration, REFRESH_WINDOW_SECS,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth token <url>` — print the stored token raw to stdout.
#[derive(Debug, Args)]
pub struct TokenArgs {
    /// URL of the MCP server whose token should be printed.
    pub url: String,
}

/// Execute the `token` subcommand.
///
/// Prints the stored token raw to stdout with a trailing newline, refreshing
/// through the SDK when within [`REFRESH_WINDOW_SECS`] of expiry. Status
/// messages — including any migration notice — go to stderr so stdout stays
/// scriptable (`TOKEN=$(cargo pmcp auth token URL)`).
///
/// The lookup is OFFLINE for a live token: the authorization server is read from
/// the store's own issuer record rather than rediscovered, so printing a token
/// costs no network round-trip.
///
/// # A failed renewal never falls back to the stale token
///
/// When the stored token is close to expiry the request is handed to the SDK,
/// and any failure there propagates. Printing the expired token instead would
/// be worse than failing: the caller is a script that would send it, get a 401
/// it has no way to interpret, and never learn that a re-login was required.
pub async fn execute(args: TokenArgs, _global_flags: &GlobalFlags) -> Result<()> {
    let store = open_store();
    let key = normalize_server_key(&args.url)?;

    let found = load_for_server(&store, &key).await?;
    report_migration(&store).await?;

    let Some((credential_key, credentials)) = found else {
        anyhow::bail!(
            "no cached token for {}. Run `cargo pmcp auth login {}` first.",
            key,
            key
        );
    };

    let token = if is_near_expiry(&credentials, REFRESH_WINDOW_SECS) {
        // Reported AFTER the fact, and only when the token actually changed:
        // the SDK serves an unexpired token verbatim, so announcing a refresh
        // up front would claim work that frequently does not happen.
        let served = refresh_through_sdk(store.clone(), &args.url, credential_key.issuer()).await?;
        if served != credentials.access_token() {
            eprintln!("Renewed the stored token for {}.", key);
        }
        served
    } else {
        credentials.access_token().to_string()
    };

    println!("{}", token);
    Ok(())
}
