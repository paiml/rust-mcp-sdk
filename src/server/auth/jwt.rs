//! JWT token validator with JWKS support.
//!
//! This module provides stateless JWT validation using JSON Web Key Sets (JWKS).
//! It supports all major OAuth providers through configuration.
//!
//! # Feature Flag
//!
//! This module requires the `jwt-auth` feature:
//!
//! ```toml
//! [dependencies]
//! pmcp = { version = "1.8", features = ["jwt-auth"] }
//! ```

use super::config::JwtValidatorConfig;
use super::traits::{AuthContext, ClaimMappings, TokenValidator};
use crate::error::{Error, ErrorCode, Result};
// Gated to match its ONLY use site, not the module. `mod jwt`/`mod jwt_validator`
// compile under `http-client`, but the reqwest body-cap read lives inside a
// `jwt-auth` + non-wasm arm, so an `http-client`-without-`jwt-auth` build (and
// every wasm32 build) left this import unused and `-D warnings` made it fatal.
#[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
use crate::shared::http_body_cap::{collect_reqwest_body_within_cap, DEFAULT_AUTH_RESPONSE_BYTES};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Cached JWKS keys with expiration.
struct CachedJwks {
    #[cfg(feature = "jwt-auth")]
    keys: HashMap<String, jsonwebtoken::DecodingKey>,
    #[cfg(not(feature = "jwt-auth"))]
    keys: HashMap<String, ()>,
    fetched_at: Instant,
    ttl: Duration,
}

impl std::fmt::Debug for CachedJwks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedJwks")
            .field("keys_count", &self.keys.len())
            .field("fetched_at", &self.fetched_at)
            .field("ttl", &self.ttl)
            .finish()
    }
}

impl CachedJwks {
    fn new(ttl: Duration) -> Self {
        Self {
            keys: HashMap::new(),
            fetched_at: Instant::now(),
            ttl,
        }
    }

    #[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}

/// JWT validator using JWKS for stateless token validation.
///
/// This validator fetches and caches the OAuth provider's public keys (JWKS)
/// and uses them to verify JWT signatures locally, without making a request
/// to the auth server for each token validation.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::auth::{JwtValidator, TokenValidator};
/// use pmcp::server::auth::config::JwtValidatorConfig;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create validator for AWS Cognito
/// let config = JwtValidatorConfig::cognito("us-east-1", "us-east-1_xxxxx", "client-id");
/// let validator = JwtValidator::new(config).await?;
///
/// // Validate a token
/// let auth_context = validator.validate("eyJhbGci...").await?;
/// println!("User: {}", auth_context.user_id());
/// # Ok(())
/// # }
/// ```
///
/// # Provider Support
///
/// Works with any OIDC-compliant provider:
/// - AWS Cognito
/// - Microsoft Entra ID (Azure AD)
/// - Google Identity
/// - Okta
/// - Auth0
/// - Any provider with a JWKS endpoint
#[derive(Debug)]
pub struct JwtValidator {
    /// Validator configuration.
    #[allow(dead_code)]
    config: JwtValidatorConfig,
    /// Cached JWKS.
    #[allow(dead_code)]
    jwks: Arc<RwLock<CachedJwks>>,
    /// HTTP client for fetching JWKS.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    http_client: reqwest::Client,
}

impl JwtValidator {
    /// Create a new JWT validator with the given configuration.
    ///
    /// This will fetch the JWKS from the configured endpoint and cache it.
    ///
    /// # Errors
    ///
    /// Returns an error if the JWKS cannot be fetched or parsed.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(config: JwtValidatorConfig) -> Result<Self> {
        let ttl = config.cache_ttl();
        let validator = Self {
            config,
            jwks: Arc::new(RwLock::new(CachedJwks::new(ttl))),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?,
        };

        // Fetch JWKS on startup
        validator.refresh_keys_internal().await?;

        Ok(validator)
    }

