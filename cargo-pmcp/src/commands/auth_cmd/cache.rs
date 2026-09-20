//! Thin adapters onto the SDK's credential store for `cargo pmcp auth`.
//!
//! # One machine, one credential store
//!
//! This module used to be a SECOND, unrelated implementation of credential
//! storage: its own record type, its own document format, its own reader and
//! its own atomic writer, sitting beside the SDK's. A single machine therefore
//! carried two credential stores with two formats and two sets of semantics.
//!
//! It no longer does. The record, the document format, the migration and the
//! on-disk I/O all live in the SDK:
//!
//! - [`pmcp::shared::credential_store`] — the `(issuer, account, server)` key,
//!   the record, the document format and the migration, all I/O-free.
//! - [`pmcp::shared::credential_file`] — `FileCredentialStore`, the default
//!   on-disk implementation, which performs a serialized read-modify-write per
//!   mutation and writes `0o600` files into a `0o700` parent.
//!
//! What survives here is the FILE and its DATA — still
//! `~/.pmcp/oauth-cache.json` — plus the handful of CLI-shaped adapters the five
//! `auth` subcommands need. The migration is deliberately NOT reimplemented
//! here: it lives in the SDK's pure parser, so a hosting platform and this CLI
//! cannot diverge on what an existing login means.
//!
//! # The key is three-part, and the account is empty
//!
//! Every key this crate builds is `(issuer, "", normalized_server_url)`. The
//! account is [`CLI_ACCOUNT_SCOPE`] — the empty string, the single-user CLI
//! case. The SERVER component is what makes `auth logout <url>` mean "this
//! server" rather than "everything issued by this authorization server", which
//! matters as soon as two MCP servers share one authorization server.
//!
//! # This module compiles in the library target
//!
//! It is mounted into `cargo_pmcp`'s lib target via `#[path]` (see the crate
//! root) so integration tests can reach the same adapters the binary uses. It
//! must therefore never reference `crate::commands::*`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use pmcp::client::oauth::{Interactivity, OAuthConfig, OAuthHelper};
use pmcp::shared::credential_store::{
    CredentialKey, CredentialStore, CredentialStoreAdmin, StoredCredentials,
};
use pmcp::{default_credential_path, FileCredentialStore};

/// The SDK's MCP-server-URL normalizer, re-exported as the one this crate uses.
///
/// This replaced a crate-local `normalize_cache_key` with identical behaviour
/// (lowercase host, strip path, strip trailing slash, strip default ports); the
/// idempotence property that made it safe moved into the SDK with it.
pub use pmcp::shared::credential_store::normalize_server_key;

/// The account component every credential this CLI stores is keyed under.
///
/// Empty on purpose: `cargo pmcp` is single-user, and the account scope exists
/// for multi-tenant platform callers. Inventing an identity concept in the CLI
/// would make its keys unreachable from the platform seam and vice versa.
pub const CLI_ACCOUNT_SCOPE: &str = "";

/// Transparent refresh fires when the stored access token is within this many
/// seconds of expiry.
pub const REFRESH_WINDOW_SECS: u64 = 60;

/// The loopback port an `auth` subcommand nominally offers for a redirect.
///
/// The refresh path never binds it — it runs under
/// [`Interactivity::RefreshOnly`], where the interactive tail is unreachable by
/// construction — but `OAuthConfig` requires a value.
const REFRESH_REDIRECT_PORT: u16 = 8080;

/// Returns `~/.pmcp/oauth-cache.json` (or `./.pmcp/oauth-cache.json` as a
/// fallback when the home directory cannot be resolved).
///
/// The same path [`pmcp::default_credential_path`] resolves, so the SDK and this
/// CLI operate on ONE document. The fallback preserves this crate's
/// pre-existing behaviour of degrading rather than failing outright.
pub fn default_multi_cache_path() -> PathBuf {
    default_credential_path().unwrap_or_else(|_| {
        let mut path = PathBuf::from(".");
        path.push(".pmcp");
        path.push("oauth-cache.json");
        path
    })
}

/// Open the shared credential store at the default location.
///
/// Construction touches no filesystem: only a read or a write does.
pub fn open_store() -> Arc<FileCredentialStore> {
    Arc::new(FileCredentialStore::new(default_multi_cache_path()))
}

