//! Authentication providers for OUTGOING HTTP requests (OAPI-03 / D-05 / H1).
//!
//! This module is the OUTBOUND counterpart to the inbound
//! [`crate::auth::AuthProvider`] (`pmcp::server::auth::AuthProvider`, which
//! authenticates an INCOMING MCP request). The two are kept deliberately
//! distinct (Pitfall 1): the trait here is [`HttpAuthProvider`] and its method
//! is [`apply`](HttpAuthProvider::apply) — it MUTATES the headers / query of a
//! request the toolkit is about to SEND to a REST backend. This module does NOT
//! re-implement the inbound request-validation surface.
//!
//! # The six auth modes (D-05) split into two construction strategies
//!
//! [`AuthConfig`] has SIX variants — `None` + five authenticated ones. They
//! split by HOW the credential is obtained:
//!
//! - **Static** (`None`/`ApiKey`/`Bearer`/`Basic`/`OAuth2ClientCredentials`):
//!   fully determined by `config.toml` (operator credentials / `${ENV}` secrets).
//!   Built ONCE at startup via [`create_auth_provider`] and shared as
//!   `Arc<dyn HttpAuthProvider>`. They IGNORE any inbound MCP client token.
//! - **Per-request passthrough** (`OAuthPassthrough`): needs the INCOMING MCP
//!   client token for EACH request, so it cannot be fully built at startup.
//!   [`apply`](HttpAuthProvider::apply) accepts an OPTIONAL `inbound_token` so a
//!   SINGLE trait serves both strategies — static providers ignore it,
//!   [`OAuthPassthroughAuth`] forwards it. Plan 04 carries the per-request token
//!   to `apply`; Plan 06 wires the inbound `TokenCaptureAuthProvider` so the
//!   captured token lands in `AuthContext` and is threaded into this `apply`.
//!
//! # Ownership
//!
//! [`AuthConfig`] and the provider types are OWNED HERE so Plan 01 and Plan 02
//! changes stay confined — Plan 02 RE-EXPORTS
//! `pmcp_server_toolkit::http::auth::AuthConfig` rather than redefining it.

use super::HttpConnectorError;
// The `${VAR}` / `env:VAR` grammar lives in the backend-neutral `crate::env_ref`
// module, NOT here: this module is `#[cfg(feature = "http")]`, so a chokepoint
// defined here would exist only in `http` builds while `[backend].base_url`
// resolution and `token_secret` need the same grammar in every build.
use crate::env_ref::parse_env_ref;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Default `required` flag (true) for authenticated [`AuthConfig`] variants.
fn default_true() -> bool {
    true
}

/// Default outgoing header for [`AuthConfig::OAuthPassthrough`].
fn default_auth_header() -> String {
    "Authorization".to_string()
}

/// Outgoing-HTTP authentication configuration (OAPI-03 / D-05).
///
/// Lifted near-verbatim from the pmcp-run reference `AuthConfig`. The
/// `#[serde(tag = "type", rename_all = "snake_case")]` shape means a
/// `config.toml` `[backend.auth]` block selects the variant via `type = "..."`
/// (`none`, `api_key`, `bearer`, `basic`, `oauth2_client_credentials`,
/// `oauth_passthrough`). [`Default`] is [`AuthConfig::None`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// No authentication.
    #[default]
    None,

    /// API key passed as query parameters and/or headers.
    ApiKey {
        /// API key values carried as query parameters.
        #[serde(default)]
        query_params: HashMap<String, String>,
        /// API key values carried as headers.
        #[serde(default)]
        headers: HashMap<String, String>,
        /// Whether authentication is required.
        #[serde(default = "default_true")]
        required: bool,
    },

    /// Bearer token (`Authorization: Bearer <token>`).
    Bearer {
        /// Token value. Supports a `${VAR}` or `env:VAR` reference resolved from
        /// the process environment at provider-build time (an unset reference
        /// collapses to no-auth; the literal placeholder never reaches the wire).
        token: String,
        /// Whether authentication is required.
        #[serde(default = "default_true")]
        required: bool,
    },

    /// HTTP Basic auth (`Authorization: Basic <base64(user:pass)>`).
    Basic {
        /// Username. Supports a `${VAR}` / `env:VAR` reference (resolved at
        /// provider-build time) for symmetry with `password`.
        username: String,
        /// Password. Supports a `${VAR}` or `env:VAR` reference resolved from the
        /// process environment at provider-build time (the literal placeholder
        /// never reaches the wire).
        password: String,
        /// Whether authentication is required.
        #[serde(default = "default_true")]
        required: bool,
    },

    /// OAuth2 client-credentials grant.
    ///
    /// `rename_all = "snake_case"` derives the tag `o_auth2_client_credentials`,
    /// but the documented config form (README, line-56 doc comment) is
    /// `type = "oauth2_client_credentials"`. The alias accepts the documented
    /// spelling so `[backend.auth]` configs deserialize as documented.
    #[serde(alias = "oauth2_client_credentials")]
    OAuth2ClientCredentials {
        /// Token endpoint URL.
        token_url: String,
        /// Client ID. Supports a `${VAR}` / `env:VAR` reference resolved at
        /// provider-build time.
        client_id: String,
        /// Client secret. Supports a `${VAR}` or `env:VAR` reference resolved from
        /// the process environment at provider-build time (the literal
        /// placeholder never reaches the token endpoint).
        client_secret: String,
        /// Requested scopes.
        #[serde(default)]
        scopes: Vec<String>,
        /// Whether authentication is required.
        #[serde(default = "default_true")]
        required: bool,
    },

    /// Forward the INCOMING MCP client token to the backend (SSO passthrough, H1).
    ///
    /// `rename_all = "snake_case"` derives the tag `o_auth_passthrough`, but the
    /// documented config form (README, line-56 doc comment) is
    /// `type = "oauth_passthrough"`. The alias accepts the documented spelling so
    /// `[backend.auth]` configs deserialize as documented.
    #[serde(alias = "oauth_passthrough")]
    OAuthPassthrough {
        /// Outgoing header to set (default `Authorization`).
        #[serde(default = "default_auth_header")]
        target_header: String,
        /// Whether to fail when no inbound token is present.
        #[serde(default = "default_true")]
        required: bool,
    },
}

