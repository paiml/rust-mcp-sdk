//! OAuth 2.0 server implementation for MCP.

use crate::error::{Error, ErrorCode, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::RwLock;
use uuid::Uuid;

/// OAuth 2.0 grant types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrantType {
    /// Authorization code grant type.
    #[serde(rename = "authorization_code")]
    AuthorizationCode,
    /// Refresh token grant type.
    #[serde(rename = "refresh_token")]
    RefreshToken,
    /// Client credentials grant type.
    #[serde(rename = "client_credentials")]
    ClientCredentials,
    /// Resource owner password credentials grant type.
    #[serde(rename = "password")]
    Password,
}

/// OAuth 2.0 response types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseType {
    /// Authorization code response type.
    #[serde(rename = "code")]
    Code,
    /// Implicit token response type.
    #[serde(rename = "token")]
    Token,
}

/// OAuth 2.0 token types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    /// Bearer token type.
    Bearer,
}

/// OAuth 2.0 client registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClient {
    /// Client identifier.
    pub client_id: String,

    /// Client secret (confidential clients only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Client name.
    pub client_name: String,

    /// Redirect URIs.
    pub redirect_uris: Vec<String>,

    /// Allowed grant types.
    pub grant_types: Vec<GrantType>,

    /// Allowed response types.
    pub response_types: Vec<ResponseType>,

    /// Allowed scopes.
    pub scopes: Vec<String>,

    /// Client metadata.
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// OAuth 2.0 authorization code.
#[derive(Debug, Clone)]
pub struct AuthorizationCode {
    /// Authorization code value.
    pub code: String,

    /// Client ID this code was issued to.
    pub client_id: String,

    /// User ID this code was issued for.
    pub user_id: String,

    /// Redirect URI used in authorization request.
    pub redirect_uri: String,

    /// Requested scopes.
    pub scopes: Vec<String>,

    /// PKCE code challenge if used.
    pub code_challenge: Option<String>,

    /// PKCE challenge method.
    pub code_challenge_method: Option<String>,

    /// Expiration timestamp.
    pub expires_at: u64,
}

/// OAuth 2.0 access token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    /// Token value.
    pub access_token: String,

    /// Token type (always "bearer").
    pub token_type: TokenType,

    /// Expiration time in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,

    /// Refresh token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Granted scopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// Additional token metadata.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// OAuth 2.0 token info for introspection.
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// Token value.
    pub token: String,

    /// Client ID.
    pub client_id: String,

    /// User ID.
    pub user_id: String,

    /// Granted scopes.
    pub scopes: Vec<String>,

    /// Expiration timestamp.
    pub expires_at: u64,

    /// Token type.
    pub token_type: TokenType,
}

/// OAuth 2.0 error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthError {
    /// Error code.
    pub error: String,

    /// Error description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,

    /// Error URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_uri: Option<String>,
}

/// RFC 8414 §2 default for an absent `grant_types_supported`.
///
/// The spec says `["authorization_code", "implicit"]`. `GrantType` has no
/// `Implicit` variant, so this returns the representable part rather than an
/// empty list.
fn default_grant_types_supported() -> Vec<GrantType> {
    vec![GrantType::AuthorizationCode]
}

/// RFC 8414 §2 default for an absent `token_endpoint_auth_methods_supported`.
fn default_token_endpoint_auth_methods() -> Vec<String> {
    vec!["client_secret_basic".to_string()]
}

