//! Lightweight multi-tenant JWT validator.
//!
//! This module provides a `JwtValidator` that can validate tokens from multiple
//! OIDC providers efficiently with a shared JWKS cache. It's designed for:
//!
//! - **Lambda Authorizers**: Minimal overhead, config per-request
//! - **Multi-tenant applications**: One validator instance for all tenants
//! - **API Gateways**: Validate tokens from multiple issuers
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    JwtValidator                             │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │           Multi-tenant JWKS Cache                    │   │
//! │  │  ┌─────────────────┐  ┌─────────────────┐           │   │
//! │  │  │ cognito/pool1   │  │ google.com      │  ...      │   │
//! │  │  │ key1, key2      │  │ key1, key2      │           │   │
//! │  │  └─────────────────┘  └─────────────────┘           │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use pmcp::server::auth::{JwtValidator, ValidationConfig};
//!
//! // Create one validator (typically at application start)
//! let validator = JwtValidator::new();
//!
//! // Validate Cognito token
//! let cognito_config = ValidationConfig::cognito("us-east-1", "pool-id", "client-id");
//! let auth = validator.validate(&token, &cognito_config).await?;
//!
//! // Same validator can validate Google tokens (different issuer)
//! let google_config = ValidationConfig::google("google-client-id");
//! let auth = validator.validate(&google_token, &google_config).await?;
//! ```
//!
//! # Feature Flag
//!
//! This module requires the `jwt-auth` feature:
//!
//! ```toml
//! [dependencies]
//! pmcp = { version = "0.3", features = ["jwt-auth"] }
//! ```

use super::traits::{AuthContext, ClaimMappings};
use crate::error::{Error, ErrorCode, Result};
// Gated to match its ONLY use site, not the module. `mod jwt`/`mod jwt_validator`
// compile under `http-client`, but the reqwest body-cap read lives inside a
// `jwt-auth` + non-wasm arm, so an `http-client`-without-`jwt-auth` build (and
// every wasm32 build) left this import unused and `-D warnings` made it fatal.
#[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
use crate::shared::http_body_cap::{collect_reqwest_body_within_cap, DEFAULT_AUTH_RESPONSE_BYTES};
#[cfg(feature = "jwt-auth")]
use std::collections::HashMap;
#[cfg(feature = "jwt-auth")]
use std::sync::Arc;
use std::time::Duration;
#[cfg(feature = "jwt-auth")]
use std::time::Instant;
#[cfg(feature = "jwt-auth")]
use tokio::sync::RwLock;

/// Cached JWKS keys for a single issuer.
#[cfg(feature = "jwt-auth")]
struct CachedJwks {
    keys: HashMap<String, jsonwebtoken::DecodingKey>,
    fetched_at: Instant,
    ttl: Duration,
}

#[cfg(feature = "jwt-auth")]
impl std::fmt::Debug for CachedJwks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedJwks")
            .field("keys_count", &self.keys.len())
            .field("fetched_at", &self.fetched_at)
            .field("ttl", &self.ttl)
            .finish()
    }
}

#[cfg(feature = "jwt-auth")]
impl CachedJwks {
    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}

/// Lightweight multi-tenant JWT validator.
///
/// This validator maintains a shared JWKS cache for multiple issuers,
/// making it efficient for Lambda authorizers and multi-tenant applications.
///
/// # Key Features
///
/// - **Multi-tenant JWKS cache**: Caches keys from multiple issuers in one instance
/// - **Config per request**: Pass `ValidationConfig` with each validation call
/// - **Automatic key rotation**: Refreshes JWKS when unknown `kid` encountered
/// - **Minimal overhead**: Only JWT validation, no OIDC flows
///
/// # Thread Safety
///
/// `JwtValidator` is `Send + Sync` and can be shared across threads.
/// The JWKS cache uses interior mutability with `RwLock`.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::auth::{JwtValidator, ValidationConfig};
///
/// // Create validator (typically once at startup)
/// let validator = JwtValidator::new();
///
/// // Validate tokens from different providers
/// let auth1 = validator.validate(&cognito_token, &ValidationConfig::cognito(...)).await?;
/// let auth2 = validator.validate(&google_token, &ValidationConfig::google(...)).await?;
/// ```
#[derive(Debug)]
pub struct JwtValidator {
    /// Multi-tenant JWKS cache: JWKS URI -> cached keys
    #[cfg(feature = "jwt-auth")]
    jwks_cache: Arc<RwLock<HashMap<String, CachedJwks>>>,
    /// HTTP client for fetching JWKS
    #[cfg(not(target_arch = "wasm32"))]
    http_client: reqwest::Client,
    /// Default cache TTL for JWKS
    cache_ttl: Duration,
}

