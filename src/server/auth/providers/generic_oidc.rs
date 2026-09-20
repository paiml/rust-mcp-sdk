//! Generic OIDC identity provider.
//!
//! This module provides a generic OIDC provider implementation that works with
//! any OIDC-compliant identity provider (Google, Auth0, Okta, Azure AD, etc.).
//! JWT validation is delegated to [`JwtValidator`] for code reuse and shared JWKS caching.
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
//! `src/client/auth.rs` and `src/server/auth/providers/cognito.rs` implement the
//! same three-part shape — ordered probe, anchor check, bounded reads — over the
//! SAME shared derivation
//! ([`discovery_url_candidates`](crate::shared::oauth_validation::discovery_url_candidates))
//! and the SAME outcome matrix
//! ([`classify_discovery_failure`](crate::shared::oauth_validation::classify_discovery_failure)).
//! The three are deliberately written to mirror each other so they can be
//! reviewed side by side; a change to one of them belongs in all three.
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
use crate::server::auth::traits::{AuthContext, ClaimMappings};
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

/// Configuration for creating a generic OIDC provider.
#[derive(Debug, Clone)]
pub struct GenericOidcConfig {
    /// Unique identifier for this provider.
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// OIDC issuer URL.
    pub issuer: String,
    /// Client ID.
    pub client_id: String,
    /// Client secret (for confidential clients).
    pub client_secret: Option<String>,
    /// Custom claim mappings.
    pub claim_mappings: ClaimMappings,
    /// Cache TTL in seconds.
    pub cache_ttl: Duration,
    /// Clock skew leeway in seconds.
    pub leeway_seconds: u64,
}

impl GenericOidcConfig {
    /// Create a new configuration with required fields.
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        issuer: impl Into<String>,
        client_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            issuer: issuer.into(),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::default(),
            cache_ttl: Duration::from_hours(1),
            leeway_seconds: 60,
        }
    }

    /// Set client secret (for confidential clients).
    pub fn with_client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    /// Set custom claim mappings.
    pub fn with_claim_mappings(mut self, mappings: ClaimMappings) -> Self {
        self.claim_mappings = mappings;
        self
    }

    /// Create configuration for Google Identity.
    pub fn google(client_id: impl Into<String>) -> Self {
        Self {
            id: "google".to_string(),
            display_name: "Google Identity".to_string(),
            issuer: "https://accounts.google.com".to_string(),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::google(),
            cache_ttl: Duration::from_hours(1),
            leeway_seconds: 60,
        }
    }

    /// Create configuration for Auth0.
    pub fn auth0(domain: impl Into<String>, client_id: impl Into<String>) -> Self {
        let domain = domain.into();
        Self {
            id: "auth0".to_string(),
            display_name: "Auth0".to_string(),
            issuer: format!("https://{}/", domain),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::auth0(),
            cache_ttl: Duration::from_hours(1),
            leeway_seconds: 60,
        }
    }

    /// Create configuration for Okta.
    pub fn okta(domain: impl Into<String>, client_id: impl Into<String>) -> Self {
        let domain = domain.into();
        Self {
            id: "okta".to_string(),
            display_name: "Okta".to_string(),
            issuer: format!("https://{}", domain),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::okta(),
            cache_ttl: Duration::from_hours(1),
            leeway_seconds: 60,
        }
    }

    /// Create configuration for Microsoft Entra ID (Azure AD).
    pub fn entra(tenant_id: impl Into<String>, client_id: impl Into<String>) -> Self {
        let tenant_id = tenant_id.into();
        Self {
            id: "entra".to_string(),
            display_name: "Microsoft Entra ID".to_string(),
            issuer: format!("https://login.microsoftonline.com/{}/v2.0", tenant_id),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::entra(),
            cache_ttl: Duration::from_hours(1),
            leeway_seconds: 60,
        }
    }
}

/// Type alias for discovery cache.
#[cfg(not(target_arch = "wasm32"))]
type DiscoveryCache = Arc<RwLock<Option<CachedData<OidcDiscovery>>>>;

