use anyhow::{bail, Context, Result};
use oauth2::{
    basic::{
        BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
        BasicTokenType,
    },
    AuthUrl, AuthorizationCode, Client, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, StandardRevocableToken, StandardTokenResponse, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

// Custom token fields to capture Cognito's id_token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitoTokenFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}

impl oauth2::ExtraTokenFields for CognitoTokenFields {}

type CognitoTokenResponse = StandardTokenResponse<CognitoTokenFields, BasicTokenType>;

// Custom OAuth2 client with Cognito token fields
// This is like BasicClient but with custom token response type
type CognitoClient<
    HasAuthUrl = oauth2::EndpointNotSet,
    HasDeviceAuthUrl = oauth2::EndpointNotSet,
    HasIntrospectionUrl = oauth2::EndpointNotSet,
    HasRevocationUrl = oauth2::EndpointNotSet,
    HasTokenUrl = oauth2::EndpointNotSet,
> = Client<
    BasicErrorResponse,
    CognitoTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    HasAuthUrl,
    HasDeviceAuthUrl,
    HasIntrospectionUrl,
    HasRevocationUrl,
    HasTokenUrl,
>;

// OAuth callback port for local server
const CALLBACK_PORT: u16 = 8787;

// Production defaults for pmcp.run
const DEFAULT_API_URL: &str = "https://api.pmcp.run";
const DEFAULT_AUTH_DOMAIN: &str = "auth.pmcp.run";
pub(crate) const DEFAULT_GRAPHQL_URL: &str = "https://api.pmcp.run/graphql";

// Cache duration for discovered config (1 hour)
const CONFIG_CACHE_DURATION_SECS: u64 = 3600;

/// pmcp.run service configuration discovered from well-known endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmcpRunConfig {
    /// OAuth client ID for authentication
    pub cognito_client_id: String,
    /// Cognito domain for OAuth flows (may include https:// prefix)
    pub cognito_domain: String,
    /// GraphQL API URL
    #[serde(default)]
    pub graphql_url: Option<String>,
    /// MCP endpoint base URL (CLI appends /{serverId}/mcp)
    #[serde(default)]
    pub mcp_url: Option<String>,
    /// API type (must be "graphql" — CLI fails on unknown types)
    #[serde(default)]
    pub api_type: Option<String>,
    /// Config version for compatibility checking
    #[serde(default)]
    pub version: Option<String>,
}

/// Cache entry tagged with the api_url that produced it; mismatched endpoints invalidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedConfig {
    config: PmcpRunConfig,
    cached_at: String,
    /// `#[serde(default)]` lets pre-field caches deserialize and miss the endpoint check.
    #[serde(default)]
    source_api_url: String,
}

impl CachedConfig {
    fn matches_endpoint(&self, current: &str) -> bool {
        normalize_api_url(&self.source_api_url) == normalize_api_url(current)
    }
}