/// `OpenID Connect Discovery` metadata.
/// Represents the well-known configuration for OAuth 2.0/OIDC servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscoveryMetadata {
    /// Issuer identifier.
    pub issuer: String,

    /// Authorization endpoint URL.
    pub authorization_endpoint: String,

    /// Token endpoint URL.
    pub token_endpoint: String,

    /// JWKS (JSON Web Key Set) URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwks_uri: Option<String>,

    /// User info endpoint URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userinfo_endpoint: Option<String>,

    /// Registration endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_endpoint: Option<String>,

    /// Revocation endpoint URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revocation_endpoint: Option<String>,

    /// Introspection endpoint URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introspection_endpoint: Option<String>,

    /// Device authorization endpoint URL (RFC 8628).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_authorization_endpoint: Option<String>,

    /// Supported response types.
    pub response_types_supported: Vec<ResponseType>,

    /// Supported grant types.
    ///
    /// OPTIONAL per RFC 8414 §2. When absent the spec's default is
    /// `["authorization_code", "implicit"]`; `GrantType` models no `Implicit`
    /// variant (the implicit flow is deprecated by OAuth 2.1 and unimplemented
    /// here), so the representable part of that default is used. Never empty by
    /// default — an empty list would claim the server supports no grants.
    #[serde(default = "default_grant_types_supported")]
    pub grant_types_supported: Vec<GrantType>,

    /// Supported scopes.
    /// RECOMMENDED per RFC 8414 §2, with no specified default.
    #[serde(default)]
    pub scopes_supported: Vec<String>,

    /// Supported token endpoint auth methods.
    /// OPTIONAL per RFC 8414 §2; the spec's default when absent is
    /// `["client_secret_basic"]`.
    #[serde(default = "default_token_endpoint_auth_methods")]
    pub token_endpoint_auth_methods_supported: Vec<String>,

    /// Supported PKCE code challenge methods.
    ///
    /// OPTIONAL per RFC 8414 §2 with no specified default, so absence means
    /// "not advertised" rather than "not supported" — Amazon Cognito supports
    /// S256 while omitting this field. Safe to default to empty because the
    /// client never gates on it: PKCE is unconditional in `client::oauth`.
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,
}

/// OAuth 2.0 server metadata (alias for backward compatibility).
pub type OAuthMetadata = OidcDiscoveryMetadata;

/// OAuth 2.0 authorization request.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthorizationRequest {
    /// Response type.
    pub response_type: ResponseType,

    /// Client ID.
    pub client_id: String,

    /// Redirect URI.
    pub redirect_uri: String,

    /// Requested scope.
    #[serde(default)]
    pub scope: String,

    /// State parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// PKCE code challenge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_challenge: Option<String>,

    /// PKCE challenge method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_challenge_method: Option<String>,
}

/// OAuth 2.0 token request.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenRequest {
    /// Grant type.
    pub grant_type: GrantType,

    /// Authorization code (for `authorization_code` grant).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Redirect URI (for `authorization_code` grant).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,

    /// Client ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// Client secret.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Refresh token (for `refresh_token` grant).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Username (for password grant).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Password (for password grant).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Requested scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// PKCE code verifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_verifier: Option<String>,
}

/// OAuth 2.0 revocation request.
#[derive(Debug, Clone, Deserialize)]
pub struct RevocationRequest {
    /// Token to revoke.
    pub token: String,

    /// Token type hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type_hint: Option<String>,

    /// Client ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// Client secret.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

/// OAuth 2.0 server provider trait.
#[async_trait]
pub trait OAuthProvider: Send + Sync {
    /// Register a new client.
    async fn register_client(&self, client: OAuthClient) -> Result<OAuthClient>;

    /// Get client by ID.
    async fn get_client(&self, client_id: &str) -> Result<Option<OAuthClient>>;

    /// Validate authorization request.
    async fn validate_authorization(&self, request: &AuthorizationRequest) -> Result<()>;

    /// Create authorization code.
    async fn create_authorization_code(
        &self,
        client_id: &str,
        user_id: &str,
        redirect_uri: &str,
        scopes: Vec<String>,
        code_challenge: Option<String>,
        code_challenge_method: Option<String>,
    ) -> Result<String>;

    /// Exchange authorization code for token.
    async fn exchange_code(&self, request: &TokenRequest) -> Result<AccessToken>;

    /// Create access token.
    async fn create_access_token(
        &self,
        client_id: &str,
        user_id: &str,
        scopes: Vec<String>,
    ) -> Result<AccessToken>;

    /// Refresh access token.
    async fn refresh_token(&self, refresh_token: &str) -> Result<AccessToken>;

    /// Revoke token.
    async fn revoke_token(&self, token: &str) -> Result<()>;

    /// Validate access token.
    async fn validate_token(&self, token: &str) -> Result<TokenInfo>;

    /// Get server metadata.
    async fn metadata(&self) -> Result<OAuthMetadata>;

    /// Discover OIDC configuration from well-known endpoint.
    /// Returns the discovery metadata if successful.
    /// Implementations should handle retries for network failures.
    async fn discover(&self, _issuer_url: &str) -> Result<OidcDiscoveryMetadata> {
        // Default implementation that would fetch from .well-known/openid-configuration
        // For now, return an error indicating it needs implementation
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "OIDC discovery not implemented for this provider",
        ))
    }
}