/// Generic OIDC identity provider.
///
/// Works with any OIDC-compliant identity provider by auto-discovering
/// endpoints from the OIDC discovery document. JWT validation is delegated
/// to [`JwtValidator`] for efficient shared JWKS caching.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::auth::providers::{GenericOidcProvider, GenericOidcConfig};
///
/// // Create provider for Google
/// let config = GenericOidcConfig::google("your-client-id");
/// let google = GenericOidcProvider::new(config).await?;
///
/// // Or create a custom provider
/// let custom_config = GenericOidcConfig::new(
///     "my-provider",
///     "My Identity Provider",
///     "https://auth.example.com",
///     "my-client-id",
/// );
/// let provider = GenericOidcProvider::new(custom_config).await?;
/// ```
pub struct GenericOidcProvider {
    /// Provider configuration.
    config: GenericOidcConfig,
    /// Provider ID (leaked string for 'static lifetime).
    id: &'static str,
    /// Display name (leaked string for 'static lifetime).
    display_name: &'static str,
    /// JWT validator with shared JWKS cache.
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    jwt_validator: JwtValidator,
    /// Validation config (built after discovery to get JWKS URI).
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    validation_config: ValidationConfig,
    /// Cached discovery document.
    #[cfg(not(target_arch = "wasm32"))]
    discovery_cache: DiscoveryCache,
    /// HTTP client for token, JWKS, `UserInfo`, revocation and DCR requests.
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
}

impl std::fmt::Debug for GenericOidcProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericOidcProvider")
            .field("id", &self.id)
            .field("display_name", &self.display_name)
            .field("issuer", &self.config.issuer)
            .field("client_id", &self.config.client_id)
            .finish()
    }
}

impl GenericOidcProvider {
    /// Create a new generic OIDC provider.
    ///
    /// This constructor fetches the OIDC discovery document to determine the JWKS URI.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(config: GenericOidcConfig) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?;
        let discovery_client = hardened_discovery_client(DISCOVERY_TIMEOUT)?;

        // Leak strings for static lifetime (these are typically created once per app)
        let id: &'static str = Box::leak(config.id.clone().into_boxed_str());
        let display_name: &'static str = Box::leak(config.display_name.clone().into_boxed_str());

        // Fetch discovery to get JWKS URI
        let discovery = fetch_discovery_doc(&discovery_client, &config.issuer).await?;

        // Cache the discovery document
        let discovery_cache = Arc::new(RwLock::new(Some(CachedData::new(
            discovery.clone(),
            config.cache_ttl,
        ))));

