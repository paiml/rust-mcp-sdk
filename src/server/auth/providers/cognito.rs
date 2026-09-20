//! AWS Cognito identity provider.
//!
//! This module provides a Cognito-specific implementation of [`IdentityProvider`].
//! JWT validation is delegated to [`MultiTenantJwtValidator`] for code reuse.
//!
//! # Discovery is an ordered probe with a validated anchor
//!
//! Discovery follows the MCP specification's ordered authorization-server
//! metadata probe (SEP-2351) rather than one constructed URL, and every document
//! it retrieves is compared against the issuer the URL was built from
//! (RFC 8414 §3.3 / `OpenID` Connect Discovery §4.3) **before** it leaves the
//! fetch helper. Without that second half the anchor is a value the served
//! document chose for itself.
//!
//! `src/client/auth.rs` and `src/server/auth/providers/generic_oidc.rs`
//! implement the same three-part shape — ordered probe, anchor check, bounded
//! reads — over the SAME shared derivation
//! ([`discovery_url_candidates`](crate::shared::oauth_validation::discovery_url_candidates))
//! and the SAME outcome matrix
//! ([`classify_discovery_failure`](crate::shared::oauth_validation::classify_discovery_failure)).
//! The three are deliberately written to mirror each other so they can be
//! reviewed side by side; a change to one of them belongs in all three.
//!
//! This provider carries a TTL cache directly ABOVE the probe, which is why it
//! needs no second, candidate-index cache: the ordered candidates are probed at
//! most once per `cache_ttl`. An anchor-REJECTED document is never written to
//! that cache — caching one would turn a one-shot spoof into a persistent one.
//!
//! Every whole-body read here is bounded, on success AND on error paths — a
//! hostile identity provider controls its error bodies too.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::{Error, ErrorCode, Result};
#[cfg(feature = "jwt-auth")]
use crate::server::auth::jwt_validator::{JwtValidator, ValidationConfig};
use crate::server::auth::provider::{
    AuthorizationParams, DcrRequest, DcrResponse, IdentityProvider, OidcDiscovery,
    ProviderCapabilities, TokenExchangeParams, TokenResponse,
};
use crate::server::auth::traits::AuthContext;
#[cfg(not(target_arch = "wasm32"))]
use crate::shared::http_body_cap::{
    collect_reqwest_body_within_cap, hardened_discovery_client, is_body_over_cap,
    is_redirect_refusal, DEFAULT_AUTH_RESPONSE_BYTES,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::shared::oauth_validation::{
    classify_discovery_failure, discovery_url_candidates, issuer_matches_metadata,
    DiscoveryFailure, DiscoveryOutcome,
};

/// How long a single discovery request may take.
///
/// The same budget the provider's general-purpose client already uses, so the
/// ordered probe cannot hang on a candidate that never answers.
#[cfg(not(target_arch = "wasm32"))]
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// How many attempts ONE candidate gets before the probe falls back to the next.
#[cfg(not(target_arch = "wasm32"))]
const DISCOVERY_MAX_ATTEMPTS: usize = 3;

/// Delay between attempts against a single candidate.
#[cfg(not(target_arch = "wasm32"))]
const DISCOVERY_RETRY_DELAY: Duration = Duration::from_millis(200);

/// How much of a peer-supplied issuer string a refusal may reproduce.
///
/// RFC 8414 §3.3's whole value is telling an operator which two values
/// disagreed, so the refusal names both — but the DOCUMENT's issuer is arbitrary
/// attacker-chosen text, and truncating it stops a megabyte-long "issuer" from
/// flooding a log.
#[cfg(not(target_arch = "wasm32"))]
const MAX_ECHOED_DOCUMENT_ISSUER: usize = 256;

/// Cached data with expiration.
struct CachedData<T: std::fmt::Debug> {
    data: T,
    fetched_at: Instant,
    ttl: Duration,
}

impl<T: std::fmt::Debug> std::fmt::Debug for CachedData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedData")
            .field("data", &self.data)
            .field("fetched_at", &self.fetched_at)
            .field("ttl", &self.ttl)
            .finish()
    }
}

impl<T: std::fmt::Debug> CachedData<T> {
    fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            fetched_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}

/// Type alias for discovery cache.
#[cfg(not(target_arch = "wasm32"))]
type DiscoveryCache = Arc<RwLock<Option<CachedData<OidcDiscovery>>>>;

/// AWS Cognito identity provider.
///
/// Provides token validation and OIDC discovery for AWS Cognito user pools.
/// Uses [`JwtValidator`] internally for efficient JWKS caching and JWT validation.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::auth::providers::CognitoProvider;
///
/// let cognito = CognitoProvider::new(
///     "us-east-1",
///     "us-east-1_xxxxx",
///     "your-client-id",
/// ).await?;
///
/// // Validate a token
/// let auth = cognito.validate_token("eyJ...").await?;
/// println!("User: {}", auth.user_id());
/// ```
#[derive(Debug)]
pub struct CognitoProvider {
    /// AWS region.
    region: String,
    /// Cognito user pool ID.
    user_pool_id: String,
    /// App client ID.
    client_id: String,
    /// Issuer URL.
    issuer: String,
    /// JWKS URI.
    #[allow(dead_code)]
    jwks_uri: String,
    /// JWT validator with shared JWKS cache.
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    jwt_validator: JwtValidator,
    /// Validation config for this provider.
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    validation_config: ValidationConfig,
    /// Cached discovery document.
    #[cfg(not(target_arch = "wasm32"))]
    discovery_cache: DiscoveryCache,
    /// HTTP client for non-JWT operations.
    #[cfg(not(target_arch = "wasm32"))]
    http_client: reqwest::Client,
    /// HTTP client for DISCOVERY only, whose redirect policy refuses any
    /// redirect that leaves the issuer's origin.
    ///
    /// A separate client on purpose: the origin pin is a statement about who may
    /// AUTHOR the metadata document, and applying it to the token or `UserInfo`
    /// endpoints — which legitimately redirect at some providers — would change
    /// behaviour this plan does not own. Both discovery call sites in this crate
    /// build theirs from the same [`hardened_discovery_client`], so the policy
    /// cannot diverge between them.
    #[cfg(not(target_arch = "wasm32"))]
    discovery_client: reqwest::Client,
    /// Cache TTL.
    cache_ttl: Duration,
}