impl Default for JwtValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for JwtValidator {
    fn clone(&self) -> Self {
        Self {
            #[cfg(feature = "jwt-auth")]
            jwks_cache: Arc::clone(&self.jwks_cache),
            #[cfg(not(target_arch = "wasm32"))]
            http_client: self.http_client.clone(),
            cache_ttl: self.cache_ttl,
        }
    }
}

impl JwtValidator {
    /// Create a new JWT validator with default settings.
    ///
    /// Default cache TTL is 1 hour.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Self {
        Self::with_cache_ttl(Duration::from_hours(1))
    }

    /// Create a new JWT validator with custom cache TTL.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_cache_ttl(cache_ttl: Duration) -> Self {
        Self {
            #[cfg(feature = "jwt-auth")]
            jwks_cache: Arc::new(RwLock::new(HashMap::new())),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
            cache_ttl,
        }
    }

    /// Create a validator with a custom HTTP client.
    ///
    /// Useful for testing or when you need custom HTTP settings.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_http_client(http_client: reqwest::Client, cache_ttl: Duration) -> Self {
        Self {
            #[cfg(feature = "jwt-auth")]
            jwks_cache: Arc::new(RwLock::new(HashMap::new())),
            http_client,
            cache_ttl,
        }
    }

    /// Validate a JWT token with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token string (without "Bearer " prefix)
    /// * `config` - Validation configuration specifying issuer, audience, etc.
    ///
    /// # Returns
    ///
    /// Returns `AuthContext` on successful validation, containing:
    /// - User ID (subject claim)
    /// - Scopes
    /// - Email, name (if present)
    /// - Groups (if present)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Token format is invalid
    /// - Signature verification fails
    /// - Token is expired
    /// - Issuer or audience doesn't match
    /// - JWKS cannot be fetched
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = ValidationConfig::cognito("us-east-1", "pool-id", "client-id");
    /// let auth = validator.validate(&token, &config).await?;
    /// println!("User: {}", auth.user_id());
    /// ```
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    pub async fn validate(&self, token: &str, config: &ValidationConfig) -> Result<AuthContext> {
        use jsonwebtoken::{decode, decode_header, Algorithm, Validation};

        // 1. Decode header to get key ID
        let header = decode_header(token).map_err(|e| {
            Error::protocol(
                ErrorCode::AUTHENTICATION_REQUIRED,
                format!("Invalid token header: {}", e),
            )
        })?;

        let kid = header.kid.ok_or_else(|| {
            Error::protocol(
                ErrorCode::AUTHENTICATION_REQUIRED,
                "Token missing key ID (kid)",
            )
        })?;

        // 2. Get the signing key (from cache or fetch)
        let key = self.get_key(&config.jwks_uri, &kid).await?;

        // 3. Build validation rules
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&config.issuer]);
        validation.set_audience(&[&config.audience]);
        validation.leeway = config.leeway_seconds;

        // 4. Decode and verify token
        let token_data = decode::<serde_json::Value>(token, &key, &validation).map_err(|e| {
            let msg = match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => "Token expired",
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => "Invalid issuer",
                jsonwebtoken::errors::ErrorKind::InvalidAudience => "Invalid audience",
                jsonwebtoken::errors::ErrorKind::InvalidSignature => "Invalid signature",
                jsonwebtoken::errors::ErrorKind::ImmatureSignature => "Token not yet valid",
                _ => "Token validation failed",
            };
            Error::protocol(ErrorCode::AUTHENTICATION_REQUIRED, msg)
        })?;

        // 5. Validate token_use if required (Cognito-specific)
        if let Some(ref required_use) = config.required_token_use {
            if let Some(token_use) = token_data.claims.get("token_use").and_then(|v| v.as_str()) {
                if token_use != required_use {
                    return Err(Error::protocol(
                        ErrorCode::AUTHENTICATION_REQUIRED,
                        format!(
                            "Invalid token_use: expected {}, got {}",
                            required_use, token_use
                        ),
                    ));
                }
            }
        }

        // 6. Normalize claims using mappings
        let normalized_claims = config.claim_mappings.normalize_claims(&token_data.claims);

        // 7. Extract subject (required)
        let subject = normalized_claims
            .get("sub")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if subject.is_empty() {
            return Err(Error::protocol(
                ErrorCode::AUTHENTICATION_REQUIRED,
                "Token missing subject claim",
            ));
        }

        // 8. Extract scopes
        let scopes = parse_scopes(&token_data.claims);

        // 9. Extract optional fields
        let client_id = token_data
            .claims
            .get("azp")
            .or_else(|| token_data.claims.get("client_id"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let expires_at = token_data.claims.get("exp").and_then(|v| v.as_u64());

        // 10. Build AuthContext
        Ok(AuthContext {
            subject,
            scopes,
            claims: normalized_claims,
            token: Some(token.to_string()),
            client_id,
            expires_at,
            authenticated: true,
        })
    }

    /// Validate a JWT token (stub for non-jwt-auth builds).
    #[cfg(any(target_arch = "wasm32", not(feature = "jwt-auth")))]
    pub async fn validate(&self, _token: &str, _config: &ValidationConfig) -> Result<AuthContext> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "JWT validation requires the 'jwt-auth' feature and non-WASM target",
        ))
    }

    /// Get a key from cache, fetching JWKS if needed.
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    async fn get_key(&self, jwks_uri: &str, kid: &str) -> Result<jsonwebtoken::DecodingKey> {
        // Try cache first
        {
            let cache = self.jwks_cache.read().await;
            if let Some(cached) = cache.get(jwks_uri) {
                if !cached.is_expired() {
                    if let Some(key) = cached.keys.get(kid) {
                        return Ok(key.clone());
                    }
                }
            }
        }

        // Cache miss or expired - fetch JWKS
        self.refresh_jwks(jwks_uri).await?;

        // Try again after refresh
        {
            let cache = self.jwks_cache.read().await;
            if let Some(cached) = cache.get(jwks_uri) {
                if let Some(key) = cached.keys.get(kid) {
                    return Ok(key.clone());
                }
            }
        }

        // Key still not found after refresh (might be key rotation)
        Err(Error::protocol(
            ErrorCode::AUTHENTICATION_REQUIRED,
            format!("Unknown key ID: {}", kid),
        ))
    }

    /// Fetch and cache JWKS from the given URI.
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    async fn refresh_jwks(&self, jwks_uri: &str) -> Result<()> {
        tracing::debug!(jwks_uri = %jwks_uri, "Fetching JWKS");

        let response = self
            .http_client
            .get(jwks_uri)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to fetch JWKS: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "JWKS endpoint returned status {}",
                response.status()
            )));
        }

        // The JWKS body is chosen by the identity provider, so it is peer-supplied
        // bytes: read it under the same cap as every other auth-surface read
        // (D-15/AUTH-03) rather than letting `.json()` buffer without limit.
        let body = collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await?;
        let jwks: JwksResponse = serde_json::from_slice(&body)
            .map_err(|e| Error::internal(format!("Failed to parse JWKS: {}", e)))?;

        // Parse keys
        let mut keys = HashMap::new();
        for key in jwks.keys {
            if let (Some(kid), Some(n), Some(e)) = (&key.kid, &key.n, &key.e) {
                match jsonwebtoken::DecodingKey::from_rsa_components(n, e) {
                    Ok(decoding_key) => {
                        keys.insert(kid.clone(), decoding_key);
                    },
                    Err(err) => {
                        tracing::warn!(kid = %kid, error = %err, "Failed to parse JWK");
                    },
                }
            }
        }

        if keys.is_empty() {
            return Err(Error::internal("No valid keys found in JWKS"));
        }

        tracing::info!(jwks_uri = %jwks_uri, keys_count = keys.len(), "Cached JWKS keys");

        // Update cache
        let mut cache = self.jwks_cache.write().await;
        let cached = CachedJwks {
            keys,
            fetched_at: Instant::now(),
            ttl: self.cache_ttl,
        };
        cache.insert(jwks_uri.to_string(), cached);

        Ok(())
    }

    /// Clear all cached JWKS.
    #[cfg(feature = "jwt-auth")]
    pub async fn clear_cache(&self) {
        let mut cache = self.jwks_cache.write().await;
        cache.clear();
    }

    /// Clear cached JWKS for a specific issuer.
    #[cfg(feature = "jwt-auth")]
    pub async fn clear_issuer_cache(&self, jwks_uri: &str) {
        let mut cache = self.jwks_cache.write().await;
        cache.remove(jwks_uri);
    }

    /// Get the number of cached issuers.
    #[cfg(feature = "jwt-auth")]
    pub async fn cache_size(&self) -> usize {
        let cache = self.jwks_cache.read().await;
        cache.len()
    }
}