        let provider = Self {
            #[cfg(feature = "jwt-auth")]
            jwt_validator: JwtValidator::new(),
            #[cfg(feature = "jwt-auth")]
            validation_config: ValidationConfig::new(
                &config.issuer,
                &discovery.jwks_uri,
                &config.client_id,
            )
            .with_leeway(config.leeway_seconds)
            .with_claim_mappings(config.claim_mappings.clone()),
            config,
            id,
            display_name,
            discovery_cache,
            http_client,
            discovery_client,
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
    /// use pmcp::server::auth::{MultiTenantJwtValidator, GenericOidcProvider, GenericOidcConfig};
    ///
    /// // Create shared validator
    /// let validator = MultiTenantJwtValidator::new();
    ///
    /// // Create providers that share the validator
    /// let google_config = GenericOidcConfig::google("google-client-id");
    /// let google = GenericOidcProvider::with_validator(google_config, validator.clone()).await?;
    ///
    /// let auth0_config = GenericOidcConfig::auth0("tenant.auth0.com", "auth0-client-id");
    /// let auth0 = GenericOidcProvider::with_validator(auth0_config, validator.clone()).await?;
    /// ```
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    pub async fn with_validator(
        config: GenericOidcConfig,
        jwt_validator: JwtValidator,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?;
        let discovery_client = hardened_discovery_client(DISCOVERY_TIMEOUT)?;

        let id: &'static str = Box::leak(config.id.clone().into_boxed_str());
        let display_name: &'static str = Box::leak(config.display_name.clone().into_boxed_str());

        // Fetch discovery to get JWKS URI
        let discovery = fetch_discovery_doc(&discovery_client, &config.issuer).await?;

        let discovery_cache = Arc::new(RwLock::new(Some(CachedData::new(
            discovery.clone(),
            config.cache_ttl,
        ))));

        Ok(Self {
            jwt_validator,
            validation_config: ValidationConfig::new(
                &config.issuer,
                &discovery.jwks_uri,
                &config.client_id,
            )
            .with_leeway(config.leeway_seconds)
            .with_claim_mappings(config.claim_mappings.clone()),
            config,
            id,
            display_name,
            discovery_cache,
            http_client,
            discovery_client,
        })
    }

    /// Get the client ID.
    pub fn client_id(&self) -> &str {
        &self.config.client_id
    }

    /// Fetch and cache the OIDC discovery document.
    ///
    /// The TTL cache sits directly ABOVE the ordered probe and short-circuits it
    /// entirely, so a warm provider issues no discovery request at all. It is
    /// also why this provider needs no second, candidate-index cache: the probe
    /// runs at most once per `cache_ttl`.
    ///
    /// An anchor-REJECTED document never reaches the cache write below, because
    /// [`fetch_discovery_doc`] returns `Err` and the `?` short-circuits. Caching
    /// a rejected document would turn a one-shot spoof into a persistent one.
    #[cfg(not(target_arch = "wasm32"))]
    async fn fetch_discovery(&self) -> Result<OidcDiscovery> {
        // Check cache first
        {
            let cache = self.discovery_cache.read().await;
            if let Some(ref cached) = *cache {
                if !cached.is_expired() {
                    return Ok(cached.data.clone());
                }
            }
        }

        // Fetch discovery document
        let discovery = fetch_discovery_doc(&self.discovery_client, &self.config.issuer).await?;

        // Cache the discovery document
        {
            let mut cache = self.discovery_cache.write().await;
            *cache = Some(CachedData::new(discovery.clone(), self.config.cache_ttl));
        }

        Ok(discovery)
    }

    /// Determine capabilities from discovery document.
    ///
    /// This method can be used to detect provider capabilities dynamically
    /// by inspecting the OIDC discovery document.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    async fn detect_capabilities(&self) -> ProviderCapabilities {
        let Ok(discovery) = self.fetch_discovery().await else {
            return ProviderCapabilities::basic_oidc();
        };

        ProviderCapabilities {
            oidc: true,
            dcr: discovery.registration_endpoint.is_some(),
            pkce: discovery
                .code_challenge_methods_supported
                .iter()
                .any(|m| m == "S256"),
            refresh_tokens: discovery
                .grant_types_supported
                .iter()
                .any(|g| g == "refresh_token"),
            revocation: discovery.revocation_endpoint.is_some(),
            introspection: discovery.introspection_endpoint.is_some(),
            custom_scopes: !discovery.scopes_supported.is_empty(),
            device_flow: discovery
                .grant_types_supported
                .iter()
                .any(|g| g == "urn:ietf:params:oauth:grant-type:device_code"),
        }
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
/// metadata leaves this function. `expected_issuer` is the ISSUER, not the
/// candidate URL: every candidate for one issuer shares the same expected value.
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
                 used. The document's value is peer-controlled and is truncated at \
                 {MAX_ECHOED_DOCUMENT_ISSUER} characters here",
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

#[async_trait]
impl IdentityProvider for GenericOidcProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn capabilities(&self) -> ProviderCapabilities {
        // Return basic capabilities synchronously; full detection requires async
        ProviderCapabilities::basic_oidc()
    }

    #[cfg(target_arch = "wasm32")]
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::basic_oidc()
    }