/// In-memory OAuth 2.0 provider implementation.
#[derive(Debug)]
pub struct InMemoryOAuthProvider {
    /// Base URL for endpoints.
    base_url: String,

    /// Registered clients.
    clients: Arc<RwLock<HashMap<String, OAuthClient>>>,

    /// Active authorization codes.
    codes: Arc<RwLock<HashMap<String, AuthorizationCode>>>,

    /// Active access tokens.
    tokens: Arc<RwLock<HashMap<String, TokenInfo>>>,

    /// Refresh tokens.
    refresh_tokens: Arc<RwLock<HashMap<String, String>>>,

    /// Token expiration time in seconds.
    token_expiration: u64,

    /// Code expiration time in seconds.
    code_expiration: u64,

    /// Supported scopes.
    supported_scopes: Vec<String>,
}

impl InMemoryOAuthProvider {
    /// Create a new in-memory OAuth provider.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            clients: Arc::new(RwLock::new(HashMap::new())),
            codes: Arc::new(RwLock::new(HashMap::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
            refresh_tokens: Arc::new(RwLock::new(HashMap::new())),
            token_expiration: 3600, // 1 hour
            code_expiration: 600,   // 10 minutes
            supported_scopes: vec!["read".to_string(), "write".to_string()],
        }
    }

    /// Generate a secure random token.
    fn generate_token() -> String {
        Uuid::new_v4().to_string()
    }

    /// Get current timestamp.
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Verify PKCE code challenge.
    fn verify_pkce(verifier: &str, challenge: &str, method: &str) -> bool {
        match method {
            "plain" => verifier == challenge,
            "S256" => {
                use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
                use sha2::{Digest, Sha256};

                let mut hasher = Sha256::new();
                hasher.update(verifier.as_bytes());
                let result = hasher.finalize();
                let encoded = URL_SAFE_NO_PAD.encode(result);
                encoded == challenge
            },
            _ => false,
        }
    }
}

#[async_trait]
impl OAuthProvider for InMemoryOAuthProvider {
    async fn register_client(&self, mut client: OAuthClient) -> Result<OAuthClient> {
        // Generate client credentials if not provided
        if client.client_id.is_empty() {
            client.client_id = Self::generate_token();
        }
        if client.client_secret.is_none() {
            client.client_secret = Some(Self::generate_token());
        }

        // Store client
        let mut clients = self.clients.write().await;
        clients.insert(client.client_id.clone(), client.clone());

        Ok(client)
    }

    async fn get_client(&self, client_id: &str) -> Result<Option<OAuthClient>> {
        let clients = self.clients.read().await;
        Ok(clients.get(client_id).cloned())
    }