/// Configuration for JWT validation.
///
/// Pass this to `JwtValidator::validate()` to specify validation parameters.
///
/// # Example
///
/// ```rust
/// use pmcp::server::auth::ValidationConfig;
///
/// // Use provider-specific constructor
/// let config = ValidationConfig::cognito("us-east-1", "us-east-1_xxx", "client-id");
///
/// // Or build manually
/// let config = ValidationConfig::new(
///     "https://accounts.google.com",
///     "https://www.googleapis.com/oauth2/v3/certs",
///     "your-client-id",
/// );
/// ```
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Expected issuer (validated against `iss` claim).
    pub issuer: String,

    /// JWKS URI for fetching signing keys.
    pub jwks_uri: String,

    /// Expected audience (validated against `aud` claim).
    pub audience: String,

    /// Clock skew tolerance in seconds (default: 60).
    pub leeway_seconds: u64,

    /// Required `token_use` claim value (e.g., "access" for Cognito).
    pub required_token_use: Option<String>,

    /// Claim mappings for normalizing provider-specific claims.
    pub claim_mappings: ClaimMappings,
}

impl ValidationConfig {
    /// Create a new validation config.
    pub fn new(
        issuer: impl Into<String>,
        jwks_uri: impl Into<String>,
        audience: impl Into<String>,
    ) -> Self {
        Self {
            issuer: issuer.into(),
            jwks_uri: jwks_uri.into(),
            audience: audience.into(),
            leeway_seconds: 60,
            required_token_use: None,
            claim_mappings: ClaimMappings::default(),
        }
    }