impl AuthConfig {
    /// The first credential field whose configured value is REFERENCE-shaped but
    /// names no settable environment variable, as a field path within
    /// `[backend.auth]` (e.g. `"token"`, `"password"`, `"query_params.app_key"`).
    /// `None` when every credential field is a plain literal or a well-formed
    /// single reference.
    ///
    /// # Why this exists
    ///
    /// [`crate::env_ref::parse_env_ref`] maps every unsettable brace form — the
    /// empty `${}`, a composition like `${A}://${B}`, and a non-portable name
    /// like `${TFL-APP-KEY}` — to the EMPTY name. [`resolve_secret_ref`] then
    /// resolves the empty name to the empty string, and the empty string is the
    /// credential path's "omit this credential" signal: [`expand_api_key_map`]
    /// DROPS the entry and the bearer / basic / oauth2 arms of
    /// [`create_auth_provider`] collapse to [`NoAuth`]. So a malformed reference
    /// produced a server that booted fine and sent every backend request
    /// UNAUTHENTICATED, with no error and no log line.
    ///
    /// That omission rule is correct for an UNSET variable — an optional
    /// credential the target environment chose not to supply — and it is
    /// deliberately kept. A MALFORMED reference is a different thing: it is a
    /// mistake in the config file that no environment could ever satisfy, so it
    /// gets the refusal `[backend].base_url` already gets
    /// ([`crate::error::ConfigValidationError::MalformedBackendBaseUrlRef`]).
    ///
    /// The returned path names the FIELD only, never the value: a malformed
    /// value is by definition not a resolvable reference, so it may well be a
    /// mistyped literal secret that must not reach a log.
    ///
    /// # Determinism
    ///
    /// `query_params` / `headers` are [`HashMap`]s, so the offending entry is
    /// selected by `min` over the keys rather than by iteration order — an
    /// error message that named a different field run to run would be unusable.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_server_toolkit::http::auth::AuthConfig;
    ///
    /// // A dash is not a portably settable variable name.
    /// let bad = AuthConfig::Bearer { token: "${TFL-APP-KEY}".into(), required: true };
    /// assert_eq!(bad.malformed_env_ref_field().as_deref(), Some("token"));
    ///
    /// // A well-formed reference and a plain literal are both fine.
    /// let good = AuthConfig::Bearer { token: "${TFL_APP_KEY}".into(), required: true };
    /// assert_eq!(good.malformed_env_ref_field(), None);
    /// ```
    #[must_use]
    pub fn malformed_env_ref_field(&self) -> Option<String> {
        // A malformed reference is exactly `Some("")`: reference-SHAPED (so it
        // must never ship as a literal) but naming nothing settable.
        fn scalar(label: &str, value: &str) -> Option<String> {
            (parse_env_ref(value) == Some("")).then(|| label.to_string())
        }
        fn entry(label: &str, map: &HashMap<String, String>) -> Option<String> {
            map.iter()
                .filter(|(_, v)| parse_env_ref(v) == Some(""))
                .map(|(k, _)| k)
                .min()
                .map(|k| format!("{label}.{k}"))
        }

        match self {
            // No credential fields to check.
            Self::None | Self::OAuthPassthrough { .. } => None,
            Self::ApiKey {
                query_params,
                headers,
                ..
            } => entry("query_params", query_params).or_else(|| entry("headers", headers)),
            Self::Bearer { token, .. } => scalar("token", token),
            Self::Basic {
                username, password, ..
            } => scalar("username", username).or_else(|| scalar("password", password)),
            // `token_url` is deliberately absent: it is an endpoint used
            // verbatim, not routed through `resolve_secret_ref`, so it cannot
            // be silently omitted the way a credential can.
            Self::OAuth2ClientCredentials {
                client_id,
                client_secret,
                ..
            } => scalar("client_id", client_id).or_else(|| scalar("client_secret", client_secret)),
        }
    }

    /// Whether this configuration requires authentication to succeed.
    #[must_use]
    pub fn is_required(&self) -> bool {
        match self {
            Self::None => false,
            Self::ApiKey { required, .. }
            | Self::Bearer { required, .. }
            | Self::Basic { required, .. }
            | Self::OAuth2ClientCredentials { required, .. }
            | Self::OAuthPassthrough { required, .. } => *required,
        }
    }
}

/// Outbound HTTP authentication provider (OAPI-03).
///
/// DISTINCT from the inbound [`crate::auth::AuthProvider`] (Pitfall 1): this
/// MUTATES the outgoing request. [`apply`](HttpAuthProvider::apply) accepts an
/// OPTIONAL `inbound_token` — the per-request MCP client token captured via the
/// `AuthContext` bridge (H1). Static providers ignore it; the passthrough
/// provider forwards it.
#[async_trait]
pub trait HttpAuthProvider: Send + Sync + 'static {
    /// Apply credentials to the outgoing request's `headers` and `query`.
    ///
    /// `inbound_token` is the per-request MCP client token (when present). Static
    /// providers MUST ignore it; [`OAuthPassthroughAuth`] forwards it.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Auth`] when a required credential is absent,
    /// or [`HttpConnectorError::InvalidHeader`] when a header name/value cannot be
    /// constructed. No error message echoes the token or credential value.
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        query: &mut HashMap<String, String>,
        inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError>;
}

/// No authentication — a no-op provider.
pub struct NoAuth;

#[async_trait]
impl HttpAuthProvider for NoAuth {
    async fn apply(
        &self,
        _headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        Ok(())
    }
}

/// Provider that always fails — used when a required passthrough token is absent.
pub struct MissingTokenAuth;