impl CognitoProvider {
    /// Create a new Cognito provider.
    ///
    /// # Arguments
    ///
    /// * `region` - AWS region (e.g., "us-east-1")
    /// * `user_pool_id` - Cognito user pool ID (e.g., "us-east-1_xxxxx")
    /// * `client_id` - App client ID
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(region: &str, user_pool_id: &str, client_id: &str) -> Result<Self> {
        let issuer = format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            region, user_pool_id
        );
        let jwks_uri = format!("{}/.well-known/jwks.json", issuer);

        let provider = Self {
            region: region.to_string(),
            user_pool_id: user_pool_id.to_string(),
            client_id: client_id.to_string(),
            issuer,
            jwks_uri,
            #[cfg(feature = "jwt-auth")]
            jwt_validator: JwtValidator::new(),
            #[cfg(feature = "jwt-auth")]
            validation_config: ValidationConfig::cognito(region, user_pool_id, client_id),
            discovery_cache: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?,
            discovery_client: hardened_discovery_client(DISCOVERY_TIMEOUT)?,
            cache_ttl: Duration::from_hours(1),
        };

        Ok(provider)
    }

    /// Create a provider with a shared JWT validator.
    ///
    /// Use this when you want multiple providers to share the same JWKS cache.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use pmcp::server::auth::{MultiTenantJwtValidator, CognitoProvider};
    ///
    /// // Create shared validator
    /// let validator = MultiTenantJwtValidator::new();
    ///
    /// // Create providers that share the validator
    /// let provider1 = CognitoProvider::with_validator("us-east-1", "pool1", "client1", validator.clone()).await?;
    /// let provider2 = CognitoProvider::with_validator("us-west-2", "pool2", "client2", validator.clone()).await?;
    /// ```
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    pub async fn with_validator(
        region: &str,
        user_pool_id: &str,
        client_id: &str,
        jwt_validator: JwtValidator,
    ) -> Result<Self> {
        let issuer = format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            region, user_pool_id
        );
        let jwks_uri = format!("{}/.well-known/jwks.json", issuer);

        let provider = Self {
            region: region.to_string(),
            user_pool_id: user_pool_id.to_string(),
            client_id: client_id.to_string(),
            issuer,
            jwks_uri,
            jwt_validator,
            validation_config: ValidationConfig::cognito(region, user_pool_id, client_id),
            discovery_cache: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?,
            discovery_client: hardened_discovery_client(DISCOVERY_TIMEOUT)?,
            cache_ttl: Duration::from_hours(1),
        };

        Ok(provider)
    }

    /// Get the AWS region.
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Get the user pool ID.
    pub fn user_pool_id(&self) -> &str {
        &self.user_pool_id
    }

    /// Get the client ID.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Get the Cognito hosted UI authorization endpoint.
    fn hosted_ui_domain(&self) -> String {
        // Default hosted UI domain pattern
        format!(
            "https://{}.auth.{}.amazoncognito.com",
            self.user_pool_id, self.region
        )
    }
}