    /// Create config for AWS Cognito.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::ValidationConfig;
    ///
    /// let config = ValidationConfig::cognito("us-east-1", "us-east-1_xxxxx", "client-id");
    /// ```
    pub fn cognito(region: &str, user_pool_id: &str, client_id: &str) -> Self {
        let issuer = format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            region, user_pool_id
        );
        let jwks_uri = format!("{}/.well-known/jwks.json", issuer);

        Self {
            issuer,
            jwks_uri,
            audience: client_id.to_string(),
            leeway_seconds: 60,
            required_token_use: Some("access".to_string()),
            claim_mappings: ClaimMappings::cognito(),
        }
    }

    /// Create config for Google Identity.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::ValidationConfig;
    ///
    /// let config = ValidationConfig::google("client-id.apps.googleusercontent.com");
    /// ```
    pub fn google(client_id: &str) -> Self {
        Self {
            issuer: "https://accounts.google.com".to_string(),
            jwks_uri: "https://www.googleapis.com/oauth2/v3/certs".to_string(),
            audience: client_id.to_string(),
            leeway_seconds: 60,
            required_token_use: None,
            claim_mappings: ClaimMappings::google(),
        }
    }

    /// Create config for Auth0.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::ValidationConfig;
    ///
    /// let config = ValidationConfig::auth0("your-tenant.auth0.com", "client-id");
    /// ```
    pub fn auth0(domain: &str, client_id: &str) -> Self {
        let issuer = format!("https://{}/", domain);
        let jwks_uri = format!("https://{}/.well-known/jwks.json", domain);

        Self {
            issuer,
            jwks_uri,
            audience: client_id.to_string(),
            leeway_seconds: 60,
            required_token_use: None,
            claim_mappings: ClaimMappings::auth0(),
        }
    }

    /// Create config for Okta.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::ValidationConfig;
    ///
    /// let config = ValidationConfig::okta("your-domain.okta.com", "client-id");
    /// ```
    pub fn okta(domain: &str, client_id: &str) -> Self {
        let issuer = format!("https://{}", domain);
        let jwks_uri = format!("https://{}/oauth2/v1/keys", domain);

        Self {
            issuer,
            jwks_uri,
            audience: client_id.to_string(),
            leeway_seconds: 60,
            required_token_use: None,
            claim_mappings: ClaimMappings::okta(),
        }
    }

    /// Create config for Microsoft Entra ID (Azure AD).
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::ValidationConfig;
    ///
    /// let config = ValidationConfig::entra("tenant-id", "client-id");
    /// ```
    pub fn entra(tenant_id: &str, client_id: &str) -> Self {
        let issuer = format!("https://login.microsoftonline.com/{}/v2.0", tenant_id);
        let jwks_uri = format!(
            "https://login.microsoftonline.com/{}/discovery/v2.0/keys",
            tenant_id
        );

        Self {
            issuer,
            jwks_uri,
            audience: client_id.to_string(),
            leeway_seconds: 60,
            required_token_use: None,
            claim_mappings: ClaimMappings::entra(),
        }
    }

    /// Set clock skew tolerance.
    pub fn with_leeway(mut self, seconds: u64) -> Self {
        self.leeway_seconds = seconds;
        self
    }

    /// Set required `token_use` claim (e.g., "access" for Cognito).
    pub fn with_required_token_use(mut self, token_use: impl Into<String>) -> Self {
        self.required_token_use = Some(token_use.into());
        self
    }

    /// Set custom claim mappings.
    pub fn with_claim_mappings(mut self, mappings: ClaimMappings) -> Self {
        self.claim_mappings = mappings;
        self
    }
}

