//! Identity provider plugin interface.
//!
//! This module defines the `IdentityProvider` trait for integrating with external
//! OAuth/OIDC identity providers like Google, Auth0, Cognito, Azure AD, Okta, etc.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         PMCP SDK                                         │
//! │  ┌─────────────────────────────────────────────────────────────────────┐│
//! │  │                    IdentityProvider Trait                           ││
//! │  │  (Plugin interface for external identity providers)                 ││
//! │  └─────────────────────────────────────────────────────────────────────┘│
//! │                              │                                           │
//! │         ┌────────────────────┼────────────────────┐                     │
//! │         │                    │                    │                     │
//! │         ▼                    ▼                    ▼                     │
//! │  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐            │
//! │  │   Cognito   │      │ GenericOidc │      │   NoAuth    │            │
//! │  │  Provider   │      │  Provider   │      │  Provider   │            │
//! │  └─────────────┘      └─────────────┘      └─────────────┘            │
//! │                              │                                           │
//! │                    Works with any OIDC provider:                        │
//! │                    Google, Auth0, Azure AD, Okta...                     │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Plugin Development
//!
//! Third-party providers (Google, Microsoft, Auth0) can implement this trait
//! in separate crates (e.g., `pmcp-auth-google`, `pmcp-auth-auth0`).
//!
//! # Example
//!
//! ```rust,ignore
//! use pmcp::server::auth::provider::{IdentityProvider, ProviderCapabilities};
//!
//! // Use the generic OIDC provider for any compliant provider
//! let google = GenericOidcProvider::new(
//!     "google",
//!     "https://accounts.google.com",
//!     "your-client-id",
//! );
//!
//! // Validate a token
//! let claims = google.validate_token("eyJ...").await?;
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

use crate::error::Result;
use crate::server::auth::AuthContext;

/// Capabilities supported by an identity provider.
///
/// Different providers support different OAuth/OIDC features.
/// This struct allows runtime capability discovery.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProviderCapabilities {
    /// Supports `OpenID` Connect (OIDC) protocol.
    #[serde(default)]
    pub oidc: bool,

    /// Supports Dynamic Client Registration (RFC 7591).
    #[serde(default)]
    pub dcr: bool,

    /// Supports PKCE (RFC 7636) for public clients.
    #[serde(default)]
    pub pkce: bool,

    /// Supports refresh tokens.
    #[serde(default)]
    pub refresh_tokens: bool,

    /// Supports token revocation (RFC 7009).
    #[serde(default)]
    pub revocation: bool,

    /// Supports token introspection (RFC 7662).
    #[serde(default)]
    pub introspection: bool,

    /// Supports custom scopes beyond standard OIDC scopes.
    #[serde(default)]
    pub custom_scopes: bool,

    /// Supports device authorization grant (RFC 8628).
    #[serde(default)]
    pub device_flow: bool,
}

impl ProviderCapabilities {
    /// Create capabilities for a full-featured OIDC provider.
    pub fn full_oidc() -> Self {
        Self {
            oidc: true,
            dcr: true,
            pkce: true,
            refresh_tokens: true,
            revocation: true,
            introspection: true,
            custom_scopes: true,
            device_flow: false,
        }
    }

    /// Create capabilities for basic OIDC (no DCR).
    pub fn basic_oidc() -> Self {
        Self {
            oidc: true,
            dcr: false,
            pkce: true,
            refresh_tokens: true,
            revocation: false,
            introspection: false,
            custom_scopes: false,
            device_flow: false,
        }
    }
}

/// OIDC Discovery document (from .well-known/openid-configuration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscovery {
    /// Issuer identifier.
    pub issuer: String,

    /// Authorization endpoint URL.
    pub authorization_endpoint: String,

    /// Token endpoint URL.
    pub token_endpoint: String,

    /// `UserInfo` endpoint URL (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userinfo_endpoint: Option<String>,

    /// JWKS URI for token validation.
    pub jwks_uri: String,

    /// Registration endpoint for DCR (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_endpoint: Option<String>,

    /// Revocation endpoint (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revocation_endpoint: Option<String>,

    /// Introspection endpoint (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introspection_endpoint: Option<String>,

    /// End session/logout endpoint (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_session_endpoint: Option<String>,

    /// Supported scopes.
    #[serde(default)]
    pub scopes_supported: Vec<String>,

    /// Supported response types.
    #[serde(default)]
    pub response_types_supported: Vec<String>,

    /// Supported grant types.
    #[serde(default)]
    pub grant_types_supported: Vec<String>,

    /// Supported subject types.
    #[serde(default)]
    pub subject_types_supported: Vec<String>,

    /// Supported ID token signing algorithms.
    #[serde(default)]
    pub id_token_signing_alg_values_supported: Vec<String>,

    /// Supported token endpoint auth methods.
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Vec<String>,

    /// Supported claims.
    #[serde(default)]
    pub claims_supported: Vec<String>,

    /// Whether PKCE is supported.
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,

    /// Additional metadata.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Parameters for building an authorization URL.
#[derive(Debug, Clone)]
pub struct AuthorizationParams {
    /// Redirect URI for the callback.
    pub redirect_uri: String,

    /// Requested scopes.
    pub scopes: Vec<String>,

    /// State parameter for CSRF protection.
    pub state: String,

    /// Nonce for ID token validation (OIDC).
    pub nonce: Option<String>,

    /// PKCE code challenge.
    pub code_challenge: Option<String>,

    /// PKCE code challenge method (S256 or plain).
    pub code_challenge_method: Option<String>,

    /// Additional parameters.
    pub extra: HashMap<String, String>,
}