#[async_trait]
impl IdentityProvider for CognitoProvider {
    fn id(&self) -> &'static str {
        "cognito"
    }

    fn display_name(&self) -> &'static str {
        "AWS Cognito"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            oidc: true,
            dcr: false, // Cognito doesn't support DCR
            pkce: true,
            refresh_tokens: true,
            revocation: true,
            introspection: false,
            custom_scopes: true,
            device_flow: false,
        }
    }

    fn issuer(&self) -> &str {
        &self.issuer
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    async fn validate_token(&self, token: &str) -> Result<AuthContext> {
        // Delegate to shared JWT validator
        self.jwt_validator
            .validate(token, &self.validation_config)
            .await
    }

    #[cfg(any(target_arch = "wasm32", not(feature = "jwt-auth")))]
    async fn validate_token(&self, _token: &str) -> Result<AuthContext> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "JWT validation requires the 'jwt-auth' feature and non-WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn discovery(&self) -> Result<OidcDiscovery> {
        // Check cache first
        {
            let cache = self.discovery_cache.read().await;
            if let Some(ref cached) = *cache {
                if !cached.is_expired() {
                    return Ok(cached.data.clone());
                }
            }
        }

        // Fetch discovery document. The TTL cache above short-circuits this
        // entirely, so a warm provider probes no candidate at all.
        //
        // An anchor-REJECTED document never reaches the cache write below,
        // because `?` short-circuits on the refusal. Caching a rejected document
        // would turn a one-shot spoof into a persistent one for the whole TTL.
        let discovery = fetch_discovery_doc(&self.discovery_client, &self.issuer).await?;

        // Cache the discovery document
        {
            let mut cache = self.discovery_cache.write().await;
            *cache = Some(CachedData::new(discovery.clone(), self.cache_ttl));
        }

        Ok(discovery)
    }

    #[cfg(target_arch = "wasm32")]
    async fn discovery(&self) -> Result<OidcDiscovery> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Discovery not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn jwks(&self) -> Result<serde_json::Value> {
        let response = self
            .http_client
            .get(&self.jwks_uri)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to fetch JWKS: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "JWKS endpoint returned status {}",
                response.status()
            )));
        }

        read_json_within_cap(response, "JWKS response").await
    }

    #[cfg(target_arch = "wasm32")]
    async fn jwks(&self) -> Result<serde_json::Value> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "JWKS not available on WASM target",
        ))
    }

    async fn authorization_url(&self, params: AuthorizationParams) -> Result<String> {
        let hosted_ui = self.hosted_ui_domain();

        let mut url = format!(
            "{}/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            hosted_ui,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&params.redirect_uri),
            urlencoding::encode(&params.scopes.join(" ")),
            urlencoding::encode(&params.state),
        );

        if let Some(nonce) = &params.nonce {
            url.push_str(&format!("&nonce={}", urlencoding::encode(nonce)));
        }

        if let Some(challenge) = &params.code_challenge {
            url.push_str(&format!(
                "&code_challenge={}&code_challenge_method={}",
                urlencoding::encode(challenge),
                params.code_challenge_method.as_deref().unwrap_or("S256")
            ));
        }

        for (key, value) in &params.extra {
            url.push_str(&format!(
                "&{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            ));
        }

        Ok(url)
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn exchange_code(&self, params: TokenExchangeParams) -> Result<TokenResponse> {
        let hosted_ui = self.hosted_ui_domain();
        let token_url = format!("{}/oauth2/token", hosted_ui);

        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("client_id", self.client_id.clone()),
            ("code", params.code),
            ("redirect_uri", params.redirect_uri),
        ];

        if let Some(verifier) = params.code_verifier {
            form.push(("code_verifier", verifier));
        }

        let response = self
            .http_client
            .post(&token_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token exchange failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = read_error_body_within_cap(response).await;
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token exchange failed: {}", error_text),
            ));
        }

        read_json_within_cap(response, "token exchange response").await
    }

    #[cfg(target_arch = "wasm32")]
    async fn exchange_code(&self, _params: TokenExchangeParams) -> Result<TokenResponse> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Code exchange not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        let hosted_ui = self.hosted_ui_domain();
        let token_url = format!("{}/oauth2/token", hosted_ui);

        let form = vec![
            ("grant_type", "refresh_token"),
            ("client_id", &self.client_id),
            ("refresh_token", refresh_token),
        ];

        let response = self
            .http_client
            .post(&token_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token refresh failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = read_error_body_within_cap(response).await;
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token refresh failed: {}", error_text),
            ));
        }

        read_json_within_cap(response, "token refresh response").await
    }

    #[cfg(target_arch = "wasm32")]
    async fn refresh_token(&self, _refresh_token: &str) -> Result<TokenResponse> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Token refresh not available on WASM target",
        ))
    }

    async fn register_client(&self, _request: DcrRequest) -> Result<DcrResponse> {
        Err(Error::protocol(
            ErrorCode::INVALID_REQUEST,
            "AWS Cognito does not support Dynamic Client Registration",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn revoke_token(&self, token: &str) -> Result<()> {
        let hosted_ui = self.hosted_ui_domain();
        let revoke_url = format!("{}/oauth2/revoke", hosted_ui);

        let form = vec![("token", token), ("client_id", &self.client_id)];

        let response = self
            .http_client
            .post(&revoke_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token revocation failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = read_error_body_within_cap(response).await;
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token revocation failed: {}", error_text),
            ));
        }

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    async fn revoke_token(&self, _token: &str) -> Result<()> {
        Ok(()) // No-op on WASM
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn user_info(&self, access_token: &str) -> Result<serde_json::Value> {
        let hosted_ui = self.hosted_ui_domain();
        let userinfo_url = format!("{}/oauth2/userInfo", hosted_ui);

        let response = self
            .http_client
            .get(&userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| Error::internal(format!("UserInfo request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = read_error_body_within_cap(response).await;
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("UserInfo request failed: {}", error_text),
            ));
        }

        read_json_within_cap(response, "UserInfo response").await
    }

    #[cfg(target_arch = "wasm32")]
    async fn user_info(&self, _access_token: &str) -> Result<serde_json::Value> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "UserInfo not available on WASM target",
        ))
    }
}

/// Fetch and VALIDATE the OIDC discovery document for `issuer`.
///
/// Probes the specification's ordered candidate endpoints (SEP-2351) instead of
/// one constructed URL, and every failure is classified by the shared matrix:
/// `Retry` re-attempts the same candidate within [`DISCOVERY_MAX_ATTEMPTS`],
/// `Fallback` moves to the next candidate, and `Terminal` aborts the whole
/// probe. When every candidate fails, the error enumerates all of them rather
/// than only the last.
///
/// Routing through the shared derivation also closes a defect this file had and
/// its `generic_oidc` sibling did not: the old `format!` never trimmed a
/// trailing slash from the issuer, so `https://as.example/` produced
/// `https://as.example//.well-known/openid-configuration`.
#[cfg(not(target_arch = "wasm32"))]
async fn fetch_discovery_doc(http_client: &reqwest::Client, issuer: &str) -> Result<OidcDiscovery> {
    let candidates = discovery_url_candidates(issuer)?;
    let mut attempted: Vec<String> = Vec::new();

    for url in &candidates {
        tracing::debug!("Fetching OIDC discovery from {}", url);
        match probe_discovery_candidate(http_client, url, issuer).await {
            Ok(document) => return Ok(document),
            // A TERMINAL outcome aborts the WHOLE discovery. Falling through
            // here would turn a security failure into a silent DOWNGRADE: an
            // attacker who can make one candidate fail in a security-relevant
            // way would steer this provider onto a candidate they serve. Do NOT
            // "simplify" this back into `if !ok { continue }`.
            Err((DiscoveryOutcome::Terminal, error)) => return Err(error),
            Err((_, error)) => attempted.push(format!("{url}: {error}")),
        }
    }

    Err(every_candidate_failed(issuer, &attempted))
}

/// Probe ONE candidate URL, applying the shared outcome matrix.
///
/// `Retry` re-attempts this candidate within [`DISCOVERY_MAX_ATTEMPTS`] and
/// becomes `Fallback` once that budget is spent; `Terminal` propagates unchanged
/// so the caller aborts.
#[cfg(not(target_arch = "wasm32"))]
async fn probe_discovery_candidate(
    http_client: &reqwest::Client,
    url: &url::Url,
    expected_issuer: &str,
) -> std::result::Result<OidcDiscovery, (DiscoveryOutcome, Error)> {
    let mut attempts: usize = 0;
    loop {
        let (failure, error) =
            match fetch_discovery_candidate(http_client, url, expected_issuer).await {
                Ok(document) => return Ok(document),
                Err(pair) => pair,
            };

        match classify_discovery_failure(failure) {
            DiscoveryOutcome::Terminal => return Err((DiscoveryOutcome::Terminal, error)),
            DiscoveryOutcome::Fallback => return Err((DiscoveryOutcome::Fallback, error)),
            DiscoveryOutcome::Retry => {
                attempts += 1;
                if attempts >= DISCOVERY_MAX_ATTEMPTS {
                    return Err((DiscoveryOutcome::Fallback, error));
                }
                tokio::time::sleep(DISCOVERY_RETRY_DELAY).await;
            },
        }
    }
}

