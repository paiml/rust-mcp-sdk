//! Shared authentication middleware resolution for server-connecting commands.
//!
//! Converts an [`AuthMethod`] (produced by [`AuthFlags::resolve()`]) into
//! the HTTP middleware chain expected by `ServerTester`, `LoadTestEngine`,
//! and other consumers. This module centralizes the auth wiring so that
//! every command uses the same logic.

use std::sync::Arc;

use anyhow::Result;
use pmcp::client::http_middleware::HttpMiddlewareChain;
use pmcp::client::oauth::{default_cache_path, OAuthConfig, OAuthHelper};
use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};

use super::auth_cmd::cache::{
    is_near_expiry, load_for_server, normalize_server_key, open_store, refresh_through_sdk,
    REFRESH_WINDOW_SECS,
};
use super::flags::AuthMethod;

/// Look up an access token for `mcp_server_url` in the shared credential store.
///
/// Transparently refreshes through the SDK when within [`REFRESH_WINDOW_SECS`]
/// of expiry.
///
/// Returns `Ok(None)` when:
/// - the credential file does not exist, OR
/// - nothing is stored for the normalized URL.
async fn try_cache_token(mcp_server_url: &str) -> Result<Option<String>> {
    let store = open_store();
    let key = normalize_server_key(mcp_server_url)?;
    // A missing credential file reads as an empty store, not as an error.
    let Some((credential_key, credentials)) = load_for_server(&store, &key).await? else {
        return Ok(None);
    };

    if is_near_expiry(&credentials, REFRESH_WINDOW_SECS) {
        let refreshed = refresh_through_sdk(store.clone(), mcp_server_url, credential_key.issuer())
            .await
            .map_err(|e| anyhow::anyhow!(
                "cached token for {key} expired and refresh failed: {e}\nRun `cargo pmcp auth login {key}` to re-authenticate."
            ))?;
        Ok(Some(refreshed))
    } else {
        Ok(Some(credentials.access_token().to_string()))
    }
}

/// Wrap a bearer `access_token` in a single-middleware `HttpMiddlewareChain`.
///
/// Shared by the `AuthMethod::None` cache-hit path and the `AuthMethod::ApiKey`
/// path so that both produce an identical bearer-token chain.
fn bearer_chain(access_token: String) -> Arc<HttpMiddlewareChain> {
    let bearer = BearerToken::new(access_token);
    let middleware = OAuthClientMiddleware::new(bearer);
    let mut chain = HttpMiddlewareChain::new();
    chain.add(Arc::new(middleware));
    Arc::new(chain)
}

/// Build an [`OAuthHelper`] from the fields of an [`AuthMethod::OAuth`] variant.
///
/// Centralizes `OAuthConfig` construction so that `resolve_auth_middleware`
/// and `resolve_auth_header` share a single code path.
fn build_oauth_helper(
    mcp_server_url: &str,
    client_id: &str,
    issuer: &Option<String>,
    scopes: &[String],
    no_cache: bool,
    redirect_port: u16,
) -> Result<OAuthHelper> {
    let cache_file = if no_cache {
        None
    } else {
        Some(default_cache_path())
    };
    let config = OAuthConfig {
        issuer: issuer.clone(),
        mcp_server_url: Some(mcp_server_url.to_string()),
        client_id: Some(client_id.to_string()),
        client_name: None,
        dcr_enabled: false,
        scopes: scopes.to_vec(),
        cache_file,
        redirect_port,
    };
    OAuthHelper::new(config).map_err(|e| anyhow::anyhow!("OAuth setup failed: {e}"))
}

/// Convert an [`AuthMethod`] into an optional HTTP middleware chain.
///
/// Returns `Ok(None)` when no auth is configured. For API key auth, wraps
/// the key in a `BearerToken` middleware. For OAuth, runs the full PKCE
/// flow (or loads a cached token) via `OAuthHelper`.
///
/// Call this once at command startup -- the returned chain is `Arc`-wrapped
/// and safe to share across concurrent requests.
pub async fn resolve_auth_middleware(
    mcp_server_url: &str,
    auth_method: &AuthMethod,
) -> Result<Option<Arc<HttpMiddlewareChain>>> {
    match auth_method {
        // Cache is the lowest-precedence fallback, consulted only when no
        // explicit flag or env var was provided.
        AuthMethod::None => match try_cache_token(mcp_server_url).await? {
            Some(token) => Ok(Some(bearer_chain(token))),
            None => Ok(None),
        },

        AuthMethod::ApiKey(key) => Ok(Some(bearer_chain(key.clone()))),

        AuthMethod::OAuth {
            client_id,
            issuer,
            scopes,
            no_cache,
            redirect_port,
        } => {
            let helper = build_oauth_helper(
                mcp_server_url,
                client_id,
                issuer,
                scopes,
                *no_cache,
                *redirect_port,
            )?;
            let chain = helper
                .create_middleware_chain()
                .await
                .map_err(|e| anyhow::anyhow!("OAuth authentication failed: {e}"))?;
            Ok(Some(chain))
        },
    }
}

/// Resolve auth into an `Authorization` header value (e.g. `"Bearer sk-..."`).
///
/// For API key auth, returns the key as a bearer token. For OAuth, runs the
/// full PKCE flow (or loads a cached token) and returns the access token.
/// Returns `Ok(None)` when no auth is configured.
///
/// Use this for consumers that need a plain header string (preview, schema)
/// rather than an [`HttpMiddlewareChain`] (test check, loadtest).
pub async fn resolve_auth_header(
    mcp_server_url: &str,
    auth_method: &AuthMethod,
) -> Result<Option<String>> {
    match auth_method {
        AuthMethod::None => match try_cache_token(mcp_server_url).await? {
            Some(token) => Ok(Some(format!("Bearer {token}"))),
            None => Ok(None),
        },
        AuthMethod::ApiKey(key) => Ok(Some(format!("Bearer {}", key))),
        AuthMethod::OAuth {
            client_id,
            issuer,
            scopes,
            no_cache,
            redirect_port,
        } => {
            let helper = build_oauth_helper(
                mcp_server_url,
                client_id,
                issuer,
                scopes,
                *no_cache,
                *redirect_port,
            )?;
            let token = helper
                .get_access_token()
                .await
                .map_err(|e| anyhow::anyhow!("OAuth token acquisition failed: {e}"))?;
            Ok(Some(format!("Bearer {}", token)))
        },
    }
}

#[cfg(test)]
mod cache_fallback_tests {
    use super::*;

    /// Non-cached URL — guaranteed not in the user's real `~/.pmcp/oauth-cache.json`.
    ///
    /// This confirms `try_cache_token` returns `Ok(None)` on key-miss without
    /// crashing when the cache file does not exist OR exists without this key.
    #[tokio::test]
    async fn cache_miss_returns_none() {
        let result = try_cache_token("https://nonexistent-this-should-not-be-cached-74.example")
            .await
            .unwrap();
        assert!(result.is_none());
    }
}