/// Normalize an api_url for cache-key equality: trim, drop trailing `/`, ASCII-lowercase.
/// `https://api.example.com/` and `HTTPS://api.example.com` collapse to the same key.
fn normalize_api_url(s: &str) -> String {
    s.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// Get the base API URL, honoring (in order):
/// 1. `PMCP_API_URL` env var (preferred one-off override)
/// 2. `PMCP_RUN_API_URL` env var (legacy alias)
/// 3. The `api_url` persisted for a `pmcp-run` target via `cargo pmcp configure`
///    (`~/.pmcp/config.toml`) — this is what lets private deployments on a custom
///    domain be reached without exporting an env var on every command
/// 4. `DEFAULT_API_URL` (`https://api.pmcp.run`)
///
/// Step 3 closes the gap for non-target-consuming commands (e.g. `secret set`),
/// which never run the Phase 77 resolver that would otherwise inject the configured
/// `api_url` into `PMCP_API_URL`. Without it, `cargo pmcp configure`'s custom base
/// URL is silently ignored and discovery always hits `api.pmcp.run`.
fn get_api_base_url() -> String {
    if let Some(url) = nonempty_env("PMCP_API_URL") {
        return url;
    }
    if let Some(url) = nonempty_env("PMCP_RUN_API_URL") {
        return url;
    }
    if let Some(url) = configured_api_base_url() {
        return url;
    }
    DEFAULT_API_URL.to_string()
}

/// Read an env var, returning `Some(trimmed)` only when set and non-empty.
fn nonempty_env(key: &str) -> Option<String> {
    let v = std::env::var(key).ok()?;
    let t = v.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Resolve the `api_url` a user configured for a `pmcp-run` target via
/// `cargo pmcp configure add <name> --type pmcp-run --api-url <url>`, stored in
/// `~/.pmcp/config.toml`.
///
/// Resolution:
/// 1. The active named target (`PMCP_TARGET` env > `.pmcp/active-target` marker),
///    when it is a `pmcp-run` target carrying a non-empty `api_url`.
/// 2. Otherwise, when exactly one `pmcp-run` target with an `api_url` is defined,
///    use it — a single-deployment convenience so `configure use` is not required.
///
/// Returns `None` (→ caller falls back to the default) on any read/parse error or
/// when no unambiguous configured base URL exists.
fn configured_api_base_url() -> Option<String> {
    use crate::commands::configure::config::{
        default_user_config_path, TargetConfigV1, TargetEntry,
    };
    use crate::commands::configure::resolver::resolve_active_target_name;

    let cfg = TargetConfigV1::read(&default_user_config_path()).ok()?;

    // 1. Explicitly selected named target (PMCP_TARGET env > active-target marker).
    if let Ok(Some((name, _src))) = resolve_active_target_name(None) {
        return match cfg.targets.get(&name) {
            Some(TargetEntry::PmcpRun(e)) => e
                .api_url
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .map(str::to_string),
            _ => None,
        };
    }

    // 2. No target selected, but exactly one pmcp-run target defines an api_url.
    let mut urls = cfg.targets.values().filter_map(|entry| match entry {
        TargetEntry::PmcpRun(e) => e
            .api_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(str::to_string),
        _ => None,
    });
    let first = urls.next()?;
    if urls.next().is_none() {
        Some(first)
    } else {
        None
    }
}

/// Get the config cache file path
fn config_cache_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let pmcp_dir = home.join(".pmcp");
    if !pmcp_dir.exists() {
        std::fs::create_dir_all(&pmcp_dir)?;
    }
    Ok(pmcp_dir.join("pmcp-run-config.json"))
}

/// Load cached config if valid (within TTL AND keyed to the current `PMCP_API_URL`).
/// Exposed as pub(crate) so graphql.rs and secrets provider can reuse it
/// instead of reimplementing cache reads.
pub(crate) fn load_cached_config() -> Option<PmcpRunConfig> {
    let path = config_cache_path().ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let cached: CachedConfig = serde_json::from_str(&content).ok()?;

    if !cached.matches_endpoint(&get_api_base_url()) {
        return None;
    }

    let cached_at = chrono::DateTime::parse_from_rfc3339(&cached.cached_at).ok()?;
    let age = chrono::Utc::now()
        .signed_duration_since(cached_at)
        .num_seconds();

    if age < CONFIG_CACHE_DURATION_SECS as i64 {
        Some(cached.config)
    } else {
        None
    }
}

/// Save config to cache, stamped with the api_url that produced it.
fn save_config_cache(config: &PmcpRunConfig) -> Result<()> {
    let path = config_cache_path()?;
    let cached = CachedConfig {
        config: config.clone(),
        cached_at: chrono::Utc::now().to_rfc3339(),
        source_api_url: get_api_base_url(),
    };
    std::fs::write(&path, serde_json::to_string_pretty(&cached)?)?;
    Ok(())
}

/// Fetch configuration from pmcp.run discovery endpoint.
/// Retries once on transient failure before giving up.
async fn fetch_pmcp_config() -> Result<PmcpRunConfig> {
    let api_url = get_api_base_url();
    let discovery_url = format!("{}/.well-known/pmcp-config", api_url);
    let client = reqwest::Client::new();

    let mut last_err = None;
    for attempt in 0..2 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let send_result = client
            .get(&discovery_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        match try_fetch_once(send_result, &discovery_url).await? {
            FetchOutcome::Success(config) => return Ok(config),
            FetchOutcome::TransientError(e) => {
                last_err = Some(e);
                continue;
            },
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Discovery endpoint unreachable")))
}

/// One attempt of the discovery-endpoint fetch — classified into either a
/// successful config (validated + cached) or a transient error that warrants
/// a retry.
enum FetchOutcome {
    Success(PmcpRunConfig),
    TransientError(anyhow::Error),
}

async fn try_fetch_once(
    send_result: Result<reqwest::Response, reqwest::Error>,
    discovery_url: &str,
) -> Result<FetchOutcome> {
    let response = match send_result {
        Ok(r) => r,
        Err(e) => return Ok(FetchOutcome::TransientError(e.into())),
    };

    if !response.status().is_success() {
        let status = response.status();
        // 4xx errors are not transient — bubble out as a hard failure.
        if status.is_client_error() {
            bail!(
                "Discovery endpoint returned status {}: {}",
                status,
                discovery_url
            );
        }
        return Ok(FetchOutcome::TransientError(anyhow::anyhow!(
            "Discovery endpoint returned status {}: {}",
            status,
            discovery_url
        )));
    }

    let config: PmcpRunConfig = response
        .json()
        .await
        .context("Failed to parse pmcp.run configuration")?;

    // Validate api_type if present
    validate_api_type(config.api_type.as_deref())?;

    // Cache the fetched config (best-effort — warn on failure)
    if let Err(e) = save_config_cache(&config) {
        eprintln!("⚠️  Warning: Could not cache config: {}", e);
    }

    Ok(FetchOutcome::Success(config))
}

/// Enforce that the discovered config advertises a supported api_type.
fn validate_api_type(api_type: Option<&str>) -> Result<()> {
    if let Some(api_type) = api_type {
        if api_type != "graphql" {
            bail!(
                "Unsupported API type: \"{}\". This version of cargo-pmcp only supports \"graphql\".\n\
                 💡 Update cargo-pmcp: cargo install cargo-pmcp",
                api_type
            );
        }
    }
    Ok(())
}

/// Get pmcp.run configuration with fallback chain:
/// 1. Environment variables (highest priority)
/// 2. Cached config from previous discovery
/// 3. Discovery endpoint fetch
/// 4. Default values (fallback)
pub async fn get_pmcp_config() -> Result<PmcpRunConfig> {
    // Check for environment variable overrides first
    let env_client_id = std::env::var("PMCP_RUN_COGNITO_CLIENT_ID").ok();
    let env_domain = std::env::var("PMCP_RUN_COGNITO_DOMAIN").ok();

    // If both env vars are set, use them directly (legacy env vars take precedence)
    if let (Some(client_id), Some(domain)) = (env_client_id.clone(), env_domain.clone()) {
        return Ok(PmcpRunConfig {
            cognito_client_id: client_id,
            cognito_domain: domain,
            graphql_url: std::env::var("PMCP_RUN_GRAPHQL_URL").ok(),
            mcp_url: None,
            api_type: Some("graphql".to_string()),
            version: None,
        });
    }

    // Try cached config
    if let Some(cached) = load_cached_config() {
        // Apply any env var overrides to cached config
        return Ok(PmcpRunConfig {
            cognito_client_id: env_client_id.unwrap_or(cached.cognito_client_id),
            cognito_domain: env_domain.unwrap_or(cached.cognito_domain),
            graphql_url: std::env::var("PMCP_RUN_GRAPHQL_URL")
                .ok()
                .or(cached.graphql_url),
            mcp_url: cached.mcp_url,
            api_type: cached.api_type,
            version: cached.version,
        });
    }

    // Try discovery endpoint (retries once on transient failure)
    match fetch_pmcp_config().await {
        Ok(config) => {
            // Apply any env var overrides
            Ok(PmcpRunConfig {
                cognito_client_id: env_client_id.unwrap_or(config.cognito_client_id),
                cognito_domain: env_domain.unwrap_or(config.cognito_domain),
                graphql_url: std::env::var("PMCP_RUN_GRAPHQL_URL")
                    .ok()
                    .or(config.graphql_url),
                mcp_url: config.mcp_url,
                api_type: config.api_type,
                version: config.version,
            })
        },
        Err(e) => {
            // Discovery failed - check if we have partial env vars
            if env_client_id.is_some() || env_domain.is_some() {
                eprintln!(
                    "⚠️  Discovery failed, using partial environment config: {}",
                    e
                );
                Ok(PmcpRunConfig {
                    cognito_client_id: env_client_id.unwrap_or_default(),
                    cognito_domain: env_domain.unwrap_or_else(|| DEFAULT_AUTH_DOMAIN.to_string()),
                    graphql_url: std::env::var("PMCP_RUN_GRAPHQL_URL").ok(),
                    mcp_url: None,
                    api_type: Some("graphql".to_string()),
                    version: None,
                })
            } else {
                // No env vars, no cache, discovery failed
                bail!(
                    "❌ Could not retrieve pmcp.run configuration\n\n\
                     Discovery endpoint failed: {}\n\n\
                     💡 Options:\n\
                     1. Configure a custom deployment base URL (private/custom-domain deployments):\n\
                        cargo pmcp configure add <name> --type pmcp-run --api-url https://your.domain\n\
                        cargo pmcp configure use <name>\n\
                     2. Or set PMCP_API_URL to your deployment base URL for a one-off override\n\
                     3. Check your internet connection\n\
                     4. Set legacy environment variables manually:\n\
                        PMCP_RUN_COGNITO_CLIENT_ID=<client_id>\n\
                        PMCP_RUN_COGNITO_DOMAIN=<domain>\n\
                     5. Visit https://pmcp.run/settings for configuration values\n",
                    e
                )
            }
        },
    }
}

/// Get Cognito domain as bare hostname (strips scheme prefix if present).
/// The discovery endpoint may return the full URL per spec v1.0.
fn get_cognito_domain_from_config(config: &PmcpRunConfig) -> String {
    config
        .cognito_domain
        .strip_prefix("https://")
        .or_else(|| config.cognito_domain.strip_prefix("http://"))
        .unwrap_or(&config.cognito_domain)
        .trim_end_matches('/')
        .to_string()
}

/// Get Cognito client ID (legacy function for compatibility)
fn get_cognito_client_id_from_config(config: &PmcpRunConfig) -> String {
    config.cognito_client_id.clone()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_at: String,
}

/// Get credentials file path
fn credentials_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let pmcp_dir = home.join(".pmcp");

    // Create directory if it doesn't exist
    if !pmcp_dir.exists() {
        std::fs::create_dir_all(&pmcp_dir)?;
    }

    Ok(pmcp_dir.join("credentials.toml"))
}

/// Load credentials from file or environment (for CI/CD)
///
/// This function supports two authentication methods:
///
/// 1. **Interactive (developers)**: Reads from `~/.pmcp/credentials.toml`
///    after running `cargo pmcp deploy login --target pmcp-run`
///
/// 2. **Client Credentials (CI/CD)**: Uses OAuth 2.0 client_credentials flow
///    when `PMCP_CLIENT_ID` and `PMCP_CLIENT_SECRET` environment variables are set.
///    This is ideal for automated deployments in CI/CD pipelines like GitHub Actions,
///    GitLab CI, AWS CodeBuild, etc.
///
/// For CI/CD setup, create a Cognito App Client with client_credentials grant enabled,
/// then set the environment variables with your client credentials.
pub async fn get_credentials() -> Result<Credentials> {
    // Check for client credentials flow (M2M / service account for CI/CD)
    if let (Ok(client_id), Ok(client_secret)) = (
        std::env::var("PMCP_CLIENT_ID"),
        std::env::var("PMCP_CLIENT_SECRET"),
    ) {
        return get_credentials_via_client_credentials(&client_id, &client_secret).await;
    }

    // Check for direct access token (alternative CI/CD method)
    if let Ok(access_token) = std::env::var("PMCP_ACCESS_TOKEN") {
        return Ok(Credentials {
            access_token,
            refresh_token: String::new(),
            id_token: std::env::var("PMCP_ID_TOKEN").unwrap_or_default(),
            expires_at: chrono::Utc::now()
                .checked_add_signed(chrono::Duration::hours(1))
                .unwrap()
                .to_rfc3339(),
        });
    }

    // Fall back to file-based credentials (interactive login)
    let path = credentials_path()?;

    if !path.exists() {
        bail!(
            "❌ Not authenticated with pmcp.run\n\n\
             💡 Authentication options:\n\n\
             For interactive use (developers):\n\
               cargo pmcp deploy login --target pmcp-run\n\n\
             For CI/CD pipelines:\n\
               Set PMCP_CLIENT_ID and PMCP_CLIENT_SECRET environment variables\n\
               (requires a Cognito App Client with client_credentials grant)\n"
        );
    }

    let content = std::fs::read_to_string(&path)?;
    let value: toml::Value = toml::from_str(&content)?;

    let pmcp_run = value
        .get("pmcp-run")
        .context("pmcp-run credentials not found")?;

    let credentials: Credentials = toml::from_str(&toml::to_string(pmcp_run)?)?;

    // Check if expired
    let expires_at = chrono::DateTime::parse_from_rfc3339(&credentials.expires_at)
        .context("Invalid expires_at format")?;

    if expires_at < chrono::Utc::now() {
        // Try to refresh
        return refresh_credentials(&credentials.refresh_token).await;
    }

    Ok(credentials)
}

/// OAuth 2.0 client_credentials token response
#[derive(Debug, Deserialize)]
struct ClientCredentialsResponse {
    access_token: String,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    token_type: Option<String>,
}

/// Get credentials using OAuth 2.0 client_credentials flow (M2M authentication)
///
/// This is used for CI/CD pipelines and automated deployments where interactive
/// login is not possible. Requires a Cognito App Client configured with:
/// - client_credentials grant type enabled
/// - A client secret
/// - Appropriate resource server scopes
async fn get_credentials_via_client_credentials(
    client_id: &str,
    client_secret: &str,
) -> Result<Credentials> {
    let config = get_pmcp_config().await?;
    let cognito_domain = get_cognito_domain_from_config(&config);
    let token_url = format!("https://{}/oauth2/token", cognito_domain);

    let client = reqwest::Client::new();
    let response = client
        .post(&token_url)
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .context("Failed to request access token via client_credentials")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!(
            "❌ Client credentials authentication failed\n\n\
             Status: {}\n\
             Response: {}\n\n\
             💡 Verify that:\n\
             1. PMCP_CLIENT_ID and PMCP_CLIENT_SECRET are correct\n\
             2. The Cognito App Client has client_credentials grant enabled\n\
             3. The client secret matches the App Client configuration\n",
            status,
            body
        );
    }

    let token_response: ClientCredentialsResponse = response
        .json()
        .await
        .context("Failed to parse token response")?;

    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(
            token_response.expires_in.unwrap_or(3600) as i64,
        ))
        .unwrap()
        .to_rfc3339();

    Ok(Credentials {
        access_token: token_response.access_token,
        refresh_token: String::new(), // client_credentials doesn't return refresh token
        id_token: token_response.id_token.unwrap_or_default(),
        expires_at,
    })
}