    async fn validate_authorization(&self, request: &AuthorizationRequest) -> Result<()> {
        // Get client
        let client = self
            .get_client(&request.client_id)
            .await?
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Invalid client_id"))?;

        // Validate redirect URI
        if !client.redirect_uris.contains(&request.redirect_uri) {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Invalid redirect_uri",
            ));
        }

        // Validate response type
        if !client.response_types.contains(&request.response_type) {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Unsupported response_type",
            ));
        }

        // Validate scopes
        let requested_scopes: Vec<&str> = request.scope.split_whitespace().collect();
        for scope in &requested_scopes {
            if !self.supported_scopes.iter().any(|s| s == scope) {
                return Err(Error::protocol(ErrorCode::INVALID_REQUEST, "Invalid scope"));
            }
        }

        Ok(())
    }

    async fn create_authorization_code(
        &self,
        client_id: &str,
        user_id: &str,
        redirect_uri: &str,
        scopes: Vec<String>,
        code_challenge: Option<String>,
        code_challenge_method: Option<String>,
    ) -> Result<String> {
        let code = Self::generate_token();
        let expires_at = Self::now() + self.code_expiration;

        let auth_code = AuthorizationCode {
            code: code.clone(),
            client_id: client_id.to_string(),
            user_id: user_id.to_string(),
            redirect_uri: redirect_uri.to_string(),
            scopes,
            code_challenge,
            code_challenge_method,
            expires_at,
        };

        let mut codes = self.codes.write().await;
        codes.insert(code.clone(), auth_code);

        Ok(code)
    }

    async fn exchange_code(&self, request: &TokenRequest) -> Result<AccessToken> {
        // Validate grant type
        if request.grant_type != GrantType::AuthorizationCode {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Invalid grant_type",
            ));
        }

        // Get code
        let code = request
            .code
            .as_ref()
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Missing code"))?;

        let mut codes = self.codes.write().await;
        let auth_code = codes
            .remove(code)
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Invalid code"))?;

        // Check expiration
        if auth_code.expires_at < Self::now() {
            return Err(Error::protocol(ErrorCode::INVALID_REQUEST, "Code expired"));
        }

        // Validate client
        let client_id = request
            .client_id
            .as_ref()
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Missing client_id"))?;

        if auth_code.client_id != *client_id {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Invalid client_id",
            ));
        }

        // Validate redirect URI
        let redirect_uri = request
            .redirect_uri
            .as_ref()
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Missing redirect_uri"))?;

        if auth_code.redirect_uri != *redirect_uri {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Invalid redirect_uri",
            ));
        }

        // Verify PKCE if used
        if let Some(challenge) = &auth_code.code_challenge {
            let verifier = request.code_verifier.as_ref().ok_or_else(|| {
                Error::protocol(ErrorCode::INVALID_REQUEST, "Missing code_verifier")
            })?;

            let method = auth_code
                .code_challenge_method
                .as_deref()
                .unwrap_or("plain");
            if !Self::verify_pkce(verifier, challenge, method) {
                return Err(Error::protocol(
                    ErrorCode::INVALID_REQUEST,
                    "Invalid code_verifier",
                ));
            }
        }

        // Create access token
        self.create_access_token(&auth_code.client_id, &auth_code.user_id, auth_code.scopes)
            .await
    }

    async fn create_access_token(
        &self,
        client_id: &str,
        user_id: &str,
        scopes: Vec<String>,
    ) -> Result<AccessToken> {
        let access_token = Self::generate_token();
        let refresh_token = Self::generate_token();
        let expires_at = Self::now() + self.token_expiration;

        // Store token info
        let token_info = TokenInfo {
            token: access_token.clone(),
            client_id: client_id.to_string(),
            user_id: user_id.to_string(),
            scopes: scopes.clone(),
            expires_at,
            token_type: TokenType::Bearer,
        };

        let mut tokens = self.tokens.write().await;
        tokens.insert(access_token.clone(), token_info);

        // Store refresh token mapping
        let mut refresh_tokens = self.refresh_tokens.write().await;
        refresh_tokens.insert(refresh_token.clone(), access_token.clone());

        Ok(AccessToken {
            access_token,
            token_type: TokenType::Bearer,
            expires_in: Some(self.token_expiration),
            refresh_token: Some(refresh_token),
            scope: Some(scopes.join(" ")),
            extra: HashMap::new(),
        })
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<AccessToken> {
        // Get associated access token
        let refresh_tokens = self.refresh_tokens.read().await;
        let old_token = refresh_tokens
            .get(refresh_token)
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Invalid refresh_token"))?
            .clone();

        // Get token info
        let tokens = self.tokens.read().await;
        let token_info = tokens
            .get(&old_token)
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Invalid refresh_token"))?;

        let client_id = token_info.client_id.clone();
        let user_id = token_info.user_id.clone();
        let scopes = token_info.scopes.clone();

        drop(tokens);
        drop(refresh_tokens);

        // Remove old tokens
        let mut tokens = self.tokens.write().await;
        tokens.remove(&old_token);
        drop(tokens);

        let mut refresh_tokens = self.refresh_tokens.write().await;
        refresh_tokens.remove(refresh_token);
        drop(refresh_tokens);

        // Create new token
        self.create_access_token(&client_id, &user_id, scopes).await
    }

    async fn revoke_token(&self, token: &str) -> Result<()> {
        // Try to revoke as access token
        let mut tokens = self.tokens.write().await;
        if tokens.remove(token).is_some() {
            return Ok(());
        }
        drop(tokens);

        // Try to revoke as refresh token
        let mut refresh_tokens = self.refresh_tokens.write().await;
        if let Some(access_token) = refresh_tokens.remove(token) {
            let mut tokens = self.tokens.write().await;
            tokens.remove(&access_token);
        }

        Ok(())
    }

    async fn validate_token(&self, token: &str) -> Result<TokenInfo> {
        let tokens = self.tokens.read().await;
        let token_info = tokens
            .get(token)
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Invalid token"))?;

        // Check expiration
        if token_info.expires_at < Self::now() {
            return Err(Error::protocol(ErrorCode::INVALID_REQUEST, "Token expired"));
        }

        Ok(token_info.clone())
    }

    async fn metadata(&self) -> Result<OAuthMetadata> {
        Ok(OAuthMetadata {
            issuer: self.base_url.clone(),
            authorization_endpoint: format!("{}/oauth2/authorize", self.base_url),
            token_endpoint: format!("{}/oauth2/token", self.base_url),
            jwks_uri: Some(format!("{}/oauth2/jwks", self.base_url)),
            userinfo_endpoint: Some(format!("{}/oauth2/userinfo", self.base_url)),
            registration_endpoint: Some(format!("{}/oauth2/register", self.base_url)),
            revocation_endpoint: Some(format!("{}/oauth2/revoke", self.base_url)),
            introspection_endpoint: Some(format!("{}/oauth2/introspect", self.base_url)),
            device_authorization_endpoint: Some(format!(
                "{}/oauth2/device/authorize",
                self.base_url
            )),
            response_types_supported: vec![ResponseType::Code],
            grant_types_supported: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
            scopes_supported: self.supported_scopes.clone(),
            token_endpoint_auth_methods_supported: vec![
                "client_secret_basic".to_string(),
                "client_secret_post".to_string(),
            ],
            code_challenge_methods_supported: vec!["plain".to_string(), "S256".to_string()],
        })
    }
}