/// Fetch and VALIDATE a discovery document from ONE candidate URL.
///
/// The body is read through the bounded reader and the document's `issuer` is
/// compared against `expected_issuer` per RFC 8414 §3.3 — both before any
/// metadata leaves this function, and therefore before anything can be written
/// to the TTL cache. `expected_issuer` is the ISSUER, not the candidate URL:
/// every candidate for one issuer shares the same expected value.
#[cfg(not(target_arch = "wasm32"))]
async fn fetch_discovery_candidate(
    http_client: &reqwest::Client,
    url: &url::Url,
    expected_issuer: &str,
) -> std::result::Result<OidcDiscovery, (DiscoveryFailure, Error)> {
    let response = http_client
        .get(url.as_str())
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| discovery_request_failure(url, &e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(discovery_status_failure(url, status));
    }

    let bytes = collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES)
        .await
        .map_err(|e| {
            let failure = if is_body_over_cap(&e) {
                DiscoveryFailure::BodyOverCap
            } else {
                DiscoveryFailure::Transport
            };
            (failure, e)
        })?;

    let document: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| unparseable_discovery_document(url, &e))?;

    // RFC 8414 section 3.3 / OpenID Connect Discovery section 4.3, BEFORE the
    // document escapes this function. This provider deserializes into its own
    // `OidcDiscovery` type, so the anchor is read from the parsed JSON first: an
    // absent or wrongly-typed `issuer` must be a refusal, never a `None` that
    // reads as "nothing to check".
    let document_issuer = discovery_document_issuer(url, &document)?;
    if !issuer_matches_metadata(expected_issuer, document_issuer) {
        return Err(discovery_issuer_mismatch(
            url,
            expected_issuer,
            document_issuer,
        ));
    }

    serde_json::from_slice(&bytes).map_err(|e| unparseable_discovery_document(url, &e))
}

/// The document's `issuer`, which MUST be present and MUST be a string.
///
/// It is the value every downstream trust decision is anchored on, so neither
/// absence nor a wrong type may be tolerated into a `None`.
#[cfg(not(target_arch = "wasm32"))]
fn discovery_document_issuer<'a>(
    url: &url::Url,
    document: &'a serde_json::Value,
) -> std::result::Result<&'a str, (DiscoveryFailure, Error)> {
    match document.get("issuer") {
        Some(serde_json::Value::String(issuer)) => Ok(issuer),
        Some(_) => Err(malformed_discovery_metadata(
            url,
            "`issuer` is present but is not a JSON string. It is the value the RFC 8414 \
             section 3.3 anchor comparison is made against, so a wrongly-typed issuer cannot be \
             tolerated",
        )),
        None => Err(malformed_discovery_metadata(
            url,
            "`issuer` is absent. RFC 8414 section 3.3 requires it, and an absent anchor must not \
             read as `nothing to check`",
        )),
    }
}

/// Classify a failed discovery REQUEST.
///
/// A refused redirect is a statement about who would have AUTHORED the document,
/// not about availability: retrying returns the same answer, and falling through
/// to another candidate is the downgrade the redirect policy exists to prevent.
/// Everything else — connect failures, DNS, TLS, timeouts — is an availability
/// failure.
#[cfg(not(target_arch = "wasm32"))]
fn discovery_request_failure(url: &url::Url, source: &reqwest::Error) -> (DiscoveryFailure, Error) {
    let failure = if is_redirect_refusal(source) {
        DiscoveryFailure::MalformedSecurityMetadata
    } else {
        DiscoveryFailure::Transport
    };
    (
        failure,
        Error::internal(format!(
            "Failed to fetch discovery document from {url}: {}",
            rendered_source_chain(source)
        )),
    )
}

/// Classify a discovery response whose HTTP status is not a success.
#[cfg(not(target_arch = "wasm32"))]
fn discovery_status_failure(
    url: &url::Url,
    status: reqwest::StatusCode,
) -> (DiscoveryFailure, Error) {
    let failure = if status == reqwest::StatusCode::NOT_FOUND {
        DiscoveryFailure::NotFound
    } else {
        DiscoveryFailure::HttpStatus(status.as_u16())
    };
    (
        failure,
        Error::internal(format!("Discovery endpoint {url} returned status {status}")),
    )
}

/// The refusal for a body that is not the JSON this endpoint serves.
///
/// Carries `serde_json`'s CLASSIFICATION plus line and column, never its
/// message: a `serde_json` data error reproduces the offending value, and this
/// body is peer-controlled.
#[cfg(not(target_arch = "wasm32"))]
fn unparseable_discovery_document(
    url: &url::Url,
    source: &serde_json::Error,
) -> (DiscoveryFailure, Error) {
    (
        DiscoveryFailure::InvalidJson,
        Error::internal(format!(
            "Discovery document from {url} is not the JSON document this endpoint serves \
             ({:?} error at line {}, column {}). The parser's own message is not reproduced here \
             because a data error echoes the offending input",
            source.classify(),
            source.line(),
            source.column()
        )),
    )
}

/// The RFC 8414 §3.3 refusal, naming BOTH issuers.
#[cfg(not(target_arch = "wasm32"))]
fn discovery_issuer_mismatch(
    url: &url::Url,
    expected_issuer: &str,
    document_issuer: &str,
) -> (DiscoveryFailure, Error) {
    (
        DiscoveryFailure::IssuerMismatch,
        Error::protocol(
            ErrorCode::INVALID_REQUEST,
            format!(
                "Discovery document fetched from {url} declares issuer `{}`, but the URL was \
                 built from issuer `{expected_issuer}`. RFC 8414 section 3.3 and OpenID Connect \
                 Discovery section 4.3 require these to be identical, so the metadata is NOT \
                 used and is NOT cached. The document's value is peer-controlled and is \
                 truncated at {MAX_ECHOED_DOCUMENT_ISSUER} characters here",
                truncate_for_message(document_issuer)
            ),
        ),
    )
}