/// Refresh access token using refresh token
async fn refresh_credentials(refresh_token: &str) -> Result<Credentials> {
    println!("🔄 Refreshing access token...");

    let config = get_pmcp_config().await?;
    let client = create_oauth_client_from_config(&config)?;
    // Use oauth2's re-exported reqwest client for token exchange compatibility.
    // The oauth2 crate requires its own reqwest type for request_async().
    let http_client = oauth2::reqwest::Client::new();

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request_async(&http_client)
        .await
        .map_err(|e| {
            eprintln!("❌ Token refresh failed: {}", e);
            eprintln!();
            eprintln!("💡 Your refresh token may have expired or become invalid.");
            eprintln!("   Please login again:");
            eprintln!("   cargo pmcp deploy login --target pmcp-run");
            eprintln!();
            anyhow::anyhow!("Failed to refresh token: {}", e)
        })?;

    let credentials = Credentials {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: refresh_token.to_string(), // Keep existing refresh token
        id_token: token_result
            .extra_fields()
            .id_token
            .clone()
            .unwrap_or_default(),
        expires_at: chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(
                token_result
                    .expires_in()
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(3600),
            ))
            .unwrap()
            .to_rfc3339(),
    };

    save_credentials(&credentials)?;
    println!("✅ Token refreshed successfully");

    Ok(credentials)
}