impl AuthorizationParams {
    /// Create new authorization params with required fields.
    pub fn new(redirect_uri: impl Into<String>, state: impl Into<String>) -> Self {
        Self {
            redirect_uri: redirect_uri.into(),
            scopes: vec!["openid".to_string()],
            state: state.into(),
            nonce: None,
            code_challenge: None,
            code_challenge_method: None,
            extra: HashMap::new(),
        }
    }

    /// Add scopes.
    pub fn with_scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Add PKCE challenge.
    pub fn with_pkce(mut self, challenge: impl Into<String>, method: impl Into<String>) -> Self {
        self.code_challenge = Some(challenge.into());
        self.code_challenge_method = Some(method.into());
        self
    }

    /// Add nonce for OIDC.
    pub fn with_nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = Some(nonce.into());
        self
    }
}

/// Parameters for token exchange.
#[derive(Debug, Clone)]
pub struct TokenExchangeParams {
    /// Authorization code.
    pub code: String,

    /// Redirect URI (must match authorization request).
    pub redirect_uri: String,

    /// PKCE code verifier.
    pub code_verifier: Option<String>,
}

/// Response from token exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// Access token.
    pub access_token: String,

    /// Token type (usually "Bearer").
    pub token_type: String,

    /// Token lifetime in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,

    /// Refresh token (if supported).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Granted scope (space-separated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// ID token (OIDC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,

    /// Additional response data.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The wire key OIDC Dynamic Client Registration uses for the application type (SEP-837).
///
/// Every read and write of that parameter in this module — [`DcrRequest::application_type`],
/// [`DcrRequest::set_application_type`] and [`DcrResponse::application_type`] — routes through
/// this one literal, so the reader and the writer cannot drift apart and no duplicate
/// `application_type` key can ever reach the wire.
///
/// It is exported precisely so a caller who insists on writing [`DcrRequest::extra`] by hand
/// cannot misspell the key. See [`DcrRequest::set_application_type`] for the precedence rule
/// that governs mixing the accessors with raw map writes.
pub const DCR_APPLICATION_TYPE_KEY: &str = "application_type";

/// Dynamic Client Registration request (RFC 7591).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcrRequest {
    /// Redirect URIs.
    pub redirect_uris: Vec<String>,

    /// Client name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,

    /// Client URI (homepage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_uri: Option<String>,

    /// Logo URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,

    /// Contacts (email addresses).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<String>,

    /// Token endpoint auth method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_method: Option<String>,

    /// Grant types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grant_types: Vec<String>,

    /// Response types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub response_types: Vec<String>,

    /// Requested scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// Software ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software_id: Option<String>,

    /// Software version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software_version: Option<String>,

    /// Additional metadata.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl DcrRequest {
    /// Read the OIDC `application_type` registration parameter (SEP-837).
    ///
    /// The value lives in the `#[serde(flatten)]` [`extra`](Self::extra) map under
    /// [`DCR_APPLICATION_TYPE_KEY`], so it round-trips through serialize/deserialize and
    /// lands as a **top-level** `"application_type"` key on the wire — byte-identical to
    /// what a declared serde field would emit, and reachable without a struct-literal
    /// change at any of this type's construction sites.
    ///
    /// Returns `None` when the parameter is absent **and** when the carrier holds a
    /// non-string JSON value: a deserialized registration request is untrusted input, so
    /// the accessor projects a JSON string or nothing at all. It never stringifies a
    /// number, an array or an object, and it never panics.
    ///
    /// See [`set_application_type`](Self::set_application_type) for the precedence rule
    /// when the accessor and a raw [`extra`](Self::extra) write are mixed.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::server::auth::provider::DcrRequest;
    ///
    /// let request: DcrRequest = serde_json::from_value(serde_json::json!({
    ///     "redirect_uris": ["http://127.0.0.1:8080/callback"],
    ///     "application_type": "native"
    /// }))?;
    /// assert_eq!(request.application_type(), Some("native"));
    ///
    /// // A non-string value is refused rather than coerced.
    /// let hostile: DcrRequest = serde_json::from_value(serde_json::json!({
    ///     "redirect_uris": ["http://127.0.0.1:8080/callback"],
    ///     "application_type": 42
    /// }))?;
    /// assert_eq!(hostile.application_type(), None);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    #[must_use]
    pub fn application_type(&self) -> Option<&str> {
        self.extra
            .get(DCR_APPLICATION_TYPE_KEY)
            .and_then(serde_json::Value::as_str)
    }

    /// Set the OIDC `application_type` registration parameter (SEP-837).
    ///
    /// # Values
    ///
    /// SEP-837 defines two. `"native"` covers desktop applications, mobile apps, CLI tools
    /// and locally-hosted web applications accessed via `localhost`; `"web"` covers remote
    /// browser-based applications served from a non-local host. **Omitting the parameter
    /// defaults to `web` under OIDC, which can conflict with native-style redirect URIs**;
    /// non-OIDC authorization servers safely ignore it.
    ///
    /// The value is deliberately **not** validated here: the spec permits an authorization
    /// server to define further values, and this setter is also the override path for a
    /// derived one.
    ///
    /// # Precedence
    ///
    /// This setter and a hand-written `extra.insert("application_type", …)` address the
    /// **same single map entry**. There is therefore no "typed versus raw" duality and no
    /// way to emit two `application_type` keys on the wire: **last write wins**, whichever
    /// route the write arrived through. Prefer this accessor; a caller who insists on
    /// writing the key directly should use [`DCR_APPLICATION_TYPE_KEY`] so it cannot be
    /// misspelled.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::server::auth::provider::{DcrRequest, DCR_APPLICATION_TYPE_KEY};
    ///
    /// let mut request: DcrRequest = serde_json::from_value(serde_json::json!({
    ///     "redirect_uris": ["http://127.0.0.1:8080/callback"]
    /// }))?;
    /// request.set_application_type("native");
    /// assert_eq!(request.application_type(), Some("native"));
    ///
    /// // The flattened carrier puts it at the top level, not under an `extra` object.
    /// let wire = serde_json::to_value(&request)?;
    /// assert_eq!(wire["application_type"], serde_json::json!("native"));
    ///
    /// // Last write wins, and the raw route addresses the very same entry.
    /// request
    ///     .extra
    ///     .insert(DCR_APPLICATION_TYPE_KEY.to_string(), serde_json::json!("web"));
    /// assert_eq!(request.application_type(), Some("web"));
    /// assert_eq!(request.extra.len(), 1);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn set_application_type(&mut self, value: impl Into<String>) {
        self.extra.insert(
            DCR_APPLICATION_TYPE_KEY.to_string(),
            serde_json::Value::String(value.into()),
        );
    }
}