/// The refusal for a document that parsed but whose security metadata is broken.
#[cfg(not(target_arch = "wasm32"))]
fn malformed_discovery_metadata(url: &url::Url, detail: &str) -> (DiscoveryFailure, Error) {
    (
        DiscoveryFailure::MalformedSecurityMetadata,
        Error::protocol(
            ErrorCode::INVALID_REQUEST,
            format!("Discovery document from {url} carries malformed security metadata: {detail}"),
        ),
    )
}

/// The refusal when the ordered probe is exhausted, enumerating every candidate.
#[cfg(not(target_arch = "wasm32"))]
fn every_candidate_failed(issuer: &str, attempted: &[String]) -> Error {
    Error::internal(format!(
        "Failed to discover OIDC configuration for issuer `{issuer}`. Every candidate endpoint \
         was tried and none served a usable document:\n  - {}",
        attempted.join("\n  - ")
    ))
}

/// Bound a peer-controlled string before it reaches an error message.
#[cfg(not(target_arch = "wasm32"))]
fn truncate_for_message(value: &str) -> String {
    if value.chars().count() <= MAX_ECHOED_DOCUMENT_ISSUER {
        return value.to_owned();
    }
    let head: String = value.chars().take(MAX_ECHOED_DOCUMENT_ISSUER).collect();
    format!("{head}… (truncated)")
}

/// Render an error together with its whole source chain.
///
/// `reqwest::Error`'s own `Display` omits the source, which is where the
/// hardened client's redirect refusal explains itself.
#[cfg(not(target_arch = "wasm32"))]
fn rendered_source_chain(error: &dyn std::error::Error) -> String {
    let mut rendered = error.to_string();
    let mut current = error.source();
    while let Some(cause) = current {
        rendered.push_str(" <- ");
        rendered.push_str(&cause.to_string());
        current = cause.source();
    }
    rendered
}

/// Read a peer-supplied SUCCESS body and deserialize it, bounded.
///
/// Every whole-body read in this module goes through here or through
/// [`read_error_body_within_cap`], so no identity provider ever chooses this
/// crate's allocation. The parse refusal carries `serde_json`'s CLASSIFICATION
/// plus line and column and never its message, because a data error reproduces
/// the offending input and a token response body carries credentials.
#[cfg(not(target_arch = "wasm32"))]
async fn read_json_within_cap<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    what: &str,
) -> Result<T> {
    let bytes = collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES)
        .await
        .map_err(|e| Error::internal(format!("Failed to read {what} body: {e}")))?;
    serde_json::from_slice(&bytes).map_err(|e| {
        Error::internal(format!(
            "Failed to parse {what} ({:?} error at line {}, column {}). The parser's own message \
             is not reproduced here because a data error echoes the offending input",
            e.classify(),
            e.line(),
            e.column()
        ))
    })
}

