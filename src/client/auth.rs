//! Authentication helpers for MCP clients.
//!
//! This module provides utilities for handling OAuth 2.0/OIDC authentication
//! in MCP clients, including discovery and token management.
//!
//! # Discovery is an ordered probe with a validated anchor
//!
//! `OidcDiscoveryClient` implements the MCP specification's ordered
//! authorization-server metadata probe (SEP-2351) rather than a single
//! constructed URL, and validates every document it retrieves against the
//! issuer the URL was built from (RFC 8414 §3.3 / `OpenID` Connect Discovery
//! §4.3) **before** the metadata leaves the fetch function. That second half is
//! what makes the RFC 9207 `iss` check downstream meaningful: without it the
//! comparison is anchored on a value the served document chose for itself.
//!
//! Every whole-body read in this module is bounded, on success AND on error
//! paths — a hostile authorization server controls its error bodies too.

use crate::error::{Error, ErrorCode, Result};
use crate::server::auth::oauth2::OidcDiscoveryMetadata;
use crate::shared::http_body_cap::{
    collect_reqwest_body_within_cap, hardened_discovery_client, is_body_over_cap,
    is_redirect_refusal, DEFAULT_AUTH_RESPONSE_BYTES,
};
use crate::shared::oauth_validation::{
    classify_discovery_failure, discovery_url_candidates, issuer_matches_metadata,
    DiscoveryFailure, DiscoveryOutcome,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

/// How long a single discovery request may take.
///
/// `with_settings` configures the RETRY budget, not a timeout, so both
/// constructors use this value. Without it a hostile or hung authorization
/// server holds the probe open indefinitely.
const DEFAULT_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// How much of a peer-supplied issuer string a refusal may reproduce.
///
/// The expected issuer has already passed `validate_issuer_url`, but the
/// DOCUMENT's issuer is arbitrary attacker-chosen text and the refusal names it
/// because RFC 8414 §3.3's whole value is telling an operator which two values
/// disagreed. Truncating it stops a megabyte-long "issuer" from flooding a log.
const MAX_ECHOED_DOCUMENT_ISSUER: usize = 256;

/// What a discovery probe learned about the authorization server BEYOND the
/// fields [`OidcDiscoveryMetadata`] carries.
///
/// # Why this is a separate type
///
/// [`OidcDiscoveryMetadata`] is public, has all-public fields and is **not**
/// `#[non_exhaustive]`, so adding a field to it trips
/// `constructible_struct_adds_field` — a MAJOR semver break for every downstream
/// struct-literal construction, of which this repository alone has two. A new
/// sibling type and a new method are both semver-minor.
///
/// `#[non_exhaustive]` with private fields, so later discovery-derived values
/// can join it without another break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AuthorizationServerExtras {
    iss_parameter_supported: Option<bool>,
}

impl AuthorizationServerExtras {
    /// Whether the authorization server advertises RFC 9207
    /// `authorization_response_iss_parameter_supported`.
    ///
    /// `None` means the key was ABSENT, which is legal and means "not
    /// advertised". A key that is present but not a JSON boolean never reaches
    /// here: it aborts discovery as malformed security metadata, because
    /// reporting it as `None` would relax the very strictness the authorization
    /// server was trying to declare.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::client::auth::OidcDiscoveryClient;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let client = OidcDiscoveryClient::new();
    /// let (metadata, extras) = client.discover_with_extras("https://auth.example.com").await?;
    /// if extras.iss_parameter_supported() == Some(true) {
    ///     // The authorization server promises an `iss` on every callback.
    /// }
    /// # let _ = metadata;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub const fn iss_parameter_supported(&self) -> Option<bool> {
        self.iss_parameter_supported
    }
}