/// Parse scopes from token claims.
///
/// Handles both space-separated string format and array format.
#[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
fn parse_scopes(claims: &serde_json::Value) -> Vec<String> {
    // Try "scope" claim (space-separated string or array)
    if let Some(scope) = claims.get("scope") {
        if let Some(s) = scope.as_str() {
            return s.split_whitespace().map(String::from).collect();
        }
        if let Some(arr) = scope.as_array() {
            return arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect();
        }
    }

    // Try "scp" claim (Azure AD style)
    if let Some(scp) = claims.get("scp") {
        if let Some(arr) = scp.as_array() {
            return arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect();
        }
        if let Some(s) = scp.as_str() {
            return s.split_whitespace().map(String::from).collect();
        }
    }

    Vec::new()
}

/// JWKS response structure.
#[cfg(feature = "jwt-auth")]
#[derive(Debug, serde::Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

/// Individual JWK key structure.
#[cfg(feature = "jwt-auth")]
#[derive(Debug, serde::Deserialize)]
struct JwkKey {
    /// Key ID
    kid: Option<String>,
    /// Key type (e.g., "RSA")
    #[allow(dead_code)]
    kty: String,
    /// RSA modulus (base64url-encoded)
    n: Option<String>,
    /// RSA exponent (base64url-encoded)
    e: Option<String>,
    /// Algorithm (e.g., "RS256")
    #[allow(dead_code)]
    alg: Option<String>,
    /// Key use (e.g., "sig")
    #[serde(rename = "use")]
    #[allow(dead_code)]
    key_use: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_config_cognito() {
        let config = ValidationConfig::cognito("us-east-1", "us-east-1_xxxxx", "client-123");

        assert_eq!(
            config.issuer,
            "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
        );
        assert_eq!(
            config.jwks_uri,
            "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx/.well-known/jwks.json"
        );
        assert_eq!(config.audience, "client-123");
        assert_eq!(config.required_token_use, Some("access".to_string()));
    }