/// Save credentials to file
fn save_credentials(credentials: &Credentials) -> Result<()> {
    let path = credentials_path()?;

    let mut config = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    let creds_toml = toml::to_string(credentials)?;
    let creds_value: toml::Value = toml::from_str(&creds_toml)?;

    config
        .as_table_mut()
        .context("Invalid TOML structure")?
        .insert("pmcp-run".to_string(), creds_value);

    std::fs::write(&path, toml::to_string(&config)?)?;

    Ok(())
}

/// Create OAuth 2.0 client from config
fn create_oauth_client_from_config(
    config: &PmcpRunConfig,
) -> Result<
    CognitoClient<
        oauth2::EndpointSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointSet,
    >,
> {
    let cognito_domain = get_cognito_domain_from_config(config);
    let cognito_client_id = get_cognito_client_id_from_config(config);

    let auth_url = AuthUrl::new(format!("https://{}/oauth2/authorize", cognito_domain))
        .context("Invalid auth URL")?;
    let token_url = TokenUrl::new(format!("https://{}/oauth2/token", cognito_domain))
        .context("Invalid token URL")?;

    Ok(Client::new(ClientId::new(cognito_client_id))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url))
}

/// Start local HTTP server to receive OAuth callback
fn start_callback_server() -> Result<String> {
    let (tx, rx) = mpsc::channel();

    println!(
        "🌐 Starting local callback server on http://localhost:{}...",
        CALLBACK_PORT
    );

    std::thread::spawn(move || run_callback_server_loop(&tx));

    rx.recv_timeout(Duration::from_secs(300))
        .context("Authentication timed out (5 minutes)")
}

