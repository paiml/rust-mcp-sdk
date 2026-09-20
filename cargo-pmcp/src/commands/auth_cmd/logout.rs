//! `cargo pmcp auth logout [<url> | --all]` — remove stored credentials.

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp::shared::credential_store::CredentialStoreAdmin;

use crate::commands::auth_cmd::cache::{normalize_server_key, open_store, report_migration};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth logout [<url> | --all]` — remove one server or wipe the store.
#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// URL of the MCP server to log out from (mutually exclusive with --all).
    #[arg(conflicts_with = "all")]
    pub url: Option<String>,

    /// Log out from every stored server.
    #[arg(long)]
    pub all: bool,
}

/// Execute the `logout` subcommand.
///
/// With no args: errors out. `--all` clears every credential; a positional URL
/// removes exactly that server's.
///
/// # Four load-bearing semantics
///
/// This is the only subcommand that DESTROYS credentials, so all four of its
/// behaviours are pinned by tests asserting the exact message text:
///
/// 1. neither a URL nor `--all` is an error naming both options;
/// 2. `--all` clears everything and reports the count;
/// 3. a positional URL removes exactly that server — and only that server, even
///    when a second server shares its authorization server, because
///    `delete_by_server` operates on the key's SERVER component;
/// 4. a URL with nothing stored is a friendly no-op, not an error.
///
/// All four are expressed through declared [`CredentialStoreAdmin`] methods.
/// Nothing here reaches around the trait into the file, which is what stops the
/// two-store divergence this port exists to end from quietly reappearing.
pub async fn execute(args: LogoutArgs, global_flags: &GlobalFlags) -> Result<()> {
    if args.url.is_none() && !args.all {
        anyhow::bail!("specify a server URL or --all to log out of everything");
    }

    let store = open_store();

    if args.all {
        let count = store
            .clear_all()
            .await
            .context("clearing the credential store")?;
        report_migration(&store).await?;
        if global_flags.should_output() {
            println!("Logged out of {} cached server(s).", count);
        }
        return Ok(());
    }

    let raw_url = args.url.as_deref().expect("url set (checked above)");
    let key = normalize_server_key(raw_url)?;
    let removed = store
        .delete_by_server(&key)
        .await
        .with_context(|| format!("removing stored credentials for {key}"))?;
    report_migration(&store).await?;

    if global_flags.should_output() {
        if removed > 0 {
            println!("Logged out of {}.", key.bright_green());
        } else {
            println!(
                "No cached credentials for {} (nothing to do).",
                key.bright_yellow()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::GlobalFlags;

    fn gf() -> GlobalFlags {
        GlobalFlags {
            verbose: false,
            no_color: true,
            quiet: true,
        }
    }

    #[tokio::test]
    async fn no_args_errors() {
        let err = execute(
            LogoutArgs {
                url: None,
                all: false,
            },
            &gf(),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{err}").contains("specify a server URL or --all"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn clap_rejects_url_with_all() {
        use clap::Parser;
        #[derive(clap::Parser)]
        struct T {
            #[command(flatten)]
            a: LogoutArgs,
        }
        let r = T::try_parse_from(["t", "https://x.example", "--all"]);
        assert!(r.is_err());
    }
}