    #[test]
    fn test_validation_config_google() {
        let config = ValidationConfig::google("client-123.apps.googleusercontent.com");

        assert_eq!(config.issuer, "https://accounts.google.com");
        assert_eq!(
            config.jwks_uri,
            "https://www.googleapis.com/oauth2/v3/certs"
        );
        assert!(config.required_token_use.is_none());
    }

    #[test]
    fn test_validation_config_auth0() {
        let config = ValidationConfig::auth0("tenant.auth0.com", "client-123");

        assert_eq!(config.issuer, "https://tenant.auth0.com/");
        assert_eq!(
            config.jwks_uri,
            "https://tenant.auth0.com/.well-known/jwks.json"
        );
    }

    #[test]
    fn test_validation_config_okta() {
        let config = ValidationConfig::okta("dev-123.okta.com", "client-123");

        assert_eq!(config.issuer, "https://dev-123.okta.com");
        assert_eq!(config.jwks_uri, "https://dev-123.okta.com/oauth2/v1/keys");
    }

    #[test]
    fn test_validation_config_entra() {
        let config = ValidationConfig::entra("tenant-id-123", "client-123");

        assert_eq!(
            config.issuer,
            "https://login.microsoftonline.com/tenant-id-123/v2.0"
        );
        assert_eq!(
            config.jwks_uri,
            "https://login.microsoftonline.com/tenant-id-123/discovery/v2.0/keys"
        );
    }

    #[test]
    fn test_validation_config_builder() {
        let config = ValidationConfig::new(
            "https://issuer.example.com",
            "https://issuer.example.com/.well-known/jwks.json",
            "my-audience",
        )
        .with_leeway(120)
        .with_required_token_use("access");

        assert_eq!(config.leeway_seconds, 120);
        assert_eq!(config.required_token_use, Some("access".to_string()));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_jwt_validator_creation() {
        let validator = JwtValidator::new();
        assert_eq!(validator.cache_ttl, Duration::from_hours(1));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_jwt_validator_custom_ttl() {
        let validator = JwtValidator::with_cache_ttl(Duration::from_hours(2));
        assert_eq!(validator.cache_ttl, Duration::from_hours(2));
    }

    #[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
    #[test]
    fn test_jwt_validator_clone() {
        use std::sync::Arc;
        let validator1 = JwtValidator::new();
        let validator2 = validator1.clone();

        // Cloned validators share the same cache
        assert!(Arc::ptr_eq(&validator1.jwks_cache, &validator2.jwks_cache));
    }

    #[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
    #[test]
    fn test_parse_scopes_space_separated() {
        let claims = serde_json::json!({
            "scope": "read write admin"
        });
        let scopes = parse_scopes(&claims);
        assert_eq!(scopes, vec!["read", "write", "admin"]);
    }

    #[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
    #[test]
    fn test_parse_scopes_array() {
        let claims = serde_json::json!({
            "scope": ["read", "write", "admin"]
        });
        let scopes = parse_scopes(&claims);
        assert_eq!(scopes, vec!["read", "write", "admin"]);
    }

    #[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
    #[test]
    fn test_parse_scopes_scp_array() {
        let claims = serde_json::json!({
            "scp": ["User.Read", "User.Write"]
        });
        let scopes = parse_scopes(&claims);
        assert_eq!(scopes, vec!["User.Read", "User.Write"]);
    }

    #[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
    #[test]
    fn test_parse_scopes_scp_string() {
        let claims = serde_json::json!({
            "scp": "User.Read User.Write"
        });
        let scopes = parse_scopes(&claims);
        assert_eq!(scopes, vec!["User.Read", "User.Write"]);
    }

    #[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
    #[test]
    fn test_parse_scopes_empty() {
        let claims = serde_json::json!({});
        let scopes = parse_scopes(&claims);
        assert!(scopes.is_empty());
    }

    #[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
    #[tokio::test]
    async fn test_clear_cache() {
        let validator = JwtValidator::new();

        // Cache should start empty
        assert_eq!(validator.cache_size().await, 0);

        // Clear should work on empty cache
        validator.clear_cache().await;
        assert_eq!(validator.cache_size().await, 0);
    }
}