/// Dynamic Client Registration response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcrResponse {
    /// Assigned client ID.
    pub client_id: String,

    /// Client secret (for confidential clients).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Client secret expiration (Unix timestamp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret_expires_at: Option<u64>,

    /// Registration access token (for client management).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_access_token: Option<String>,

    /// Registration client URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_client_uri: Option<String>,

    /// Token endpoint auth method assigned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_method: Option<String>,

    /// Additional response data (echoed from request).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl DcrResponse {
    /// Read the `application_type` the authorization server echoed back (SEP-837).
    ///
    /// Read-only by design: RFC 7591 § 3.2.1 explicitly permits the authorization server to
    /// modify any requested client metadata, so this value is the server's answer, not the
    /// client's request. Mutating it locally would misrepresent what was registered, which
    /// is why there is deliberately no setter here — the request-side counterpart is
    /// [`DcrRequest::set_application_type`].
    ///
    /// Returns `None` when the server omitted the parameter **and** when it echoed a
    /// non-string JSON value. The whole response is attacker-influenced input, so the
    /// accessor projects a JSON string or nothing at all: no stringification, no unwrap and
    /// no panic, even over bytes that arrived from an untrusted registration endpoint.
    ///
    /// The same single-entry, last-write-wins rule documented on
    /// [`DcrRequest::set_application_type`] governs [`extra`](Self::extra) here.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::server::auth::provider::DcrResponse;
    ///
    /// let echoed: DcrResponse =
    ///     serde_json::from_str(r#"{"client_id":"c-1","application_type":"native"}"#)?;
    /// assert_eq!(echoed.application_type(), Some("native"));
    ///
    /// // A server that stays silent, and a server that answers with a non-string, are
    /// // both reported as "no application type" rather than as a failure.
    /// let silent: DcrResponse = serde_json::from_str(r#"{"client_id":"c-1"}"#)?;
    /// assert_eq!(silent.application_type(), None);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    #[must_use]
    pub fn application_type(&self) -> Option<&str> {
        self.extra
            .get(DCR_APPLICATION_TYPE_KEY)
            .and_then(serde_json::Value::as_str)
    }
}

/// Error from identity provider operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderError {
    /// Error code (e.g., `invalid_token`, `invalid_grant`).
    pub error: String,

    /// Human-readable error description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,

    /// URI for more information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_uri: Option<String>,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(ref desc) = self.error_description {
            write!(f, ": {}", desc)?;
        }
        Ok(())
    }
}

impl std::error::Error for ProviderError {}

/// Core trait for identity provider plugins.
///
/// This trait defines the interface for integrating with external OAuth/OIDC
/// identity providers. Implementations can be built-in (`GenericOidc`, `Cognito`)
/// or provided by third-party crates (pmcp-auth-google, pmcp-auth-auth0).
///
/// # Implementation Guidelines
///
/// 1. **Token Validation**: The `validate_token` method is the most critical.
///    It should verify the token signature, expiration, and issuer.
///
/// 2. **Caching**: Implementations should cache JWKS and discovery documents
///    to avoid excessive network requests.
///
/// 3. **Error Handling**: Return `ProviderError` for OAuth-specific errors
///    (`invalid_token`, `expired_token`) and `Error` for infrastructure issues.
///
/// # Example Implementation
///
/// ```rust,ignore
/// use pmcp::server::auth::provider::{IdentityProvider, ProviderCapabilities};
///
/// pub struct MyProvider {
///     issuer: String,
///     client_id: String,
/// }
///
/// #[async_trait]
/// impl IdentityProvider for MyProvider {
///     fn id(&self) -> &'static str { "my-provider" }
///     fn display_name(&self) -> &'static str { "My Provider" }
///     fn capabilities(&self) -> ProviderCapabilities {
///         ProviderCapabilities::basic_oidc()
///     }
///     // ... implement other methods
/// }
/// ```
#[async_trait]
pub trait IdentityProvider: Send + Sync + Debug {
    // =========================================================================
    // Provider Identity
    // =========================================================================

    /// Unique identifier for this provider (e.g., "google", "auth0", "cognito").
    ///
    /// This is used in configuration files and for provider selection.
    fn id(&self) -> &'static str;

    /// Human-readable display name (e.g., "Google Identity", "Auth0").
    fn display_name(&self) -> &'static str;

    /// Provider capabilities.
    ///
    /// Returns what features this provider supports. Used for runtime
    /// capability checking and configuration validation.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Get the issuer URL for this provider.
    fn issuer(&self) -> &str;