/// OIDC discovery client for fetching server configuration.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::client::auth::OidcDiscoveryClient;
/// use std::time::Duration;
///
/// # async fn example() -> pmcp::Result<()> {
/// // Create a default client
/// let client = OidcDiscoveryClient::new();
///
/// // Or create with custom retry settings
/// let custom_client = OidcDiscoveryClient::with_settings(
///     5,  // max retries
///     Duration::from_secs(1)  // retry delay
/// );
///
/// // Discover OIDC configuration
/// let metadata = client.discover("https://auth.example.com").await?;
/// println!("Authorization endpoint: {}", metadata.authorization_endpoint);
/// println!("Token endpoint: {}", metadata.token_endpoint);
/// # Ok(())
/// # }
/// ```
///
/// ## Retry Behavior
///
/// ```rust
/// use pmcp::client::auth::OidcDiscoveryClient;
/// use std::time::Duration;
///
/// // Create client with specific retry behavior
/// let client = OidcDiscoveryClient::with_settings(
///     0,  // No retries
///     Duration::from_secs(0)  // No delay
/// );
///
/// // Client with aggressive retries
/// let aggressive = OidcDiscoveryClient::with_settings(
///     10,  // Many retries
///     Duration::from_millis(100)  // Short delay
/// );
/// ```
#[derive(Debug)]
pub struct OidcDiscoveryClient {
    /// HTTP client for making requests, or the reason one could not be built.
    ///
    /// `new` and `with_settings` cannot return `Result` without a MAJOR semver
    /// break, and falling back to a default client would silently DISCARD the
    /// redirect policy discovery depends on. So the failure is carried here and
    /// surfaced by the first `discover` call instead.
    client: std::result::Result<reqwest::Client, String>,
    /// Maximum number of attempts against a single candidate before falling
    /// back to the next one.
    max_retries: usize,
    /// Delay between retry attempts.
    retry_delay: Duration,
    /// The candidate INDEX that last succeeded, per issuer.
    ///
    /// A path-bearing issuer such as
    /// `https://login.microsoftonline.com/common/v2.0` otherwise pays two 404s
    /// on every probe. Three constraints make the shortcut safe, and all three
    /// are asserted by tests:
    ///
    /// - it holds an INDEX, never a document, so a poisoned entry cannot supply
    ///   metadata;
    /// - a cache-hit attempt that fails restarts the FULL ordered sequence from
    ///   candidate 0 rather than continuing from the next index, so a stale or
    ///   hostile entry cannot pin discovery to one path;
    /// - a cache hit runs the SAME anchor validation as a cold probe. The cache
    ///   short-circuits URL CHOICE, never trust.
    candidate_cache: RwLock<HashMap<String, usize>>,
}

impl Default for OidcDiscoveryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OidcDiscoveryClient {
    /// Create a new OIDC discovery client with default settings.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::client::auth::OidcDiscoveryClient;
    ///
    /// let client = OidcDiscoveryClient::new();
    /// // Client is configured with:
    /// // - max_retries: 3
    /// // - retry_delay: 500ms
    /// ```
    pub fn new() -> Self {
        Self::with_settings(3, Duration::from_millis(500))
    }

