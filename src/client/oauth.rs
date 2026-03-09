//! OAuth authentication support for CLI tools.
//!
//! This module implements multiple OAuth 2.0 flows for CLI authentication:
//! - Authorization Code Flow with PKCE (RFC 7636) - browser-based, most compatible
//! - Device Code Flow (RFC 8628) - fallback for servers that support it
//!
//! Supports automatic OAuth discovery via:
//! - OpenID Connect Discovery (/.well-known/openid-configuration)
//! - OAuth 2.0 Server Metadata (/.well-known/oauth-authorization-server)
//!
//! # Feature Gate
//!
//! This module is only available when the `oauth` feature is enabled:
//!
//! ```toml
//! pmcp = { version = "1.11", features = ["oauth"] }
//! ```

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::sleep;
use url::Url;

use crate::client::auth::{OidcDiscoveryClient, TokenExchangeClient};
use crate::client::http_middleware::HttpMiddlewareChain;
use crate::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
use crate::error::{Error, Result};
use crate::server::auth::oauth2::OidcDiscoveryMetadata;

/// OAuth configuration for CLI authentication flows.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// OAuth issuer URL (e.g., `https://auth.example.com`).
    /// If `None`, will auto-discover from MCP server URL.
    pub issuer: Option<String>,
    /// MCP server URL for auto-discovery (required if issuer is `None`).
    pub mcp_server_url: Option<String>,
    /// OAuth client ID.
    pub client_id: String,
    /// OAuth scopes to request.
    pub scopes: Vec<String>,
    /// Cache file path for storing tokens.
    pub cache_file: Option<PathBuf>,
    /// Redirect port for localhost callback (default: 8080).
    pub redirect_port: u16,
}

/// Token cache stored on disk.
#[derive(Debug, Serialize, Deserialize)]
struct TokenCache {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
    scopes: Vec<String>,
}

/// Device code authorization response.
#[derive(Debug, Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

/// Token response from the OAuth token endpoint.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    token_type: String,
}

/// OAuth helper for CLI authentication flows.
///
/// Supports both Authorization Code Flow with PKCE and Device Code Flow,
/// with automatic discovery of OAuth endpoints from the MCP server URL.
#[derive(Debug)]
pub struct OAuthHelper {
    config: OAuthConfig,
    client: reqwest::Client,
}