    // =========================================================================
    // Token Validation (Required)
    // =========================================================================

    /// Validate an access token and extract authentication context.
    ///
    /// This is the primary method for authenticating requests. It should:
    /// 1. Verify the token signature using JWKS
    /// 2. Check token expiration
    /// 3. Validate issuer and audience
    /// 4. Extract claims and return `AuthContext`
    ///
    /// # Arguments
    ///
    /// * `token` - The access token (without "Bearer " prefix)
    ///
    /// # Returns
    ///
    /// * `Ok(AuthContext)` - Valid token with extracted claims
    /// * `Err(_)` - Invalid token (expired, bad signature, wrong issuer, etc.)
    async fn validate_token(&self, token: &str) -> Result<AuthContext>;

    // =========================================================================
    // OIDC Discovery (Required for OIDC providers)
    // =========================================================================

    /// Fetch OIDC discovery document.
    ///
    /// Implementations should cache this document (typically for 24 hours).
    /// The document is fetched from `{issuer}/.well-known/openid-configuration`.
    async fn discovery(&self) -> Result<OidcDiscovery>;

    /// Fetch JSON Web Key Set for token validation.
    ///
    /// Implementations should cache JWKS and refresh when keys rotate.
    /// Most implementations check for unknown `kid` and refresh if needed.
    async fn jwks(&self) -> Result<serde_json::Value>;

    // =========================================================================
    // Authorization Flow (Optional - for user authentication)
    // =========================================================================

    /// Build authorization URL for user authentication.
    ///
    /// Returns a URL that the client should redirect the user to for login.
    /// After authentication, the provider redirects back to `redirect_uri`
    /// with an authorization code.
    ///
    /// # Default Implementation
    ///
    /// Returns an error indicating the provider doesn't support user auth flows.
    /// Override this for providers that support authorization code flow.
    async fn authorization_url(&self, _params: AuthorizationParams) -> Result<String> {
        Err(crate::error::Error::protocol(
            crate::error::ErrorCode::INVALID_REQUEST,
            format!(
                "Provider '{}' does not support authorization flow",
                self.id()
            ),
        ))
    }

    /// Exchange authorization code for tokens.
    ///
    /// After the user authenticates and is redirected back with a code,
    /// this method exchanges that code for access/refresh/ID tokens.
    ///
    /// # Default Implementation
    ///
    /// Returns an error. Override for providers that support authorization code flow.
    async fn exchange_code(&self, _params: TokenExchangeParams) -> Result<TokenResponse> {
        Err(crate::error::Error::protocol(
            crate::error::ErrorCode::INVALID_REQUEST,
            format!("Provider '{}' does not support code exchange", self.id()),
        ))
    }

    /// Refresh an access token.
    ///
    /// Uses a refresh token to obtain a new access token without user interaction.
    ///
    /// # Default Implementation
    ///
    /// Returns an error. Override for providers that support refresh tokens.
    async fn refresh_token(&self, _refresh_token: &str) -> Result<TokenResponse> {
        Err(crate::error::Error::protocol(
            crate::error::ErrorCode::INVALID_REQUEST,
            format!("Provider '{}' does not support token refresh", self.id()),
        ))
    }

    // =========================================================================
    // Dynamic Client Registration (Optional)
    // =========================================================================

    /// Register a new OAuth client dynamically (RFC 7591).
    ///
    /// Used by MCP clients to register themselves with the provider.
    /// Not all providers support DCR.
    ///
    /// # Default Implementation
    ///
    /// Returns an error indicating DCR is not supported.
    async fn register_client(&self, _request: DcrRequest) -> Result<DcrResponse> {
        Err(crate::error::Error::protocol(
            crate::error::ErrorCode::INVALID_REQUEST,
            format!(
                "Provider '{}' does not support dynamic client registration",
                self.id()
            ),
        ))
    }

    // =========================================================================
    // Token Management (Optional)
    // =========================================================================

    /// Revoke an access or refresh token.
    ///
    /// # Default Implementation
    ///
    /// Returns Ok(()) as a no-op. Override for providers that support revocation.
    async fn revoke_token(&self, _token: &str) -> Result<()> {
        Ok(())
    }

    /// Introspect a token to get its metadata.
    ///
    /// Returns information about a token without validating it for use.
    /// Useful for debugging and admin interfaces.
    ///
    /// # Default Implementation
    ///
    /// Falls back to `validate_token`. Override for providers with introspection endpoints.
    async fn introspect_token(&self, token: &str) -> Result<AuthContext> {
        self.validate_token(token).await
    }

    // =========================================================================
    // User Info (Optional)
    // =========================================================================

    /// Fetch user information from the `UserInfo` endpoint.
    ///
    /// Some providers include all claims in the ID token, others require
    /// a separate call to the `UserInfo` endpoint.
    ///
    /// # Default Implementation
    ///
    /// Returns an error. Override for providers with `UserInfo` endpoints.
    async fn user_info(&self, _access_token: &str) -> Result<serde_json::Value> {
        Err(crate::error::Error::protocol(
            crate::error::ErrorCode::INVALID_REQUEST,
            format!(
                "Provider '{}' does not support UserInfo endpoint",
                self.id()
            ),
        ))
    }
}

/// Registry for identity providers.
///
/// Allows registering and looking up providers by ID.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, std::sync::Arc<dyn IdentityProvider>>,
}