    fn issuer(&self) -> &str {
        &self.config.issuer
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
        self.fetch_discovery().await
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
        let discovery = self.fetch_discovery().await?;

        let response = self
            .http_client
            .get(&discovery.jwks_uri)
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

    #[cfg(not(target_arch = "wasm32"))]
    async fn authorization_url(&self, params: AuthorizationParams) -> Result<String> {
        let discovery = self.fetch_discovery().await?;

        let mut url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            discovery.authorization_endpoint,
            urlencoding::encode(&self.config.client_id),
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

    #[cfg(target_arch = "wasm32")]
    async fn authorization_url(&self, _params: AuthorizationParams) -> Result<String> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Authorization URL not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn exchange_code(&self, params: TokenExchangeParams) -> Result<TokenResponse> {
        let discovery = self.fetch_discovery().await?;

        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("client_id", self.config.client_id.clone()),
            ("code", params.code),
            ("redirect_uri", params.redirect_uri),
        ];

        if let Some(verifier) = params.code_verifier {
            form.push(("code_verifier", verifier));
        }

        let mut request = self.http_client.post(&discovery.token_endpoint).form(&form);

        // Add client authentication if secret is configured
        if let Some(ref secret) = self.config.client_secret {
            request = request.basic_auth(&self.config.client_id, Some(secret));
        }

        let response = request
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
        let discovery = self.fetch_discovery().await?;

        let form = vec![
            ("grant_type", "refresh_token"),
            ("client_id", &self.config.client_id),
            ("refresh_token", refresh_token),
        ];

        let mut request = self.http_client.post(&discovery.token_endpoint).form(&form);

        // Add client authentication if secret is configured
        if let Some(ref secret) = self.config.client_secret {
            request = request.basic_auth(&self.config.client_id, Some(secret));
        }

        let response = request
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

    #[cfg(not(target_arch = "wasm32"))]
    async fn register_client(&self, request: DcrRequest) -> Result<DcrResponse> {
        let discovery = self.fetch_discovery().await?;

        let registration_endpoint = discovery.registration_endpoint.ok_or_else(|| {
            Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!(
                    "Provider '{}' does not support Dynamic Client Registration",
                    self.display_name
                ),
            )
        })?;

        let response = self
            .http_client
            .post(&registration_endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::internal(format!("DCR request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = read_error_body_within_cap(response).await;
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("DCR failed: {}", error_text),
            ));
        }

        read_json_within_cap(response, "DCR response").await
    }

    #[cfg(target_arch = "wasm32")]
    async fn register_client(&self, _request: DcrRequest) -> Result<DcrResponse> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "DCR not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn revoke_token(&self, token: &str) -> Result<()> {
        let discovery = self.fetch_discovery().await?;

        let Some(revocation_endpoint) = discovery.revocation_endpoint else {
            return Ok(()); // No-op if revocation not supported
        };

        let form = vec![("token", token), ("client_id", &self.config.client_id)];

        let mut request = self.http_client.post(&revocation_endpoint).form(&form);

        // Add client authentication if secret is configured
        if let Some(ref secret) = self.config.client_secret {
            request = request.basic_auth(&self.config.client_id, Some(secret));
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token revocation failed: {}", e)))?;

        // Revocation endpoints typically return 200 even for invalid tokens
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
        let discovery = self.fetch_discovery().await?;

        let userinfo_endpoint = discovery.userinfo_endpoint.ok_or_else(|| {
            Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!(
                    "Provider '{}' does not support UserInfo endpoint",
                    self.display_name
                ),
            )
        })?;

        let response = self
            .http_client
            .get(&userinfo_endpoint)
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // GenericOidcConfig Factory Methods Tests
    // =========================================================================

    #[test]
    fn test_google_config() {
        let config = GenericOidcConfig::google("test-client");
        assert_eq!(config.id, "google");
        assert_eq!(config.display_name, "Google Identity");
        assert_eq!(config.issuer, "https://accounts.google.com");
        assert_eq!(config.client_id, "test-client");
        assert!(config.client_secret.is_none());
    }

    #[test]
    fn test_auth0_config() {
        let config = GenericOidcConfig::auth0("example.auth0.com", "test-client");
        assert_eq!(config.id, "auth0");
        assert_eq!(config.display_name, "Auth0");
        assert_eq!(config.issuer, "https://example.auth0.com/");
        assert_eq!(config.client_id, "test-client");
    }

    #[test]
    fn test_okta_config() {
        let config = GenericOidcConfig::okta("example.okta.com", "test-client");
        assert_eq!(config.id, "okta");
        assert_eq!(config.display_name, "Okta");
        assert_eq!(config.issuer, "https://example.okta.com");
        assert_eq!(config.client_id, "test-client");
    }

    #[test]
    fn test_entra_config() {
        let config = GenericOidcConfig::entra("tenant-id", "test-client");
        assert_eq!(config.id, "entra");
        assert_eq!(config.display_name, "Microsoft Entra ID");
        assert_eq!(
            config.issuer,
            "https://login.microsoftonline.com/tenant-id/v2.0"
        );
        assert_eq!(config.client_id, "test-client");
    }

    // =========================================================================
    // GenericOidcConfig Builder Tests
    // =========================================================================

    #[test]
    fn test_config_new() {
        let config = GenericOidcConfig::new(
            "custom",
            "Custom Provider",
            "https://auth.example.com",
            "my-client-id",
        );

        assert_eq!(config.id, "custom");
        assert_eq!(config.display_name, "Custom Provider");
        assert_eq!(config.issuer, "https://auth.example.com");
        assert_eq!(config.client_id, "my-client-id");
        assert!(config.client_secret.is_none());
        assert_eq!(config.cache_ttl, Duration::from_hours(1));
        assert_eq!(config.leeway_seconds, 60);
    }

    #[test]
    fn test_config_with_client_secret() {
        let config = GenericOidcConfig::new("test", "Test", "https://test.com", "client")
            .with_client_secret("my-secret");

        assert_eq!(config.client_secret, Some("my-secret".to_string()));
    }

    #[test]
    fn test_config_with_claim_mappings() {
        let config = GenericOidcConfig::new("test", "Test", "https://test.com", "client")
            .with_claim_mappings(ClaimMappings::google());

        // Google claim mappings should be applied
        assert!(config.claim_mappings.tenant_id.is_none()); // Google doesn't have tenant
    }

    #[test]
    fn test_config_clone() {
        let config = GenericOidcConfig::google("test-client").with_client_secret("secret");
        let cloned = config.clone();

        assert_eq!(config.id, cloned.id);
        assert_eq!(config.client_secret, cloned.client_secret);
    }

    #[test]
    fn test_config_debug() {
        let config = GenericOidcConfig::google("test-client");
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("GenericOidcConfig"));
        assert!(debug_str.contains("google"));
    }