impl OAuthHelper {
    /// Create a new OAuth helper with the given configuration.
    pub fn new(config: OAuthConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { config, client })
    }

    /// Extract base URL from MCP server URL.
    ///
    /// For example, `https://api.example.com/mcp` becomes `https://api.example.com`.
    fn extract_base_url(mcp_url: &str) -> Result<String> {
        let parsed = Url::parse(mcp_url)
            .map_err(|e| Error::internal(format!("Invalid MCP server URL: {e}")))?;

        // Build base URL with scheme, host, and port
        let mut base = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""));
        if let Some(port) = parsed.port() {
            // Only add port if it's not the default for the scheme
            let is_default_port = (parsed.scheme() == "https" && port == 443)
                || (parsed.scheme() == "http" && port == 80);
            if !is_default_port {
                base.push_str(&format!(":{}", port));
            }
        }

        Ok(base)
    }

    /// Discover OAuth metadata from MCP server URL using OIDC discovery.
    async fn discover_metadata(&self, mcp_url: &str) -> Result<OidcDiscoveryMetadata> {
        let base_url = Self::extract_base_url(mcp_url)?;

        tracing::info!("Discovering OAuth configuration from {}...", base_url);

        let discovery_client = OidcDiscoveryClient::new();

        match discovery_client.discover(&base_url).await {
            Ok(metadata) => {
                tracing::info!("OAuth discovery successful");
                tracing::debug!("Issuer: {}", metadata.issuer);
                if let Some(ref device_endpoint) = metadata.device_authorization_endpoint {
                    tracing::debug!("Device endpoint: {}", device_endpoint);
                }
                Ok(metadata)
            },
            Err(e) => Err(Error::internal(format!(
                "Failed to discover OAuth configuration at {}: {}\n\
                 \n\
                 Please provide --oauth-issuer explicitly, or ensure the server\n\
                 exposes OAuth metadata at {}/.well-known/openid-configuration",
                base_url, e, base_url
            ))),
        }
    }

    /// Get OAuth metadata (either by discovering or constructing from issuer).
    async fn get_metadata(&self) -> Result<OidcDiscoveryMetadata> {
        if let Some(ref mcp_url) = self.config.mcp_server_url {
            // Discover from MCP server URL
            self.discover_metadata(mcp_url).await
        } else if let Some(ref issuer) = self.config.issuer {
            // Manually provided issuer - try to discover from it
            tracing::info!("Discovering OAuth configuration from {}...", issuer);

            let discovery_client = OidcDiscoveryClient::new();
            match discovery_client.discover(issuer).await {
                Ok(metadata) => {
                    tracing::info!("OAuth discovery successful");
                    Ok(metadata)
                },
                Err(e) => Err(Error::internal(format!(
                    "Failed to discover OAuth configuration from issuer {}: {}\n\
                     \n\
                     Please ensure the issuer URL exposes OAuth metadata at\n\
                     {}/.well-known/openid-configuration",
                    issuer, e, issuer
                ))),
            }
        } else {
            Err(Error::internal(
                "Either oauth_issuer or mcp_server_url must be provided for OAuth authentication"
                    .to_string(),
            ))
        }
    }

    /// Get or refresh access token, performing OAuth flow if needed.
    pub async fn get_access_token(&self) -> Result<String> {
        // Try to load cached token first
        if let Some(ref cache_file) = self.config.cache_file {
            if let Ok(cached) = self.load_cached_token(cache_file).await {
                // Check if token is still valid
                if let Some(expires_at) = cached.expires_at {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if now < expires_at {
                        tracing::info!("Using cached OAuth token");
                        return Ok(cached.access_token);
                    }
                }

                // Try to refresh if we have a refresh token
                if let Some(refresh_token) = cached.refresh_token {
                    tracing::warn!("OAuth token expired, refreshing...");
                    if let Ok(new_token) = self.refresh_token(&refresh_token).await {
                        self.cache_token(&new_token, cache_file).await?;
                        return Ok(new_token.access_token);
                    }
                }
            }
        }

        // No valid cached token, try authorization code flow first
        tracing::info!("No cached token found, starting OAuth flow...");

        // Get metadata to see what flows are supported
        let metadata = self.get_metadata().await?;

        // Try authorization code flow first (more common, works with MCP Inspector-like servers)
        match self.authorization_code_flow(&metadata).await {
            Ok(token) => Ok(token),
            Err(e) => {
                tracing::warn!("Authorization code flow failed: {}", e);

                // Fall back to device code flow if available
                if metadata.device_authorization_endpoint.is_some() {
                    tracing::info!("Trying device code flow...");
                    return self.device_code_flow_with_metadata(&metadata).await;
                }
                Err(Error::internal(
                    "No supported OAuth flow available.\n\
                     \n\
                     The server must support either:\n\
                     - Authorization code flow (authorization_endpoint), or\n\
                     - Device code flow (device_authorization_endpoint)"
                        .to_string(),
                ))
            },
        }
    }

    /// Generate PKCE code verifier (RFC 7636).
    fn generate_code_verifier() -> String {
        let random_bytes: [u8; 32] = rand::rng().random();
        URL_SAFE_NO_PAD.encode(random_bytes)
    }

    /// Generate PKCE code challenge from verifier (RFC 7636).
    fn generate_code_challenge(verifier: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        URL_SAFE_NO_PAD.encode(hash)
    }

    /// Perform OAuth authorization code flow with PKCE.
    async fn authorization_code_flow(&self, metadata: &OidcDiscoveryMetadata) -> Result<String> {
        tracing::info!("Starting OAuth authorization code flow...");

        // Generate PKCE challenge
        let code_verifier = Self::generate_code_verifier();
        let code_challenge = Self::generate_code_challenge(&code_verifier);

        // Start local callback server on configured port
        let redirect_port = self.config.redirect_port;
        let redirect_uri = format!("http://localhost:{}/callback", redirect_port);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", redirect_port))
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to bind to localhost:{}.\n\
                     \n\
                     This port may already be in use. Try a different port with:\n\
                     --oauth-redirect-port PORT\n\
                     \n\
                     Error: {e}",
                    redirect_port
                ))
            })?;

        tracing::debug!("Local callback server listening on port {}", redirect_port);
        tracing::warn!(
            "Ensure the redirect URI is registered in your OAuth provider: {}",
            redirect_uri
        );

        // Build authorization URL
        let mut auth_url = Url::parse(&metadata.authorization_endpoint)
            .map_err(|e| Error::internal(format!("Invalid authorization endpoint: {e}")))?;

        auth_url
            .query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", &self.config.scopes.join(" "))
            .append_pair("code_challenge", &code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &Self::generate_code_verifier()); // Random state for CSRF protection

        tracing::info!("OAuth Authentication Required");
        tracing::info!("Opening browser for authentication...");
        tracing::info!("If the browser doesn't open, visit: {}", auth_url.as_str());

        // Open browser
        if let Err(e) = webbrowser::open(auth_url.as_str()) {
            tracing::warn!(
                "Failed to open browser: {}. Please open the URL manually.",
                e
            );
        }

        // Wait for OAuth callback
        let (tx, rx) = oneshot::channel();
        let callback_task = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut reader = BufReader::new(&mut stream);
                let mut request_line = String::new();

                if reader.read_line(&mut request_line).await.is_ok() {
                    // Parse the request line to extract the authorization code
                    if let Some(path) = request_line.split_whitespace().nth(1) {
                        if let Ok(callback_url) = Url::parse(&format!("http://localhost{}", path)) {
                            let code = callback_url
                                .query_pairs()
                                .find(|(key, _)| key == "code")
                                .map(|(_, value)| value.to_string());

                            // Send success response to browser
                            let response = if code.is_some() {
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                 <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
                                 <h1 style='color: green;'>Authentication Successful!</h1>\
                                 <p>You can close this window and return to the terminal.</p>\
                                 </body></html>"
                            } else {
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                                 <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
                                 <h1 style='color: red;'>Authentication Failed</h1>\
                                 <p>No authorization code received. Please try again.</p>\
                                 </body></html>"
                            };

                            let _ = stream.write_all(response.as_bytes()).await;
                            let _ = stream.flush().await;

                            if let Some(code) = code {
                                let _ = tx.send(code);
                            }
                        }
                    }
                }
            }
        });

        tracing::info!("Waiting for authorization...");

        // Wait for callback with timeout
        let authorization_code = tokio::time::timeout(Duration::from_secs(300), rx)
            .await
            .map_err(|_| {
                Error::internal("Timeout waiting for OAuth callback (5 minutes)".to_string())
            })?
            .map_err(|e| Error::internal(format!("OAuth callback channel error: {e}")))?;

        callback_task.abort();

        tracing::info!("Authorization code received");

        // Exchange authorization code for access token
        tracing::debug!("Exchanging authorization code for access token...");

        let token_exchange = TokenExchangeClient::new();
        let token_response = token_exchange
            .exchange_code(
                &metadata.token_endpoint,
                &authorization_code,
                &self.config.client_id,
                None, // No client secret for public clients
                &redirect_uri,
                Some(&code_verifier), // PKCE verifier
            )
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to exchange authorization code for token: {e}"
                ))
            })?;

        tracing::info!("Authentication successful");

        // Cache the token
        if let Some(ref cache_file) = self.config.cache_file {
            self.cache_token_from_response(&token_response, cache_file)
                .await?;
        }

        Ok(token_response.access_token)
    }

    /// Perform OAuth device code flow (with pre-fetched metadata).
    async fn device_code_flow_with_metadata(
        &self,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<String> {
        tracing::info!("Starting OAuth device code flow...");

        // Check if device flow is supported
        let device_auth_endpoint =
            metadata
                .device_authorization_endpoint
                .as_ref()
                .ok_or_else(|| {
                    Error::internal(
                        "Device authorization endpoint not found in OAuth metadata.\n\
                         \n\
                         The OAuth server does not support device code flow (RFC 8628)."
                            .to_string(),
                    )
                })?;

        // Rest of device code flow implementation...
        self.device_code_flow_internal(metadata, device_auth_endpoint)
            .await
    }

    /// Internal implementation of device code flow.
    async fn device_code_flow_internal(
        &self,
        metadata: &OidcDiscoveryMetadata,
        device_auth_endpoint: &str,
    ) -> Result<String> {
        // Step 1: Request device code
        let scope = self.config.scopes.join(" ");

        let response = self
            .client
            .post(device_auth_endpoint)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("scope", &scope),
            ])
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to request device code: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Device authorization failed: {}",
                response.text().await.unwrap_or_default()
            )));
        }

        let device_auth: DeviceAuthResponse = response.json().await.map_err(|e| {
            Error::internal(format!(
                "Failed to parse device authorization response: {e}"
            ))
        })?;

        // Step 2: Display user code and verification URL
        tracing::info!("OAuth device code flow");
        tracing::info!("1. Visit: {}", device_auth.verification_uri);
        tracing::info!("2. Enter code: {}", device_auth.user_code);

        if let Some(complete_uri) = &device_auth.verification_uri_complete {
            tracing::info!("Or visit directly: {}", complete_uri);
        }

        // Step 3: Poll for token
        let poll_interval = Duration::from_secs(device_auth.interval.unwrap_or(5));
        let token_endpoint = &metadata.token_endpoint;
        let expires_at = SystemTime::now() + Duration::from_secs(device_auth.expires_in);

        loop {
            if SystemTime::now() > expires_at {
                return Err(Error::internal(
                    "Device code expired. Please try again.".to_string(),
                ));
            }

            sleep(poll_interval).await;
            tracing::debug!("Polling for authorization...");

            let response = self
                .client
                .post(token_endpoint)
                .form(&[
                    ("client_id", self.config.client_id.as_str()),
                    ("device_code", &device_auth.device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| Error::internal(format!("Failed to poll for token: {e}")))?;

            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|e| Error::internal(format!("Failed to read token response body: {e}")))?;

            if status.is_success() {
                let token_response: TokenResponse = serde_json::from_str(&body)
                    .map_err(|e| Error::internal(format!("Failed to parse token response: {e}")))?;

                tracing::info!("Authentication successful");

                // Cache the token
                if let Some(ref cache_file) = self.config.cache_file {
                    self.cache_token(&token_response, cache_file).await?;
                }

                return Ok(token_response.access_token);
            }

            // Check error response
            if let Ok(error) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(error_code) = error.get("error").and_then(|e| e.as_str()) {
                    match error_code {
                        "authorization_pending" => continue,
                        "slow_down" => {
                            sleep(poll_interval).await;
                            continue;
                        },
                        "access_denied" => {
                            return Err(Error::internal("User denied authorization".to_string()));
                        },
                        "expired_token" => {
                            return Err(Error::internal("Device code expired".to_string()));
                        },
                        _ => {
                            return Err(Error::internal(format!("OAuth error: {}", error_code)));
                        },
                    }
                }
            }
        }
    }

    /// Refresh an existing token.
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        let metadata = self.get_metadata().await?;
        let token_endpoint = &metadata.token_endpoint;

        let response = self
            .client
            .post(token_endpoint)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to refresh token: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Token refresh failed: {}",
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse token response: {e}")))
    }

    /// Load cached token from disk.
    async fn load_cached_token(&self, cache_file: &PathBuf) -> Result<TokenCache> {
        let content = tokio::fs::read_to_string(cache_file)
            .await
            .map_err(|e| Error::internal(format!("Failed to read token cache: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| Error::internal(format!("Failed to parse token cache: {e}")))
    }

    /// Cache token to disk.
    async fn cache_token(&self, token: &TokenResponse, cache_file: &PathBuf) -> Result<()> {
        let expires_at = token.expires_in.map(|secs| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + secs
        });

        let cache = TokenCache {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_at,
            scopes: self.config.scopes.clone(),
        };

        // Ensure directory exists
        if let Some(parent) = cache_file.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::internal(format!("Failed to create cache directory: {e}")))?;
        }

        let json = serde_json::to_string_pretty(&cache)
            .map_err(|e| Error::internal(format!("Failed to serialize cache: {e}")))?;
        tokio::fs::write(cache_file, json)
            .await
            .map_err(|e| Error::internal(format!("Failed to write token cache: {e}")))?;

        tracing::debug!("Token cached to: {}", cache_file.display());

        Ok(())
    }

    /// Cache token from the SDK's auth `TokenResponse` type.
    async fn cache_token_from_response(
        &self,
        token: &crate::client::auth::TokenResponse,
        cache_file: &PathBuf,
    ) -> Result<()> {
        // Convert to internal TokenResponse
        let internal_token = TokenResponse {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_in: token.expires_in,
            token_type: token.token_type.clone(),
        };
        self.cache_token(&internal_token, cache_file).await
    }

    /// Create HTTP middleware chain with OAuth bearer token.
    ///
    /// Obtains an access token (from cache, refresh, or interactive flow)
    /// and wraps it in a middleware chain suitable for HTTP transports.
    pub async fn create_middleware_chain(&self) -> Result<Arc<HttpMiddlewareChain>> {
        let access_token = self.get_access_token().await?;

        tracing::debug!(
            "Creating OAuth middleware with token: {}...",
            &access_token[..access_token.len().min(20)]
        );

        let bearer_token = BearerToken::new(access_token);
        let oauth_middleware = OAuthClientMiddleware::new(bearer_token);

        let mut chain = HttpMiddlewareChain::new();
        chain.add(Arc::new(oauth_middleware));

        tracing::info!("OAuth middleware added to chain");

        Ok(Arc::new(chain))
    }
}

/// Get default cache file path (`~/.pmcp/oauth-tokens.json`).
///
/// Uses the user's home directory to store cached OAuth tokens.
/// Falls back to the current directory if the home directory cannot be determined.
pub fn default_cache_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".pmcp");
    path.push("oauth-tokens.json");
    path
}

/// Create an OAuth middleware chain from configuration.
///
/// This is a one-liner convenience for tools that just need a middleware chain:
/// ```no_run
/// # use pmcp::client::oauth::{OAuthConfig, create_oauth_middleware};
/// # async fn example() -> pmcp::Result<()> {
/// let config = OAuthConfig {
///     issuer: Some("https://auth.example.com".to_string()),
///     mcp_server_url: None,
///     client_id: "my-client".to_string(),
///     scopes: vec!["openid".to_string()],
///     cache_file: None,
///     redirect_port: 8080,
/// };
/// let chain = create_oauth_middleware(config).await?;
/// // Pass chain to HttpClient or transport
/// # Ok(())
/// # }
/// ```
pub async fn create_oauth_middleware(config: OAuthConfig) -> Result<Arc<HttpMiddlewareChain>> {
    let helper = OAuthHelper::new(config)?;
    helper.create_middleware_chain().await
}