    /// Create a validator from an existing reqwest client.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn with_client(
        config: JwtValidatorConfig,
        http_client: reqwest::Client,
    ) -> Result<Self> {
        let ttl = config.cache_ttl();
        let validator = Self {
            config,
            jwks: Arc::new(RwLock::new(CachedJwks::new(ttl))),
            http_client,
        };

        validator.refresh_keys_internal().await?;

        Ok(validator)
    }

    /// Get the issuer URL.
    pub fn issuer(&self) -> &str {
        &self.config.issuer
    }

    /// Get the expected audience.
    pub fn audience(&self) -> &str {
        &self.config.audience
    }

    /// Get the claim mappings.
    pub fn claim_mappings(&self) -> &ClaimMappings {
        &self.config.claim_mappings
    }

    /// Refresh the JWKS cache.
    #[cfg(not(target_arch = "wasm32"))]
    async fn refresh_keys_internal(&self) -> Result<()> {
        #[cfg(feature = "jwt-auth")]
        {
            let jwks_uri = self.config.jwks_uri();
            tracing::debug!("Fetching JWKS from {}", jwks_uri);

            let response = self
                .http_client
                .get(&jwks_uri)
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
            let body =
                collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await?;
            let jwks: JwksResponse = serde_json::from_slice(&body)
                .map_err(|e| Error::internal(format!("Failed to parse JWKS: {}", e)))?;

            let mut keys = HashMap::new();
            for key in jwks.keys {
                if let (Some(kid), Some(n), Some(e)) = (&key.kid, &key.n, &key.e) {
                    match jsonwebtoken::DecodingKey::from_rsa_components(n, e) {
                        Ok(decoding_key) => {
                            keys.insert(kid.clone(), decoding_key);
                        },
                        Err(err) => {
                            tracing::warn!("Failed to parse key {}: {}", kid, err);
                        },
                    }
                }
            }

            if keys.is_empty() {
                return Err(Error::internal("No valid keys found in JWKS"));
            }

            tracing::info!("Loaded {} keys from JWKS", keys.len());

            let mut cache = self.jwks.write().await;
            cache.keys = keys;
            cache.fetched_at = Instant::now();

            Ok(())
        }

        #[cfg(not(feature = "jwt-auth"))]
        {
            Err(Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                "JWT validation requires the 'jwt-auth' feature",
            ))
        }
    }

    /// Validate a JWT and extract the auth context.
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    async fn validate_jwt(&self, token: &str) -> Result<AuthContext> {
        use jsonwebtoken::{decode, decode_header, Algorithm, Validation};

        // Decode header to get key ID
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

        // Get the key from cache
        let key = {
            let cache = self.jwks.read().await;

            // Refresh if expired
            if cache.is_expired() {
                drop(cache);
                self.refresh_keys_internal().await?;
                let cache = self.jwks.read().await;
                cache.keys.get(&kid).cloned()
            } else {
                cache.keys.get(&kid).cloned()
            }
        };

        let key = key.ok_or_else(|| {
            Error::protocol(
                ErrorCode::AUTHENTICATION_REQUIRED,
                format!("Unknown key ID: {}", kid),
            )
        })?;

        // Build validation
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.audience]);
        validation.leeway = self.config.leeway_seconds;

        // Decode and verify token
        let token_data = decode::<serde_json::Value>(token, &key, &validation).map_err(|e| {
            let msg = match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => "Token expired",
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => "Invalid issuer",
                jsonwebtoken::errors::ErrorKind::InvalidAudience => "Invalid audience",
                jsonwebtoken::errors::ErrorKind::InvalidSignature => "Invalid signature",
                _ => "Token validation failed",
            };
            Error::protocol(ErrorCode::AUTHENTICATION_REQUIRED, msg)
        })?;

        // Normalize claims using mappings
        let normalized_claims = self
            .config
            .claim_mappings
            .normalize_claims(&token_data.claims);

        // Extract subject
        let subject = normalized_claims
            .get("sub")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        // Extract scopes
        let scopes = parse_scopes(&token_data.claims);

        // Extract client ID
        let client_id = token_data
            .claims
            .get("azp")
            .or_else(|| token_data.claims.get("client_id"))
            .and_then(|v| v.as_str())
            .map(String::from);

        // Extract expiration
        let expires_at = token_data.claims.get("exp").and_then(|v| v.as_u64());

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
}

#[async_trait]
impl TokenValidator for JwtValidator {
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    async fn validate(&self, token: &str) -> Result<AuthContext> {
        self.validate_jwt(token).await
    }

    #[cfg(any(target_arch = "wasm32", not(feature = "jwt-auth")))]
    async fn validate(&self, _token: &str) -> Result<AuthContext> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "JWT validation requires the 'jwt-auth' feature and non-WASM target",
        ))
    }
}

/// Parse scopes from token claims.
///
/// Handles both space-separated string format and array format.
#[cfg(all(feature = "jwt-auth", not(target_arch = "wasm32")))]
fn parse_scopes(claims: &serde_json::Value) -> Vec<String> {
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

    if let Some(scp) = claims.get("scp") {
        if let Some(arr) = scp.as_array() {
            return arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect();
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

#[cfg(all(test, feature = "jwt-auth", not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scopes_string() {
        let claims = serde_json::json!({
            "scope": "read write admin"
        });
        let scopes = parse_scopes(&claims);
        assert_eq!(scopes, vec!["read", "write", "admin"]);
    }

    #[test]
    fn test_parse_scopes_array() {
        let claims = serde_json::json!({
            "scope": ["read", "write", "admin"]
        });
        let scopes = parse_scopes(&claims);
        assert_eq!(scopes, vec!["read", "write", "admin"]);
    }

    #[test]
    fn test_parse_scopes_scp() {
        let claims = serde_json::json!({
            "scp": ["User.Read", "User.Write"]
        });
        let scopes = parse_scopes(&claims);
        assert_eq!(scopes, vec!["User.Read", "User.Write"]);
    }

    #[test]
    fn test_parse_scopes_empty() {
        let claims = serde_json::json!({});
        let scopes = parse_scopes(&claims);
        assert!(scopes.is_empty());
    }
}