    /// Create a new OIDC discovery client with custom settings.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::client::auth::OidcDiscoveryClient;
    /// use std::time::Duration;
    ///
    /// // More aggressive retry strategy
    /// let client = OidcDiscoveryClient::with_settings(
    ///     10,  // Try up to 10 times
    ///     Duration::from_millis(200)  // 200ms between retries
    /// );
    /// ```
    pub fn with_settings(max_retries: usize, retry_delay: Duration) -> Self {
        Self {
            client: hardened_discovery_client(DEFAULT_DISCOVERY_TIMEOUT).map_err(|e| e.to_string()),
            max_retries,
            retry_delay,
            candidate_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Discover OIDC configuration from an issuer URL.
    ///
    /// Probes the specification's ordered candidate endpoints and validates the
    /// retrieved document's `issuer` against `issuer_url` before returning.
    /// Availability failures fall back to the next candidate (5xx and transport
    /// failures are retried first); a document that arrives and cannot be
    /// trusted aborts the probe outright.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::client::auth::OidcDiscoveryClient;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let client = OidcDiscoveryClient::new();
    ///
    /// // Discover from various providers
    /// let google = client.discover("https://accounts.google.com").await?;
    /// let microsoft = client.discover("https://login.microsoftonline.com/common/v2.0").await?;
    ///
    /// // URL normalization - trailing slashes are handled
    /// let metadata1 = client.discover("https://auth.example.com").await?;
    /// let metadata2 = client.discover("https://auth.example.com/").await?;
    /// // Both derive the same ordered candidate list
    /// # Ok(())
    /// # }
    /// ```
    pub async fn discover(&self, issuer_url: &str) -> Result<OidcDiscoveryMetadata> {
        self.discover_with_extras(issuer_url)
            .await
            .map(|(metadata, _)| metadata)
    }

    /// Discover OIDC configuration and the discovery-only values that do not fit
    /// on [`OidcDiscoveryMetadata`].
    ///
    /// Identical to [`Self::discover`] in every respect except that it also
    /// returns [`AuthorizationServerExtras`]. See that type for why the RFC 9207
    /// flag cannot simply become a field on the metadata struct.
    ///
    /// # Errors
    ///
    /// Returns an error when the issuer itself is unusable, when no candidate
    /// endpoint served a usable document (the message enumerates every candidate
    /// tried), or when a document ARRIVED and cannot be trusted — a mismatched
    /// `issuer`, a body over the read cap, or malformed security metadata. The
    /// last three abort the probe rather than falling through to a later
    /// candidate.
    pub async fn discover_with_extras(
        &self,
        issuer_url: &str,
    ) -> Result<(OidcDiscoveryMetadata, AuthorizationServerExtras)> {
        let candidates = discovery_url_candidates(issuer_url)?;
        let client = self.http_client()?;
        let mut attempted: Vec<String> = Vec::new();

        for index in self.probe_order(issuer_url, candidates.len()) {
            let url = &candidates[index];
            match self.probe_candidate(client, url, issuer_url).await {
                Ok(found) => {
                    self.remember_candidate(issuer_url, index);
                    return Ok(found);
                },
                // A TERMINAL outcome aborts the WHOLE discovery. Falling through
                // here would turn a security failure into a silent DOWNGRADE: an
                // attacker who can make one candidate fail in a security-relevant
                // way would steer the client onto a candidate they serve. Do NOT
                // "simplify" this back into `if !ok { continue }`.
                Err((DiscoveryOutcome::Terminal, error)) => return Err(error),
                Err((_, error)) => attempted.push(format!("{url}: {error}")),
            }
        }

        Err(every_candidate_failed(issuer_url, &attempted))
    }

    /// The HTTP client, or the recorded reason there is none.
    fn http_client(&self) -> Result<&reqwest::Client> {
        self.client
            .as_ref()
            .map_err(|message| Error::internal(message.clone()))
    }

    /// The candidate indices to try, in order.
    ///
    /// A remembered index goes FIRST and the full ordered sequence follows, so a
    /// cache hit that fails restarts from candidate 0 rather than continuing
    /// from the next index.
    fn probe_order(&self, issuer_url: &str, candidate_count: usize) -> Vec<usize> {
        let remembered = self
            .candidate_cache
            .read()
            .get(issuer_url)
            .copied()
            .filter(|index| *index < candidate_count);

        remembered.map_or_else(
            || (0..candidate_count).collect(),
            |first| {
                std::iter::once(first)
                    .chain((0..candidate_count).filter(move |index| *index != first))
                    .collect()
            },
        )
    }

    /// Remember the candidate index that served a usable document.
    fn remember_candidate(&self, issuer_url: &str, index: usize) {
        self.candidate_cache
            .write()
            .insert(issuer_url.to_owned(), index);
    }

    /// Probe ONE candidate, applying the shared outcome matrix.
    ///
    /// `Retry` re-attempts this candidate within the `max_retries` budget and
    /// becomes `Fallback` once that budget is spent; `Terminal` propagates
    /// unchanged so the caller aborts.
    async fn probe_candidate(
        &self,
        client: &reqwest::Client,
        url: &Url,
        expected_issuer: &str,
    ) -> std::result::Result<
        (OidcDiscoveryMetadata, AuthorizationServerExtras),
        (DiscoveryOutcome, Error),
    > {
        let mut attempts: usize = 0;
        loop {
            let (failure, error) = match fetch_discovery(client, url, expected_issuer).await {
                Ok(found) => return Ok(found),
                Err(pair) => pair,
            };

            match classify_discovery_failure(failure) {
                DiscoveryOutcome::Terminal => return Err((DiscoveryOutcome::Terminal, error)),
                DiscoveryOutcome::Fallback => return Err((DiscoveryOutcome::Fallback, error)),
                DiscoveryOutcome::Retry => {
                    attempts += 1;
                    if attempts >= self.max_retries {
                        return Err((DiscoveryOutcome::Fallback, error));
                    }
                    tokio::time::sleep(self.retry_delay).await;
                },
            }
        }
    }
}

/// Fetch and VALIDATE a discovery document from one candidate URL.
///
/// The document is read through the bounded reader, its `issuer` is compared
/// against `expected_issuer` per RFC 8414 §3.3, and its RFC 9207 flag is typed —
/// all before any metadata leaves this function. `expected_issuer` is the
/// ISSUER, not the candidate URL: every candidate for one issuer shares the same
/// expected value.
async fn fetch_discovery(
    client: &reqwest::Client,
    url: &Url,
    expected_issuer: &str,
) -> std::result::Result<
    (OidcDiscoveryMetadata, AuthorizationServerExtras),
    (DiscoveryFailure, Error),
> {
    let response = client
        .get(url.as_str())
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| request_failure(url, &e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(status_failure(url, status));
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

    let document: Value =
        serde_json::from_slice(&bytes).map_err(|e| unparseable_document(url, &e))?;

    // RFC 8414 section 3.3 / OpenID Connect Discovery section 4.3, BEFORE the
    // metadata escapes this function.
    let document_issuer = document_issuer_field(url, &document)?;
    if !issuer_matches_metadata(expected_issuer, document_issuer) {
        return Err(issuer_mismatch(url, expected_issuer, document_issuer));
    }

    let iss_parameter_supported = iss_parameter_flag(url, &document)?;

    let metadata: OidcDiscoveryMetadata =
        serde_json::from_slice(&bytes).map_err(|e| unparseable_document(url, &e))?;

    Ok((
        metadata,
        AuthorizationServerExtras {
            iss_parameter_supported,
        },
    ))
}

/// The document's `issuer`, which MUST be present and MUST be a string.
///
/// It is the value RFC 9207's `iss` comparison is anchored on, so neither
/// absence nor a wrong type may be tolerated into a `None`.
fn document_issuer_field<'a>(
    url: &Url,
    document: &'a Value,
) -> std::result::Result<&'a str, (DiscoveryFailure, Error)> {
    match document.get("issuer") {
        Some(Value::String(issuer)) => Ok(issuer),
        Some(_) => Err(malformed_security_metadata(
            url,
            "`issuer` is present but is not a JSON string. It is the value RFC 9207's `iss` \
             comparison is anchored on, so a wrongly-typed issuer cannot be tolerated",
        )),
        None => Err(malformed_security_metadata(
            url,
            "`issuer` is absent. RFC 8414 section 3.3 requires it, and it is the value RFC 9207's \
             `iss` comparison is anchored on",
        )),
    }
}

/// The RFC 9207 `authorization_response_iss_parameter_supported` flag.
///
/// Absent is legal and means "not advertised". Present-but-not-a-boolean is
/// malformed security metadata and aborts discovery: reading it as `None` would
/// make an ABSENT `iss` acceptable, so a broken or hostile value would silently
/// RELAX the strictness the authorization server was declaring.
fn iss_parameter_flag(
    url: &Url,
    document: &Value,
) -> std::result::Result<Option<bool>, (DiscoveryFailure, Error)> {
    match document.get("authorization_response_iss_parameter_supported") {
        None => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(malformed_security_metadata(
            url,
            "`authorization_response_iss_parameter_supported` is present but is not a JSON \
             boolean. Treating it as absent would relax strictness — an absent `iss` on the \
             callback would become acceptable — so a malformed value aborts discovery instead",
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
fn request_failure(url: &Url, source: &reqwest::Error) -> (DiscoveryFailure, Error) {
    let failure = if is_redirect_refusal(source) {
        DiscoveryFailure::MalformedSecurityMetadata
    } else {
        DiscoveryFailure::Transport
    };
    (
        failure,
        Error::protocol(
            ErrorCode::INTERNAL_ERROR,
            format!(
                "Failed to fetch discovery document from {url}: {}",
                rendered_source_chain(source)
            ),
        ),
    )
}

/// Classify a discovery response whose HTTP status is not a success.
fn status_failure(url: &Url, status: reqwest::StatusCode) -> (DiscoveryFailure, Error) {
    let failure = if status == reqwest::StatusCode::NOT_FOUND {
        DiscoveryFailure::NotFound
    } else {
        DiscoveryFailure::HttpStatus(status.as_u16())
    };
    (
        failure,
        Error::protocol(
            ErrorCode::INTERNAL_ERROR,
            format!("Discovery endpoint {url} returned status: {status}"),
        ),
    )
}

/// The refusal for a body that is not the JSON this endpoint serves.
///
/// Carries `serde_json`'s CLASSIFICATION plus line and column, never its
/// message: a `serde_json` data error reproduces the offending value, and this
/// body is peer-controlled.
fn unparseable_document(url: &Url, source: &serde_json::Error) -> (DiscoveryFailure, Error) {
    (
        DiscoveryFailure::InvalidJson,
        Error::protocol(
            ErrorCode::PARSE_ERROR,
            format!(
                "Discovery document from {url} is not the JSON document this endpoint serves \
                 ({:?} error at line {}, column {}). The parser's own message is not reproduced \
                 here because a data error echoes the offending input",
                source.classify(),
                source.line(),
                source.column()
            ),
        ),
    )
}

/// The RFC 8414 §3.3 refusal, naming BOTH issuers.
fn issuer_mismatch(
    url: &Url,
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
fn malformed_security_metadata(url: &Url, detail: &str) -> (DiscoveryFailure, Error) {
    (
        DiscoveryFailure::MalformedSecurityMetadata,
        Error::protocol(
            ErrorCode::INVALID_REQUEST,
            format!("Discovery document from {url} carries malformed security metadata: {detail}"),
        ),
    )
}

/// The refusal when the ordered probe is exhausted, enumerating every candidate.
fn every_candidate_failed(issuer_url: &str, attempted: &[String]) -> Error {
    Error::protocol(
        ErrorCode::INTERNAL_ERROR,
        format!(
            "Failed to discover OIDC configuration for issuer `{issuer_url}`. Every candidate \
             endpoint was tried and none served a usable document:\n  - {}",
            attempted.join("\n  - ")
        ),
    )
}

/// Bound a peer-controlled string before it reaches an error message.
fn truncate_for_message(value: &str) -> String {
    if value.chars().count() <= MAX_ECHOED_DOCUMENT_ISSUER {
        return value.to_owned();
    }
    let head: String = value.chars().take(MAX_ECHOED_DOCUMENT_ISSUER).collect();
    format!("{head}… (truncated)")
}

/// Render an error together with its whole source chain.
///
/// `reqwest::Error`'s own `Display` omits the source, which is where a custom
/// redirect refusal's explanation lives.
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

/// OAuth 2.0 token response.
///
/// # Examples
///
/// ```rust
/// use pmcp::client::auth::TokenResponse;
///
/// // Parse a token response from JSON
/// let json = r#"{
///     "access_token": "eyJhbGciOiJSUzI1NiIs...",
///     "token_type": "Bearer",
///     "expires_in": 3600,
///     "refresh_token": "8xLOxBtZp8",
///     "scope": "openid profile email"
/// }"#;
///
/// let response: TokenResponse = serde_json::from_str(json).unwrap();
/// assert_eq!(response.token_type, "Bearer");
/// assert_eq!(response.expires_in, Some(3600));
///
/// // Create a token response
/// let token = TokenResponse {
///     access_token: "token123".to_string(),
///     token_type: "Bearer".to_string(),
///     expires_in: Some(7200),
///     refresh_token: None,
///     scope: Some("read write".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// Access token.
    pub access_token: String,

    /// Token type (usually "Bearer").
    pub token_type: String,

    /// Token expiration time in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,

    /// Refresh token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Granted scopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// OAuth 2.0 token exchange client.
#[derive(Debug)]
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::client::auth::TokenExchangeClient;
///
/// # async fn example() -> pmcp::Result<()> {
/// let client = TokenExchangeClient::new();
///
/// // Exchange authorization code for tokens
/// let tokens = client.exchange_code(
///     "https://auth.example.com/token",
///     "auth_code_123",
///     "client_id",
///     Some("client_secret"),
///     "https://app.example.com/callback",
///     None,  // No PKCE verifier
/// ).await?;
///
/// println!("Access token: {}", tokens.access_token);
///
/// // Refresh an access token
/// if let Some(refresh_token) = tokens.refresh_token {
///     let new_tokens = client.refresh_token(
///         "https://auth.example.com/token",
///         &refresh_token,
///         "client_id",
///         Some("client_secret"),
///         None,  // Keep same scope
///     ).await?;
///     println!("New access token: {}", new_tokens.access_token);
/// }
/// # Ok(())
/// # }
/// ```
pub struct TokenExchangeClient {
    /// HTTP client for making requests.
    client: reqwest::Client,
}

impl Default for TokenExchangeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenExchangeClient {
    /// Create a new token exchange client.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Exchange an authorization code for tokens.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, when the token endpoint answers
    /// with a non-success status (its error body is read through the same cap as
    /// a success body — a hostile authorization server controls both), or when
    /// the response is not a parsable token response.
    pub async fn exchange_code(
        &self,
        token_endpoint: &str,
        code: &str,
        client_id: &str,
        client_secret: Option<&str>,
        redirect_uri: &str,
        code_verifier: Option<&str>,
    ) -> Result<TokenResponse> {
        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
        ];

        if let Some(verifier) = code_verifier {
            params.push(("code_verifier", verifier));
        }

        let mut request = self
            .client
            .post(token_endpoint)
            .header("Accept", "application/json") // Explicitly set Accept header
            .form(&params);

        // Add client authentication if secret is provided
        if let Some(secret) = client_secret {
            request = request.basic_auth(client_id, Some(secret));
        }

        let response = request.send().await.map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to exchange authorization code: {}", e),
            )
        })?;

        if !response.status().is_success() {
            let error_text = read_error_body_within_cap(response).await;
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token exchange failed: {}", error_text),
            ));
        }

        parse_token_response(&read_token_body(response, "token exchange").await?)
    }