impl ProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a provider.
    pub fn register<P: IdentityProvider + 'static>(&mut self, provider: P) {
        let id = provider.id().to_string();
        self.providers.insert(id, std::sync::Arc::new(provider));
    }

    /// Get a provider by ID.
    pub fn get(&self, id: &str) -> Option<std::sync::Arc<dyn IdentityProvider>> {
        self.providers.get(id).cloned()
    }

    /// List all registered provider IDs.
    pub fn list(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a provider is registered.
    pub fn has(&self, id: &str) -> bool {
        self.providers.contains_key(id)
    }
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ProviderCapabilities Tests
    // =========================================================================

    #[test]
    fn test_provider_capabilities_full_oidc() {
        let full = ProviderCapabilities::full_oidc();
        assert!(full.oidc);
        assert!(full.dcr);
        assert!(full.pkce);
        assert!(full.refresh_tokens);
        assert!(full.revocation);
        assert!(full.introspection);
        assert!(full.custom_scopes);
        assert!(!full.device_flow);
    }

    #[test]
    fn test_provider_capabilities_basic_oidc() {
        let basic = ProviderCapabilities::basic_oidc();
        assert!(basic.oidc);
        assert!(!basic.dcr);
        assert!(basic.pkce);
        assert!(basic.refresh_tokens);
        assert!(!basic.revocation);
        assert!(!basic.introspection);
        assert!(!basic.custom_scopes);
        assert!(!basic.device_flow);
    }

    #[test]
    fn test_provider_capabilities_default() {
        let caps = ProviderCapabilities::default();
        assert!(!caps.oidc);
        assert!(!caps.dcr);
        assert!(!caps.pkce);
        assert!(!caps.refresh_tokens);
    }

    #[test]
    fn test_provider_capabilities_serialization() {
        let caps = ProviderCapabilities::full_oidc();
        let json = serde_json::to_string(&caps).unwrap();
        assert!(json.contains("\"oidc\":true"));
        assert!(json.contains("\"dcr\":true"));

        let deserialized: ProviderCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps.oidc, deserialized.oidc);
        assert_eq!(caps.dcr, deserialized.dcr);
        assert_eq!(caps.pkce, deserialized.pkce);
    }

    // =========================================================================
    // OidcDiscovery Tests
    // =========================================================================

    #[test]
    fn test_oidc_discovery_serialization() {
        let discovery = OidcDiscovery {
            issuer: "https://accounts.google.com".to_string(),
            authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_endpoint: "https://oauth2.googleapis.com/token".to_string(),
            userinfo_endpoint: Some("https://openidconnect.googleapis.com/v1/userinfo".to_string()),
            jwks_uri: "https://www.googleapis.com/oauth2/v3/certs".to_string(),
            registration_endpoint: None,
            revocation_endpoint: Some("https://oauth2.googleapis.com/revoke".to_string()),
            introspection_endpoint: None,
            end_session_endpoint: None,
            scopes_supported: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            response_types_supported: vec!["code".to_string(), "token".to_string()],
            grant_types_supported: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            subject_types_supported: vec!["public".to_string()],
            id_token_signing_alg_values_supported: vec!["RS256".to_string()],
            token_endpoint_auth_methods_supported: vec!["client_secret_basic".to_string()],
            claims_supported: vec!["sub".to_string(), "email".to_string()],
            code_challenge_methods_supported: vec!["S256".to_string()],
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&discovery).unwrap();
        assert!(json.contains("\"issuer\":\"https://accounts.google.com\""));
        assert!(json.contains("\"jwks_uri\":"));

        let deserialized: OidcDiscovery = serde_json::from_str(&json).unwrap();
        assert_eq!(discovery.issuer, deserialized.issuer);
        assert_eq!(discovery.jwks_uri, deserialized.jwks_uri);
        assert_eq!(discovery.scopes_supported, deserialized.scopes_supported);
    }

    #[test]
    fn test_oidc_discovery_minimal() {
        let json = r#"{
            "issuer": "https://example.com",
            "authorization_endpoint": "https://example.com/authorize",
            "token_endpoint": "https://example.com/token",
            "jwks_uri": "https://example.com/.well-known/jwks.json"
        }"#;

        let discovery: OidcDiscovery = serde_json::from_str(json).unwrap();
        assert_eq!(discovery.issuer, "https://example.com");
        assert!(discovery.userinfo_endpoint.is_none());
        assert!(discovery.scopes_supported.is_empty());
    }

    // =========================================================================
    // AuthorizationParams Tests
    // =========================================================================

    #[test]
    fn test_authorization_params_new() {
        let params = AuthorizationParams::new("https://example.com/callback", "random-state");

        assert_eq!(params.redirect_uri, "https://example.com/callback");
        assert_eq!(params.state, "random-state");
        assert_eq!(params.scopes, vec!["openid"]);
        assert!(params.nonce.is_none());
        assert!(params.code_challenge.is_none());
        assert!(params.extra.is_empty());
    }

    #[test]
    fn test_authorization_params_builder() {
        let params = AuthorizationParams::new("https://example.com/callback", "random-state")
            .with_scopes(["openid", "email", "profile"])
            .with_pkce("challenge123", "S256")
            .with_nonce("nonce456");

        assert_eq!(params.redirect_uri, "https://example.com/callback");
        assert_eq!(params.state, "random-state");
        assert_eq!(params.scopes, vec!["openid", "email", "profile"]);
        assert_eq!(params.code_challenge, Some("challenge123".to_string()));
        assert_eq!(params.code_challenge_method, Some("S256".to_string()));
        assert_eq!(params.nonce, Some("nonce456".to_string()));
    }

    #[test]
    fn test_authorization_params_extra() {
        let mut params = AuthorizationParams::new("https://example.com/callback", "state");
        params
            .extra
            .insert("prompt".to_string(), "consent".to_string());
        params
            .extra
            .insert("login_hint".to_string(), "user@example.com".to_string());

        assert_eq!(params.extra.get("prompt"), Some(&"consent".to_string()));
        assert_eq!(
            params.extra.get("login_hint"),
            Some(&"user@example.com".to_string())
        );
    }

    // =========================================================================
    // TokenExchangeParams Tests
    // =========================================================================

    #[test]
    fn test_token_exchange_params() {
        let params = TokenExchangeParams {
            code: "auth_code_123".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            code_verifier: Some("verifier_456".to_string()),
        };

        assert_eq!(params.code, "auth_code_123");
        assert_eq!(params.redirect_uri, "https://example.com/callback");
        assert_eq!(params.code_verifier, Some("verifier_456".to_string()));
    }

    #[test]
    fn test_token_exchange_params_without_pkce() {
        let params = TokenExchangeParams {
            code: "auth_code_123".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            code_verifier: None,
        };

        assert!(params.code_verifier.is_none());
    }

    // =========================================================================
    // TokenResponse Tests
    // =========================================================================

    #[test]
    fn test_token_response_serialization() {
        let response = TokenResponse {
            access_token: "access_token_123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("refresh_token_456".to_string()),
            scope: Some("openid email".to_string()),
            id_token: Some("id_token_789".to_string()),
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"access_token\":\"access_token_123\""));
        assert!(json.contains("\"token_type\":\"Bearer\""));
        assert!(json.contains("\"expires_in\":3600"));

        let deserialized: TokenResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.access_token, deserialized.access_token);
        assert_eq!(response.expires_in, deserialized.expires_in);
    }

    #[test]
    fn test_token_response_minimal() {
        let json = r#"{
            "access_token": "token123",
            "token_type": "Bearer"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "token123");
        assert_eq!(response.token_type, "Bearer");
        assert!(response.expires_in.is_none());
        assert!(response.refresh_token.is_none());
    }

    #[test]
    fn test_token_response_with_extra_fields() {
        let json = r#"{
            "access_token": "token123",
            "token_type": "Bearer",
            "custom_field": "custom_value"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "token123");
        assert_eq!(
            response.extra.get("custom_field"),
            Some(&serde_json::json!("custom_value"))
        );
    }

    // =========================================================================
    // DcrRequest Tests
    // =========================================================================

    #[test]
    fn test_dcr_request_serialization() {
        let request = DcrRequest {
            redirect_uris: vec!["https://example.com/callback".to_string()],
            client_name: Some("My App".to_string()),
            client_uri: Some("https://example.com".to_string()),
            logo_uri: None,
            contacts: vec!["admin@example.com".to_string()],
            token_endpoint_auth_method: Some("client_secret_basic".to_string()),
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            response_types: vec!["code".to_string()],
            scope: Some("openid email profile".to_string()),
            software_id: Some("my-software-id".to_string()),
            software_version: Some("1.0.0".to_string()),
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"redirect_uris\":[\"https://example.com/callback\"]"));
        assert!(json.contains("\"client_name\":\"My App\""));

        let deserialized: DcrRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request.redirect_uris, deserialized.redirect_uris);
        assert_eq!(request.client_name, deserialized.client_name);
    }

    #[test]
    fn test_dcr_request_minimal() {
        let request = DcrRequest {
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
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: DcrRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request.redirect_uris, deserialized.redirect_uris);
    }

    // =========================================================================
    // DcrResponse Tests
    // =========================================================================

    #[test]
    fn test_dcr_response_serialization() {
        let response = DcrResponse {
            client_id: "client_123".to_string(),
            client_secret: Some("secret_456".to_string()),
            client_secret_expires_at: Some(1_735_689_600),
            registration_access_token: Some("rat_789".to_string()),
            registration_client_uri: Some("https://auth.example.com/clients/123".to_string()),
            token_endpoint_auth_method: Some("client_secret_basic".to_string()),
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"client_id\":\"client_123\""));

        let deserialized: DcrResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.client_id, deserialized.client_id);
        assert_eq!(response.client_secret, deserialized.client_secret);
    }

    #[test]
    fn test_dcr_response_public_client() {
        let json = r#"{
            "client_id": "public_client_123"
        }"#;

        let response: DcrResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.client_id, "public_client_123");
        assert!(response.client_secret.is_none());
    }

    // =========================================================================
    // application_type Accessor Tests (SEP-837 / AUTH-02)
    //
    // These pin the D-09 contract: `application_type` is carried by the
    // `#[serde(flatten)] extra` map through inherent accessors, never by a new
    // public field, so `cargo semver-checks`' `constructible_struct_adds_field`
    // (a MAJOR finding) is never triggered.
    // =========================================================================

    /// A `DcrRequest` built without touching `extra` advertises no application type.
    #[test]
    fn test_dcr_request_application_type_absent_by_default() {
        let request = minimal_dcr_request();
        assert_eq!(request.application_type(), None);
    }

    /// The setter is readable through the getter — the pair addresses one map entry.
    #[test]
    fn test_dcr_request_set_application_type_is_readable() {
        let mut request = minimal_dcr_request();
        request.set_application_type("native");
        assert_eq!(request.application_type(), Some("native"));
    }

    /// The flatten carrier emits a TOP-LEVEL key, byte-identical to a real serde field.
    #[test]
    fn test_dcr_request_application_type_serializes_at_top_level() {
        let mut request = minimal_dcr_request();
        request.set_application_type("native");

        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["application_type"], serde_json::json!("native"));
        assert!(
            value.get("extra").is_none(),
            "the carrier must not surface as a nested `extra` object"
        );

        // Exactly one occurrence on the wire — no typed/raw duality is possible.
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(text.matches("\"application_type\"").count(), 1);
    }

    /// Serialize then deserialize preserves the value, as a real field would.
    #[test]
    fn test_dcr_request_application_type_survives_round_trip() {
        let mut request = minimal_dcr_request();
        request.set_application_type("web");

        let text = serde_json::to_string(&request).unwrap();
        let back: DcrRequest = serde_json::from_str(&text).unwrap();
        assert_eq!(back.application_type(), Some("web"));
    }

    /// Collision order A: a hand-written raw key, then the setter — last write wins.
    #[test]
    fn test_dcr_request_application_type_raw_then_setter_last_write_wins() {
        let mut request = minimal_dcr_request();
        request.extra.insert(
            DCR_APPLICATION_TYPE_KEY.to_string(),
            serde_json::json!("web"),
        );
        request.set_application_type("native");

        assert_eq!(request.application_type(), Some("native"));
        assert_eq!(request.extra.len(), 1, "one key, never two");
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(text.matches("\"application_type\"").count(), 1);
        assert!(!text.contains("\"web\""));
    }

    /// Collision order B: the setter, then a hand-written raw key — same rule, stated symmetrically.
    #[test]
    fn test_dcr_request_application_type_setter_then_raw_last_write_wins() {
        let mut request = minimal_dcr_request();
        request.set_application_type("native");
        request.extra.insert(
            DCR_APPLICATION_TYPE_KEY.to_string(),
            serde_json::json!("web"),
        );

        assert_eq!(request.application_type(), Some("web"));
        assert_eq!(request.extra.len(), 1, "one key, never two");
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(text.matches("\"application_type\"").count(), 1);
        assert!(!text.contains("\"native\""));
    }

    /// A non-string JSON value projects to `None` rather than panicking or stringifying.
    #[test]
    fn test_dcr_request_application_type_non_string_value_is_none() {
        for hostile in [
            serde_json::json!(42),
            serde_json::json!(null),
            serde_json::json!(true),
            serde_json::json!(["native"]),
            serde_json::json!({ "value": "native" }),
        ] {
            let mut request = minimal_dcr_request();
            request
                .extra
                .insert(DCR_APPLICATION_TYPE_KEY.to_string(), hostile.clone());
            assert_eq!(
                request.application_type(),
                None,
                "non-string value {hostile} must project to None"
            );
        }
    }

    /// The response accessor reads the AS's echo, and reports absence as `None`.
    #[test]
    fn test_dcr_response_application_type_reads_the_echo() {
        let echoed: DcrResponse =
            serde_json::from_str(r#"{"client_id":"client_123","application_type":"native"}"#)
                .unwrap();
        assert_eq!(echoed.application_type(), Some("native"));

        let silent: DcrResponse = serde_json::from_str(r#"{"client_id":"client_123"}"#).unwrap();
        assert_eq!(silent.application_type(), None);
    }

    /// Arbitrary bytes reach the response accessor without a panic — `Err` or `None`, never abort.
    /// 116-08 fuzzes this same pair; this test pins three shapes by hand.
    #[test]
    fn test_dcr_response_application_type_from_malformed_bytes_never_panics() {
        for malformed in [
            &b"null"[..],
            &b"[\"application_type\"]"[..],
            &br#"{"application_type": {"a":1}}"#[..],
            &b"\xff\xfe not json at all"[..],
            &b""[..],
        ] {
            // A refusal (`Err`) is an acceptable outcome here; a panic is not. Only the
            // parsed-successfully branch carries an assertion.
            if let Ok(response) = serde_json::from_slice::<DcrResponse>(malformed) {
                assert_eq!(response.application_type(), None);
            }
        }

        // A well-formed response whose echoed value is an object still yields None.
        let objectish: DcrResponse =
            serde_json::from_slice(br#"{"client_id":"c","application_type":{"a":1}}"#).unwrap();
        assert_eq!(objectish.application_type(), None);
    }

    /// Reader and writer share ONE literal, so the wire key cannot drift between them.
    #[test]
    fn test_dcr_application_type_key_is_the_single_wire_literal() {
        assert_eq!(DCR_APPLICATION_TYPE_KEY, "application_type");

        let mut request = minimal_dcr_request();
        request.set_application_type("native");
        assert_eq!(
            request
                .extra
                .get(DCR_APPLICATION_TYPE_KEY)
                .and_then(serde_json::Value::as_str),
            Some("native")
        );
    }

    /// Shared fixture: the smallest legal `DcrRequest`, with an untouched carrier.
    fn minimal_dcr_request() -> DcrRequest {
        DcrRequest {
            redirect_uris: vec!["http://127.0.0.1:8080/callback".to_string()],
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
            extra: HashMap::new(),
        }
    }

    // =========================================================================
    // ProviderError Tests
    // =========================================================================

    #[test]
    fn test_provider_error_display() {
        let error = ProviderError {
            error: "invalid_token".to_string(),
            error_description: Some("The token has expired".to_string()),
            error_uri: None,
        };

        assert_eq!(format!("{}", error), "invalid_token: The token has expired");
    }

    #[test]
    fn test_provider_error_display_no_description() {
        let error = ProviderError {
            error: "invalid_request".to_string(),
            error_description: None,
            error_uri: None,
        };

        assert_eq!(format!("{}", error), "invalid_request");
    }

    #[test]
    fn test_provider_error_serialization() {
        let error = ProviderError {
            error: "access_denied".to_string(),
            error_description: Some("User denied access".to_string()),
            error_uri: Some("https://example.com/errors/access_denied".to_string()),
        };

        let json = serde_json::to_string(&error).unwrap();
        let deserialized: ProviderError = serde_json::from_str(&json).unwrap();
        assert_eq!(error.error, deserialized.error);
        assert_eq!(error.error_description, deserialized.error_description);
    }

    #[test]
    fn test_provider_error_is_std_error() {
        let error = ProviderError {
            error: "server_error".to_string(),
            error_description: None,
            error_uri: None,
        };

        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &error;
    }

    // =========================================================================
    // ProviderRegistry Tests
    // =========================================================================

    #[test]
    fn test_provider_registry_new() {
        let registry = ProviderRegistry::new();
        assert!(registry.list().is_empty());
        assert!(!registry.has("google"));
    }

    #[test]
    fn test_provider_registry_default() {
        let registry = ProviderRegistry::default();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_provider_registry_debug() {
        let registry = ProviderRegistry::new();
        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("ProviderRegistry"));
        assert!(debug_str.contains("providers"));
    }

    // =========================================================================
    // Mock Provider for Registry Tests
    // =========================================================================

    #[derive(Debug)]
    struct MockProvider {
        provider_id: &'static str,
    }

    #[async_trait]
    impl IdentityProvider for MockProvider {
        fn id(&self) -> &'static str {
            self.provider_id
        }

        fn display_name(&self) -> &'static str {
            "Mock Provider"
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::basic_oidc()
        }

        #[allow(clippy::unnecessary_literal_bound)]
        fn issuer(&self) -> &str {
            "https://mock.example.com"
        }

        async fn validate_token(&self, _token: &str) -> Result<AuthContext> {
            Ok(AuthContext::new("mock-user"))
        }

        async fn discovery(&self) -> Result<OidcDiscovery> {
            Ok(OidcDiscovery {
                issuer: "https://mock.example.com".to_string(),
                authorization_endpoint: "https://mock.example.com/authorize".to_string(),
                token_endpoint: "https://mock.example.com/token".to_string(),
                userinfo_endpoint: None,
                jwks_uri: "https://mock.example.com/.well-known/jwks.json".to_string(),
                registration_endpoint: None,
                revocation_endpoint: None,
                introspection_endpoint: None,
                end_session_endpoint: None,
                scopes_supported: vec!["openid".to_string()],
                response_types_supported: vec!["code".to_string()],
                grant_types_supported: vec!["authorization_code".to_string()],
                subject_types_supported: vec![],
                id_token_signing_alg_values_supported: vec!["RS256".to_string()],
                token_endpoint_auth_methods_supported: vec![],
                claims_supported: vec![],
                code_challenge_methods_supported: vec!["S256".to_string()],
                extra: HashMap::new(),
            })
        }

        async fn jwks(&self) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "keys": [] }))
        }
    }

    #[test]
    fn test_provider_registry_register_and_get() {
        let mut registry = ProviderRegistry::new();

        let mock = MockProvider {
            provider_id: "mock",
        };
        registry.register(mock);

        assert!(registry.has("mock"));
        assert!(!registry.has("other"));

        let provider = registry.get("mock");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id(), "mock");
    }

    #[test]
    fn test_provider_registry_list() {
        let mut registry = ProviderRegistry::new();

        registry.register(MockProvider {
            provider_id: "provider1",
        });
        registry.register(MockProvider {
            provider_id: "provider2",
        });

        let list = registry.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"provider1"));
        assert!(list.contains(&"provider2"));
    }

    #[test]
    fn test_provider_registry_get_nonexistent() {
        let registry = ProviderRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_mock_provider_validate_token() {
        let provider = MockProvider {
            provider_id: "mock",
        };
        let auth = provider.validate_token("any-token").await.unwrap();
        assert_eq!(auth.user_id(), "mock-user");
        assert!(auth.authenticated);
    }

    #[tokio::test]
    async fn test_mock_provider_discovery() {
        let provider = MockProvider {
            provider_id: "mock",
        };
        let discovery = provider.discovery().await.unwrap();
        assert_eq!(discovery.issuer, "https://mock.example.com");
        assert!(discovery
            .code_challenge_methods_supported
            .contains(&"S256".to_string()));
    }

    #[tokio::test]
    async fn test_mock_provider_default_authorization_url() {
        let provider = MockProvider {
            provider_id: "mock",
        };
        let params = AuthorizationParams::new("https://example.com/callback", "state");

        // Default implementation returns error
        let result = provider.authorization_url(params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_provider_default_exchange_code() {
        let provider = MockProvider {
            provider_id: "mock",
        };
        let params = TokenExchangeParams {
            code: "code".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            code_verifier: None,
        };

        // Default implementation returns error
        let result = provider.exchange_code(params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_provider_default_refresh_token() {
        let provider = MockProvider {
            provider_id: "mock",
        };

        // Default implementation returns error
        let result = provider.refresh_token("refresh_token").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_provider_default_register_client() {
        let provider = MockProvider {
            provider_id: "mock",
        };
        let request = DcrRequest {
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
            extra: HashMap::new(),
        };

        // Default implementation returns error
        let result = provider.register_client(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_provider_default_revoke_token() {
        let provider = MockProvider {
            provider_id: "mock",
        };

        // Default implementation is a no-op
        let result = provider.revoke_token("token").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_provider_default_introspect_token() {
        let provider = MockProvider {
            provider_id: "mock",
        };

        // Default implementation falls back to validate_token
        let result = provider.introspect_token("token").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().user_id(), "mock-user");
    }

    #[tokio::test]
    async fn test_mock_provider_default_user_info() {
        let provider = MockProvider {
            provider_id: "mock",
        };

        // Default implementation returns error
        let result = provider.user_info("access_token").await;
        assert!(result.is_err());
    }
}