/// Tell the operator about a migration the store performed while reading, and
/// clear it so it is not repeated within this process.
///
/// A dropped entry is a FORCED RE-LOGIN, so it is named individually along with
/// the command that fixes it. Both counts are reported, because "2 migrated"
/// alone hides the fact that a third login was lost.
///
/// Everything goes to **stderr**: `auth token`'s stdout must stay exactly the
/// token so `TOKEN=$(cargo pmcp auth token URL)` keeps working.
pub async fn report_migration(store: &FileCredentialStore) -> Result<()> {
    let Some(report) = store
        .take_migration_report()
        .await
        .context("reading the credential store's migration report")?
    else {
        return Ok(());
    };

    if report.migrated() > 0 {
        eprintln!(
            "Migrated {} cached credential(s) from the previous cache format.",
            report.migrated()
        );
    }
    for dropped in report.dropped() {
        eprintln!(
            "warning: dropped cached credentials for {} — {}. \
             Run `cargo pmcp auth login {}` to re-authenticate.",
            dropped.server_key(),
            dropped.reason(),
            dropped.server_key()
        );
    }
    if !report.dropped().is_empty() {
        eprintln!(
            "Dropped {} cached credential(s) that could not be migrated.",
            report.dropped().len()
        );
    }
    Ok(())
}

/// Returns `true` when the stored credentials expire within `grace_secs`.
///
/// Credentials with no recorded expiry are treated as long-lived, exactly as
/// before this module was ported onto the SDK's store.
///
/// # What this gate buys after the port, and what it no longer buys
///
/// It decides whether to hand the request to [`refresh_through_sdk`] at all, so
/// a token with plenty of life left still costs no network round-trip. It no
/// longer forces a rotation: the SDK renews a token once it has EXPIRED and
/// serves an unexpired one verbatim, so a credential inside this window is
/// returned unchanged. That is a deliberate narrowing — the expiry decision now
/// lives in one place instead of two — and it is why neither `auth token` nor
/// `auth refresh` announces a refresh it has not observed.
pub fn is_near_expiry(credentials: &StoredCredentials, grace_secs: u64) -> bool {
    let Some(expires_at) = credentials.expires_at() else {
        return false;
    };
    expires_at.saturating_sub(grace_secs) <= current_unix_secs()
}

/// Seconds since the Unix epoch, saturating to `0` on a clock set before 1970.
pub fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Every key stored for `server_key`, in document order.
pub async fn keys_for_server(
    store: &FileCredentialStore,
    server_key: &str,
) -> Result<Vec<CredentialKey>> {
    Ok(store
        .list_keys()
        .await
        .context("listing stored credentials")?
        .into_iter()
        .filter(|key| key.server() == server_key)
        .collect())
}

/// Resolve the credentials recorded for one MCP server.
///
/// The issuer comes from the store's own last-seen-issuer record, which is
/// written in the same update as the credentials themselves, so the key can be
/// rebuilt without a network round-trip. The `list_keys` fallback exists because
/// a store written by something other than this CLI could hold credentials
/// without that record, and a lookup that gave up there would report a login the
/// operator can plainly see in `auth status` as missing.
pub async fn load_for_server(
    store: &FileCredentialStore,
    server_key: &str,
) -> Result<Option<(CredentialKey, StoredCredentials)>> {
    let key = match store
        .last_issuer(server_key)
        .await
        .context("reading the recorded authorization server")?
    {
        Some(issuer) => CredentialKey::new(issuer, CLI_ACCOUNT_SCOPE, server_key),
        None => match keys_for_server(store, server_key).await?.into_iter().next() {
            Some(key) => key,
            None => return Ok(None),
        },
    };

    let found = store
        .load(&key)
        .await
        .context("reading the credential store")?;
    Ok(found.map(|credentials| (key, credentials)))
}