    // =========================================================================
    // ClaimMappings for Different Providers
    // =========================================================================

    #[test]
    fn test_google_claim_mappings() {
        let mappings = ClaimMappings::google();
        assert_eq!(mappings.user_id, "sub");
        assert!(mappings.tenant_id.is_none()); // Google doesn't have tenant concept
        assert_eq!(mappings.email, Some("email".to_string()));
    }

    #[test]
    fn test_auth0_claim_mappings() {
        let mappings = ClaimMappings::auth0();
        assert_eq!(mappings.user_id, "sub");
        assert_eq!(mappings.tenant_id, Some("org_id".to_string()));
        assert_eq!(mappings.groups, Some("roles".to_string()));
    }

    #[test]
    fn test_okta_claim_mappings() {
        let mappings = ClaimMappings::okta();
        assert_eq!(mappings.user_id, "uid");
        assert_eq!(mappings.tenant_id, Some("org_id".to_string()));
        assert_eq!(mappings.groups, Some("groups".to_string()));
    }

    #[test]
    fn test_entra_claim_mappings() {
        let mappings = ClaimMappings::entra();
        assert_eq!(mappings.user_id, "oid");
        assert_eq!(mappings.tenant_id, Some("tid".to_string()));
        assert_eq!(mappings.email, Some("preferred_username".to_string()));
        assert_eq!(mappings.groups, Some("groups".to_string()));
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
        std::thread::sleep(Duration::from_millis(10));
        assert!(data.is_expired());
    }