    /// Refresh an access token using a refresh token.
    ///
    /// # Errors
    ///
    /// Same failure modes as [`Self::exchange_code`].
    pub async fn refresh_token(
        &self,
        token_endpoint: &str,
        refresh_token: &str,
        client_id: &str,
        client_secret: Option<&str>,
        scope: Option<&str>,
    ) -> Result<TokenResponse> {
        let mut params = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ];

        if let Some(s) = scope {
            params.push(("scope", s));
        }

        let mut request = self
            .client
            .post(token_endpoint)
            .header("Accept", "application/json") // Explicitly set Accept header
            .form(&params);

        // Add client authentication if secret is provided
        if let Some(secret) = client_secret {
            request = request.basic_auth(client_id, Some(secret));
        }

        let response = request.send().await.map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to refresh token: {}", e),
            )
        })?;

        if !response.status().is_success() {
            let error_text = read_error_body_within_cap(response).await;
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token refresh failed: {}", error_text),
            ));
        }

        parse_token_response(&read_token_body(response, "token refresh").await?)
    }
}

/// Read a token-endpoint SUCCESS body, bounded.
async fn read_token_body(response: reqwest::Response, what: &str) -> Result<Vec<u8>> {
    collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES)
        .await
        .map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to read {what} response body: {e}"),
            )
        })
}