/// Refresh `server_url`'s access token through the SDK's own refresh path and
/// persist the result.
///
/// The SDK path is used rather than a second refresh implementation here so that
/// this crate inherits its three properties for free: an authorization server
/// that omits `refresh_token` no longer costs the stored one, a
/// dynamically-registered client refreshes with the `client_id` the store holds
/// rather than one this CLI never had, and the request carries exactly the
/// GRANTED scope or none at all.
///
/// [`Interactivity::RefreshOnly`] means no browser is opened and no loopback
/// listener is bound: `auth token` and `auth refresh` are scripting commands, and
/// a five-minute wait on a callback nobody is watching is not an acceptable
/// outcome for either.
///
/// # This is NOT a force-refresh, and callers must not describe it as one
///
/// The SDK serves a stored token that has not yet expired VERBATIM and spends a
/// refresh only once it has. The implementation this replaced posted to the
/// token endpoint unconditionally. Routing through the SDK is the plan's
/// binding instruction — a second refresh implementation here is exactly the
/// two-store divergence this port exists to end — so the force is genuinely
/// gone, and every caller reports what it OBSERVED rather than what it asked
/// for. Announcing a rotation that did not happen would be worse than the
/// missing force: an operator diagnosing a stale token needs to know whether
/// the authorization server was contacted at all.
pub async fn refresh_through_sdk(
    store: Arc<FileCredentialStore>,
    server_url: &str,
    issuer: &str,
) -> Result<String> {
    let config = OAuthConfig {
        // The recorded issuer is the discovery SEED. Passing it preserves the
        // behaviour of the implementation this replaced, which fetched the
        // recorded issuer's discovery document directly rather than deriving an
        // authorization server from the MCP base URL.
        issuer: Some(issuer.to_string()),
        mcp_server_url: Some(server_url.to_string()),
        // Deliberately absent: the effective client id comes from the stored
        // record, which is the only place a dynamically-registered one exists.
        client_id: None,
        client_name: None,
        dcr_enabled: false,
        scopes: Vec::new(),
        // Persistence goes through the injected store below, never through a
        // path in this config.
        cache_file: None,
        redirect_port: REFRESH_REDIRECT_PORT,
    };

    let credential_store: Arc<dyn CredentialStore> = store;
    let helper = OAuthHelper::new(config)
        .map_err(|e| anyhow::anyhow!("OAuth setup failed: {e}"))?
        .with_credential_store(credential_store)
        .with_account_scope(CLI_ACCOUNT_SCOPE)
        .with_interactivity(Interactivity::RefreshOnly);

    helper
        .get_access_token()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_cmd_default_path_ends_in_oauth_cache_json() {
        let p = default_multi_cache_path();
        let s = p.to_string_lossy();
        assert!(
            s.ends_with(".pmcp/oauth-cache.json") || s.ends_with(".pmcp\\oauth-cache.json"),
            "got: {s}"
        );
    }

    #[test]
    fn auth_cmd_normalize_folds_slash_case_and_default_port() {
        assert_eq!(
            normalize_server_key("HTTPS://MCP.Example.Com/").unwrap(),
            "https://mcp.example.com"
        );
        assert_eq!(
            normalize_server_key("https://mcp.example.com:443/v1/api").unwrap(),
            "https://mcp.example.com"
        );
        assert_eq!(
            normalize_server_key("http://localhost:8080/mcp").unwrap(),
            "http://localhost:8080"
        );
        assert!(normalize_server_key("not a url").is_err());
    }

    #[test]
    fn auth_cmd_near_expiry_window_matches_the_documented_60_seconds() {
        let now = current_unix_secs();
        assert!(is_near_expiry(
            &StoredCredentials::new("at", "c").with_expires_at(now + 30),
            REFRESH_WINDOW_SECS
        ));
        assert!(!is_near_expiry(
            &StoredCredentials::new("at", "c").with_expires_at(now + 3600),
            REFRESH_WINDOW_SECS
        ));
        // No recorded expiry is treated as long-lived, not as "refresh now".
        assert!(!is_near_expiry(
            &StoredCredentials::new("at", "c"),
            REFRESH_WINDOW_SECS
        ));
    }

    #[test]
    fn auth_cmd_account_scope_is_empty_so_the_cli_invents_no_identity() {
        assert_eq!(CLI_ACCOUNT_SCOPE, "");
        let key = CredentialKey::new("https://as.example", CLI_ACCOUNT_SCOPE, "https://x.example");
        assert_eq!(key.account(), "");
        assert_eq!(key.issuer(), "https://as.example");
        assert_eq!(key.server(), "https://x.example");
    }

    #[tokio::test]
    async fn auth_cmd_load_for_server_resolves_through_the_recorded_issuer() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileCredentialStore::new(dir.path().join("oauth-cache.json"));
        let key = CredentialKey::new(
            "https://as.example",
            CLI_ACCOUNT_SCOPE,
            "https://mcp.example",
        );
        store
            .save_with_issuer(
                &key,
                &StoredCredentials::new("at", "cid"),
                "https://mcp.example",
                "https://as.example",
            )
            .await
            .unwrap();

        let (found_key, found) = load_for_server(&store, "https://mcp.example")
            .await
            .unwrap()
            .expect("credentials present");
        assert_eq!(found_key, key);
        assert_eq!(found.access_token(), "at");

        assert!(load_for_server(&store, "https://other.example")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn auth_cmd_load_for_server_is_a_miss_on_a_store_that_was_never_written() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileCredentialStore::new(dir.path().join("does-not-exist.json"));
        assert!(load_for_server(&store, "https://mcp.example")
            .await
            .unwrap()
            .is_none());
    }
}