/// Main loop of the OAuth callback HTTP server: accept one matching request,
/// send success/failure HTML, and forward the decoded code through `tx`.
fn run_callback_server_loop(tx: &mpsc::Sender<String>) {
    let server = tiny_http::Server::http(format!("127.0.0.1:{}", CALLBACK_PORT)).unwrap();

    for request in server.incoming_requests() {
        let code_value = extract_code_from_url(request.url());

        if let Some(code) = code_value {
            respond_callback_success(request);
            let decoded = urlencoding::decode(&code).unwrap();
            tx.send(decoded.to_string()).unwrap();
            return;
        }

        respond_callback_failure(request);
    }
}

/// Parse a callback URL's query string and return the first `code` value.
fn extract_code_from_url(url: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            if key == "code" {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Send the success HTML response for the callback.
fn respond_callback_success(request: tiny_http::Request) {
    let response = tiny_http::Response::from_string(
        "<html><body><h1>✅ Authentication Successful!</h1>\
        <p>You can close this window and return to your terminal.</p>\
        </body></html>",
    )
    .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap());
    let _ = request.respond(response);
}

/// Send the failure HTML response for the callback.
fn respond_callback_failure(request: tiny_http::Request) {
    let response = tiny_http::Response::from_string(
        "<html><body><h1>❌ Authentication Failed</h1>\
        <p>No code received. Please try again.</p>\
        </body></html>",
    );
    let _ = request.respond(response);
}

/// Execute OAuth login flow with PKCE
pub async fn login() -> Result<()> {
    println!("🔐 Authenticating with pmcp.run...");
    println!();

    // Fetch configuration (from discovery endpoint or env vars)
    println!("📡 Fetching pmcp.run configuration...");
    let config = get_pmcp_config().await?;
    println!("   Using auth domain: {}", config.cognito_domain);
    println!();

    let client = create_oauth_client_from_config(&config)?;

    // Generate PKCE challenge for enhanced security
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Build authorization URL
    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .set_redirect_uri(std::borrow::Cow::Owned(
            RedirectUrl::new(format!("http://localhost:{}", CALLBACK_PORT))
                .context("Invalid redirect URL")?,
        ))
        .url();

    // Start callback server in background
    let code_future = tokio::task::spawn_blocking(start_callback_server);

    // Open browser
    println!("📱 Opening browser for authentication...");
    println!("   If the browser doesn't open, visit:");
    println!("   {}", auth_url);
    println!();

    if let Err(e) = open::that(auth_url.as_str()) {
        println!("⚠️  Could not open browser automatically: {}", e);
        println!("   Please open the URL manually");
        println!();
    }

    println!("⏳ Waiting for authentication callback...");

    // Wait for authorization code
    let code = code_future.await??;

    // Exchange code for tokens
    println!("🔐 Exchanging authorization code for tokens...");
    let redirect_url = RedirectUrl::new(format!("http://localhost:{}", CALLBACK_PORT))
        .context("Invalid redirect URL")?;

    // Use oauth2's re-exported reqwest client for token exchange compatibility.
    // The oauth2 crate requires its own reqwest type for request_async().
    let http_client = oauth2::reqwest::Client::new();
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .set_redirect_uri(std::borrow::Cow::Owned(redirect_url))
        .request_async(&http_client)
        .await
        .map_err(|e| {
            eprintln!("Token exchange error details: {:?}", e);
            anyhow::anyhow!("Failed to exchange authorization code for tokens: {:?}", e)
        })?;

    // Extract tokens
    let credentials = Credentials {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result
            .refresh_token()
            .map(|t| t.secret().clone())
            .unwrap_or_default(),
        id_token: token_result
            .extra_fields()
            .id_token
            .clone()
            .unwrap_or_default(),
        expires_at: chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(
                token_result
                    .expires_in()
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(3600),
            ))
            .unwrap()
            .to_rfc3339(),
    };

    save_credentials(&credentials)?;

    println!();
    println!("✅ Successfully authenticated with pmcp.run!");
    println!("   Access token expires: {}", credentials.expires_at);
    println!();
    println!("💡 You can now deploy with: cargo pmcp deploy --target pmcp-run");

    Ok(())
}