/// Proxy OAuth provider that delegates to an upstream OAuth server.
#[derive(Debug)]
pub struct ProxyOAuthProvider {
    /// Upstream OAuth server URL.
    _upstream_url: String,

    /// Local token cache.
    _token_cache: Arc<RwLock<HashMap<String, TokenInfo>>>,
}

impl ProxyOAuthProvider {
    /// Create a new proxy OAuth provider.
    pub fn new(upstream_url: impl Into<String>) -> Self {
        Self {
            _upstream_url: upstream_url.into(),
            _token_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// Note: ProxyOAuthProvider implementation would require HTTP client functionality
// which is beyond the scope of this initial implementation

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oauth_flow() {
        let provider = InMemoryOAuthProvider::new("http://localhost:8080");

        // Register client
        let client = OAuthClient {
            client_id: String::new(),
            client_secret: None,
            client_name: "Test Client".to_string(),
            redirect_uris: vec!["http://localhost:3000/callback".to_string()],
            grant_types: vec![GrantType::AuthorizationCode],
            response_types: vec![ResponseType::Code],
            scopes: vec!["read".to_string(), "write".to_string()],
            metadata: HashMap::new(),
        };

        let registered = provider.register_client(client).await.unwrap();
        assert!(!registered.client_id.is_empty());
        assert!(registered.client_secret.is_some());

        // Validate authorization request
        let auth_req = AuthorizationRequest {
            response_type: ResponseType::Code,
            client_id: registered.client_id.clone(),
            redirect_uri: "http://localhost:3000/callback".to_string(),
            scope: "read write".to_string(),
            state: Some("test-state".to_string()),
            code_challenge: None,
            code_challenge_method: None,
        };

        provider.validate_authorization(&auth_req).await.unwrap();

        // Create authorization code
        let code = provider
            .create_authorization_code(
                &registered.client_id,
                "user-123",
                &auth_req.redirect_uri,
                vec!["read".to_string(), "write".to_string()],
                None,
                None,
            )
            .await
            .unwrap();

        // Exchange code for token
        let token_req = TokenRequest {
            grant_type: GrantType::AuthorizationCode,
            code: Some(code),
            redirect_uri: Some(auth_req.redirect_uri),
            client_id: Some(registered.client_id.clone()),
            client_secret: registered.client_secret.clone(),
            refresh_token: None,
            username: None,
            password: None,
            scope: None,
            code_verifier: None,
        };

        let token = provider.exchange_code(&token_req).await.unwrap();
        assert_eq!(token.token_type, TokenType::Bearer);
        assert!(token.refresh_token.is_some());

        // Validate token
        let token_info = provider.validate_token(&token.access_token).await.unwrap();
        assert_eq!(token_info.client_id, registered.client_id);
        assert_eq!(token_info.user_id, "user-123");
    }
}
