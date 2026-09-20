//! `cargo pmcp auth login` — PKCE + optional DCR, persisted through the SDK's
//! credential store.

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp::client::oauth::{OAuthConfig, OAuthHelper};

use crate::commands::auth_cmd::cache::{
    current_unix_secs, normalize_server_key, open_store, report_migration, CLI_ACCOUNT_SCOPE,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth login <url> [flags]`
///
/// Runs a full OAuth authorization (PKCE + optional DCR) against the named MCP
/// server. The resulting access token, refresh token, granted scopes and
/// effective client id are persisted by the SDK under the
/// `(issuer, account, server)` key in `~/.pmcp/oauth-cache.json`, together with
/// the authorization server that issued them.
#[derive(Debug, Args)]
pub struct LoginArgs {
    /// URL of the MCP server to authenticate against
    pub url: String,

    /// Client name for Dynamic Client Registration (RFC 7591).
    /// Mutually exclusive with `--oauth-client-id`.
    #[arg(long, conflicts_with = "oauth_client_id")]
    pub client: Option<String>,

    /// Pre-registered OAuth client ID. Skips DCR entirely.
    #[arg(long, env = "MCP_OAUTH_CLIENT_ID")]
    pub oauth_client_id: Option<String>,

    /// OAuth issuer URL for OIDC discovery. Set this when the authorization
    /// server is a third party (Cognito, Auth0, Okta, Entra) rather than the
    /// MCP server itself; discovery then runs against this issuer instead of
    /// one derived from <URL>.
    #[arg(long, env = "MCP_OAUTH_ISSUER")]
    pub oauth_issuer: Option<String>,

    /// OAuth scopes (comma-separated).
    #[arg(long, env = "MCP_OAUTH_SCOPES", value_delimiter = ',')]
    pub oauth_scopes: Option<Vec<String>>,

    /// Localhost port for the OAuth redirect callback.
    #[arg(long, env = "MCP_OAUTH_REDIRECT_PORT", default_value = "8080")]
    pub oauth_redirect_port: u16,
}

/// Execute the `login` subcommand — run the OAuth flow and persist the result.
///
/// Persistence happens inside the SDK, through the store injected below: there
/// is exactly one write, and it records the credentials and the issuer that
/// issued them in the same update.
pub async fn execute(args: LoginArgs, global_flags: &GlobalFlags) -> Result<()> {
    let key = normalize_server_key(&args.url)
        .with_context(|| format!("normalizing login URL {}", args.url))?;

    let client_name = args.client.clone().or_else(|| {
        if args.oauth_client_id.is_none() {
            Some("cargo-pmcp".to_string())
        } else {
            None
        }
    });

    let scopes = args
        .oauth_scopes
        .clone()
        .unwrap_or_else(|| vec!["openid".to_string()]);

    let config = OAuthConfig {
        issuer: args.oauth_issuer.clone(),
        mcp_server_url: Some(args.url.clone()),
        client_id: args.oauth_client_id.clone(),
        client_name,
        dcr_enabled: args.oauth_client_id.is_none(),
        scopes: scopes.clone(),
        cache_file: None,
        redirect_port: args.oauth_redirect_port,
    };

    if global_flags.should_output() {
        println!();
        println!("{}", "OAuth Login".bright_cyan().bold());
        println!("  URL: {}", args.url.bright_white());
        if let Some(ref n) = args.client {
            println!("  Client name (DCR): {}", n.bright_white());
        }
        if args.oauth_client_id.is_some() {
            println!("  Client ID: (pre-registered, DCR skipped)");
        }
        // Stated explicitly because it changes WHERE discovery goes and which
        // value the RFC 8414 section 3.3 anchor check is applied to. An
        // operator overriding discovery should see that the override took
        // effect rather than having to infer it.
        if let Some(ref issuer) = args.oauth_issuer {
            println!(
                "  Issuer: {} {}",
                issuer.bright_white(),
                "(explicit — discovery uses this, not a value derived from the URL above)".dimmed()
            );
        }
        println!();
    }

    let store = open_store();
    let helper = OAuthHelper::new(config.clone())
        .context("OAuth setup failed")?
        .with_credential_store(store.clone())
        .with_account_scope(CLI_ACCOUNT_SCOPE);
    let result = helper
        .authorize_with_details()
        .await
        .context("OAuth flow failed")?;

    // Reported after the store has actually been read and written, so the
    // counts describe work that really happened.
    report_migration(&store).await?;

    if global_flags.should_output() {
        let scope_str = if result.scopes.is_empty() {
            if scopes.is_empty() {
                "<none>".to_string()
            } else {
                scopes.join(",")
            }
        } else {
            result.scopes.join(",")
        };
        let issuer_str = result
            .issuer
            .clone()
            .unwrap_or_else(|| "<auto>".to_string());
        let expires_str = match result.expires_at {
            Some(exp) => {
                let now = current_unix_secs();
                if exp > now {
                    format!("{}s", exp - now)
                } else {
                    "already expired".to_string()
                }
            },
            None => "n/a (IdP did not advertise expires_in)".to_string(),
        };
        // Token is never printed — shell history and shared-terminal hygiene.
        println!(
            "Logged in to {} (issuer: {}, scopes: {}, expires in: {})",
            key.bright_green().bold(),
            issuer_str,
            scope_str,
            expires_str,
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_rejects_client_with_oauth_client_id() {
        use clap::Parser;
        #[derive(clap::Parser)]
        struct TestCli {
            #[command(flatten)]
            args: LoginArgs,
        }
        let result = TestCli::try_parse_from([
            "test-cli",
            "https://x.example",
            "--client",
            "claude-desktop",
            "--oauth-client-id",
            "some-id",
        ]);
        assert!(
            result.is_err(),
            "clap must reject --client with --oauth-client-id (D-19)"
        );
    }

    #[test]
    fn clap_accepts_client_alone() {
        use clap::Parser;
        #[derive(clap::Parser)]
        struct TestCli {
            #[command(flatten)]
            args: LoginArgs,
        }
        let ok = TestCli::try_parse_from([
            "test-cli",
            "https://x.example",
            "--client",
            "claude-desktop",
        ]);
        assert!(ok.is_ok());
    }
}