#[async_trait]
impl HttpAuthProvider for MissingTokenAuth {
    async fn apply(
        &self,
        _headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        // Honour a late-arriving per-request token if the static constructor was
        // built without one (the passthrough construction-time fallback).
        if inbound_token.map(str::is_empty) == Some(false) {
            return Ok(());
        }
        Err(HttpConnectorError::Auth(
            "authentication required but no inbound token was provided".to_string(),
        ))
    }
}

/// API key authentication (query params and/or headers). STATIC: ignores `inbound_token`.
pub struct ApiKeyAuth {
    query_params: HashMap<String, String>,
    headers: HashMap<String, String>,
}

#[async_trait]
impl HttpAuthProvider for ApiKeyAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        for (key, value) in &self.query_params {
            query.insert(key.clone(), value.clone());
        }
        for (key, value) in &self.headers {
            let name = HeaderName::try_from(key.as_str()).map_err(|_| {
                HttpConnectorError::InvalidHeader("invalid header name".to_string())
            })?;
            let val = HeaderValue::try_from(value.as_str()).map_err(|_| {
                HttpConnectorError::InvalidHeader("invalid header value".to_string())
            })?;
            headers.insert(name, val);
        }
        Ok(())
    }
}

/// Bearer token authentication. STATIC: ignores `inbound_token`.
pub struct BearerAuth {
    token: String,
}

#[async_trait]
impl HttpAuthProvider for BearerAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        let value = format!("Bearer {}", self.token);
        let header_value = HeaderValue::try_from(value)
            .map_err(|_| HttpConnectorError::InvalidHeader("invalid bearer token".to_string()))?;
        headers.insert(reqwest::header::AUTHORIZATION, header_value);
        Ok(())
    }
}

/// HTTP Basic authentication. STATIC: ignores `inbound_token`.
pub struct BasicAuth {
    username: String,
    password: String,
}

#[async_trait]
impl HttpAuthProvider for BasicAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        use base64::Engine;
        let credentials = format!("{}:{}", self.username, self.password);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        let value = format!("Basic {encoded}");
        let header_value = HeaderValue::try_from(value).map_err(|_| {
            HttpConnectorError::InvalidHeader("invalid basic credentials".to_string())
        })?;
        headers.insert(reqwest::header::AUTHORIZATION, header_value);
        Ok(())
    }
}

/// OAuth2 client-credentials authentication. STATIC config; ignores `inbound_token`.
///
/// The token is fetched lazily from `token_url` on first `apply` and cached. The
/// fetch uses a fresh `reqwest::Client` (mirrors the reference). The cached token
/// is stored under a `tokio::sync::RwLock`.
pub struct OAuth2ClientCredentialsAuth {
    token_url: String,
    client_id: String,
    client_secret: String,
    scopes: Vec<String>,
    cached: tokio::sync::RwLock<Option<String>>,
}

impl OAuth2ClientCredentialsAuth {
    /// Construct a client-credentials provider (no network until first `apply`).
    #[must_use]
    pub fn new(
        token_url: String,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            token_url,
            client_id,
            client_secret,
            scopes,
            cached: tokio::sync::RwLock::new(None),
        }
    }

    async fn fetch_token(&self) -> Result<String, HttpConnectorError> {
        let client = reqwest::Client::new();
        let mut params = vec![
            ("grant_type", "client_credentials".to_string()),
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone()),
        ];
        if !self.scopes.is_empty() {
            params.push(("scope", self.scopes.join(" ")));
        }
        let response = client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|_| HttpConnectorError::Auth("oauth2 token request failed".to_string()))?;
        if !response.status().is_success() {
            return Err(HttpConnectorError::Auth(format!(
                "oauth2 token endpoint returned status {}",
                response.status().as_u16()
            )));
        }
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
        }
        let token: TokenResponse = response.json().await.map_err(|_| {
            HttpConnectorError::Auth("oauth2 token response unparseable".to_string())
        })?;
        Ok(token.access_token)
    }
}

#[async_trait]
impl HttpAuthProvider for OAuth2ClientCredentialsAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        {
            let cached = self.cached.read().await;
            if cached.is_none() {
                drop(cached);
                let fetched = self.fetch_token().await?;
                *self.cached.write().await = Some(fetched);
            }
        }
        let cached = self.cached.read().await;
        if let Some(access_token) = cached.as_ref() {
            let value = format!("Bearer {access_token}");
            let header_value = HeaderValue::try_from(value).map_err(|_| {
                HttpConnectorError::InvalidHeader("invalid oauth2 access token".to_string())
            })?;
            headers.insert(reqwest::header::AUTHORIZATION, header_value);
        }
        Ok(())
    }
}

/// OAuth passthrough — forwards the INCOMING MCP client token to the backend (H1).
///
/// PER-REQUEST: prefers the per-request `inbound_token` arg to `apply`; falls back
/// to the construction-time captured `incoming_token` (via
/// [`create_passthrough_auth_provider`]). When neither is present and the config
/// is `required`, `apply` returns [`HttpConnectorError::Auth`].
///
/// # Trust boundary (WR-04)
///
/// This provider relays a **client-controlled** value into an
/// **operator-controlled** destination — the trust posture is intentional and
/// must stay visible at the type:
///
/// - The MCP **client controls the forwarded token VALUE**: it is the raw
///   inbound `Authorization` header captured by `TokenCaptureAuthProvider` and
///   forwarded verbatim (bare tokens are prefixed with `Bearer ` in [`apply`]).
/// - The **operator controls the destination header NAME** (`target_header`),
///   fixed in the committed config; the client cannot redirect the token to a
///   different header.
///
/// Relaying the client's own credential to the backend is the **intended**
/// SSO-passthrough behavior — use it only when the backend should receive the
/// MCP client's own identity. The `HeaderValue::try_from` control-character
/// rejection in [`apply`] is the protection against header injection; a
/// malformed token value is rejected, not relayed.
///
/// [`apply`]: OAuthPassthroughAuth::apply
pub struct OAuthPassthroughAuth {
    target_header: String,
    incoming_token: Option<String>,
    required: bool,
}