/// Logout (remove credentials)
pub fn logout() -> Result<()> {
    let path = credentials_path()?;

    if !path.exists() {
        println!("ℹ️  Not currently authenticated with pmcp.run");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let mut config: toml::Value = toml::from_str(&content)?;

    if let Some(table) = config.as_table_mut() {
        table.remove("pmcp-run");

        if table.is_empty() {
            std::fs::remove_file(&path)?;
            println!("✅ Logged out from pmcp.run (removed credentials file)");
        } else {
            std::fs::write(&path, toml::to_string(&config)?)?;
            println!("✅ Logged out from pmcp.run");
        }
    }

    Ok(())
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use serial_test::serial;
    use std::ffi::{OsStr, OsString};

    /// RAII guard that restores an env var on drop — including on panic, so a
    /// failing test cannot leak mutated env state into other `#[serial]` tests.
    struct EnvGuard {
        key: &'static str,
        prev: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let prev = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, prev }
        }

        fn remove(key: &'static str) -> Self {
            let prev = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    /// Run `f` with `HOME` pointing at a fresh tempdir, `PMCP_API_URL` set to
    /// `api_url`, and `PMCP_RUN_API_URL` cleared. All three are restored on
    /// drop (even if `f` panics).
    fn with_isolated_env<F, R>(api_url: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let home_tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("HOME", home_tmp.path());
        let _api = EnvGuard::set("PMCP_API_URL", api_url);
        let _legacy = EnvGuard::remove("PMCP_RUN_API_URL");
        f()
    }

    fn fixture_config(mcp_url: &str) -> PmcpRunConfig {
        PmcpRunConfig {
            cognito_client_id: "client-id".into(),
            cognito_domain: "https://auth.example.com".into(),
            graphql_url: Some("https://graphql.example.com/graphql".into()),
            mcp_url: Some(mcp_url.into()),
            api_type: Some("graphql".into()),
            version: Some("1.0".into()),
        }
    }

    #[test]
    #[serial]
    fn cache_hits_when_api_url_unchanged() {
        with_isolated_env("https://dev.api.example.com", || {
            let config = fixture_config("https://dev.mcp.example.com");
            save_config_cache(&config).unwrap();
            let loaded = load_cached_config().expect("cache should hit on same api_url");
            assert_eq!(
                loaded.mcp_url.as_deref(),
                Some("https://dev.mcp.example.com")
            );
        });
    }

    #[test]
    #[serial]
    fn cache_misses_when_api_url_changes() {
        // Save under the dev api_url, then read under prod — must miss.
        with_isolated_env("https://dev.api.example.com", || {
            let config = fixture_config("https://dev.mcp.example.com");
            save_config_cache(&config).unwrap();

            // Switch api_url within the same isolated HOME.
            std::env::set_var("PMCP_API_URL", "https://prod.api.example.com");
            assert!(
                load_cached_config().is_none(),
                "cache must NOT hit when PMCP_API_URL differs from the cached source"
            );
        });
    }

    #[test]
    #[serial]
    fn cache_hits_when_api_url_differs_only_in_trailing_slash_or_case() {
        // Trailing-slash and case differences must not produce a spurious miss.
        with_isolated_env("https://Dev.API.example.com/", || {
            let config = fixture_config("https://dev.mcp.example.com");
            save_config_cache(&config).unwrap();

            // Same endpoint, different casing + no trailing slash.
            std::env::set_var("PMCP_API_URL", "https://dev.api.example.com");
            assert!(
                load_cached_config().is_some(),
                "normalize_api_url should treat case + trailing-slash variants as equal"
            );
        });
    }

    #[test]
    #[serial]
    fn cache_misses_for_legacy_payload_with_no_source_api_url() {
        // Caches written before #2 will have an empty source_api_url after
        // serde-default. They must fail the api_url match and refresh.
        with_isolated_env("https://dev.api.example.com", || {
            let path = config_cache_path().unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            // Legacy shape: no `source_api_url` field at all.
            let legacy = serde_json::json!({
                "config": {
                    "cognito_client_id": "cid",
                    "cognito_domain": "https://auth.example.com",
                    "graphql_url": "https://graphql.example.com/graphql",
                    "mcp_url": "https://stale.example.com",
                    "api_type": "graphql",
                    "version": "1.0",
                },
                "cached_at": chrono::Utc::now().to_rfc3339(),
            });
            std::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();
            assert!(
                load_cached_config().is_none(),
                "legacy cache (missing source_api_url) must miss so it gets refreshed"
            );
        });
    }

    /// Write `~/.pmcp/config.toml` (HOME must already be an isolated tempdir) with a
    /// single `pmcp-run` target carrying `api_url`.
    fn write_single_pmcp_run_target(name: &str, api_url: &str) {
        let home = std::env::var("HOME").expect("HOME must be set by the test harness");
        let pmcp_dir = std::path::Path::new(&home).join(".pmcp");
        std::fs::create_dir_all(&pmcp_dir).unwrap();
        let body = format!(
            "schema_version = 1\n\n[targets.{name}]\ntype = \"pmcp-run\"\napi_url = \"{api_url}\"\n"
        );
        std::fs::write(pmcp_dir.join("config.toml"), body).unwrap();
    }

    #[test]
    #[serial]
    fn env_pmcp_api_url_overrides_configured_target() {
        // PMCP_API_URL (one-off override) must win over the persisted configure api_url.
        let home_tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("HOME", home_tmp.path());
        let _target = EnvGuard::remove("PMCP_TARGET");
        let _legacy = EnvGuard::remove("PMCP_RUN_API_URL");
        let _api = EnvGuard::set("PMCP_API_URL", "https://env.example.com");
        write_single_pmcp_run_target("prod", "https://configured.example.com");

        assert_eq!(get_api_base_url(), "https://env.example.com");
    }

    #[test]
    #[serial]
    fn configured_target_base_url_used_when_no_env() {
        // With no env override, the api_url from `cargo pmcp configure` (selected via
        // PMCP_TARGET) must be honored instead of the hardcoded api.pmcp.run default.
        let home_tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("HOME", home_tmp.path());
        let _api = EnvGuard::remove("PMCP_API_URL");
        let _legacy = EnvGuard::remove("PMCP_RUN_API_URL");
        let _target = EnvGuard::set("PMCP_TARGET", "prod");
        write_single_pmcp_run_target("prod", "https://private.customer.example");

        assert_eq!(get_api_base_url(), "https://private.customer.example");
    }

    #[test]
    #[serial]
    fn falls_back_to_default_when_nothing_configured() {
        // No env vars, no config file → the hardcoded default remains the behavior.
        let home_tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("HOME", home_tmp.path());
        let _api = EnvGuard::remove("PMCP_API_URL");
        let _legacy = EnvGuard::remove("PMCP_RUN_API_URL");
        let _target = EnvGuard::remove("PMCP_TARGET");

        assert_eq!(get_api_base_url(), DEFAULT_API_URL);
    }
}