    #[test]
    fn test_cached_data_debug() {
        let data: CachedData<String> = CachedData::new("test".to_string(), Duration::from_mins(1));
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("CachedData"));
    }

    // =========================================================================
    // URL Generation Tests (Unit tests without network)
    // =========================================================================

    /// SEP-2351: discovery is an ORDERED probe, not a single constructed URL.
    ///
    /// Both rows previously re-implemented the removed `format!` inline and
    /// asserted its output, so they were green while measuring nothing about the
    /// provider — and they pinned the single-URL shape this plan replaced. They
    /// now assert the derivation the provider actually calls.
    #[test]
    fn test_discovery_url_candidates_for_a_path_less_issuer() {
        // For a path-less issuer the RFC 8414 inserted form and the OIDC
        // appended form coincide, so the list is TWO, not three.
        let rendered: Vec<String> = discovery_url_candidates("https://accounts.google.com")
            .unwrap()
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        assert_eq!(
            rendered,
            vec![
                "https://accounts.google.com/.well-known/oauth-authorization-server",
                "https://accounts.google.com/.well-known/openid-configuration",
            ]
        );
    }

    #[test]
    fn test_discovery_url_candidates_with_trailing_slash() {
        // A trailing slash is a formatting difference, not a path component, so
        // it derives the SAME list and never a doubled slash.
        let with_slash: Vec<String> = discovery_url_candidates("https://example.auth0.com/")
            .unwrap()
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let without: Vec<String> = discovery_url_candidates("https://example.auth0.com")
            .unwrap()
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        assert_eq!(with_slash, without);
        assert_eq!(
            with_slash,
            vec![
                "https://example.auth0.com/.well-known/oauth-authorization-server",
                "https://example.auth0.com/.well-known/openid-configuration",
            ]
        );
    }

    #[test]
    fn test_discovery_refusals_name_the_rule_and_bound_the_peer_value() {
        let url = url::Url::parse("https://as.example/.well-known/openid-configuration").unwrap();

        let (failure, error) =
            discovery_issuer_mismatch(&url, "https://as.example", "https://honest.example");
        assert_eq!(failure, DiscoveryFailure::IssuerMismatch);
        assert_eq!(
            classify_discovery_failure(failure),
            DiscoveryOutcome::Terminal
        );
        let message = error.to_string();
        assert!(message.contains("https://as.example"), "{message}");
        assert!(message.contains("https://honest.example"), "{message}");

        // A peer-chosen issuer is arbitrary text of unbounded length.
        let flood = "z".repeat(10_000);
        let (_, error) = discovery_issuer_mismatch(&url, "https://as.example", &flood);
        assert!(
            error.to_string().len() < 2_000,
            "a peer-chosen issuer must not flood the message"
        );
    }

    #[test]
    fn test_discovery_document_issuer_rows() {
        let url = url::Url::parse("https://as.example/.well-known/openid-configuration").unwrap();

        let good = serde_json::json!({ "issuer": "https://as.example" });
        assert_eq!(
            discovery_document_issuer(&url, &good).unwrap(),
            "https://as.example"
        );

        // An absent or wrongly-typed anchor is TERMINAL, never a quiet `None`.
        for hostile in [
            serde_json::json!({}),
            serde_json::json!({ "issuer": 7 }),
            serde_json::json!({ "issuer": null }),
        ] {
            let (failure, error) = discovery_document_issuer(&url, &hostile).unwrap_err();
            assert_eq!(
                failure,
                DiscoveryFailure::MalformedSecurityMetadata,
                "issuer {hostile} must be malformed security metadata"
            );
            assert_eq!(
                classify_discovery_failure(failure),
                DiscoveryOutcome::Terminal
            );
            assert!(error.to_string().contains("issuer"));
        }
    }

    #[test]
    fn test_discovery_status_classification_rows() {
        let url = url::Url::parse("https://as.example/.well-known/openid-configuration").unwrap();

        // 404 is the ordinary "not this form" answer the ordered probe is FOR.
        let (failure, _) = discovery_status_failure(&url, reqwest::StatusCode::NOT_FOUND);
        assert_eq!(failure, DiscoveryFailure::NotFound);
        assert_eq!(
            classify_discovery_failure(failure),
            DiscoveryOutcome::Fallback
        );

        // 5xx: the endpoint is the right one and is temporarily unwell.
        let (failure, _) = discovery_status_failure(&url, reqwest::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(failure, DiscoveryFailure::HttpStatus(503));
        assert_eq!(classify_discovery_failure(failure), DiscoveryOutcome::Retry);

        // Another 4xx: the endpoint answered and refused; another form may serve.
        let (failure, _) = discovery_status_failure(&url, reqwest::StatusCode::UNAUTHORIZED);
        assert_eq!(failure, DiscoveryFailure::HttpStatus(401));
        assert_eq!(
            classify_discovery_failure(failure),
            DiscoveryOutcome::Fallback
        );
    }

    #[test]
    fn test_authorization_url_components() {
        let authorization_endpoint = "https://accounts.google.com/o/oauth2/v2/auth";
        let client_id = "test-client-id";
        let redirect_uri = "https://example.com/callback";
        let scopes = ["openid", "email", "profile"];
        let state = "random-state";

        let url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            authorization_endpoint,
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&scopes.join(" ")),
            urlencoding::encode(state),
        );

        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fexample.com%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid%20email%20profile"));
        assert!(url.contains("state=random-state"));
    }

    #[test]
    fn test_authorization_url_with_pkce() {
        let base_url = "https://auth.example.com/authorize?client_id=test";
        let code_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        let code_challenge_method = "S256";

        let url = format!(
            "{}&code_challenge={}&code_challenge_method={}",
            base_url,
            urlencoding::encode(code_challenge),
            code_challenge_method
        );

        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_authorization_url_with_nonce() {
        let base_url = "https://auth.example.com/authorize?client_id=test";
        let nonce = "n-0S6_WzA2Mj";

        let url = format!("{}&nonce={}", base_url, urlencoding::encode(nonce));

        assert!(url.contains("nonce=n-0S6_WzA2Mj"));
    }

    // =========================================================================
    // Provider Capabilities Tests
    // =========================================================================

    #[test]
    fn test_basic_oidc_capabilities() {
        let caps = ProviderCapabilities::basic_oidc();
        assert!(caps.oidc);
        assert!(!caps.dcr);
        assert!(caps.pkce);
        assert!(caps.refresh_tokens);
        assert!(!caps.revocation);
        assert!(!caps.introspection);
    }

    // =========================================================================
    // Integration-style Tests (without network)
    // =========================================================================

    #[test]
    fn test_config_chain() {
        // Test fluent API
        let config = GenericOidcConfig::new(
            "custom-provider",
            "Custom Identity Provider",
            "https://identity.example.com",
            "client-123",
        )
        .with_client_secret("secret-456")
        .with_claim_mappings(ClaimMappings::default());

        assert_eq!(config.id, "custom-provider");
        assert_eq!(config.display_name, "Custom Identity Provider");
        assert_eq!(config.issuer, "https://identity.example.com");
        assert_eq!(config.client_id, "client-123");
        assert_eq!(config.client_secret, Some("secret-456".to_string()));
    }

    #[test]
    fn test_claim_normalization_google() {
        let mappings = ClaimMappings::google();

        let claims = serde_json::json!({
            "sub": "google-user-123",
            "email": "user@gmail.com",
            "name": "Test User",
            "picture": "https://example.com/photo.jpg"
        });

        let normalized = mappings.normalize_claims(&claims);

        assert_eq!(
            normalized.get("sub").and_then(|v| v.as_str()),
            Some("google-user-123")
        );
        assert_eq!(
            normalized.get("email").and_then(|v| v.as_str()),
            Some("user@gmail.com")
        );
        assert_eq!(
            normalized.get("name").and_then(|v| v.as_str()),
            Some("Test User")
        );
    }

    #[test]
    fn test_claim_normalization_entra() {
        let mappings = ClaimMappings::entra();

        let claims = serde_json::json!({
            "oid": "entra-user-456",
            "tid": "tenant-789",
            "preferred_username": "user@contoso.com",
            "name": "Enterprise User",
            "groups": ["group1", "group2"]
        });

        let normalized = mappings.normalize_claims(&claims);

        // oid should be mapped to sub
        assert_eq!(
            normalized.get("sub").and_then(|v| v.as_str()),
            Some("entra-user-456")
        );
        // tid should be mapped to tenant_id
        assert_eq!(
            normalized.get("tenant_id").and_then(|v| v.as_str()),
            Some("tenant-789")
        );
        // preferred_username should be mapped to email
        assert_eq!(
            normalized.get("email").and_then(|v| v.as_str()),
            Some("user@contoso.com")
        );
        // groups should be normalized
        assert!(normalized.contains_key("groups"));
    }

    #[test]
    fn test_claim_normalization_auth0() {
        let mappings = ClaimMappings::auth0();

        let claims = serde_json::json!({
            "sub": "auth0|user123",
            "org_id": "org_ABC123",
            "email": "user@example.com",
            "roles": ["admin", "user"]
        });

        let normalized = mappings.normalize_claims(&claims);

        assert_eq!(
            normalized.get("sub").and_then(|v| v.as_str()),
            Some("auth0|user123")
        );
        assert_eq!(
            normalized.get("tenant_id").and_then(|v| v.as_str()),
            Some("org_ABC123")
        );
        assert!(normalized.contains_key("groups"));
    }
}