#[async_trait]
impl HttpAuthProvider for OAuthPassthroughAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        // Prefer the per-request token; fall back to the construction-time capture.
        let token: Option<&str> = inbound_token
            .filter(|t| !t.is_empty())
            .or_else(|| self.incoming_token.as_deref().filter(|t| !t.is_empty()));

        match token {
            Some(tok) => {
                let header_name =
                    HeaderName::try_from(self.target_header.as_str()).map_err(|_| {
                        HttpConnectorError::InvalidHeader(
                            "invalid passthrough target header".to_string(),
                        )
                    })?;
                // Forward the token verbatim if it already carries a scheme,
                // otherwise prefix with "Bearer ".
                let value = if tok.starts_with("Bearer ") || tok.starts_with("Basic ") {
                    tok.to_string()
                } else {
                    format!("Bearer {tok}")
                };
                let header_value = HeaderValue::try_from(value).map_err(|_| {
                    HttpConnectorError::InvalidHeader("invalid passthrough token value".to_string())
                })?;
                // TRUST BOUNDARY (WR-04): we relay a CLIENT-controlled value
                // (`tok`, the raw inbound Authorization header captured by
                // TokenCaptureAuthProvider) into an OPERATOR-controlled
                // destination (`header_name`, from the committed `target_header`).
                // Forwarding the client's own credential is INTENDED SSO
                // passthrough — use only when the backend should receive the MCP
                // client's identity. The HeaderValue::try_from guard above is the
                // protection: it rejects control chars, so a malformed token is
                // rejected rather than injected. See the type doc-comment.
                headers.insert(header_name, header_value);
                Ok(())
            },
            None if self.required => Err(HttpConnectorError::Auth(
                "passthrough authentication required but no inbound token was provided".to_string(),
            )),
            None => Ok(()),
        }
    }
}

/// Resolve a single credential value, expanding a `${VAR}` or `env:VAR` reference
/// from the process environment — the ONE chokepoint applied to every credential
/// field (api_key, bearer `token`, basic `password`, oauth2 `client_secret`) as
/// it enters [`create_auth_provider`].
///
/// A credential frequently holds a secret reference (`"${GITHUB_PAT}"`) rather
/// than a literal — mirroring the `token_secret` convention in
/// [`crate::code_mode`]. Without expansion the LITERAL `${GITHUB_PAT}` would be
/// sent to the backend, so 100% of authenticated calls would fail (this is a
/// correctness requirement, not a convenience).
///
/// Resolution rules (matching the `token_secret` env-ref discipline):
/// - `"${VAR}"` / `"env:VAR"` → the value of `VAR` from the process env.
/// - An UNSET or set-but-empty/whitespace `VAR` resolves to an empty string, so
///   a `required = false` credential is OMITTED rather than sent as a degenerate
///   empty/placeholder value (each variant's existing empty→`NoAuth` check then
///   collapses the provider to no-auth — the correct failure mode, NOT shipping
///   the literal `${...}`).
/// - A plain literal (no `${...}` / `env:` prefix) is returned verbatim.
/// - A malformed reference (e.g. `"${}"`) resolves to an empty string — but see
///   below: it is REFUSED before it ever reaches this function.
///
/// No error path: an unresolvable reference yields an empty string (omission),
/// never a panic and never the literal `${...}` reaching the wire.
///
/// # Omission applies to UNSET variables, not to malformed references
///
/// The empty-string-means-omit rule is the right answer for an optional
/// credential whose variable the target environment did not supply. It is the
/// WRONG answer for a MALFORMED reference — `${}`, `${A}://${B}`, `${WITH-DASH}`
/// — because no environment could ever supply those, so "omit" means every
/// backend request silently goes out unauthenticated.
///
/// [`AuthConfig::malformed_env_ref_field`] therefore rejects that shape at two
/// points upstream of here: [`crate::config::ServerConfig::validate`] at load
/// time and [`create_auth_provider`] at provider-build time. The empty-name arm
/// below is retained as a total-function fallback for direct callers, not as
/// the operative policy.
fn resolve_secret_ref(raw: &str) -> String {
    match parse_env_ref(raw) {
        // Plain literal — used verbatim.
        None => raw.to_string(),
        // Malformed reference (e.g. `"${}"`) → empty. Refused upstream by
        // `AuthConfig::malformed_env_ref_field`; kept here so the function stays
        // total. Spelled `Some("")` to match the sibling endpoint arm in
        // `crate::config::BackendSection::resolved_base_url`.
        Some("") => String::new(),
        Some(name) => std::env::var(name)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_default(),
    }
}

/// Expand every value in an api_key map, dropping entries that resolve to empty
/// (an unset `required = false` reference is omitted, not sent empty).
fn expand_api_key_map(map: &HashMap<String, String>) -> HashMap<String, String> {
    map.iter()
        .filter_map(|(k, v)| {
            let resolved = resolve_secret_ref(v);
            (!resolved.is_empty()).then(|| (k.clone(), resolved))
        })
        .collect()
}