/// Read a peer-supplied ERROR body, bounded, for interpolation into a message.
///
/// Error paths matter as much as success paths: a hostile identity provider
/// controls its error bodies too, and this one is interpolated into a refusal.
#[cfg(not(target_arch = "wasm32"))]
async fn read_error_body_within_cap(response: reqwest::Response) -> String {
    match collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) => format!("<error body not read: {e}>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::auth::traits::ClaimMappings;

    // =========================================================================
    // Discovery: the SEP-2351 ordered probe, the RFC 8414 §3.3 anchor, and the
    // TTL cache that must keep short-circuiting both.
    //
    // These rows are INLINE rather than in `tests/oauth_provider_discovery.rs`
    // because `CognitoProvider::new(region, user_pool_id, client_id)` DERIVES
    // its issuer as `https://cognito-idp.{region}.amazonaws.com/{pool}`. No
    // public constructor can be aimed at a local mock, and this plan adds no
    // public surface — so the struct is built directly here, which only a
    // module-internal test can do. The reachable-from-outside rows (the real
    // provider's issuer flowing into the shared derivation, and the
    // trailing-slash defect) live in the integration file.
    // =========================================================================

    /// The user-pool path segment, so the three candidate paths below are
    /// derivable by eye.
    #[cfg(not(target_arch = "wasm32"))]
    const POOL: &str = "us-east-1_TEST";
    /// RFC 8414 §3.1 insertion, `oauth-authorization-server` suffix.
    #[cfg(not(target_arch = "wasm32"))]
    const CANDIDATE_1: &str = "/.well-known/oauth-authorization-server/us-east-1_TEST";
    /// RFC 8414 §3.1 insertion, `openid-configuration` suffix.
    #[cfg(not(target_arch = "wasm32"))]
    const CANDIDATE_2: &str = "/.well-known/openid-configuration/us-east-1_TEST";
    /// `OpenID` Connect Discovery §4.1 append — this provider's ONLY form before
    /// this plan, and now the last candidate rather than the only one.
    #[cfg(not(target_arch = "wasm32"))]
    const CANDIDATE_3: &str = "/us-east-1_TEST/.well-known/openid-configuration";

    /// A discovery document whose `"issuer"` is a free variable, well formed in
    /// every other respect — so a suite that could not tell "validated" from
    /// "not validated" would accept the lying fixture.
    #[cfg(not(target_arch = "wasm32"))]
    fn discovery_body(document_issuer: &str, base: &str) -> String {
        serde_json::json!({
            "issuer": document_issuer,
            "authorization_endpoint": format!("{base}/authorize"),
            "token_endpoint": format!("{base}/token"),
            "jwks_uri": format!("{base}/jwks"),
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code"],
            "scopes_supported": ["openid"],
            "token_endpoint_auth_methods_supported": ["none"],
            "code_challenge_methods_supported": ["S256"],
        })
        .to_string()
    }

    /// Build a provider aimed at `issuer`, bypassing the AWS-derived
    /// constructor. Every field is exactly what `new` would have produced.
    #[cfg(not(target_arch = "wasm32"))]
    fn provider_for(issuer: &str, cache_ttl: Duration) -> CognitoProvider {
        CognitoProvider {
            region: "us-east-1".to_string(),
            user_pool_id: POOL.to_string(),
            client_id: "client".to_string(),
            jwks_uri: format!("{}/.well-known/jwks.json", issuer.trim_end_matches('/')),
            issuer: issuer.to_string(),
            #[cfg(feature = "jwt-auth")]
            jwt_validator: JwtValidator::new(),
            #[cfg(feature = "jwt-auth")]
            validation_config: ValidationConfig::cognito("us-east-1", POOL, "client"),
            discovery_cache: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::new(),
            discovery_client: hardened_discovery_client(DISCOVERY_TIMEOUT)
                .expect("the hardened discovery client must build"),
            cache_ttl,
        }
    }

    /// One hour — long enough that no test in this file crosses it by accident.
    #[cfg(not(target_arch = "wasm32"))]
    fn long_ttl() -> Duration {
        Duration::from_hours(1)
    }

    /// A mock serving 404. Every candidate expected to fall through is mocked
    /// EXPLICITLY: `mockito` answers an unmatched request with 501, which the
    /// shared matrix classifies as `Retry`, so an unmocked path would silently
    /// spend the retry budget instead of falling through.
    #[cfg(not(target_arch = "wasm32"))]
    async fn mock_404(
        server: &mut mockito::ServerGuard,
        path: &str,
        expected_hits: usize,
    ) -> mockito::Mock {
        server
            .mock("GET", path)
            .with_status(404)
            .expect(expected_hits)
            .create_async()
            .await
    }

    /// A mock serving an HONEST, well-formed discovery document for `issuer`.
    #[cfg(not(target_arch = "wasm32"))]
    async fn mock_valid(
        server: &mut mockito::ServerGuard,
        path: &str,
        issuer: &str,
        expected_hits: usize,
    ) -> mockito::Mock {
        let base = server.url();
        server
            .mock("GET", path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery_body(issuer, &base))
            .expect(expected_hits)
            .create_async()
            .await
    }

    /// A mock serving a document that LIES about its issuer — the
    /// specification's own worked attack.
    #[cfg(not(target_arch = "wasm32"))]
    async fn mock_lying(
        server: &mut mockito::ServerGuard,
        path: &str,
        expected_hits: usize,
    ) -> mockito::Mock {
        let base = server.url();
        server
            .mock("GET", path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery_body("https://honest.example", &base))
            .expect(expected_hits)
            .create_async()
            .await
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn cognito_probes_the_spec_candidates_before_the_appended_form() {
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        let c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
        let c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
        let c3 = mock_valid(&mut server, CANDIDATE_3, &issuer, 1).await;

        provider_for(&issuer, long_ttl())
            .discovery()
            .await
            .expect("candidate 3 serves an honest document");

        // Hit counts prove candidates 1 and 2 were attempted FIRST. Before this
        // plan both were zero: the provider built candidate 3 and nothing else.
        c1.assert_async().await;
        c2.assert_async().await;
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn a_cognito_candidate_one_success_never_requests_candidate_three() {
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        let c1 = mock_valid(&mut server, CANDIDATE_1, &issuer, 1).await;
        let c2 = mock_404(&mut server, CANDIDATE_2, 0).await;
        let c3 = mock_404(&mut server, CANDIDATE_3, 0).await;

        provider_for(&issuer, long_ttl()).discovery().await.unwrap();

        c1.assert_async().await;
        c2.assert_async().await;
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn a_lying_cognito_discovery_document_is_rejected_naming_both_values() {
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
        let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
        let _c3 = mock_lying(&mut server, CANDIDATE_3, 1).await;

        let message = provider_for(&issuer, long_ttl())
            .discovery()
            .await
            .unwrap_err()
            .to_string();
        assert!(
            message.contains(&issuer),
            "the refusal must name the EXPECTED issuer: {message}"
        );
        assert!(
            message.contains("https://honest.example"),
            "the refusal must name the DOCUMENT's issuer: {message}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn a_lying_cognito_document_aborts_the_probe_instead_of_downgrading() {
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        let _c1 = mock_lying(&mut server, CANDIDATE_1, 1).await;
        // A PERFECTLY VALID document behind candidate 3 with `expect(0)`: a
        // fall-through would satisfy the caller and fail this test.
        let c3 = mock_valid(&mut server, CANDIDATE_3, &issuer, 0).await;

        assert!(provider_for(&issuer, long_ttl()).discovery().await.is_err());
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn an_oversized_cognito_body_aborts_the_probe_instead_of_downgrading() {
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        let padding = "x".repeat(1_200_000);
        let oversized = format!(r#"{{"issuer":"{issuer}","pad":"{padding}"}}"#);
        let _c1 = server
            .mock("GET", CANDIDATE_1)
            .with_status(200)
            .with_body(oversized)
            .expect(1)
            .create_async()
            .await;
        let c3 = mock_valid(&mut server, CANDIDATE_3, &issuer, 0).await;

        let message = provider_for(&issuer, long_ttl())
            .discovery()
            .await
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("1048576"),
            "the refusal must name the cap: {message}"
        );
        assert!(
            !message.contains("xxxxxxxxxx"),
            "the refusal must echo no byte of the refused body: {message}"
        );
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn a_cross_origin_cognito_discovery_redirect_is_not_followed() {
        let mut origin = mockito::Server::new_async().await;
        let mut elsewhere = mockito::Server::new_async().await;

        // A SECOND mockito server is a genuinely different origin (its port
        // differs), so `expect(0)` proves the redirect was not followed rather
        // than merely that the call failed.
        let never = elsewhere
            .mock("GET", CANDIDATE_1)
            .with_status(200)
            .with_body("SHOULD NOT BE FETCHED")
            .expect(0)
            .create_async()
            .await;
        let issuer = format!("{}/{POOL}", origin.url());
        let _c1 = origin
            .mock("GET", CANDIDATE_1)
            .with_status(302)
            .with_header("location", &format!("{}{CANDIDATE_1}", elsewhere.url()))
            .expect(1)
            .create_async()
            .await;
        let c3 = mock_valid(&mut origin, CANDIDATE_3, &issuer, 0).await;

        assert!(provider_for(&issuer, long_ttl()).discovery().await.is_err());
        never.assert_async().await;
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn a_cognito_five_xx_is_retried_to_the_budget_then_falls_back() {
        // `expect(ATTEMPTS)` is what distinguishes "retried then fell back" from
        // "fell back immediately"; both end in the same `Ok`.
        const ATTEMPTS: usize = 3;
        assert_eq!(
            ATTEMPTS, DISCOVERY_MAX_ATTEMPTS,
            "this row asserts the production retry budget, not a number of its own"
        );
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        let c1 = server
            .mock("GET", CANDIDATE_1)
            .with_status(503)
            .expect(ATTEMPTS)
            .create_async()
            .await;
        let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
        let c3 = mock_valid(&mut server, CANDIDATE_3, &issuer, 1).await;

        provider_for(&issuer, long_ttl()).discovery().await.unwrap();

        c1.assert_async().await;
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn a_trailing_slash_cognito_issuer_produces_no_doubled_slash() {
        // The defect this row keeps fixed: `cognito.rs` built
        // `format!("{}/.well-known/openid-configuration", self.issuer)` with NO
        // `trim_end_matches('/')`, so a trailing-slash issuer requested
        // `...//.well-known/openid-configuration` and no real endpoint answers
        // that. `generic_oidc` did trim; this file did not.
        //
        // The document declares the issuer WITH its slash, exactly as Auth0 and
        // every other trailing-slash issuer does — see the sibling row below for
        // why that detail is load-bearing rather than incidental.
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}/", server.url());
        let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
        let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
        // The mocked path has no doubled slash, so a trailing-slash issuer must
        // reach exactly this mock — which it could not do before this plan.
        let c3 = mock_valid(&mut server, CANDIDATE_3, &issuer, 1).await;

        provider_for(&issuer, long_ttl())
            .discovery()
            .await
            .expect("a trailing slash is a formatting difference, not a path component");
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn a_trailing_slash_issuer_still_needs_a_byte_identical_document_issuer() {
        // The URL DERIVATION normalises a trailing slash away; the RFC 8414
        // §3.3 anchor does NOT, and must not — 116-04 pinned four normalisation
        // rows as `false` precisely so an attacker cannot exploit a lenient
        // comparison. The two rules therefore disagree on purpose, and the
        // consequence is operationally visible: an operator who configures
        // `https://as.example/pool/` against a provider whose document declares
        // `https://as.example/pool` gets a hard refusal rather than a silently
        // accepted document.
        //
        // That is what the specification requires, and the refusal names BOTH
        // values so the fix is a one-character config edit. This row exists so
        // the behaviour is pinned deliberately rather than discovered in
        // production.
        let mut server = mockito::Server::new_async().await;
        let issuer_no_slash = format!("{}/{POOL}", server.url());
        let issuer = format!("{issuer_no_slash}/");
        let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
        let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
        let _c3 = mock_valid(&mut server, CANDIDATE_3, &issuer_no_slash, 1).await;

        let message = provider_for(&issuer, long_ttl())
            .discovery()
            .await
            .unwrap_err()
            .to_string();
        assert!(
            message.contains(&issuer),
            "the refusal must name the CONFIGURED issuer, slash and all: {message}"
        );
        assert!(
            message.contains(&issuer_no_slash),
            "the refusal must name the DOCUMENT's issuer: {message}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn the_cognito_ttl_cache_still_short_circuits_the_ordered_probe() {
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        // ONE hit each across BOTH calls: the cache read sits directly above the
        // probe and must keep short-circuiting it entirely.
        let c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
        let c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
        let c3 = mock_valid(&mut server, CANDIDATE_3, &issuer, 1).await;

        let provider = provider_for(&issuer, long_ttl());
        provider.discovery().await.unwrap();
        provider.discovery().await.unwrap();

        c1.assert_async().await;
        c2.assert_async().await;
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn a_cognito_cache_miss_reprobes_the_ordered_candidates_from_the_top() {
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        // TWO hits each: with the TTL expired, the second call must re-run the
        // FULL ordered sequence rather than resuming at the candidate that
        // happened to serve last time.
        let c1 = mock_404(&mut server, CANDIDATE_1, 2).await;
        let c2 = mock_404(&mut server, CANDIDATE_2, 2).await;
        let c3 = mock_valid(&mut server, CANDIDATE_3, &issuer, 2).await;

        let provider = provider_for(&issuer, Duration::from_millis(1));
        provider.discovery().await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        provider.discovery().await.unwrap();

        c1.assert_async().await;
        c2.assert_async().await;
        c3.assert_async().await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn an_anchor_rejected_cognito_document_is_never_cached() {
        // Caching a rejected document would turn a one-shot spoof into a
        // persistent one for the whole TTL. The second call must re-attempt the
        // fetch, which is proven by the mock being hit TWICE.
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/{POOL}", server.url());
        let c1 = mock_lying(&mut server, CANDIDATE_1, 2).await;

        let provider = provider_for(&issuer, long_ttl());
        assert!(provider.discovery().await.is_err());
        assert!(
            provider.discovery().await.is_err(),
            "a rejected document must not be served from the cache either"
        );
        c1.assert_async().await;
    }

    // =========================================================================
    // CachedData Tests
    // =========================================================================

    #[test]
    fn test_cached_data_creation() {
        let data: CachedData<String> = CachedData::new("test".to_string(), Duration::from_mins(1));
        assert_eq!(data.data, "test");
        assert!(!data.is_expired());
    }

    #[test]
    fn test_cached_data_expiration() {
        let data: CachedData<String> =
            CachedData::new("test".to_string(), Duration::from_millis(1));
        // Wait for it to expire
        std::thread::sleep(Duration::from_millis(10));
        assert!(data.is_expired());
    }

    #[test]
    fn test_cached_data_debug() {
        let data: CachedData<String> = CachedData::new("test".to_string(), Duration::from_mins(1));
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("CachedData"));
        assert!(debug_str.contains("data"));
        assert!(debug_str.contains("ttl"));
    }

    // =========================================================================
    // Provider Capabilities Tests
    // =========================================================================

    #[test]
    fn test_cognito_capabilities() {
        // Test the expected capabilities for Cognito
        let caps = ProviderCapabilities {
            oidc: true,
            dcr: false, // Cognito doesn't support DCR
            pkce: true,
            refresh_tokens: true,
            revocation: true,
            introspection: false,
            custom_scopes: true,
            device_flow: false,
        };

        assert!(caps.oidc);
        assert!(!caps.dcr);
        assert!(caps.pkce);
        assert!(caps.refresh_tokens);
        assert!(caps.revocation);
        assert!(!caps.introspection);
        assert!(caps.custom_scopes);
        assert!(!caps.device_flow);
    }

    // =========================================================================
    // URL Generation Tests (Unit tests without network)
    // =========================================================================

    #[test]
    fn test_issuer_url_format() {
        let region = "us-east-1";
        let user_pool_id = "us-east-1_ABC123";
        let expected = format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            region, user_pool_id
        );
        assert_eq!(
            expected,
            "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC123"
        );
    }

    #[test]
    fn test_jwks_uri_format() {
        let issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC123";
        let jwks_uri = format!("{}/.well-known/jwks.json", issuer);
        assert!(jwks_uri.ends_with("/.well-known/jwks.json"));
        assert!(jwks_uri.contains("cognito-idp"));
    }

    #[test]
    fn test_hosted_ui_domain_format() {
        let user_pool_id = "us-east-1_ABC123";
        let region = "us-east-1";
        let expected = format!("https://{}.auth.{}.amazoncognito.com", user_pool_id, region);
        assert_eq!(
            expected,
            "https://us-east-1_ABC123.auth.us-east-1.amazoncognito.com"
        );
    }

    #[test]
    fn test_authorization_url_components() {
        // Test URL components without needing actual provider
        let hosted_ui = "https://us-east-1_ABC123.auth.us-east-1.amazoncognito.com";
        let client_id = "test-client-id";
        let redirect_uri = "https://example.com/callback";
        let scopes = ["openid", "email", "profile"];
        let state = "random-state";

        let url = format!(
            "{}/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            hosted_ui,
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&scopes.join(" ")),
            urlencoding::encode(state),
        );

        assert!(url.contains("/oauth2/authorize"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fexample.com%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid%20email%20profile"));
        assert!(url.contains("state=random-state"));
    }

    #[test]
    fn test_authorization_url_with_pkce() {
        let base_url = "https://auth.example.com/oauth2/authorize?client_id=test";
        let code_challenge = "challenge123";
        let code_challenge_method = "S256";

        let url = format!(
            "{}&code_challenge={}&code_challenge_method={}",
            base_url,
            urlencoding::encode(code_challenge),
            code_challenge_method
        );

        assert!(url.contains("code_challenge=challenge123"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_authorization_url_with_nonce() {
        let base_url = "https://auth.example.com/oauth2/authorize?client_id=test";
        let nonce = "nonce456";

        let url = format!("{}&nonce={}", base_url, urlencoding::encode(nonce));

        assert!(url.contains("nonce=nonce456"));
    }

    // =========================================================================
    // ClaimMappings Tests
    // =========================================================================

    #[test]
    fn test_cognito_claim_mappings() {
        let mappings = ClaimMappings::cognito();
        assert_eq!(mappings.user_id, "sub");
        assert_eq!(mappings.tenant_id, Some("custom:tenant_id".to_string()));
        assert_eq!(mappings.email, Some("email".to_string()));
        assert_eq!(mappings.groups, Some("cognito:groups".to_string()));
    }

    #[test]
    fn test_cognito_claim_normalization() {
        let mappings = ClaimMappings::cognito();

        let claims = serde_json::json!({
            "sub": "user-123",
            "email": "user@example.com",
            "custom:tenant_id": "tenant-456",
            "cognito:groups": ["admin", "users"]
        });

        let normalized = mappings.normalize_claims(&claims);

        assert_eq!(
            normalized.get("sub").and_then(|v| v.as_str()),
            Some("user-123")
        );
        assert_eq!(
            normalized.get("email").and_then(|v| v.as_str()),
            Some("user@example.com")
        );
        assert_eq!(
            normalized.get("tenant_id").and_then(|v| v.as_str()),
            Some("tenant-456")
        );
        assert!(normalized.contains_key("groups"));
    }

    // =========================================================================
    // Error Message Tests
    // =========================================================================

    #[test]
    fn test_dcr_not_supported_message() {
        // Cognito doesn't support DCR - verify the error message
        let error_msg = "AWS Cognito does not support Dynamic Client Registration";
        assert!(error_msg.contains("Cognito"));
        assert!(error_msg.contains("Dynamic Client Registration"));
    }

    #[tokio::test]
    async fn test_dcr_returns_error() {
        // This test would require a mock provider, but we can verify the trait default
        use crate::server::auth::provider::IdentityProvider;

        // Create a mock that has the same behavior
        struct MockCognito;

        #[async_trait]
        impl IdentityProvider for MockCognito {
            fn id(&self) -> &'static str {
                "cognito"
            }
            fn display_name(&self) -> &'static str {
                "AWS Cognito"
            }
            fn capabilities(&self) -> ProviderCapabilities {
                ProviderCapabilities {
                    oidc: true,
                    dcr: false,
                    pkce: true,
                    refresh_tokens: true,
                    revocation: true,
                    introspection: false,
                    custom_scopes: true,
                    device_flow: false,
                }
            }
            #[allow(clippy::unnecessary_literal_bound)]
            fn issuer(&self) -> &str {
                "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_test"
            }
            async fn validate_token(&self, _token: &str) -> Result<AuthContext> {
                Ok(AuthContext::new("test-user"))
            }
            async fn discovery(&self) -> Result<OidcDiscovery> {
                unimplemented!()
            }
            async fn jwks(&self) -> Result<serde_json::Value> {
                unimplemented!()
            }
            async fn register_client(
                &self,
                _request: crate::server::auth::provider::DcrRequest,
            ) -> Result<crate::server::auth::provider::DcrResponse> {
                Err(crate::error::Error::protocol(
                    crate::error::ErrorCode::INVALID_REQUEST,
                    "AWS Cognito does not support Dynamic Client Registration",
                ))
            }
        }

        impl std::fmt::Debug for MockCognito {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("MockCognito").finish()
            }
        }

        let provider = MockCognito;
        let request = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec!["https://example.com/callback".to_string()],
            client_name: None,
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: None,
            grant_types: vec![],
            response_types: vec![],
            scope: None,
            software_id: None,
            software_version: None,
            extra: std::collections::HashMap::new(),
        };

        let result = provider.register_client(request).await;
        assert!(result.is_err());
    }
}