/// Read a token-endpoint ERROR body, bounded.
///
/// Error paths matter as much as success paths: a hostile authorization server
/// controls its error bodies too, and this one is interpolated into a message.
async fn read_error_body_within_cap(response: reqwest::Response) -> String {
    match collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) => format!("<error body not read: {e}>"),
    }
}

/// Parse a token response, without reproducing the offending input.
fn parse_token_response(bytes: &[u8]) -> Result<TokenResponse> {
    serde_json::from_slice::<TokenResponse>(bytes).map_err(|e| {
        Error::protocol(
            ErrorCode::PARSE_ERROR,
            format!(
                "Failed to parse token response ({:?} error at line {}, column {}). The parser's \
                 own message is not reproduced here because a data error echoes the offending \
                 input, and a token response body carries credentials",
                e.classify(),
                e.line(),
                e.column()
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_discovery_url_construction() {
        // SEP-2351: discovery is an ORDERED probe, not a single constructed URL.
        // The third case below was this SDK's ONLY form before the probe landed;
        // it is not wrong, it is the LAST candidate — and RESEARCH Pitfall 2
        // measured it as the only form Microsoft Entra ID answers with 200.
        let test_cases: Vec<(&str, Vec<&str>)> = vec![
            (
                "https://example.com",
                vec![
                    "https://example.com/.well-known/oauth-authorization-server",
                    "https://example.com/.well-known/openid-configuration",
                ],
            ),
            (
                "https://example.com/",
                vec![
                    "https://example.com/.well-known/oauth-authorization-server",
                    "https://example.com/.well-known/openid-configuration",
                ],
            ),
            (
                "https://auth.example.com/oauth",
                vec![
                    "https://auth.example.com/.well-known/oauth-authorization-server/oauth",
                    "https://auth.example.com/.well-known/openid-configuration/oauth",
                    "https://auth.example.com/oauth/.well-known/openid-configuration",
                ],
            ),
        ];

        for (issuer, expected) in test_cases {
            let rendered: Vec<String> = discovery_url_candidates(issuer)
                .unwrap()
                .iter()
                .map(std::string::ToString::to_string)
                .collect();
            assert_eq!(rendered, expected, "issuer {issuer}");
        }
    }

    #[test]
    fn test_failure_classification_replaces_the_old_string_sniffing_retry() {
        let url = Url::parse("https://as.example/.well-known/openid-configuration").unwrap();

        // 404 is the ordinary "not this form" answer the ordered probe is FOR.
        let (failure, _) = status_failure(&url, reqwest::StatusCode::NOT_FOUND);
        assert_eq!(failure, DiscoveryFailure::NotFound);
        assert_eq!(
            classify_discovery_failure(failure),
            DiscoveryOutcome::Fallback
        );

        // 5xx: the endpoint is the right one and is temporarily unwell.
        let (failure, _) = status_failure(&url, reqwest::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(failure, DiscoveryFailure::HttpStatus(503));
        assert_eq!(classify_discovery_failure(failure), DiscoveryOutcome::Retry);

        // Another 4xx: the endpoint answered and refused; another form may serve.
        let (failure, _) = status_failure(&url, reqwest::StatusCode::UNAUTHORIZED);
        assert_eq!(failure, DiscoveryFailure::HttpStatus(401));
        assert_eq!(
            classify_discovery_failure(failure),
            DiscoveryOutcome::Fallback
        );
    }

    #[test]
    fn test_document_issuer_field_rows() {
        let url = Url::parse("https://as.example/.well-known/openid-configuration").unwrap();

        let good = json!({ "issuer": "https://as.example" });
        assert_eq!(
            document_issuer_field(&url, &good).unwrap(),
            "https://as.example"
        );

        for hostile in [json!({}), json!({ "issuer": 7 }), json!({ "issuer": null })] {
            let (failure, error) = document_issuer_field(&url, &hostile).unwrap_err();
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
    fn test_iss_parameter_flag_rows() {
        let url = Url::parse("https://as.example/.well-known/openid-configuration").unwrap();
        let key = "authorization_response_iss_parameter_supported";

        // Absence is LEGAL and means "not advertised".
        assert_eq!(iss_parameter_flag(&url, &json!({})).unwrap(), None);
        assert_eq!(
            iss_parameter_flag(&url, &json!({ key: true })).unwrap(),
            Some(true)
        );
        assert_eq!(
            iss_parameter_flag(&url, &json!({ key: false })).unwrap(),
            Some(false)
        );

        // Present but not a boolean: rejected, never quietly `None`, because
        // `None` reads as "optional" and would make an ABSENT callback `iss`
        // acceptable.
        for hostile in [json!("true"), json!(1), json!(null), json!({})] {
            let document = json!({ key: hostile });
            let (failure, _) = iss_parameter_flag(&url, &document).unwrap_err();
            assert_eq!(
                failure,
                DiscoveryFailure::MalformedSecurityMetadata,
                "flag value {hostile} must abort rather than read as None"
            );
            assert_eq!(
                classify_discovery_failure(failure),
                DiscoveryOutcome::Terminal
            );
        }
    }

    #[test]
    fn test_issuer_mismatch_names_both_values_and_bounds_the_peer_one() {
        let url =
            Url::parse("https://attacker.example/.well-known/oauth-authorization-server").unwrap();
        let (failure, error) =
            issuer_mismatch(&url, "https://attacker.example", "https://honest.example");
        assert_eq!(failure, DiscoveryFailure::IssuerMismatch);
        let message = error.to_string();
        assert!(message.contains("https://attacker.example"));
        assert!(message.contains("https://honest.example"));

        let flood = "z".repeat(10_000);
        let (_, error) = issuer_mismatch(&url, "https://as.example", &flood);
        assert!(
            error.to_string().len() < 2_000,
            "a peer-chosen issuer must not flood the message"
        );
    }

    #[test]
    fn test_probe_order_puts_a_remembered_candidate_first_then_the_full_sequence() {
        let client = OidcDiscoveryClient::new();
        assert_eq!(client.probe_order("https://as.example", 3), vec![0, 1, 2]);

        client.remember_candidate("https://as.example", 2);
        assert_eq!(client.probe_order("https://as.example", 3), vec![2, 0, 1]);

        // A remembered index that no longer exists is ignored rather than
        // panicking or pinning the probe.
        client.remember_candidate("https://as.example", 2);
        assert_eq!(client.probe_order("https://as.example", 2), vec![0, 1]);
    }

    #[test]
    fn test_discovery_client_with_settings() {
        let client = OidcDiscoveryClient::with_settings(5, Duration::from_secs(2));
        assert_eq!(client.max_retries, 5);
        assert_eq!(client.retry_delay, Duration::from_secs(2));
        assert!(client.client.is_ok(), "the hardened client must build");
    }

    #[test]
    fn test_authorization_server_extras_is_read_only_and_optional() {
        let extras = AuthorizationServerExtras {
            iss_parameter_supported: Some(true),
        };
        assert_eq!(extras.iss_parameter_supported(), Some(true));

        let unadvertised = AuthorizationServerExtras {
            iss_parameter_supported: None,
        };
        assert_eq!(unadvertised.iss_parameter_supported(), None);
    }

    #[test]
    fn test_token_response_serialization() {
        let token_response = TokenResponse {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("refresh_token".to_string()),
            scope: Some("openid profile".to_string()),
        };

        // Test serialization
        let json = serde_json::to_string(&token_response).unwrap();
        assert!(json.contains("test_token"));
        assert!(json.contains("Bearer"));

        // Test deserialization
        let deserialized: TokenResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.access_token, "test_token");
        assert_eq!(deserialized.expires_in, Some(3600));
    }

    #[test]
    fn test_token_response_parse_failure_echoes_no_input() {
        let error = parse_token_response(br#"{"access_token": 7}"#).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("line 1"), "{message}");
        assert!(
            !message.contains("access_token"),
            "a token body carries credentials; the parser message must not be reproduced: \
             {message}"
        );
    }

    #[test]
    fn test_oidc_discovery_metadata_defaults() {
        let json = r#"{
            "issuer": "https://auth.example.com",
            "authorization_endpoint": "https://auth.example.com/authorize",
            "token_endpoint": "https://auth.example.com/token",
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code"],
            "scopes_supported": ["openid", "profile"],
            "token_endpoint_auth_methods_supported": ["client_secret_basic"],
            "code_challenge_methods_supported": ["S256"]
        }"#;

        let metadata: OidcDiscoveryMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.issuer, "https://auth.example.com");
        assert_eq!(metadata.jwks_uri, None);
        assert_eq!(metadata.userinfo_endpoint, None);
    }
}