/// Build a STATIC auth provider from `cfg`, shared as `Arc<dyn HttpAuthProvider>`.
///
/// For [`AuthConfig::OAuthPassthrough`], use [`create_passthrough_auth_provider`]
/// instead — without a token this returns a [`MissingTokenAuth`] (if required) or
/// [`NoAuth`], since the per-request token is not yet known at startup.
///
/// # Errors
///
/// Returns [`HttpConnectorError::Auth`] when a credential field is a MALFORMED
/// environment reference (see [`AuthConfig::malformed_env_ref_field`]). This is
/// the construction-time validation the fallible signature was reserved for: a
/// malformed reference used to resolve to the empty string and be OMITTED,
/// which silently produced an unauthenticated client.
///
/// An UNSET or empty variable is NOT an error — that remains the deliberate
/// omission policy (the provider collapses to [`NoAuth`]).
pub fn create_auth_provider(
    cfg: &AuthConfig,
) -> Result<Arc<dyn HttpAuthProvider>, HttpConnectorError> {
    // Refuse malformed references BEFORE resolution, so a value no environment
    // could ever satisfy fails loudly here instead of being silently omitted by
    // the empty-string path below. `ServerConfig::validate` catches the same
    // defect at load time; this guard also covers an `AuthConfig` built in code
    // or loaded by a path that skips validation.
    if let Some(field) = cfg.malformed_env_ref_field() {
        return Err(HttpConnectorError::Auth(format!(
            "[backend.auth].{field} is a malformed environment reference; a reference must be \
             exactly one `${{VAR}}` (name matching [A-Za-z0-9_]+) or `env:VAR` naming a single \
             variable. The value is not echoed here because a malformed reference is often a \
             mistyped literal secret."
        )));
    }
    let provider: Arc<dyn HttpAuthProvider> = match cfg {
        AuthConfig::None => Arc::new(NoAuth),
        AuthConfig::ApiKey {
            query_params,
            headers,
            ..
        } => {
            // Expand `${VAR}` / `env:VAR` references BEFORE building the provider
            // so the RESOLVED secret (not the literal placeholder) is applied to
            // outgoing requests. Unset references are dropped (omitted).
            let query_params = expand_api_key_map(query_params);
            let headers = expand_api_key_map(headers);
            let has_values = query_params.values().any(|v| !v.is_empty())
                || headers.values().any(|v| !v.is_empty());
            if has_values {
                Arc::new(ApiKeyAuth {
                    query_params,
                    headers,
                })
            } else {
                Arc::new(NoAuth)
            }
        },
        AuthConfig::Bearer { token, .. } => {
            // Resolve `${VAR}` / `env:VAR` BEFORE the empty-check so the RESOLVED
            // token (never the literal placeholder) reaches the wire; an unset
            // ref collapses to NoAuth (the correct failure mode).
            let token = resolve_secret_ref(token);
            if token.is_empty() {
                Arc::new(NoAuth)
            } else {
                Arc::new(BearerAuth { token })
            }
        },
        AuthConfig::Basic {
            username, password, ..
        } => {
            // Resolve both fields (username typically not a secret, but support
            // `${VAR}` for symmetry) BEFORE the empty-check.
            let username = resolve_secret_ref(username);
            let password = resolve_secret_ref(password);
            if username.is_empty() && password.is_empty() {
                Arc::new(NoAuth)
            } else {
                Arc::new(BasicAuth { username, password })
            }
        },
        AuthConfig::OAuth2ClientCredentials {
            token_url,
            client_id,
            client_secret,
            scopes,
            ..
        } => {
            // Resolve client_id + client_secret BEFORE the empty-check so the
            // RESOLVED secret (never the literal placeholder) is sent to the token
            // endpoint; an unset ref collapses to NoAuth.
            let client_id = resolve_secret_ref(client_id);
            let client_secret = resolve_secret_ref(client_secret);
            if client_id.is_empty() || client_secret.is_empty() {
                Arc::new(NoAuth)
            } else {
                Arc::new(OAuth2ClientCredentialsAuth::new(
                    token_url.clone(),
                    client_id,
                    client_secret,
                    scopes.clone(),
                ))
            }
        },
        AuthConfig::OAuthPassthrough { required, .. } => {
            if *required {
                Arc::new(MissingTokenAuth)
            } else {
                Arc::new(NoAuth)
            }
        },
    };
    Ok(provider)
}

/// Build an auth provider, capturing an `incoming_token` for the
/// [`AuthConfig::OAuthPassthrough`] per-request path (H1).
///
/// For passthrough configs the captured token is stored and forwarded by
/// [`OAuthPassthroughAuth::apply`] (preferring a per-request `inbound_token` when
/// one is also passed to `apply`). For all other configs this delegates to
/// [`create_auth_provider`].
///
/// # Errors
///
/// Propagates any error from [`create_auth_provider`] for non-passthrough configs.
pub fn create_passthrough_auth_provider(
    cfg: &AuthConfig,
    incoming_token: Option<String>,
) -> Result<Arc<dyn HttpAuthProvider>, HttpConnectorError> {
    match cfg {
        AuthConfig::OAuthPassthrough {
            target_header,
            required,
        } => Ok(Arc::new(OAuthPassthroughAuth {
            target_header: target_header.clone(),
            incoming_token: incoming_token.filter(|t| !t.is_empty()),
            required: *required,
        })),
        other => create_auth_provider(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_auth() {
        let auth = create_auth_provider(&AuthConfig::None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert!(headers.is_empty());
        assert!(query.is_empty());
    }

    #[tokio::test]
    async fn test_bearer_auth() {
        let cfg = AuthConfig::Bearer {
            token: "my_token".to_string(),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        // inbound_token is ignored by a static provider.
        auth.apply(&mut headers, &mut query, Some("client-tok"))
            .await
            .unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer my_token"
        );
        assert!(query.is_empty());
    }

    #[tokio::test]
    async fn test_basic_auth() {
        let cfg = AuthConfig::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        // base64("user:pass") = "dXNlcjpwYXNz"
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Basic dXNlcjpwYXNz"
        );
    }

    #[tokio::test]
    async fn test_api_key_query_param() {
        // D-04 london-tube path: api key carried as a query param (app_key).
        let cfg = AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), "secret123".to_string())]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert_eq!(query.get("app_key"), Some(&"secret123".to_string()));
        assert!(
            headers.is_empty(),
            "api-key-in-query must not touch headers"
        );
    }

    #[tokio::test]
    async fn test_api_key_query_param_expands_braced_env_ref() {
        // The RESOLVED ${VAR} value (not the literal `${...}`) reaches the wire.
        let var = "PMCP_TEST_TFL_APP_KEY_BRACED";
        std::env::set_var(var, "dummy");
        let cfg = AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), format!("${{{var}}}"))]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: false,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert_eq!(
            query.get("app_key"),
            Some(&"dummy".to_string()),
            "resolved env value lands on the query, not the literal ${{...}}"
        );
        std::env::remove_var(var);
    }

    #[tokio::test]
    async fn test_api_key_query_param_unset_ref_is_omitted() {
        // required=false + an UNSET ${VAR} → the param is omitted (not sent
        // empty, not the literal placeholder).
        let var = "PMCP_TEST_TFL_APP_KEY_UNSET";
        std::env::remove_var(var);
        let cfg = AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), format!("${{{var}}}"))]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: false,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert!(
            !query.contains_key("app_key"),
            "an unset required=false api_key ref is omitted, not sent empty/literal"
        );
    }

    #[test]
    fn test_resolve_api_key_value_forms() {
        // api_key now resolves through the shared `resolve_secret_ref` chokepoint.
        let var = "PMCP_TEST_RESOLVE_API_KEY_FORM";
        std::env::set_var(var, "resolved");
        assert_eq!(resolve_secret_ref(&format!("${{{var}}}")), "resolved");
        assert_eq!(resolve_secret_ref(&format!("env:{var}")), "resolved");
        assert_eq!(resolve_secret_ref("plain-literal"), "plain-literal");
        std::env::remove_var(var);
        assert_eq!(resolve_secret_ref(&format!("${{{var}}}")), "");
        assert_eq!(resolve_secret_ref("${}"), "");
    }

    #[tokio::test]
    async fn test_passthrough_forwards_inbound_token() {
        // H1 per-request path: passthrough forwards the inbound token.
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        };
        let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, Some("client-tok"))
            .await
            .unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer client-tok"
        );
    }

    #[tokio::test]
    async fn test_passthrough_custom_target_header() {
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "X-Forwarded-Token".to_string(),
            required: true,
        };
        let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, Some("client-tok"))
            .await
            .unwrap();
        assert_eq!(
            headers.get("X-Forwarded-Token").unwrap(),
            "Bearer client-tok"
        );
    }

    #[tokio::test]
    async fn test_passthrough_uses_construction_time_token() {
        // Construction-time capture path: inbound_token=None falls back to stored.
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        };
        let auth =
            create_passthrough_auth_provider(&cfg, Some("captured-tok".to_string())).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer captured-tok"
        );
    }

    #[tokio::test]
    async fn test_passthrough_required_missing_token_errors() {
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        };
        let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        let err = auth
            .apply(&mut headers, &mut query, None)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectorError::Auth(_)));
    }

    #[test]
    fn test_oauth_passthrough_documented_tag_deserializes() {
        // The documented `[backend.auth]` form is `type = "oauth_passthrough"`,
        // but `rename_all = "snake_case"` derives the tag `o_auth_passthrough`.
        // The `#[serde(alias)]` must accept the documented spelling.
        let cfg: AuthConfig = toml::from_str(r#"type = "oauth_passthrough""#)
            .expect("documented oauth_passthrough tag must deserialize via the serde alias");
        assert!(matches!(cfg, AuthConfig::OAuthPassthrough { .. }));
    }

    #[test]
    fn test_oauth2_client_credentials_documented_tag_deserializes() {
        let cfg: AuthConfig = toml::from_str(
            r#"
            type = "oauth2_client_credentials"
            token_url = "https://example.test/token"
            client_id = "${CID}"
            client_secret = "${CSECRET}"
            "#,
        )
        .expect("documented oauth2_client_credentials tag must deserialize via the serde alias");
        assert!(matches!(cfg, AuthConfig::OAuth2ClientCredentials { .. }));
    }

    #[test]
    fn test_snake_case_tag_still_deserializes_after_alias() {
        // The alias is ADDITIVE — the rename_all-derived `o_auth_passthrough`
        // tag (the canonical serialized form) must still round-trip.
        let cfg: AuthConfig = toml::from_str(r#"type = "o_auth_passthrough""#)
            .expect("canonical snake_case tag must still deserialize");
        assert!(matches!(cfg, AuthConfig::OAuthPassthrough { .. }));
    }

    #[tokio::test]
    async fn test_static_provider_ignores_inbound_token() {
        // T-90-01-06: a static provider must NOT leak the inbound token into its
        // output — it applies ONLY its configured credential.
        let bearer = create_auth_provider(&AuthConfig::Bearer {
            token: "static-tok".to_string(),
            required: true,
        })
        .unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        bearer
            .apply(&mut headers, &mut query, Some("client-tok"))
            .await
            .unwrap();
        let rendered = headers
            .get(reqwest::header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(rendered, "Bearer static-tok");
        assert!(
            !rendered.contains("client-tok"),
            "static provider must not forward the inbound token"
        );

        // Same for api-key-in-query.
        let apikey = create_auth_provider(&AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), "kkk".to_string())]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: true,
        })
        .unwrap();
        let mut headers2 = HeaderMap::new();
        let mut query2 = HashMap::new();
        apikey
            .apply(&mut headers2, &mut query2, Some("client-tok"))
            .await
            .unwrap();
        assert_eq!(query2.get("app_key"), Some(&"kkk".to_string()));
        assert!(
            !query2.values().any(|v| v.contains("client-tok")),
            "static api-key provider must not forward the inbound token"
        );
        assert!(headers2.is_empty());
    }

    #[tokio::test]
    async fn test_auth_error_display_no_secret() {
        // The error surfaced when a required token is missing must not echo a token.
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        };
        let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        let err = auth
            .apply(&mut headers, &mut query, None)
            .await
            .unwrap_err();
        let rendered = err.to_string();
        for forbidden in ["Bearer", "client-tok", "app_key", "https://"] {
            assert!(
                !rendered.contains(forbidden),
                "auth error Display must not echo {forbidden:?}; got {rendered:?}"
            );
        }
    }

    #[test]
    fn test_auth_config_deserializes_snake_case_tag() {
        let toml_src = r#"type = "bearer"
token = "abc"
"#;
        let cfg: AuthConfig = toml::from_str(toml_src).unwrap();
        assert!(matches!(cfg, AuthConfig::Bearer { .. }));
        assert!(cfg.is_required());
    }

    #[test]
    fn test_auth_config_default_is_none() {
        assert!(matches!(AuthConfig::default(), AuthConfig::None));
        assert!(!AuthConfig::None.is_required());
    }

    // -------------------------------------------------------------------------
    // Plan 90-11: single secret-resolution chokepoint across ALL variants.
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolve_secret_ref_forms() {
        let var = "PMCP_TEST_RESOLVE_SECRET_REF_FORM";
        std::env::set_var(var, "secret");
        assert_eq!(resolve_secret_ref(&format!("${{{var}}}")), "secret");
        assert_eq!(resolve_secret_ref(&format!("env:{var}")), "secret");
        assert_eq!(resolve_secret_ref("plain-literal"), "plain-literal");
        std::env::remove_var(var);
        // Unset / malformed → empty (omitted), never the literal.
        assert_eq!(resolve_secret_ref(&format!("${{{var}}}")), "");
        assert_eq!(resolve_secret_ref("${}"), "");
    }

    // NOTE: `test_parse_env_ref_distinguishes_literal_from_reference` moved with
    // the function to `crate::env_ref` (Phase 120 Plan 04 Task 2) — the grammar
    // is now defined and tested in one backend-neutral place. The resolution
    // POLICY tests (empty-on-unset, omission) stay here, because that policy is
    // credential-specific and deliberately differs from `base_url`'s.

    // -------------------------------------------------------------------------
    // A MALFORMED reference is REFUSED, never silently omitted.
    //
    // The unset-variable policy tested above (resolve to empty -> omit) is
    // deliberate and unchanged. A MALFORMED reference is a different defect:
    // `${}`, a composition like `${A}://${B}`, and a non-portable name like
    // `${TFL-APP-KEY}` all parse to the empty name, and no environment can ever
    // satisfy any of them. Omitting those silently sends an UNAUTHENTICATED
    // request with no error and no log line, so they get the same refusal
    // `[backend].base_url` already gets.
    // -------------------------------------------------------------------------

    #[test]
    fn create_auth_provider_refuses_malformed_bearer_token_ref() {
        let cfg = AuthConfig::Bearer {
            token: "${TFL-APP-KEY}".to_string(),
            required: true,
        };
        // `Arc<dyn HttpAuthProvider>` is not `Debug`, so `expect_err` is unavailable.
        let Err(err) = create_auth_provider(&cfg) else {
            panic!("a malformed ref must not build a provider");
        };
        let msg = err.to_string();
        assert!(msg.contains("token"), "the error names the field: {msg}");
        assert!(
            !msg.contains("TFL-APP-KEY"),
            "the error must not echo the configured value: {msg}"
        );
    }

    #[test]
    fn create_auth_provider_refuses_malformed_api_key_map_value() {
        // The map path is the worst shape: `expand_api_key_map` DROPS an entry
        // that resolves to empty, so the request went out with the query
        // parameter simply absent.
        let cfg = AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), "${TFL-APP-KEY}".to_string())]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: true,
        };
        let Err(err) = create_auth_provider(&cfg) else {
            panic!("a malformed ref must not build a provider");
        };
        let msg = err.to_string();
        assert!(
            msg.contains("query_params.app_key"),
            "the error names the offending map entry: {msg}"
        );
    }

    #[test]
    fn create_auth_provider_refuses_malformed_composition_in_every_variant() {
        // Each credential-bearing variant routes through the same guard.
        let composed = "${A}://${B}".to_string();
        let variants = [
            AuthConfig::Bearer {
                token: composed.clone(),
                required: false,
            },
            AuthConfig::Basic {
                username: "user".to_string(),
                password: composed.clone(),
                required: false,
            },
            AuthConfig::OAuth2ClientCredentials {
                token_url: "https://example.com/token".to_string(),
                client_id: "id".to_string(),
                client_secret: composed.clone(),
                scopes: vec![],
                required: false,
            },
            AuthConfig::ApiKey {
                query_params: HashMap::new(),
                headers: [("X-Api-Key".to_string(), composed)].into_iter().collect(),
                required: false,
            },
        ];
        for cfg in variants {
            assert!(
                create_auth_provider(&cfg).is_err(),
                "every credential-bearing variant refuses a malformed ref: {cfg:?}"
            );
        }
    }

    #[test]
    fn malformed_api_key_report_is_deterministic_across_map_orderings() {
        // `query_params` / `headers` are `HashMap`s, so a naive `.find()` would
        // name a DIFFERENT field run to run when more than one entry is
        // malformed — an error message that changes under the reader's feet.
        for _ in 0..20 {
            let cfg = AuthConfig::ApiKey {
                query_params: [
                    ("zeta".to_string(), "${}".to_string()),
                    ("alpha".to_string(), "${}".to_string()),
                    ("mid".to_string(), "${}".to_string()),
                ]
                .into_iter()
                .collect(),
                headers: HashMap::new(),
                required: true,
            };
            let Err(err) = create_auth_provider(&cfg) else {
                panic!("malformed refs are refused");
            };
            let msg = err.to_string();
            assert!(
                msg.contains("query_params.alpha"),
                "the lexicographically first offender is reported every time: {msg}"
            );
        }
    }

    mod malformed_ref_properties {
        use super::super::{create_auth_provider, AuthConfig};
        use crate::env_ref::is_valid_env_var_name;
        use proptest::prelude::*;

        proptest! {
            /// PROPERTY (CLAUDE.md ALWAYS): the credential guard is EXACTLY the
            /// grammar's malformed set — a braced credential is refused if and
            /// only if its interior is not a name an environment could be told
            /// to set. Pinning the guard to `is_valid_env_var_name` rather than
            /// to a hand-listed set of bad shapes is what keeps it from drifting
            /// away from `parse_env_ref` and re-opening the silent-omission
            /// hole for some shape nobody enumerated.
            #[test]
            fn braced_credential_is_refused_exactly_when_its_name_is_unsettable(
                interior in "\\PC{0,64}"
            ) {
                let cfg = AuthConfig::Bearer {
                    token: format!("${{{interior}}}"),
                    required: true,
                };
                let settable = is_valid_env_var_name(&interior);
                prop_assert_eq!(cfg.malformed_env_ref_field().is_some(), !settable);
                prop_assert_eq!(create_auth_provider(&cfg).is_err(), !settable);
            }

            /// PROPERTY: the guard NEVER refuses a plain literal. A password is
            /// arbitrary bytes and frequently contains `$`, `{` or `}`; refusing
            /// one would turn this fix into an outage of its own.
            #[test]
            fn plain_literal_credentials_are_never_refused(raw in "\\PC{0,64}") {
                prop_assume!(!raw.starts_with("env:") && !raw.starts_with("${"));
                let cfg = AuthConfig::Basic {
                    username: "svc".to_string(),
                    password: raw,
                    required: true,
                };
                prop_assert!(cfg.malformed_env_ref_field().is_none());
                prop_assert!(create_auth_provider(&cfg).is_ok());
            }

            /// PROPERTY/FUZZ: the guard is total — arbitrary credential text
            /// yields a verdict, never a panic. Config loading must not unwind
            /// on adversarial input.
            #[test]
            fn guard_never_panics_on_arbitrary_credential_text(raw in "\\PC{0,256}") {
                let cfg = AuthConfig::Bearer { token: raw, required: true };
                let _ = cfg.malformed_env_ref_field();
                let _ = create_auth_provider(&cfg);
            }
        }
    }

    #[tokio::test]
    async fn unset_wellformed_ref_still_collapses_to_no_auth() {
        // REGRESSION GUARD for the fix above: refusing MALFORMED references must
        // not disturb the deliberate UNSET-variable policy. An optional
        // credential whose variable simply is not exported is still omitted.
        let var = "PMCP_TEST_UNSET_POLICY_UNCHANGED";
        std::env::remove_var(var);
        let cfg = AuthConfig::Bearer {
            token: format!("${{{var}}}"),
            required: false,
        };
        let auth = create_auth_provider(&cfg).expect("a well-formed ref still builds a provider");
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert!(
            !headers.contains_key(reqwest::header::AUTHORIZATION),
            "an unset optional credential is omitted, not refused"
        );
    }

    #[tokio::test]
    async fn test_bearer_resolves_braced_env_ref() {
        let var = "PMCP_TEST_BEARER_BRACED_PAT";
        std::env::set_var(var, "ghp_abc");
        let cfg = AuthConfig::Bearer {
            token: format!("${{{var}}}"),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        let rendered = headers
            .get(reqwest::header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(rendered, "Bearer ghp_abc");
        assert!(
            !rendered.contains("${"),
            "the literal ${{...}} must never reach the Authorization header"
        );
        std::env::remove_var(var);
    }

    #[tokio::test]
    async fn test_bearer_resolves_env_prefix_ref() {
        let var = "PMCP_TEST_BEARER_ENV_PAT";
        std::env::set_var(var, "ghp_xyz");
        let cfg = AuthConfig::Bearer {
            token: format!("env:{var}"),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer ghp_xyz"
        );
        std::env::remove_var(var);
    }

    #[tokio::test]
    async fn test_bearer_unset_ref_collapses_to_no_auth() {
        let var = "PMCP_TEST_BEARER_UNSET_PAT";
        std::env::remove_var(var);
        let cfg = AuthConfig::Bearer {
            token: format!("${{{var}}}"),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        // Unset ref → NoAuth: no Authorization header, and CERTAINLY not the literal.
        assert!(headers.is_empty());
        assert!(query.is_empty());
    }

    #[tokio::test]
    async fn test_basic_resolves_password_braced_env_ref() {
        use base64::Engine;
        let var = "PMCP_TEST_BASIC_BRACED_PW";
        std::env::set_var(var, "s3cr3t");
        let cfg = AuthConfig::Basic {
            username: "u".to_string(),
            password: format!("${{{var}}}"),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        let rendered = headers
            .get(reqwest::header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap();
        let expected = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("u:s3cr3t")
        );
        assert_eq!(rendered, expected);
        assert!(
            !rendered.contains("${"),
            "the literal ${{...}} must never reach the Basic credential"
        );
        std::env::remove_var(var);
    }

    #[tokio::test]
    async fn test_basic_resolves_password_env_prefix_ref() {
        use base64::Engine;
        let var = "PMCP_TEST_BASIC_ENV_PW";
        std::env::set_var(var, "p4ss");
        let cfg = AuthConfig::Basic {
            username: "user".to_string(),
            password: format!("env:{var}"),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        let expected = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("user:p4ss")
        );
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            expected.as_str()
        );
        std::env::remove_var(var);
    }

    #[tokio::test]
    async fn test_oauth2_resolves_client_secret_via_token_endpoint() {
        // Drive fetch_token against a wiremock token endpoint asserting the
        // RESOLVED client_secret (not the literal `${...}`) is in the form body.
        use wiremock::matchers::{body_string_contains, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let var = "PMCP_TEST_OAUTH2_BRACED_CS";
        std::env::set_var(var, "xyz");

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("client_secret=xyz"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "issued-token"
            })))
            .mount(&server)
            .await;

        let cfg = AuthConfig::OAuth2ClientCredentials {
            token_url: format!("{}/token", server.uri()),
            client_id: "cid".to_string(),
            client_secret: format!("${{{var}}}"),
            scopes: vec![],
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        // apply() triggers fetch_token; the wiremock body matcher (client_secret=xyz)
        // FAILS the request (404) unless the resolved secret was sent — so success
        // proves the resolved `xyz` (not the literal `${VAR}`) reached the wire.
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer issued-token"
        );
        std::env::remove_var(var);
    }

    #[tokio::test]
    async fn test_oauth2_unset_secret_collapses_to_no_auth() {
        let var = "PMCP_TEST_OAUTH2_UNSET_CS";
        std::env::remove_var(var);
        let cfg = AuthConfig::OAuth2ClientCredentials {
            token_url: "http://127.0.0.1:1/token".to_string(),
            client_id: "cid".to_string(),
            client_secret: format!("${{{var}}}"),
            scopes: vec![],
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        // Unset secret → NoAuth: apply does NOT attempt any network fetch.
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert!(headers.is_empty());
        assert!(query.is_empty());
    }
}
