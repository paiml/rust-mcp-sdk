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

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::{Arc, Once, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::sleep;
use url::Url;

use crate::client::auth::{AuthorizationServerExtras, OidcDiscoveryClient, TokenExchangeClient};
use crate::client::http_middleware::HttpMiddlewareChain;
use crate::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
use crate::error::{Error, Result};
use crate::server::auth::oauth2::OidcDiscoveryMetadata;
use crate::shared::credential_file::FileCredentialStore;
use crate::shared::credential_store::{
    normalize_server_key, CredentialKey, CredentialStore, StoredCredentials,
};
use crate::shared::http_body_cap::{
    collect_reqwest_body_within_cap, is_body_over_cap, DEFAULT_AUTH_RESPONSE_BYTES,
};
use crate::shared::oauth_validation::{
    derive_application_type, iss_presence_from, parse_iss_env_value,
    validate_authorization_response, AuthorizationRequestRecord, IssPresence,
};
use crate::shared::pkce::{code_challenge_s256, generate_code_verifier, generate_state};

/// The environment variable an operator sets to override RFC 9207 `iss`
/// strictness without a redeploy or a code change.
///
/// Accepted values are `strict` and `lenient`, parsed case-insensitively after
/// trimming by
/// [`parse_iss_env_value`](crate::shared::oauth_validation::parse_iss_env_value).
/// A value the SDK does not recognise is **announced** with a `tracing::warn!`
/// naming this variable and its two accepted values, then ignored — it never
/// silently leaves validation lenient.
const ISS_VALIDATION_ENV_VAR: &str = "PMCP_OAUTH_ISS_VALIDATION";

/// The largest HTTP request line the loopback callback listener will read.
///
/// Any process on the local machine can connect to the callback port and send
/// an unbounded request line. The authorization response is a query string —
/// a `code`, a `state` and possibly an `iss`, each well under 100 bytes — never
/// a payload, so refusing at 16 `KiB` costs nothing legitimate. The read is
/// bounded at the socket, so an oversized line is refused without the whole
/// line ever being allocated.
///
/// This is the transport-level twin of
/// [`MAX_CALLBACK_QUERY_BYTES`](crate::shared::oauth_validation::MAX_CALLBACK_QUERY_BYTES),
/// which bounds the query the pure validator will parse.
pub const MAX_CALLBACK_REQUEST_LINE_BYTES: usize = 16_384;

/// The response served when the callback validated. Byte-identical to the page
/// this module has always served on success.
const CALLBACK_SUCCESS_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
     <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
     <h1 style='color: green;'>Authentication Successful!</h1>\
     <p>You can close this window and return to the terminal.</p>\
     </body></html>";

/// The response served when the callback did NOT validate. Byte-identical to
/// the page this module has always served on failure, and deliberately so: it
/// carries no authorization-server-supplied text. RFC 9207 forbids acting on or
/// displaying an `error_description` that arrived behind a failing `iss`, so the
/// failure page must stay fixed bytes.
const CALLBACK_FAILURE_RESPONSE: &str =
    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
     <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
     <h1 style='color: red;'>Authentication Failed</h1>\
     <p>No authorization code received. Please try again.</p>\
     </body></html>";

/// The scope an OAuth client asks for when it wants a refresh token (SEP-2207).
///
/// It appears at exactly two protocol stages and means a DIFFERENT thing at
/// each, which is why this module writes it in two places and deliberately not
/// in a third:
///
/// | Stage | What writing it there means | Written here? |
/// |---|---|---|
/// | the Dynamic Client Registration request's `scope` | CLIENT METADATA: what this client is permitted to ask for | yes, when advertised |
/// | the authorization request's `scope` | the ASK itself — the only stage at which requesting it does anything | yes, when advertised |
/// | a refresh request's `scope` | a scope CHANGE on an existing grant | **no, never** |
///
/// The third row is the one that matters. RFC 6749 §6 permits a refresh request
/// to narrow the granted scope and never to widen it, so introducing a scope at
/// refresh that was never granted can have the authorization server refuse a
/// refresh that would otherwise have succeeded. Refresh sends only what was
/// GRANTED, or no `scope` at all.
///
/// Both writes are conditioned on the authorization server ADVERTISING the scope
/// in its `scopes_supported`, because SEP-2207 states the client MAY request it
/// when the server advertises support — the condition is the whole rule.
const OFFLINE_ACCESS_SCOPE: &str = "offline_access";

/// The largest Dynamic Client Registration response body this client will read,
/// on the success path and on the rejection path alike.
///
/// Defined as the shared authorization-surface cap rather than as a second
/// literal, so a hostile registration endpoint faces ONE number and the refusal
/// message that names it cannot drift from the number actually enforced.
const MAX_DCR_RESPONSE_BYTES: usize = DEFAULT_AUTH_RESPONSE_BYTES;

/// The file name the issuer-keyed credential store uses inside whichever
/// directory holds it.
///
/// This is `default_credential_path`'s own file name, and
/// `the_credential_store_file_name_matches_default_credential_path` asserts the
/// two cannot drift. They must agree: a caller who sets
/// [`OAuthConfig::cache_file`] and a caller who does not would otherwise end up
/// with two different credential stores inside one directory, and a login
/// through one would be invisible to the other.
const CREDENTIAL_STORE_FILE_NAME: &str = "oauth-cache.json";
/// A stable, non-reversible identifier for a token, safe to put in a log line.
///
/// A PREFIX of a live access token is still token material: 20 characters is
/// enough to correlate it against a leak elsewhere, and on some authorization
/// servers it is enough to identify the issuing key or the tenant. A prefix of
/// the SHA-256 digest identifies the same token across log lines without being
/// any part of it.
///
/// Twelve hex characters is 48 bits — far beyond collision range for the
/// handful of tokens one process holds, and far too short to brute-force back
/// to a token carrying at least 128 bits of entropy.
///
/// The result is deliberately prefixed with `sha256:` so a reader cannot
/// mistake it for the token itself, which is the failure mode a bare hex string
/// invites.
const FINGERPRINT_HEX_CHARS: usize = 12;

/// Lowercase hex digits for [`token_fingerprint`].
///
/// A table and a loop rather than a `format!` per byte: `clippy::format_collect`
/// rejects the latter, and this shape allocates exactly once. Declared at module
/// level because `clippy::items_after_statements` rejects a `const` inside the
/// function body.
const HEX_DIGITS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn token_fingerprint(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(FINGERPRINT_HEX_CHARS);
    for byte in digest.iter().take(FINGERPRINT_HEX_CHARS / 2) {
        hex.push(HEX_DIGITS[usize::from(byte >> 4)]);
        hex.push(HEX_DIGITS[usize::from(byte & 0x0f)]);
    }
    format!("sha256:{hex}")
}

/// Absolute Unix seconds, with a pre-epoch clock reported as `0` rather than
/// panicking.
///
/// One function for the whole module so an expiry computed on the
/// authorization-code path and one computed on the device-code path cannot
/// disagree about what "now" means. The previous `cache_token` form
/// (`.duration_since(UNIX_EPOCH).unwrap()`) panicked in library code on a clock
/// set before 1970, which is a denial of service a caller cannot catch.
fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Compose a `scope` value from the configured scopes plus
/// [`OFFLINE_ACCESS_SCOPE`], and only when the authorization server advertises
/// that scope in its `scopes_supported`.
///
/// The result is **order-stable** (configured scopes keep their order,
/// `offline_access` is appended last) and **deduplicated** (a caller who already
/// listed `offline_access` gets one entry, not two — a duplicated scope token is
/// legal but is the kind of thing a strict authorization server rejects).
///
/// It takes a slice and returns a fresh `Vec`, so `OAuthConfig::scopes` is never
/// mutated. That is not a stylistic preference: `scopes` is a public field on a
/// public struct, and a caller who reuses one [`OAuthConfig`] across two flows
/// against an advertising server must not watch it grow an entry per flow.
fn compose_scopes_with_offline_access(
    configured: &[String],
    scopes_supported: &[String],
) -> Vec<String> {
    let mut composed: Vec<String> = Vec::with_capacity(configured.len() + 1);
    for scope in configured {
        if !composed.iter().any(|held| held == scope) {
            composed.push(scope.clone());
        }
    }

    let advertised = scopes_supported
        .iter()
        .any(|scope| scope == OFFLINE_ACCESS_SCOPE);
    if advertised && !composed.iter().any(|held| held == OFFLINE_ACCESS_SCOPE) {
        composed.push(OFFLINE_ACCESS_SCOPE.to_string());
    }

    composed
}

/// Write SEP-837's `application_type` onto a registration request, DERIVING it
/// from the `redirect_uris` the request is actually registering.
///
/// Returns the value that will go on the wire, so the caller can compare it
/// against whatever the authorization server echoes back.
///
/// # Why derive from `redirect_uris` rather than from `config.redirect_port`
///
/// The redirect URIs are the values the authorization server will enforce its
/// per-type constraints against. Deriving from anything else lets the declared
/// type and the registered URIs drift apart the moment the URI shape changes,
/// which is precisely the mismatch SEP-837 warns registration will be rejected
/// for.
///
/// # The override path
///
/// An `application_type` that is ALREADY set on the request is kept, never
/// overwritten. `DcrRequest::set_application_type` performs no validation
/// exactly so that it can serve as the authoritative override for this
/// derivation (116-03 / D-09), and silently clobbering it here would delete that
/// path.
///
/// # Errors
///
/// Propagates [`derive_application_type`]'s refusal for an empty, unparseable,
/// cleartext-remote or MIXED `redirect_uris` vector. A mixed vector is an error
/// and never a pick: choosing one classification for a vector containing both
/// registers a client whose declared type contradicts some of its own redirect
/// URIs, which is an open-redirect primitive (D-10).
fn apply_application_type(request: &mut DcrRequest) -> Result<String> {
    if let Some(explicit) = request.application_type() {
        return Ok(explicit.to_string());
    }

    let derived = derive_application_type(&request.redirect_uris)?;
    request.set_application_type(derived.as_str());
    Ok(derived.as_str().to_string())
}

/// Whether the authorization server registered this client under a DIFFERENT
/// `application_type` than the one the registration request asked for.
///
/// Returns `Some((sent, echoed))` only for a genuine divergence. Both an EQUAL
/// echo and an ABSENT echo return `None`, and the second of those is the rule
/// worth stating: RFC 7591 § 3.2.1 does not require the server to echo the
/// metadata it accepted, so an omission means "no answer", never "a different
/// answer". Treating an omission as divergence would make every RFC-conformant
/// terse registration server produce a warning about a disagreement that never
/// happened.
///
/// A divergence is deliberately NOT an error anywhere it is used. The same
/// RFC § 3.2.1 permits the server to modify any requested client metadata, so
/// failing here would turn a legal server behaviour into an outage (T-116-36,
/// disposition `accept`). The value of detecting it is diagnostic: the client
/// is now registered under constraints it did not choose.
///
/// This is a pure function over two borrowed strings precisely so the RULE is
/// testable without a network, without a log subscriber, and without adding a
/// field to any public constructible type.
fn application_type_divergence(sent: &str, echoed: Option<&str>) -> Option<(String, String)> {
    match echoed {
        Some(registered) if registered != sent => Some((sent.to_string(), registered.to_string())),
        _ => None,
    }
}

/// The most characters of an authorization-server-supplied `error` or
/// `error_description` that a registration-rejection message will reproduce.
///
/// The body itself is already bounded by [`MAX_DCR_RESPONSE_BYTES`], but a
/// 1 `MiB` `error_description` is still an echo channel wearing a
/// specification-approved hat (T-116-37). Reproducing a short prefix keeps the
/// message actionable while capping what a hostile registration endpoint can
/// push into a developer's terminal and log aggregator.
const MAX_DCR_ERROR_FIELD_CHARS: usize = 200;

/// The RFC 7591 § 3.2.2 error fields of a rejected registration, projected out
/// of an entirely authorization-server-controlled body.
///
/// Every field is `Option` because none of them is guaranteed: the body may be
/// a conformant error object, an unrelated JSON document, an HTML error page or
/// nothing at all. The projection never fails and never panics — an
/// unrecognisable body simply yields two `None`s, and the rejection message
/// then names the HTTP status and what the client sent, which is still
/// actionable.
#[derive(Debug, Default)]
struct DcrRejectionFields {
    /// RFC 7591 § 3.2.2 `error` — a single ASCII error code such as
    /// `invalid_redirect_uri`.
    error: Option<String>,
    /// RFC 7591 § 3.2.2 `error_description` — human-readable detail.
    error_description: Option<String>,
}

/// Project a rejected registration's body onto its RFC 7591 § 3.2.2 error
/// fields, taking only string values and only a bounded prefix of each.
///
/// # Why `as_str` and not a stringification
///
/// The body is attacker-influenced input. A non-string `error` is `None`
/// rather than a coerced `"42"`, exactly as `DcrResponse::application_type`
/// treats the same class of value (116-03). This is the only place the
/// rejection path reads server-supplied text at all, so it is the only place
/// that rule has to hold.
fn dcr_rejection_fields(body: &[u8]) -> DcrRejectionFields {
    let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(body) else {
        return DcrRejectionFields::default();
    };

    DcrRejectionFields {
        error: bounded_error_field(parsed.get("error")),
        error_description: bounded_error_field(parsed.get("error_description")),
    }
}

/// Take a JSON value's string content, truncated to
/// [`MAX_DCR_ERROR_FIELD_CHARS`] on a character boundary.
///
/// The truncation marker names how many characters were dropped and reproduces
/// none of them, so the fact of truncation is visible without the truncated
/// content leaking through the notice itself.
fn bounded_error_field(value: Option<&serde_json::Value>) -> Option<String> {
    let text = value.and_then(serde_json::Value::as_str)?;
    let total = text.chars().count();
    if total <= MAX_DCR_ERROR_FIELD_CHARS {
        return Some(text.to_string());
    }

    let kept: String = text.chars().take(MAX_DCR_ERROR_FIELD_CHARS).collect();
    Some(format!(
        "{kept}… [truncated: {} of {total} characters withheld]",
        total - MAX_DCR_ERROR_FIELD_CHARS
    ))
}

/// Compose the error for a registration the authorization server REJECTED.
///
/// SEP-837 ¶3 obliges an MCP client to surface a MEANINGFUL error when
/// registration is refused, because the most common cause is a redirect-URI
/// constraint tied to the very `application_type` this module now sends. A bare
/// status code does not tell a developer which of their two knobs to turn, so
/// the message names FOUR things:
///
/// | Named | Why |
/// |---|---|
/// | the HTTP status | distinguishes a refusal from a transport failure |
/// | the server's `error` / `error_description` | the server's own reason, when it gave one |
/// | the `application_type` that was sent | half of the rejected pair |
/// | the `redirect_uris` that were sent | the other half |
///
/// # What is deliberately absent
///
/// No raw body content beyond the two parsed, length-bounded fields above. The
/// rejection path must not become the echo channel the bounded read exists to
/// close (T-116-37).
///
/// SEP-837's OPTIONAL retry with an adjusted `application_type` is also
/// deliberately not implemented. The specification says clients MAY retry; an
/// automatic retry would silently register the client under a type its operator
/// did not choose, which is the opposite of surfacing a meaningful error.
fn registration_rejected(
    status: reqwest::StatusCode,
    fields: &DcrRejectionFields,
    sent_application_type: &str,
    sent_redirect_uris: &[String],
) -> Error {
    let server_reason = match (&fields.error, &fields.error_description) {
        (Some(code), Some(description)) => {
            format!("error={code}; error_description={description}")
        },
        (Some(code), None) => format!("error={code}"),
        (None, Some(description)) => format!("error_description={description}"),
        (None, None) => {
            "the response body carried no RFC 7591 section 3.2.2 `error` field".to_string()
        },
    };

    Error::internal(format!(
        "DCR failed ({status}): the authorization server rejected this dynamic client \
         registration. Server reason: {server_reason}\n\
         \n\
         The registration that was rejected declared application_type=\"{sent_application_type}\" \
         with redirect_uris={sent_redirect_uris:?}. Those two are the pair an OIDC authorization \
         server enforces its redirect-URI constraints over, so they are what to change.\n\
         \n\
         No other part of the response body is reproduced here. Pass a pre-registered client_id \
         to skip DCR."
    ))
}

/// What one completed Dynamic Client Registration produced.
///
/// Exists so `registered_application_type` can leave
/// `OAuthHelper::do_dynamic_client_registration` at all. The obvious
/// alternative — a new field on [`AuthorizationResult`] — was considered and
/// REJECTED: that struct is public, all-`pub`-field and not
/// `#[non_exhaustive]`, so adding a field is `cargo-semver-checks`'
/// `constructible_struct_adds_field`, a MAJOR break, and avoiding exactly that
/// class of break is what this whole phase is built around.
/// `StoredCredentials` (116-05) has PRIVATE fields and an inherent
/// `with_registered_application_type` builder for the same value, which is why
/// the persistence hop 116-11 adds costs no semver event either.
#[derive(Debug)]
struct DcrOutcome {
    /// The parsed RFC 7591 registration response.
    response: crate::server::auth::provider::DcrResponse,
    /// The `application_type` this client ended up registered under — the
    /// server's echoed value when it echoed one, otherwise the value that was
    /// sent.
    registered_application_type: String,
}

/// How the interactive authorization URL reaches a human.
///
/// **This is a platform seam, not test scaffolding.** The interactive CLI flow
/// is one caller and not the only caller: a headless CI runner, a container
/// without a display, an SSH or remote-desktop session, or a hosting platform
/// that relays the URL through its own UI all want to PRINT or forward the URL
/// rather than open a window on whatever machine the process happens to be on.
/// Implement this trait to do that. It must not be hidden behind
/// `#[doc(hidden)]`.
///
/// A useful consequence is that the flow becomes testable end to end: a test
/// launcher can read the `state` and `code_challenge` out of the URL it
/// receives and deliver a callback, with no browser window and no human.
///
/// # Errors
///
/// [`BrowserLauncher::open`] returns `Err` only when the URL could not be
/// delivered to a human **at all**. The flow then aborts rather than waiting
/// five minutes for a callback nobody will deliver. A launcher that has a
/// fallback — as [`SystemBrowserLauncher`] does, because the flow already
/// logged the URL for manual entry — should return `Ok(())`.
pub trait BrowserLauncher: Send + Sync + std::fmt::Debug {
    /// Deliver the authorization URL to the human completing the flow.
    fn open(&self, url: &str) -> Result<()>;
}

/// The default [`BrowserLauncher`]: opens the platform's browser.
///
/// Returns `Ok(())` even when the platform browser cannot be opened, matching
/// this module's long-standing behaviour — the flow has already logged the URL
/// with "If the browser doesn't open, visit: …", so a human can still complete
/// the flow by pasting it. Aborting there would remove a working manual path.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemBrowserLauncher;

impl BrowserLauncher for SystemBrowserLauncher {
    fn open(&self, url: &str) -> Result<()> {
        if let Err(e) = webbrowser::open(url) {
            tracing::warn!(
                "Failed to open browser: {}. Please open the URL manually.",
                e
            );
        }
        Ok(())
    }
}

// Re-export RFC 7591 DCR types from the authoritative server-side definitions
// so library users can construct DCR requests via `pmcp::client::oauth::DcrRequest`.
// Source of truth: src/server/auth/provider.rs:302-382.
pub use crate::server::auth::provider::{DcrRequest, DcrResponse};

/// Whether an [`OAuthHelper`] may fall back to an INTERACTIVE browser login.
///
/// Select it with [`OAuthHelper::with_interactivity`]. [`Self::Interactive`] is
/// the default, so a helper that never calls that method behaves exactly as it
/// always has.
///
/// # What [`Self::RefreshOnly`] is for, and what it costs
///
/// With the default mode, a headless runtime that cannot complete a browser
/// login pays for the attempt anyway: [`OAuthHelper::get_access_token`] binds a
/// loopback listener nothing can reach, hands an authorization URL to a browser
/// nobody can see, and then waits **five minutes** for a callback that will
/// never arrive — per attempt. In a Lambda or a Worker that is five minutes of
/// billed wall clock ending in a timeout that does not say what is actually
/// wrong.
///
/// [`Self::RefreshOnly`] turns that into an immediate
/// [`Error::reauth_required`], which a caller can convert into whatever its own
/// runtime calls a consent-required condition — an operator notification, a
/// queued re-login, a `401` to the user. The cost is that a `RefreshOnly`
/// helper can never obtain credentials it does not already have: the mode
/// narrows the FALL-BACK, not the cache and not the refresh.
///
/// # Why this is a mode and not environment sniffing
///
/// Sniffing (`DISPLAY`, `SSH_TTY`, `AWS_LAMBDA_FUNCTION_NAME`, …) guesses, and
/// the two ways of guessing wrong are not symmetric: guessing "interactive"
/// when it is not costs five minutes and then fails anyway, while guessing
/// "headless" when it is not breaks a CLI login that would have worked. A
/// caller always knows which one it is; the SDK does not.
///
/// # The guarantee
///
/// Under [`Self::RefreshOnly`] the interactive path is unreachable BY
/// CONSTRUCTION rather than skipped by a branch: the arm that handles it calls
/// an associated function that has no `self`, and therefore no access to the
/// configured [`BrowserLauncher`] and no route to the loopback listener. Both
/// public entry points check.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::client::oauth::{Interactivity, OAuthConfig, OAuthHelper};
///
/// # async fn example() -> pmcp::Result<()> {
/// let helper = OAuthHelper::new(OAuthConfig::default())?
///     .with_interactivity(Interactivity::RefreshOnly);
///
/// match helper.get_access_token().await {
///     Ok(token) => println!("bearer {token}"),
///     Err(e) if e.is_reauth_required() => {
///         // Actionable in milliseconds instead of a five-minute timeout.
///         eprintln!("a human must log in again at {:?}", e.reauth_issuer());
///     },
///     Err(e) => return Err(e),
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Interactivity {
    /// Today's behaviour, unchanged: when no stored credential can serve the
    /// request, run the authorization-code flow — bind a loopback listener,
    /// open a browser and wait for the callback.
    #[default]
    Interactive,
    /// Serve the request from stored credentials or a refresh, or fail with
    /// [`Error::reauth_required`]. No browser is opened and no listener is
    /// bound.
    RefreshOnly,
}

/// OAuth configuration for CLI authentication flows.
///
/// # Migration note (pmcp 2.5.0)
///
/// `client_id` changed from `String` to `Option<String>` to support RFC 7591
/// Dynamic Client Registration. Existing callers that passed a pre-registered
/// id must now wrap it in `Some(...)`:
///
/// ```rust,ignore
/// // Before (pmcp < 2.5.0):
/// OAuthConfig { client_id: "my-client".to_string(), /* ... */ }
/// // After (pmcp 2.5.0+):
/// OAuthConfig {
///     client_id: Some("my-client".to_string()),
///     client_name: None,
///     dcr_enabled: false,
///     /* ... */
/// }
/// ```
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// OAuth issuer URL (e.g., `https://auth.example.com`).
    /// If `None`, will auto-discover from MCP server URL.
    pub issuer: Option<String>,
    /// MCP server URL for auto-discovery (required if issuer is `None`).
    pub mcp_server_url: Option<String>,
    /// OAuth client ID. When `None` and `dcr_enabled` is `true` and the
    /// discovery metadata advertises a `registration_endpoint`, the SDK
    /// auto-performs RFC 7591 Dynamic Client Registration to obtain one.
    pub client_id: Option<String>,
    /// Client name advertised to the authorization server during DCR
    /// (RFC 7591 §2). Only consulted when DCR fires. Falls back to the
    /// literal `"pmcp-sdk"` if `None` at DCR time.
    pub client_name: Option<String>,
    /// Enable RFC 7591 Dynamic Client Registration when `client_id` is
    /// `None` and the server advertises a `registration_endpoint`.
    /// Defaults to `true` via `Default::default()`; set to `false` to
    /// opt out and require an explicit `client_id`.
    pub dcr_enabled: bool,
    /// OAuth scopes to request.
    pub scopes: Vec<String>,
    /// Cache file path for storing tokens.
    pub cache_file: Option<PathBuf>,
    /// Redirect port for localhost callback (default: 8080).
    pub redirect_port: u16,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            issuer: None,
            mcp_server_url: None,
            client_id: None,
            client_name: None,
            dcr_enabled: true,
            scopes: Vec::new(),
            cache_file: None,
            redirect_port: 8080,
        }
    }
}

/// Result of a successful OAuth authorization flow, carrying the full set of
/// artifacts a cache consumer needs to persist and later refresh.
///
/// # Field semantics
///
/// - `access_token`: The bearer token. Put in `Authorization: Bearer <...>` headers.
/// - `refresh_token`: Present when the IdP returned one (Okta, Auth0, Keycloak
///   with offline_access). `None` when the IdP does not issue refresh tokens.
///
///   **Device-code flow (RFC 8628):** When `OAuthHelper` falls back from
///   authorization-code to device-code (e.g., the IdP does not support
///   localhost callbacks), `refresh_token` may be `None` because RFC 8628 §3.5
///   does NOT require the token response to include a `refresh_token`. Users
///   will need to re-run `cargo pmcp auth login` when the access_token
///   expires on such IdPs. `issuer` is still populated from discovery;
///   `client_id` is whatever was passed in `OAuthConfig` (or DCR-issued if
///   DCR fired); `expires_at` captures whatever `expires_in` the token
///   response provided (or `None` if absent).
/// - `expires_at`: Absolute unix seconds (not `expires_in` relative). `None`
///   when the IdP omitted `expires_in` from the token response.
/// - `scopes`: The scopes the IdP actually granted. May differ from
///   `config.scopes` (the requested scopes) when the server downgrades or
///   expands them.
/// - `issuer`: The effective issuer — caller-provided if present, else the
///   value discovered from `.well-known/openid-configuration`. Always `Some`
///   for a successful flow.
/// - `client_id`: The effective client_id — the DCR-issued id when DCR fired,
///   or the caller-provided value otherwise. Always populated.
#[derive(Debug, Clone)]
pub struct AuthorizationResult {
    /// Bearer access token.
    pub access_token: String,
    /// Refresh token, if the IdP issued one.
    pub refresh_token: Option<String>,
    /// Absolute expiration time (unix seconds).
    pub expires_at: Option<u64>,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Effective issuer (caller-provided or discovered).
    pub issuer: Option<String>,
    /// Effective client_id (DCR-issued or caller-provided).
    pub client_id: String,
}

/// The client identity one flow resolved, plus what dynamic registration
/// registered it as.
///
/// Exists because `client_id` alone is not enough to persist a credential
/// record: the `application_type` the authorization server registered (116-10)
/// has to travel from `do_dynamic_client_registration` to
/// [`StoredCredentials::with_registered_application_type`], and a new field on
/// the public, all-`pub`-field [`AuthorizationResult`] would be
/// `constructible_struct_adds_field` — a MAJOR semver break.
#[derive(Debug, Clone)]
struct ResolvedClientIdentity {
    /// The effective client id: DCR-issued when DCR fired, config-supplied
    /// otherwise.
    client_id: String,
    /// `Some` only when dynamic registration ran in THIS flow. A
    /// config-supplied client id was registered out of band, so this client has
    /// no observation to record about it — and recording a guess would be worse
    /// than recording nothing.
    registered_application_type: Option<String>,
}

/// What the credential store could contribute to one `get_access_token` call.
///
/// A plain `Option<String>` was enough while a miss always meant "run the
/// browser flow". [`Interactivity::RefreshOnly`] has to TELL a caller why there
/// is no token, so the miss now carries its reason.
#[derive(Debug)]
enum StoreOutcome {
    /// An access token that can be handed to the caller verbatim.
    Token(String),
    /// The store could not serve this request; the variant says why.
    Miss(StoreMiss),
}

/// Why the credential store could not serve a request on its own.
///
/// The three variants are kept apart rather than collapsed into one message
/// because each has a DIFFERENT operator fix, and a headless caller acting on
/// the refusal is exactly the audience that cannot go and look.
#[derive(Debug)]
enum StoreMiss {
    /// No store is configured, or it holds no entry for this
    /// `(issuer, account, server)`.
    NoCredentials,
    /// An entry exists and has expired, but carries no refresh token — RFC 6749
    /// §6 does not require one ever to have been issued.
    NoRefreshToken,
    /// A refresh was attempted and refused.
    RefreshFailed(Error),
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
///
/// # RFC 9207 `iss` validation and the `PMCP_OAUTH_ISS_VALIDATION` override
///
/// Every authorization response is validated before its `code` can be
/// exchanged: the CSRF `state` must match the value this flow generated, and an
/// `iss` that is PRESENT is always compared — byte for byte, with no
/// normalisation — against the issuer the authorization server published in its
/// own discovery document. That floor is unconditional. The only configurable
/// question is whether an ABSENT `iss` is fatal, and it is resolved from three
/// tiers, highest first:
///
/// | Tier | Source | Wins over |
/// |---|---|---|
/// | 1 | the `PMCP_OAUTH_ISS_VALIDATION` environment variable | everything below |
/// | 2 | [`OAuthHelper::with_iss_validation`] | the discovery flag |
/// | 3 | the discovery document's `authorization_response_iss_parameter_supported` | — |
///
/// `PMCP_OAUTH_ISS_VALIDATION` accepts exactly two values, compared
/// case-insensitively after trimming:
///
/// - `strict` — an authorization response with no `iss` is REJECTED.
/// - `lenient` — an absent `iss` proceeds (a present one is still compared).
///
/// **An unrecognised value is announced, not swallowed.** `true`, `1` and `yes`
/// are all plausible things to type and none of them is accepted; setting one
/// emits a `tracing::warn!` naming the variable, the value observed and the two
/// accepted values, and the next tier decides. **With the variable unset and no
/// builder call the behaviour is exactly the discovery flag's**, so an existing
/// deployment sees no change.
///
/// The variable is read at the point the flow needs it, never in a constructor,
/// so a hosting platform can supply the policy as a parameter through
/// [`OAuthHelper::with_iss_validation`] instead of through a process
/// environment.
#[derive(Debug)]
pub struct OAuthHelper {
    config: OAuthConfig,
    client: reqwest::Client,
    /// Tier 2 of the `iss` precedence chain. `None` means "not set by the
    /// builder", which is distinct from `Some(IssPresence::Optional)`.
    ///
    /// A PRIVATE field rather than an [`OAuthConfig`] field on purpose:
    /// `OAuthConfig` is public, all-pub-field and not `#[non_exhaustive]`, so a
    /// new field there is `constructible_struct_adds_field` — a MAJOR break that
    /// would invalidate every struct literal, including three in this
    /// repository. `OAuthHelper`'s fields are all private, so adding one here is
    /// semver-free.
    iss_validation: Option<IssPresence>,
    /// How the interactive authorization URL reaches a human. Defaults to
    /// [`SystemBrowserLauncher`], i.e. unchanged behaviour.
    browser_launcher: Arc<dyn BrowserLauncher>,
    /// The SEP-2352 credential store, resolved LAZILY.
    ///
    /// Empty until either [`OAuthHelper::with_credential_store`] fills it or the
    /// first store operation resolves the default [`FileCredentialStore`]. It is
    /// a `OnceLock` rather than an `Option` precisely so the resolution can
    /// happen behind `&self`: `OAuthHelper::new` must perform no filesystem and
    /// no environment access, because a hosting platform that will inject its
    /// own store a line later has no home directory for the default to find.
    credential_store: OnceLock<Arc<dyn CredentialStore>>,
    /// The account component of the [`CredentialKey`]. Empty by default — the
    /// single-user CLI case — and set by
    /// [`OAuthHelper::with_account_scope`] for a multi-tenant caller.
    account_scope: String,
    /// Fires the D-17 legacy-cache warning at most once per instance, rather
    /// than once per call.
    legacy_cache_warned: Once,
    /// Whether this helper may fall back to an interactive browser login.
    ///
    /// A PRIVATE field rather than an [`OAuthConfig`] field for the reason
    /// `iss_validation` above records: `OAuthConfig` is public, all-pub-field
    /// and not `#[non_exhaustive]`, so a new field there is a MAJOR break.
    /// [`Interactivity::Interactive`] is the default, so no existing caller
    /// changes behaviour.
    interactivity: Interactivity,
}

impl OAuthHelper {
    /// Create a new OAuth helper with the given configuration.
    ///
    /// Performs **no** filesystem and no environment access: the credential
    /// store, and therefore any home-directory resolution, is deferred to the
    /// first operation that needs it.
    pub fn new(config: OAuthConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            config,
            client,
            iss_validation: None,
            browser_launcher: Arc::new(SystemBrowserLauncher),
            credential_store: OnceLock::new(),
            account_scope: String::new(),
            legacy_cache_warned: Once::new(),
            interactivity: Interactivity::Interactive,
        })
    }

    /// Choose whether this helper may fall back to an INTERACTIVE browser
    /// login.
    ///
    /// See [`Interactivity`] for what the two modes mean, what the default one
    /// costs a headless runtime, and why this is an explicit mode rather than
    /// environment sniffing. With no call to this method the helper behaves
    /// exactly as it always has.
    ///
    /// This is an inherent builder method rather than an [`OAuthConfig`] field
    /// because that struct is exhaustively constructible downstream and gaining
    /// a field would be a MAJOR semver break.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::client::oauth::{Interactivity, OAuthConfig, OAuthHelper};
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let helper = OAuthHelper::new(OAuthConfig::default())?
    ///     .with_interactivity(Interactivity::RefreshOnly);
    /// # let _ = helper;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_interactivity(mut self, mode: Interactivity) -> Self {
        self.interactivity = mode;
        self
    }

    /// Persist credentials through `store` instead of through the default
    /// on-disk [`FileCredentialStore`].
    ///
    /// This is the platform seam. A helper built this way touches no home
    /// directory at all — not in the constructor, and not on any later call —
    /// which is what makes it usable from a Lambda, a container or any runtime
    /// that keeps credentials in a KV store or a secrets manager.
    ///
    /// This is an inherent builder method rather than an [`OAuthConfig`] field
    /// because that struct is exhaustively constructible downstream and gaining
    /// a field would be a MAJOR semver break.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
    /// use pmcp::{CredentialStore, InMemoryCredentialStore};
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    /// let helper = OAuthHelper::new(OAuthConfig::default())?
    ///     .with_credential_store(store);
    /// # let _ = helper;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_credential_store(mut self, store: Arc<dyn CredentialStore>) -> Self {
        self.credential_store = OnceLock::from(store);
        self
    }

    /// Key this helper's credentials under `account`.
    ///
    /// The account is the second component of the [`CredentialKey`], so two
    /// helpers with different account scopes never see one another's
    /// credentials even against the same authorization server and the same MCP
    /// server. The default is the empty string, which is the single-user CLI
    /// case.
    ///
    /// The value is stored verbatim — the SDK does not parse, normalise or
    /// interpret it. A platform passes whatever identifies the principal to it,
    /// such as a Cognito `sub`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let helper = OAuthHelper::new(OAuthConfig::default())?
    ///     .with_account_scope("cognito-sub-123");
    /// # let _ = helper;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_account_scope(mut self, account: impl Into<String>) -> Self {
        self.account_scope = account.into();
        self
    }

    /// Set tier 2 of the RFC 9207 `iss` precedence chain.
    ///
    /// See the [type-level documentation](Self) for the full chain. In short:
    /// `PMCP_OAUTH_ISS_VALIDATION` still wins over whatever is set here, and
    /// whatever is set here wins over the authorization server's own
    /// `authorization_response_iss_parameter_supported` flag. Passing
    /// [`IssPresence::Required`] makes an authorization response with no `iss`
    /// fatal even against a server that advertises nothing.
    ///
    /// This is an inherent builder method rather than an [`OAuthConfig`] field
    /// because that struct is exhaustively constructible downstream and gaining
    /// a field would be a MAJOR semver break.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
    /// use pmcp::IssPresence;
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let helper = OAuthHelper::new(OAuthConfig::default())?
    ///     .with_iss_validation(IssPresence::Required);
    /// # let _ = helper;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_iss_validation(mut self, presence: IssPresence) -> Self {
        self.iss_validation = Some(presence);
        self
    }

    /// Route the interactive authorization URL through a custom
    /// [`BrowserLauncher`] instead of the platform browser.
    ///
    /// Use this on a headless runner, in a container without a display, or from
    /// a hosting platform that relays the URL through its own UI. With no call
    /// to this method the flow uses [`SystemBrowserLauncher`], i.e. today's
    /// behaviour, unchanged.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use pmcp::client::oauth::{BrowserLauncher, OAuthConfig, OAuthHelper};
    ///
    /// #[derive(Debug)]
    /// struct PrintTheUrl;
    /// impl BrowserLauncher for PrintTheUrl {
    ///     fn open(&self, url: &str) -> pmcp::Result<()> {
    ///         println!("Open this URL to continue: {url}");
    ///         Ok(())
    ///     }
    /// }
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let helper = OAuthHelper::new(OAuthConfig::default())?
    ///     .with_browser_launcher(Arc::new(PrintTheUrl));
    /// # let _ = helper;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_browser_launcher(mut self, launcher: Arc<dyn BrowserLauncher>) -> Self {
        self.browser_launcher = launcher;
        self
    }

    /// Resolve the effective [`IssPresence`] for one flow, reading the
    /// `PMCP_OAUTH_ISS_VALIDATION` override at the CALL SITE.
    ///
    /// The read lives here rather than in [`OAuthHelper::new`] so construction
    /// stays I/O-free and a platform can supply the policy as a parameter. An
    /// unrecognised value warns and falls through; the precedence arithmetic
    /// itself is [`iss_presence_from`]'s, never re-implemented here.
    fn resolve_iss_presence(&self, discovery_flag: Option<bool>) -> IssPresence {
        let env_override = match std::env::var(ISS_VALIDATION_ENV_VAR) {
            Ok(raw) => {
                let parsed = parse_iss_env_value(&raw);
                if parsed.is_none() {
                    tracing::warn!(
                        "{} is set to `{}`, which is not one of its two accepted values \
                         `strict` or `lenient`. The variable is IGNORED; the builder setting \
                         or the authorization server's discovery flag decides instead.",
                        ISS_VALIDATION_ENV_VAR,
                        raw
                    );
                }
                parsed
            },
            Err(std::env::VarError::NotUnicode(_)) => {
                tracing::warn!(
                    "{} is set to a value that is not valid Unicode, so it cannot be one of \
                     the two accepted values `strict` or `lenient`. The variable is IGNORED.",
                    ISS_VALIDATION_ENV_VAR
                );
                None
            },
            Err(std::env::VarError::NotPresent) => None,
        };

        iss_presence_from(env_override, self.iss_validation, discovery_flag)
    }

    /// The credential store this helper persists through, or `None` when the
    /// caller asked for no persistence at all.
    ///
    /// Resolution order, evaluated on FIRST USE and never in the constructor:
    ///
    /// 1. a store injected through [`OAuthHelper::with_credential_store`];
    /// 2. otherwise, when [`OAuthConfig::cache_file`] is set, a
    ///    [`FileCredentialStore`] over `<that file's directory>/oauth-cache.json`;
    /// 3. otherwise **no store** — the caller opted out of caching.
    ///
    /// # Why `cache_file` names the DIRECTORY and not the store itself
    ///
    /// `cache_file` points at the flat, issuer-less `TokenCache` document this
    /// module used to write. Every such file on disk today is in that format, so
    /// pointing the issuer-keyed store at it would both READ it — which D-17
    /// forbids, because an issuer-less token cannot be attributed to an
    /// authorization server without guessing — and OVERWRITE it, when the whole
    /// point is to leave it for the user to delete. The directory is honoured;
    /// the file is not.
    ///
    /// # Why an absent `cache_file` means no persistence
    ///
    /// That is what it has always meant here: every previous cache read and
    /// write in this module was guarded by `if let Some(ref cache_file) =
    /// self.config.cache_file`. `cargo pmcp auth login --no-cache` sets the
    /// field to `None` for exactly that reason, so resolving a default store in
    /// that case would silently defeat the flag.
    ///
    /// A caller that wants a store at a specific path builds one:
    /// `with_credential_store(Arc::new(FileCredentialStore::new(path)))`.
    fn credential_store(&self) -> Option<&Arc<dyn CredentialStore>> {
        if let Some(injected) = self.credential_store.get() {
            return Some(injected);
        }

        let legacy = self.config.cache_file.as_ref()?;
        let resolved: Arc<dyn CredentialStore> = Arc::new(FileCredentialStore::new(
            legacy.with_file_name(CREDENTIAL_STORE_FILE_NAME),
        ));
        Some(self.credential_store.get_or_init(|| resolved))
    }

    /// The normalized MCP server key — the THIRD component of the
    /// [`CredentialKey`], and the one SEP-2352 does not name.
    ///
    /// Two MCP servers can share one authorization server and one account while
    /// holding different dynamic registrations, different client IDs and
    /// different granted scopes. Without this component they share one entry, so
    /// whichever authenticated last overwrites the other and a logout on one
    /// deletes the other's credentials (D-116-R1, AUTH-03 as amended in
    /// `0aebf7f6`). RFC 8707's `resource` parameter would have bound the
    /// audience instead; it is deferred by owner decision, so the key carries
    /// the binding.
    ///
    /// It is computed with [`normalize_server_key`] so that trailing-slash,
    /// path and host-case variants of one MCP server URL do not become two
    /// logins.
    ///
    /// # Errors
    ///
    /// When neither `mcp_server_url` nor `issuer` is configured there is nothing
    /// to key against, and the same configuration cannot discover metadata
    /// either. The issuer is the fallback because a helper configured with only
    /// an issuer is talking to that authorization server directly, so its URL is
    /// the only stable server identity available.
    fn server_key(&self) -> Result<String> {
        if let Some(ref url) = self.config.mcp_server_url {
            return normalize_server_key(url);
        }
        if let Some(ref issuer) = self.config.issuer {
            return normalize_server_key(issuer);
        }
        Err(Error::internal(
            "cannot address stored credentials: neither mcp_server_url nor issuer is configured"
                .to_string(),
        ))
    }

    /// The `(issuer, account, server)` address of this helper's credentials.
    ///
    /// `issuer` must be the issuer the authorization server published in its OWN
    /// discovery document — the same value AUTH-01 uses as its RFC 9207 anchor —
    /// and never `config.issuer`, which is a user-typed discovery seed.
    fn credential_key(&self, issuer: &str, server_key: &str) -> CredentialKey {
        CredentialKey::new(issuer, self.account_scope.as_str(), server_key)
    }

    /// Announce an authorization-server SUBSTITUTION for this MCP server, and
    /// refuse the flow exactly where the specification asks for a refusal
    /// (D-18, refined by RESEARCH A4).
    ///
    /// Issuer-keyed storage makes a substitution SAFE — the old credentials
    /// simply become unreachable — but it also makes it INVISIBLE: the user is
    /// walked through a fresh login at an identity provider they did not expect,
    /// and nothing says so. This is the "says so".
    ///
    /// # The mechanism the specification names is NOT the one used here
    ///
    /// The specification describes an authorization-server change as one
    /// "detected via updated protected resource metadata" — RFC 9728 Protected
    /// Resource Metadata. `pmcp` does not implement RFC 9728: absent an
    /// explicitly-configured issuer it derives the authorization server from the
    /// MCP base URL directly (see [`Self::get_metadata_with_extras`] for the
    /// precedence, and [`Self::discover_metadata_with_extras`] for the probe),
    /// and RFC 9728 discovery is
    /// **DEFERRED by owner decision (2026-08-02)**, recorded with a named owner
    /// in this phase's deferred-items file.
    ///
    /// So detection here uses the provenance signal that exists today: the
    /// issuer discovery actually RESOLVED for this MCP server URL, compared
    /// against the one last recorded for it. That is a narrower signal than the
    /// specification's, and it is stated plainly rather than presented as
    /// parity. Practically it detects a server that starts pointing at a
    /// different authorization server; it cannot detect a change announced only
    /// through protected resource metadata, because nothing reads that yet.
    ///
    /// # Why the remedy depends on credential PROVENANCE
    ///
    /// The specification's two adjacent sentences prescribe different remedies,
    /// and collapsing them into one would be wrong in both directions:
    ///
    /// - **DCR-issued** (`config.client_id` is `None`) — warn and PROCEED. The
    ///   requirement is "MUST NOT reuse client credentials from a different
    ///   authorization server and MUST re-register with the new authorization
    ///   server", which issuer-keyed storage already accomplishes by missing the
    ///   cache. Hard-failing here would convert a legitimate operational event —
    ///   a tenant move, a provider migration — into an outage, which D-18
    ///   explicitly rejects.
    /// - **Pre-registered** (`config.client_id` is `Some`) — return
    ///   [`Error::reauth_required`]. A pre-registered id is provisioned for one
    ///   authorization server and is meaningless at another; silently re-running
    ///   a browser login against an unexpected identity provider with it is
    ///   precisely the case the specification warns about. The browser flow is
    ///   not started.
    ///
    /// # Errors
    ///
    /// The ONLY error this returns is the pre-registered refusal above. Every
    /// store failure — an unreadable issuer record, an unaddressable server, a
    /// failed write — warns and proceeds, because a store that cannot be read
    /// must not be able to brick authentication and because the resulting
    /// behaviour is exactly today's: no detection. That is also what the
    /// [`CredentialStore::last_issuer`] default (`Ok(None)`) promises an
    /// implementor who declines the tracking.
    async fn announce_authorization_server_change(
        &self,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<()> {
        let Some(store) = self.credential_store() else {
            return Ok(());
        };
        let Ok(server_key) = self.server_key() else {
            return Ok(());
        };
        let discovered = metadata.issuer.as_str();

        let previous = match store.last_issuer(&server_key).await {
            Ok(previous) => previous,
            Err(e) => {
                tracing::warn!(
                    "could not read the last-seen authorization server for {server_key} ({e}); \
                     proceeding without substitution detection"
                );
                return Ok(());
            },
        };

        let Some(previous) = previous else {
            // First connection for this MCP server. Recording HERE, rather than
            // only on a successful authorization, means a login that never
            // completes still establishes the anchor a second connection needs.
            Self::record_issuer_best_effort(store, &server_key, discovered).await;
            return Ok(());
        };

        if previous == discovered {
            return Ok(());
        }

        if self.config.client_id.is_some() {
            return Err(Error::reauth_required(
                discovered,
                &format!(
                    "the authorization server for MCP server {server_key} changed from \
                     {previous} to {discovered}. This client is configured with a PRE-REGISTERED \
                     client_id, which is specific to one authorization server, so it is neither \
                     reused nor exchanged at the new one and no browser flow is started. If the \
                     change is expected, register this client with {discovered} and update \
                     OAuthConfig::client_id; if it is not, treat the MCP server as compromised."
                ),
            ));
        }

        tracing::warn!(
            "the authorization server for MCP server {} changed from {} to {}. This client's \
             credentials were issued by dynamic registration, so the previous ones are neither \
             reused nor sent anywhere — they are simply unreachable under the new issuer — and \
             this client is re-registering with {} and asking you to log in there. If you did not \
             expect that identity provider, stop and treat the MCP server as compromised.",
            server_key,
            previous,
            discovered,
            discovered
        );
        Self::record_issuer_best_effort(store, &server_key, discovered).await;
        Ok(())
    }

    /// Record a server's last-seen issuer, warning rather than failing when the
    /// store refuses.
    ///
    /// Detection is diagnostic, so a write failure here must not abort a flow.
    /// The AUTHORITATIVE record is the one `save_with_issuer` writes on success,
    /// and that failure IS propagated.
    async fn record_issuer_best_effort(
        store: &Arc<dyn CredentialStore>,
        server_key: &str,
        issuer: &str,
    ) {
        if let Err(e) = store.record_issuer(server_key, issuer).await {
            tracing::warn!(
                "could not record the authorization server for {server_key} ({e}); a later \
                 substitution may go undetected until the next successful login"
            );
        }
    }

    /// Announce, once per helper, that the legacy flat token cache is being
    /// discarded (D-17).
    ///
    /// The old `~/.pmcp/oauth-tokens.json` holds a single token and records NO
    /// issuer. It cannot be re-keyed for [`CredentialKey`] without GUESSING
    /// which authorization server issued it, and guessing is precisely what
    /// SEP-2352 forbids — so it is never opened for reading and never migrated.
    /// It is also never deleted or renamed: removing a user's file to fix a
    /// format problem is not this SDK's call.
    ///
    /// `cargo-pmcp`'s own multi-server cache is a different file that DOES
    /// record an issuer per entry, and it gets a real migration inside
    /// [`FileCredentialStore`].
    fn discard_legacy_token_cache(&self) {
        self.legacy_cache_warned.call_once(|| {
            let legacy = self
                .config
                .cache_file
                .clone()
                .unwrap_or_else(default_cache_path);
            if !legacy.exists() {
                return;
            }
            tracing::warn!(
                "the legacy OAuth token cache at {} is DISCARDED, not migrated: it records no \
                 issuer, so which authorization server issued its token cannot be determined \
                 without guessing — and guessing is what SEP-2352 forbids. One re-login is \
                 required. The file is left in place; delete it when you are ready.",
                legacy.display()
            );
        });
    }

    /// Read the credentials stored for `issuer` under this helper's account and
    /// server, treating every failure as a cache MISS.
    ///
    /// A store that cannot be read must never be able to prevent a fresh login:
    /// a corrupt document, a stale lock or a path that now holds the legacy flat
    /// format would otherwise brick authentication entirely. The failure is
    /// warned — naming the path or the reason the store gave, never any
    /// credential content — and the flow continues as if nothing were cached,
    /// which costs exactly one interactive login.
    async fn load_stored_credentials(&self, issuer: &str) -> Option<StoredCredentials> {
        let store = self.credential_store()?;
        let server_key = match self.server_key() {
            Ok(key) => key,
            Err(e) => {
                tracing::warn!("cannot address stored credentials ({e}); treating as a cache miss");
                return None;
            },
        };

        match store.load(&self.credential_key(issuer, &server_key)).await {
            Ok(found) => found,
            Err(e) => {
                tracing::warn!(
                    "the credential store could not be read ({e}); treating as a cache miss, \
                     which costs one re-login"
                );
                None
            },
        }
    }

    /// Persist one successful authorization under `(issuer, account, server)`,
    /// together with the server's last-seen issuer.
    ///
    /// `issuer` is the discovery document's issuer, never `config.issuer`.
    ///
    /// The record carries the effective `client_id`, the GRANTED scopes and the
    /// registered `application_type`. The client id is not a convenience: it is
    /// what makes SEP-2352's "MUST re-register with the new authorization
    /// server" automatic, because a DCR-issued id lives under the key of the
    /// authorization server that issued it and is unreachable from any other.
    ///
    /// The write goes through [`CredentialStore::save_with_issuer`] rather than
    /// `save` followed by `record_issuer`: two separate writes leave a window in
    /// which the store names one issuer while holding another's credentials, and
    /// [`FileCredentialStore`] implements the combined method as ONE atomic
    /// read-modify-write precisely so this call site is correct for free.
    async fn persist_credentials(
        &self,
        issuer: &str,
        result: &AuthorizationResult,
        registered_application_type: Option<&str>,
    ) -> Result<()> {
        let Some(store) = self.credential_store() else {
            return Ok(());
        };

        let server_key = self.server_key()?;
        let mut credentials = StoredCredentials::new(&result.access_token, &result.client_id)
            .with_granted_scopes(result.scopes.clone());
        if let Some(refresh_token) = result.refresh_token.as_deref() {
            credentials = credentials.with_refresh_token(refresh_token);
        }
        if let Some(expires_at) = result.expires_at {
            credentials = credentials.with_expires_at(expires_at);
        }
        if let Some(application_type) = registered_application_type {
            credentials = credentials.with_registered_application_type(application_type);
        }

        store
            .save_with_issuer(
                &self.credential_key(issuer, &server_key),
                &credentials,
                &server_key,
                issuer,
            )
            .await
    }

    /// The issuer REPORTED to a caller for its own bookkeeping: the
    /// caller-provided `config.issuer` when there is one, else the discovered
    /// value.
    ///
    /// Deliberately distinct from the value used to KEY credentials and to
    /// anchor RFC 9207 validation, both of which are `metadata.issuer` alone.
    fn effective_issuer(&self, metadata: &OidcDiscoveryMetadata) -> Option<String> {
        self.config
            .issuer
            .clone()
            .or_else(|| Some(metadata.issuer.clone()))
    }

    /// Perform RFC 7591 Dynamic Client Registration against `registration_endpoint`.
    ///
    /// Body is a public PKCE shape (`token_endpoint_auth_method: "none"`, no secret
    /// requested). `client_name` falls back to `"pmcp-sdk"` when the config value is
    /// `None`. Non-`https://` endpoints are rejected except for localhost loopback
    /// variants, guarding against discovery-spoofing. Response body size is capped
    /// at [`MAX_DCR_RESPONSE_BYTES`] on both the success and the rejection path.
    ///
    /// # The two SEPs this body carries
    ///
    /// - **SEP-837** — an `application_type` derived from the `redirect_uris`
    ///   being registered, sent UNCONDITIONALLY. Omitting it defaults to `web`
    ///   under OIDC, which contradicts the loopback redirect this very request
    ///   registers; a non-OIDC authorization server safely ignores it.
    /// - **SEP-2207** — `refresh_token` declared in `grant_types`, plus
    ///   `offline_access` in the registered `scope` when the authorization server
    ///   advertises it. See [`OFFLINE_ACCESS_SCOPE`] for why this is client
    ///   METADATA and not the request itself.
    ///
    /// `metadata` is taken as a parameter (rather than re-discovered) so
    /// `scopes_supported` is read from the same document the rest of the flow
    /// used. This is a private method, so widening its signature costs nothing.
    async fn do_dynamic_client_registration(
        &self,
        registration_endpoint: &str,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<DcrOutcome> {
        let parsed = Url::parse(registration_endpoint)
            .map_err(|e| Error::internal(format!("Invalid registration_endpoint URL: {e}")))?;
        // `url::Url::host_str()` returns IPv6 literals WITH brackets (e.g.
        // `http://[::1]/register` -> `Some("[::1]")`); match both forms.
        let scheme_ok = parsed.scheme() == "https"
            || (parsed.scheme() == "http"
                && matches!(
                    parsed.host_str(),
                    Some("localhost") | Some("127.0.0.1") | Some("::1") | Some("[::1]")
                ));
        if !scheme_ok {
            return Err(Error::internal(format!(
                "registration_endpoint must be https:// (or http://localhost, \
                 http://127.0.0.1, http://[::1]) — got {}",
                registration_endpoint
            )));
        }

        let client_name = self
            .config
            .client_name
            .clone()
            .unwrap_or_else(|| "pmcp-sdk".to_string());
        // Literal `127.0.0.1` rather than `localhost` — per RFC 8252 §7.3, avoids
        // browsers resolving `localhost` to `::1` when the listener binds IPv4-only.
        let redirect_uri = format!("http://127.0.0.1:{}/callback", self.config.redirect_port);

        // SEP-2207 client metadata: declaring `offline_access` here says what this
        // client is PERMITTED to ask for. The ask itself happens at the
        // authorization request — see `build_authorization_url`.
        let registered_scopes =
            compose_scopes_with_offline_access(&self.config.scopes, &metadata.scopes_supported);

        let mut request = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec![redirect_uri],
            client_name: Some(client_name),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".to_string()),
            // SEP-2207: an authorization server that was never told this client
            // wants a refresh grant has every reason not to issue a refresh
            // token. This is the missing prerequisite for refresh working at all.
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            // `DcrRequest` has `#[serde(skip_serializing_if = "Vec::is_empty")]`;
            // RFC 7591 §3.1 requires `response_types` in the body, so it must be
            // non-empty. `"code"` is the authorization-code public-PKCE flow.
            response_types: vec!["code".to_string()],
            // `scope` is `skip_serializing_if = "Option::is_none"`, so an empty
            // composition leaves the key off the wire entirely rather than
            // registering an empty string.
            scope: (!registered_scopes.is_empty()).then(|| registered_scopes.join(" ")),
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };

        // SEP-837, sent on every era and under every configuration: there is no
        // gate, no feature flag and no era check. `application_type` has been a
        // standard OIDC Dynamic Registration parameter since 2014, a non-OIDC
        // server ignores it, and era-gating would require plumbing a protocol
        // era into DCR that does not exist before an MCP connection is
        // established.
        let sent_application_type = apply_application_type(&mut request)?;

        let response = self
            .client
            .post(registration_endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::internal(format!("DCR request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            // The rejection body is as authorization-server-controlled as the
            // success body, so it is read through the SAME bounded reader. Its
            // over-cap refusal names the cap and reproduces no body content, and
            // it propagates verbatim: a registration endpoint that answers a
            // refusal with a gigabyte must not be able to spend a gigabyte of
            // this client's memory on the way to being refused.
            let body = collect_reqwest_body_within_cap(response, MAX_DCR_RESPONSE_BYTES).await?;
            let fields = dcr_rejection_fields(&body);
            return Err(registration_rejected(
                status,
                &fields,
                &sent_application_type,
                &request.redirect_uris,
            ));
        }

        // The cap is enforced DURING the read, not after it. The previous shape
        // read the whole body with `.bytes()` and then measured it, which bounds
        // what is ACCEPTED and not what is ALLOCATED — a registration endpoint
        // answering with a gigabyte still spent a gigabyte of this client's
        // memory on the way to being refused (D-113-V). The cap VALUE is
        // unchanged at 1 MiB: this is a change of mechanism, not of policy.
        let bytes = collect_reqwest_body_within_cap(response, MAX_DCR_RESPONSE_BYTES)
            .await
            .map_err(|e| {
                if is_body_over_cap(&e) {
                    // Re-framed as a DCR refusal so the message names the
                    // endpoint's own limit; the shared refusal's rule is kept —
                    // it names the cap and the observed size and reproduces no
                    // byte of the refused body.
                    Error::internal(format!(
                        "DCR response exceeds the {MAX_DCR_RESPONSE_BYTES} byte cap — refusing \
                         to parse it. {e}"
                    ))
                } else {
                    e
                }
            })?;
        let registration = serde_json::from_slice::<crate::server::auth::provider::DcrResponse>(
            &bytes,
        )
        .map_err(|e| {
            Error::internal(format!(
                "Failed to parse DCR response ({:?} error at line {}, column {}). The parser's \
                 own message is not reproduced here because a data error echoes the offending \
                 input, and a registration response body carries a client identity",
                e.classify(),
                e.line(),
                e.column()
            ))
        })?;

        // D-11 — echo divergence WARNS and never fails. See
        // `application_type_divergence` for why an omitted echo is not a
        // divergence, and why a real one is not an error.
        let echoed = registration.application_type();
        if let Some((requested, registered)) =
            application_type_divergence(&sent_application_type, echoed)
        {
            tracing::warn!(
                "the registration endpoint at {} registered this client with \
                 application_type=\"{}\" although \"{}\" was requested. RFC 7591 section 3.2.1 \
                 permits an authorization server to modify requested client metadata, so the \
                 registration STANDS and this is not an error — but this client is now registered \
                 under redirect-URI constraints it did not choose.",
                registration_endpoint,
                registered,
                requested
            );
        }
        // The type this client is actually registered under: the server's answer
        // when it gave one, otherwise the value that was sent, because RFC 7591
        // does not require an echo and an un-echoed request is registered as
        // requested.
        let registered_application_type =
            echoed.unwrap_or(sent_application_type.as_str()).to_string();

        Ok(DcrOutcome {
            response: registration,
            registered_application_type,
        })
    }

    /// Resolve the `client_id` for the current OAuth flow, performing DCR
    /// lazily when all three conditions hold:
    ///   1. `self.config.dcr_enabled == true`
    ///   2. `self.config.client_id.is_none()`
    ///   3. `metadata.registration_endpoint.is_some()`
    ///
    /// Returns `Err` with an actionable message when DCR is needed but the
    /// server does not advertise a `registration_endpoint`.
    ///
    /// The returned [`ResolvedClientIdentity`] carries the registered
    /// `application_type` alongside the id, because that value has to reach
    /// [`StoredCredentials::with_registered_application_type`] and there is no
    /// other hop out of the registration call that costs no semver event.
    async fn resolve_client_identity_for_flow(
        &self,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<ResolvedClientIdentity> {
        // Caller-provided client_id skips DCR entirely.
        if let Some(ref id) = self.config.client_id {
            return Ok(ResolvedClientIdentity {
                client_id: id.clone(),
                registered_application_type: None,
            });
        }

        if !self.config.dcr_enabled {
            return Err(Error::internal(
                "no client_id configured and dcr_enabled is false — \
                 provide OAuthConfig::client_id or enable dcr_enabled"
                    .to_string(),
            ));
        }

        match metadata.registration_endpoint.as_ref() {
            Some(endpoint) => {
                tracing::info!("Performing Dynamic Client Registration at {}", endpoint);
                let outcome = self
                    .do_dynamic_client_registration(endpoint, metadata)
                    .await?;
                // The registered `application_type` is both LOGGED and CARRIED:
                // the log serves a developer diagnosing a redirect-URI refusal,
                // and the carried value reaches
                // `StoredCredentials::with_registered_application_type` so the
                // same diagnosis is possible from the stored record later.
                tracing::info!(
                    "DCR succeeded — issued client_id, registered with application_type=\"{}\"",
                    outcome.registered_application_type
                );
                Ok(ResolvedClientIdentity {
                    client_id: outcome.response.client_id,
                    registered_application_type: Some(outcome.registered_application_type),
                })
            },
            None => Err(Error::internal(
                "server does not support DCR — pass a pre-registered client_id".to_string(),
            )),
        }
    }

    /// Test-only hook: drive the discovery + DCR resolver path without invoking
    /// the browser PKCE flow. Used by `tests/oauth_dcr_integration.rs`.
    ///
    /// Integration tests under `tests/` compile as a separate crate, so the
    /// library's `#[cfg(test)]` does not apply. The `oauth` feature gate plus
    /// `#[doc(hidden)]` and the `test_` prefix discourage external use.
    #[doc(hidden)]
    #[cfg(any(test, feature = "oauth"))]
    pub async fn test_resolve_client_id_from_discovery(&self) -> Result<String> {
        let metadata = self.get_metadata().await?;
        self.resolve_client_identity_for_flow(&metadata)
            .await
            .map(|identity| identity.client_id)
    }

    /// Whether an authorization-code-flow failure is a validation REFUSAL that
    /// must be surfaced verbatim rather than downgraded.
    ///
    /// Both callers of the authorization-code flow fall back to device code (or
    /// to a generic "no supported OAuth flow available" message) when it fails.
    /// That is right for "this flow was not available" and WRONG for "this flow
    /// detected an attack", for two reasons:
    ///
    /// - It destroys the stable programmatic identity. A caller that branches on
    ///   `err.is_iss_mismatch()` — the whole reason those markers exist — would
    ///   see a generic internal error instead, and message-substring matching is
    ///   exactly what the markers replaced.
    /// - It re-attempts authentication against an authorization server whose
    ///   response just failed a mix-up or CSRF check, handing the same attacker
    ///   a second attempt through a different grant.
    ///
    /// Every other failure keeps its existing fallback behaviour untouched.
    fn is_terminal_authorization_refusal(error: &Error) -> bool {
        error.is_iss_mismatch() || error.is_state_mismatch()
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

    /// Run OIDC / RFC 8414 discovery against an ALREADY-RESOLVED issuer.
    ///
    /// This is the single discovery seam: both the explicit-issuer and the
    /// derived-issuer routes go through it, so there is one client
    /// construction, one success-log block and one remediation template. It
    /// used to be the derived route only, with the explicit route carrying an
    /// inline near-copy — and that copy is how the pre-#368 remediation text
    /// survived on the very route this fix steers operators toward. One body
    /// cannot drift from itself.
    ///
    /// `derived_from` is `Some(mcp_url)` when `base_url` was DERIVED from that
    /// URL, and `None` when the operator named the issuer explicitly. It
    /// selects the remediation, and the two cases differ in KIND rather than
    /// wording: a derived issuer is a guess that naming the real one corrects,
    /// whereas for an explicitly-named issuer "name the issuer" is precisely
    /// the useless advice that made the old message a dead end.
    async fn discover_metadata_with_extras(
        &self,
        base_url: &str,
        derived_from: Option<&str>,
    ) -> Result<(OidcDiscoveryMetadata, AuthorizationServerExtras)> {
        tracing::info!("Discovering OAuth configuration from {base_url}...");

        let discovery_client = OidcDiscoveryClient::new();

        match discovery_client.discover_with_extras(base_url).await {
            Ok((metadata, extras)) => {
                tracing::info!("OAuth discovery successful");
                tracing::debug!("Issuer: {}", metadata.issuer);
                if let Some(ref device_endpoint) = metadata.device_authorization_endpoint {
                    tracing::debug!("Device endpoint: {device_endpoint}");
                }
                Ok((metadata, extras))
            },
            Err(e) => Err(Error::internal(Self::discovery_remediation(
                base_url,
                &e,
                derived_from,
            ))),
        }
    }

    /// Build the operator-facing remediation for a failed discovery.
    ///
    /// Split out so both routes share the diagnosis header and the
    /// unattended-path tail, and only the middle paragraph — the part that
    /// genuinely differs by provenance — varies.
    ///
    /// The advice is deliberately specific, because the generic text this
    /// replaced was actively misleading on the most common failure. Two
    /// corrections:
    ///
    ///  - It told the operator to "ensure the server exposes OAuth metadata at
    ///    .../openid-configuration". On an issuer MISMATCH that is a dead end:
    ///    `IssuerMismatch` is classified `Terminal`
    ///    (`classify_discovery_failure`), so the probe aborts and the
    ///    `openid-configuration` candidate is never requested. Serving a second
    ///    document changes nothing.
    ///  - It told the operator to "provide `--oauth-issuer` explicitly" without
    ///    saying that this is the fix for a THIRD-PARTY authorization server,
    ///    which is the case that produces the mismatch — and it gave that same
    ///    advice on the route where the issuer had ALREADY been named
    ///    explicitly, where it is not merely vague but impossible to act on.
    fn discovery_remediation(base_url: &str, e: &Error, derived_from: Option<&str>) -> String {
        let diagnosis = match derived_from {
            Some(mcp_url) => format!(
                "The issuer above was DERIVED from the MCP server URL `{mcp_url}`, which \
                 assumes the MCP server is also its own authorization server.\n\
                 \n\
                 If this deployment's authorization server is a third party (Amazon Cognito, \
                 Auth0, Okta, Microsoft Entra), that assumption is wrong. Name the real \
                 issuer explicitly — if the error above reports a declared issuer, that is \
                 the value to use:\n\
                 \x20   --oauth-issuer <issuer>        (or MCP_OAUTH_ISSUER=<issuer>)\n\
                 Discovery then runs against that issuer, and the RFC 8414 section 3.3 \
                 anchor check is applied to it."
            ),
            None => "The issuer above was supplied EXPLICITLY, so discovery ran against it \
                     rather than against a value derived from the MCP server URL, and the \
                     RFC 8414 section 3.3 anchor check was applied to it.\n\
                     \n\
                     Re-supplying it will not change this result. Check instead that the \
                     value is the authorization server's OWN issuer identifier, exactly as \
                     that server declares it — including scheme, and any trailing path — and \
                     that it serves RFC 8414 or OpenID Connect discovery metadata whose \
                     `issuer` field equals it byte for byte."
                .to_string(),
        };

        format!(
            "Failed to discover OAuth configuration at {base_url}: {e}\n\
             \n\
             {diagnosis}\n\
             \n\
             For an unattended caller (CI, an acceptance-test harness) supply a \
             pre-minted bearer token instead. This skips OAuth discovery altogether:\n\
             \x20   --api-key <token>              (or MCP_API_KEY=<token>)"
        )
    }

    /// Get OAuth metadata (either by discovering or constructing from issuer).
    ///
    /// Delegates to [`Self::get_metadata_with_extras`] and discards the RFC 9207
    /// flag, so callers that do not resolve `iss` policy are untouched.
    async fn get_metadata(&self) -> Result<OidcDiscoveryMetadata> {
        self.get_metadata_with_extras()
            .await
            .map(|(metadata, _)| metadata)
    }

    /// Get OAuth metadata together with the discovery-only values that do not
    /// fit on [`OidcDiscoveryMetadata`] — in particular RFC 9207's
    /// `authorization_response_iss_parameter_supported`, which is tier 3 of the
    /// `iss` precedence chain.
    ///
    /// # Precedence: an EXPLICIT issuer outranks a DERIVED one
    ///
    /// `config.issuer` is consulted FIRST, and `config.mcp_server_url` is the
    /// fallback. The two are not interchangeable sources of the same fact:
    ///
    /// - `config.issuer` is a value an operator supplied out of band
    ///   (`--oauth-issuer`, `MCP_OAUTH_ISSUER`). It is a statement about which
    ///   authorization server this deployment actually uses.
    /// - `config.mcp_server_url` yields an issuer only by DERIVATION
    ///   ([`Self::extract_base_url`] keeps scheme + host + port): it assumes the
    ///   MCP resource server is its own authorization server.
    ///
    /// That assumption is false for the common enterprise deployment, where the
    /// authorization server is a third party — Cognito, Auth0, Okta, Entra. For
    /// those, the derived issuer is simply the WRONG issuer, and because the
    /// RFC 8414 §3.3 anchor comparison is (correctly) byte-exact, discovery
    /// refuses a perfectly honest document. The right fix for the operator is to
    /// state the issuer; this ordering is what makes stating it have an effect.
    ///
    /// **This ordering does not relax the mix-up defence.** RFC 8414 §3.3
    /// requires the document's `issuer` to be identical to the issuer the
    /// well-known URI was inserted into. When the URL is built from
    /// `config.issuer`, that issuer *is* the anchor, and
    /// [`issuer_matches_metadata`](crate::shared::oauth_validation::issuer_matches_metadata)
    /// still runs byte-exact on every fetched document. An operator-supplied
    /// issuer is a BETTER-provenance anchor than a hostname heuristic — it is
    /// precisely the "validated source" the RFC 9207 guidance asks for. An
    /// attacker who controls the MCP server cannot exploit this: the client
    /// fetches from the operator's issuer and still demands the document name it.
    ///
    /// Credential ADDRESSING is deliberately unaffected — [`Self::server_key`]
    /// stays `mcp_server_url`-first, because credentials are keyed by
    /// `(issuer, account, server)` and the "server" component must remain the
    /// MCP server even when the issuer is stated explicitly.
    async fn get_metadata_with_extras(
        &self,
    ) -> Result<(OidcDiscoveryMetadata, AuthorizationServerExtras)> {
        // Resolve WHICH issuer anchors discovery, then delegate. Keeping
        // resolution and discovery separate is what stops the two routes from
        // drifting: there is exactly one discovery body below this match.
        if let Some(ref issuer) = self.config.issuer {
            // An explicitly-configured issuer wins over one derived from the MCP
            // server URL. Announced at `info` rather than `debug`: an operator
            // who overrides discovery should be able to see that the override
            // took effect, which is exactly what was previously unobservable.
            if let Some(ref mcp_url) = self.config.mcp_server_url {
                tracing::info!(
                    "Using the explicitly-configured OAuth issuer `{issuer}` for discovery \
                     instead of deriving one from the MCP server URL `{mcp_url}`. The \
                     RFC 8414 section 3.3 anchor check is applied against the configured \
                     issuer."
                );
            }

            self.discover_metadata_with_extras(issuer, None).await
        } else if let Some(ref mcp_url) = self.config.mcp_server_url {
            // Derive the issuer from the MCP server URL. Correct only when the
            // MCP server is also its own authorization server.
            //
            // DEF-116-01 (owner-deferred): this tier is a stopgap for a MISSING mechanism,
            // not a design. RFC 9728 protected-resource metadata is how a client
            // should LEARN the authorization server instead of guessing it from
            // the resource's own origin; until that lands, a third-party-IdP
            // deployment needs a human to type `--oauth-issuer`. The explicit
            // tier above is tier 1 of the eventual three (explicit > PRM
            // `authorization_servers[]` > derived), so it is not wasted work.
            let base_url = Self::extract_base_url(mcp_url)?;
            self.discover_metadata_with_extras(&base_url, Some(mcp_url))
                .await
        } else {
            Err(Error::internal(
                "Either oauth_issuer or mcp_server_url must be provided for OAuth authentication"
                    .to_string(),
            ))
        }
    }

    /// Get or refresh access token, performing OAuth flow if needed.
    ///
    /// For callers that only need a bearer-header value. Cache consumers that
    /// need to persist `refresh_token` / `expires_at` / `issuer` across runs
    /// should use [`authorize_with_details`](Self::authorize_with_details) instead.
    ///
    /// # Discovery now precedes the cache read, and it has to
    ///
    /// Credentials are addressed by the authorization server that ISSUED them
    /// (SEP-2352), so the issuer has to be known before the store can be asked
    /// anything. Discovery therefore runs first even on a cache hit. What a hit
    /// still avoids is the part that costs a human: no browser is opened and no
    /// authorization request is made.
    ///
    /// # Interactivity
    ///
    /// Under [`Interactivity::RefreshOnly`] this method never reaches the
    /// interactive tail at all: a store miss becomes
    /// [`Error::reauth_required`] immediately. See [`Interactivity`].
    pub async fn get_access_token(&self) -> Result<String> {
        self.discard_legacy_token_cache();

        // Get metadata to see what flows are supported. The extras carry the
        // RFC 9207 flag, which is tier 3 of the `iss` precedence chain.
        let (metadata, extras) = self.get_metadata_with_extras().await?;
        let iss_presence = self.resolve_iss_presence(extras.iss_parameter_supported());

        // D-18, BEFORE the cache is consulted and before anything interactive:
        // a pre-registered client must not be walked through a login at an
        // authorization server it was not provisioned for.
        self.announce_authorization_server_change(&metadata).await?;

        let miss = match self.token_from_store(&metadata).await? {
            StoreOutcome::Token(access_token) => return Ok(access_token),
            StoreOutcome::Miss(miss) => miss,
        };

        match self.interactivity {
            // NOTHING reachable from this arm can bind a socket or open a
            // browser: `refresh_only_refusal` is an associated function with no
            // `self`, so it holds no `BrowserLauncher`, no `redirect_port` and
            // no route to `authorization_code_flow_inner`. That is what makes
            // the interactive path unreachable BY CONSTRUCTION here rather than
            // merely skipped by this `match`.
            Interactivity::RefreshOnly => Err(Self::refresh_only_refusal(&metadata, &miss)),
            Interactivity::Interactive => self.interactive_token(&metadata, iss_presence).await,
        }
    }

    /// The interactive tail: bind a loopback listener, hand a URL to a browser,
    /// wait up to five minutes for the callback.
    ///
    /// Split out of [`Self::get_access_token`] so that the one call site is
    /// visibly the [`Interactivity::Interactive`] arm of a two-arm `match`. A
    /// reviewer checking D-08's guarantee has to read exactly one `match` and
    /// one call site, instead of tracing a fall-through through the rest of the
    /// function.
    async fn interactive_token(
        &self,
        metadata: &OidcDiscoveryMetadata,
        iss_presence: IssPresence,
    ) -> Result<String> {
        tracing::info!("No cached token found, starting OAuth flow...");
        self.authorize_with_fallback(metadata, iss_presence)
            .await
            .map(|result| result.access_token)
    }

    /// The typed refusal an [`Interactivity::RefreshOnly`] caller receives
    /// instead of a browser it cannot see and a five-minute wait.
    ///
    /// An associated function taking no `self` **on purpose** — see the comment
    /// at its call site. It also names WHICH of the three conditions occurred,
    /// because the operator fix differs: a rejected refresh token means
    /// re-authorize, an absent one means re-authorize asking for
    /// `offline_access`, and no credentials at all means the store was never
    /// seeded for this `(issuer, account, server)`.
    fn refresh_only_refusal(metadata: &OidcDiscoveryMetadata, miss: &StoreMiss) -> Error {
        let reason = match miss {
            StoreMiss::NoCredentials => {
                "no credentials are stored for this authorization server, account and MCP server"
                    .to_string()
            },
            StoreMiss::NoRefreshToken => {
                "the stored credentials have expired and carry no refresh token".to_string()
            },
            StoreMiss::RefreshFailed(e) => format!("the stored refresh token was refused: {e}"),
        };
        Error::reauth_required(
            &metadata.issuer,
            &format!(
                "{reason}. This helper is in Interactivity::RefreshOnly, so no browser was \
                 opened and no loopback listener was bound. An interactive authorization is \
                 required; perform one and store the result, then retry."
            ),
        )
    }

    /// Serve this request from the credential store when it can be: a live
    /// token verbatim, or a refreshed one.
    ///
    /// A [`StoreOutcome::Miss`] carries WHY, because
    /// [`Interactivity::RefreshOnly`] has to report a reason to a caller that
    /// cannot see a browser, and "nothing was stored" and "the refresh was
    /// refused" have different fixes.
    async fn token_from_store(&self, metadata: &OidcDiscoveryMetadata) -> Result<StoreOutcome> {
        let Some(cached) = self.load_stored_credentials(&metadata.issuer).await else {
            return Ok(StoreOutcome::Miss(StoreMiss::NoCredentials));
        };

        if cached.expires_at().is_some_and(|at| unix_now_secs() < at) {
            tracing::info!("Using cached OAuth token");
            return Ok(StoreOutcome::Token(cached.access_token().to_string()));
        }

        let Some(refresh_token) = cached.refresh_token() else {
            return Ok(StoreOutcome::Miss(StoreMiss::NoRefreshToken));
        };
        tracing::warn!("OAuth token expired, refreshing...");
        let refreshed = match self
            .refresh_token(
                refresh_token,
                Some(cached.client_id()),
                cached.granted_scopes(),
            )
            .await
        {
            Ok(refreshed) => refreshed,
            Err(e) => {
                // Previously SILENT. A refresh failure is the single most useful
                // line in an unattended log, because everything downstream of it
                // needs a human: the flow is about to fall back to a browser
                // nobody may be watching. The message carries the authorization
                // server's own reason and no credential content.
                tracing::warn!("OAuth token refresh failed: {e}");
                return Ok(StoreOutcome::Miss(StoreMiss::RefreshFailed(e)));
            },
        };

        // The refreshed record inherits everything the token response does not
        // restate. RFC 6749 section 6 permits an authorization server to omit a
        // new refresh token, in which case the existing one stays valid, and it
        // never restates the client id or the registered application type.
        //
        // D-14 defect 1 lives on the next three lines. `TokenResponse`'s
        // `refresh_token` is `#[serde(default)]`, so an OMITTED field arrives as
        // `None` — and `None` looks exactly like data here. Writing it over a
        // good token limits an unattended agent to exactly ONE refresh cycle
        // before it demands a human, so the rule is: replace only when the
        // response actually SUPPLIES one.
        let result = AuthorizationResult {
            access_token: refreshed.access_token,
            refresh_token: refreshed
                .refresh_token
                .or_else(|| Some(refresh_token.to_string())),
            // `saturating_add`, because `expires_in` is peer-supplied: a
            // hostile `u64::MAX` would otherwise panic in a debug build.
            expires_at: refreshed
                .expires_in
                .map(|ttl| unix_now_secs().saturating_add(ttl)),
            scopes: cached.granted_scopes().to_vec(),
            issuer: self.effective_issuer(metadata),
            client_id: cached.client_id().to_string(),
        };
        self.persist_credentials(
            &metadata.issuer,
            &result,
            cached.registered_application_type(),
        )
        .await?;
        Ok(StoreOutcome::Token(result.access_token))
    }

    /// Like `get_access_token` but returns the full authorization result for
    /// cache persistence.
    ///
    /// Cache callers (e.g., `cargo pmcp auth login`) should prefer this method;
    /// simple callers that just need a bearer header can keep using
    /// `get_access_token`.
    ///
    /// Drives DCR lazily when eligible; runs PKCE via the authorization_code
    /// flow; captures `refresh_token`, `expires_at`, `scopes`, and the
    /// effective issuer + client_id.
    ///
    /// # Device-code fallback
    ///
    /// If the authorization-code flow fails and the server advertises a
    /// `device_authorization_endpoint`, this method falls back to device code
    /// flow (RFC 8628). In that case, `refresh_token` may be `None` since
    /// RFC 8628 §3.5 does not require it, and `scopes` falls back to the
    /// requested scopes when the token response does not echo them.
    ///
    /// # Interactivity
    ///
    /// This method IS the interactive authorization, so under
    /// [`Interactivity::RefreshOnly`] it refuses with
    /// [`Error::reauth_required`] before performing any I/O at all. The mode's
    /// guarantee has to hold at BOTH public entry points, or a caller escapes
    /// it by picking the other one.
    pub async fn authorize_with_details(&self) -> Result<AuthorizationResult> {
        if self.interactivity == Interactivity::RefreshOnly {
            return Err(Error::reauth_required(
                self.config
                    .issuer
                    .as_deref()
                    .unwrap_or("the authorization server"),
                "authorize_with_details performs an interactive authorization, and this helper \
                 is in Interactivity::RefreshOnly. No browser was opened and no loopback \
                 listener was bound. Use get_access_token to serve the request from stored \
                 credentials, or build a helper without RefreshOnly to log in.",
            ));
        }

        self.discard_legacy_token_cache();

        let (metadata, extras) = self.get_metadata_with_extras().await?;
        let iss_presence = self.resolve_iss_presence(extras.iss_parameter_supported());

        // D-18, before anything interactive. Both public entry points check, so
        // a substitution cannot be reached by picking the other one.
        self.announce_authorization_server_change(&metadata).await?;

        // Deliberately does NOT consult the store: this is the "log me in" entry
        // point, and `cargo pmcp auth login` means a fresh authorization.
        // `get_access_token` is the one that reads the cache.
        self.authorize_with_fallback(&metadata, iss_presence).await
    }

    /// Run the authorization-code flow, PERSIST what it produced, and fall back
    /// to the device-code grant on a non-terminal failure.
    ///
    /// One implementation for both public entry points, so a credential that
    /// reaches `get_access_token`'s caller and one that reaches
    /// `authorize_with_details`' caller cannot be stored differently.
    async fn authorize_with_fallback(
        &self,
        metadata: &OidcDiscoveryMetadata,
        iss_presence: IssPresence,
    ) -> Result<AuthorizationResult> {
        match self
            .authorization_code_flow_inner(metadata, iss_presence)
            .await
        {
            Ok((token_response, identity)) => {
                // The scope this flow ACTUALLY requested at the authorization
                // request — `config.scopes` plus `offline_access` when the
                // server advertises it. Composed by the same function
                // `build_authorization_url` uses, so the value recorded as
                // "requested" cannot drift from the value sent. RFC 6749 §5.1
                // makes this the granted scope when the token response omits
                // `scope`, so using `config.scopes` alone would silently narrow
                // every subsequent refresh.
                let requested_scopes = compose_scopes_with_offline_access(
                    &self.config.scopes,
                    &metadata.scopes_supported,
                );
                let result = Self::build_auth_result(
                    token_response,
                    identity.client_id,
                    self.effective_issuer(metadata),
                    &requested_scopes,
                );
                self.persist_credentials(
                    &metadata.issuer,
                    &result,
                    identity.registered_application_type.as_deref(),
                )
                .await?;
                Ok(result)
            },
            Err(e) if Self::is_terminal_authorization_refusal(&e) => Err(e),
            Err(e) => {
                tracing::warn!("Authorization code flow failed: {}", e);

                if metadata.device_authorization_endpoint.is_some() {
                    tracing::info!(
                        "Trying device code flow (refresh_token may be None per RFC 8628)..."
                    );
                    return self.device_code_flow_with_metadata(metadata).await;
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

    /// Helper to construct an `AuthorizationResult` from a `TokenResponse`.
    ///
    /// `requested_scopes` must be the scope the flow ACTUALLY sent at the
    /// authorization request — including `offline_access` when it was added —
    /// because RFC 6749 §5.1 makes it the granted scope whenever the token
    /// response omits `scope`.
    fn build_auth_result(
        token_response: crate::client::auth::TokenResponse,
        client_id: String,
        effective_issuer: Option<String>,
        requested_scopes: &[String],
    ) -> AuthorizationResult {
        // Convert `expires_in` (relative seconds) to `expires_at` (absolute unix seconds).
        let expires_at = token_response
            .expires_in
            .map(|ttl| unix_now_secs().saturating_add(ttl));

        // The GRANTED scope, per RFC 6749 §5.1. The two branches look
        // interchangeable and are NOT, which is why each names its own rule:
        //
        // - `scope` PRESENT: that string IS the granted scope. It is recorded
        //   verbatim even when it is NARROWER than what was asked for — the
        //   authorization server is entitled to downgrade, and assuming the
        //   request was honoured is exactly the tampering assumption T-116-38b
        //   names.
        // - `scope` ABSENT: RFC 6749 §5.1 says the parameter is OPTIONAL "if
        //   identical to the scope requested by the client", so an omission
        //   means the request was granted in full and the REQUESTED scope is
        //   what was granted.
        //
        // 116-12 refreshes with this value and nothing else, so a wrong branch
        // here silently re-widens or narrows every subsequent refresh.
        let granted_scopes = match token_response.scope.as_deref() {
            Some(granted) => granted
                .split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>(),
            None => requested_scopes.to_vec(),
        };

        AuthorizationResult {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at,
            scopes: granted_scopes,
            issuer: effective_issuer,
            client_id,
        }
    }

    /// Bind the loopback callback listener and return it with the redirect URI
    /// that must be advertised to the authorization server.
    ///
    /// The advertised URL and the bind address MUST be the literal `127.0.0.1`
    /// (not `localhost`), otherwise browsers can resolve `localhost` to `::1`
    /// (IPv6) and hit `ERR_CONNECTION_REFUSED` on our IPv4-only listener.
    async fn bind_callback_listener(redirect_port: u16) -> Result<(TcpListener, String)> {
        let redirect_uri = format!("http://127.0.0.1:{}/callback", redirect_port);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", redirect_port))
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to bind to 127.0.0.1:{}.\n\
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

        Ok((listener, redirect_uri))
    }

    /// Build the authorization URL from the per-request record.
    ///
    /// `state` and `code_challenge` both come from the record, so the value in
    /// the URL and the value that will be compared on the callback cannot
    /// diverge.
    fn build_authorization_url(
        &self,
        metadata: &OidcDiscoveryMetadata,
        client_id: &str,
        redirect_uri: &str,
        record: &AuthorizationRequestRecord,
    ) -> Result<Url> {
        let mut auth_url = Url::parse(&metadata.authorization_endpoint)
            .map_err(|e| Error::internal(format!("Invalid authorization endpoint: {e}")))?;

        // SEP-2207: THIS is the stage at which requesting `offline_access` means
        // anything. Declaring it in the registration says what the client may
        // ask for; this is the ask. Composed from `config.scopes` — never by
        // mutating it, because a caller who reuses one `OAuthConfig` across two
        // flows must not watch its public `scopes` field grow.
        let requested_scopes =
            compose_scopes_with_offline_access(&self.config.scopes, &metadata.scopes_supported);

        auth_url
            .query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", &requested_scopes.join(" "))
            .append_pair(
                "code_challenge",
                &code_challenge_s256(record.code_verifier()),
            )
            .append_pair("code_challenge_method", "S256")
            // The CSRF `state` is the record's — a BOUND value, comparable on
            // the callback. It is produced by `generate_state()`, never by the
            // PKCE verifier generator: RFC 7636's verifier and RFC 6749 §10.12's
            // `state` are distinct roles and conflating their generators hides
            // that one of them is never checked.
            .append_pair("state", record.state());

        Ok(auth_url)
    }

    /// Read one HTTP request line, refusing anything over
    /// [`MAX_CALLBACK_REQUEST_LINE_BYTES`].
    ///
    /// The cap is applied at the socket by a limited reader, so an oversized
    /// line is refused without ever being allocated in full. There is
    /// deliberately no unbounded read here: any local process can connect to
    /// this port.
    async fn read_request_line_within_cap(stream: &mut TcpStream) -> Result<String> {
        let mut limited = BufReader::new(stream).take(MAX_CALLBACK_REQUEST_LINE_BYTES as u64 + 1);
        let mut raw = Vec::with_capacity(256);

        limited
            .read_until(b'\n', &mut raw)
            .await
            .map_err(|e| Error::internal(format!("Failed to read OAuth callback request: {e}")))?;

        if raw.len() > MAX_CALLBACK_REQUEST_LINE_BYTES {
            return Err(Error::internal(format!(
                "OAuth callback request line exceeds the \
                 MAX_CALLBACK_REQUEST_LINE_BYTES limit of {MAX_CALLBACK_REQUEST_LINE_BYTES} \
                 bytes; refused at the socket, and none of it is reproduced here"
            )));
        }

        String::from_utf8(raw)
            .map_err(|_| Error::internal("OAuth callback request line is not UTF-8".to_string()))
    }

    /// Isolate the query component of a callback request line.
    ///
    /// The request line is `GET /callback?<query> HTTP/1.1`; parsing it against
    /// a dummy origin is a convenient way to split the path from the query. The
    /// raw query is handed on undecoded — the percent-decode belongs to the
    /// validator, which performs the `application/x-www-form-urlencoded` decode
    /// RFC 9207 §2.4 requires. Never hand-roll it here.
    fn callback_query_from_request_line(request_line: &str) -> Result<String> {
        let path = request_line.split_whitespace().nth(1).ok_or_else(|| {
            Error::internal("OAuth callback request line has no request target".to_string())
        })?;

        let callback_url = Url::parse(&format!("http://localhost{}", path)).map_err(|e| {
            Error::internal(format!("OAuth callback request target is unparseable: {e}"))
        })?;

        Ok(callback_url.query().unwrap_or_default().to_string())
    }

    /// Accept ONE loopback callback, validate it, and only then write a
    /// response.
    ///
    /// The ordering is the security property, not a style choice. Reading,
    /// isolating the query and validating all happen BEFORE any response byte is
    /// committed, so the page the human sees and the code the caller receives are
    /// consequences of the SAME `Result`. Serving a success page first and
    /// validating afterwards would teach a user to trust a login that was about
    /// to be rejected, and would make the failure page unselectable — the task
    /// cannot know the outcome at the moment it must choose which page to write.
    async fn serve_one_callback(
        listener: TcpListener,
        record: &AuthorizationRequestRecord,
    ) -> Result<String> {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| Error::internal(format!("Failed to accept OAuth callback: {e}")))?;

        // Steps 1-3: bounded read, isolate the query, validate through the pure
        // tier. Not one byte has been written to the socket yet.
        let outcome = match Self::read_request_line_within_cap(&mut stream).await {
            Ok(request_line) => Self::callback_query_from_request_line(&request_line)
                .and_then(|raw_query| validate_authorization_response(&raw_query, record)),
            Err(e) => Err(e),
        };

        // Step 4: the page is chosen by the outcome that already exists. Neither
        // page carries authorization-server-supplied text.
        let response = if outcome.is_ok() {
            CALLBACK_SUCCESS_RESPONSE
        } else {
            CALLBACK_FAILURE_RESPONSE
        };
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;

        outcome
    }

    /// Wait for a validated authorization code, or the typed refusal that says
    /// why there will not be one.
    ///
    /// The channel carries a `Result`, so the caller's only route to the token
    /// exchange is the `Ok` branch. The record is CLONED into the listener task
    /// rather than shared behind an `Arc`: it is a small owned struct of two
    /// short strings, an issuer and a copy enum, and cloning keeps the task
    /// `'static` without adding a second ownership story.
    async fn await_validated_authorization_code(
        listener: TcpListener,
        record: AuthorizationRequestRecord,
    ) -> Result<String> {
        let (tx, rx) = oneshot::channel::<Result<String>>();
        let callback_task = tokio::spawn(async move {
            let _ = tx.send(Self::serve_one_callback(listener, &record).await);
        });

        tracing::info!("Waiting for authorization...");

        let received = tokio::time::timeout(Duration::from_mins(5), rx)
            .await
            .map_err(|_| {
                Error::internal("Timeout waiting for OAuth callback (5 minutes)".to_string())
            })?
            .map_err(|e| Error::internal(format!("OAuth callback channel error: {e}")))?;

        callback_task.abort();

        received
    }

    /// Inner PKCE authorization code flow returning the full token response.
    ///
    /// Returns (`TokenResponse`, [`ResolvedClientIdentity`]) so
    /// [`Self::authorize_with_fallback`] can populate `AuthorizationResult`
    /// including `refresh_token`, `expires_at`, `scopes` and the effective
    /// `client_id` — and can persist the registered `application_type`
    /// alongside them.
    ///
    /// Persistence deliberately lives in the CALLER: a flow that exchanged a
    /// code has not yet decided what the granted scope was (RFC 6749 §5.1's
    /// omission rule needs the composed request), and storing twice from two
    /// levels is how a record and its issuer come to disagree.
    ///
    /// `iss_presence` is resolved by the caller through
    /// [`Self::resolve_iss_presence`] and passed in, so the environment is read
    /// once per flow rather than once per layer.
    async fn authorization_code_flow_inner(
        &self,
        metadata: &OidcDiscoveryMetadata,
        iss_presence: IssPresence,
    ) -> Result<(crate::client::auth::TokenResponse, ResolvedClientIdentity)> {
        tracing::info!("Starting OAuth authorization code flow...");

        let identity = self.resolve_client_identity_for_flow(metadata).await?;
        let resolved_client_id = identity.client_id.clone();

        // The specification's mandated per-request record: the expected issuer,
        // the PKCE verifier, the CSRF `state` and the `iss` policy, bound into
        // ONE value. Three separate locals is exactly how `state` came to be an
        // unnamed temporary that nothing could compare.
        //
        // The anchor is `metadata.issuer` — the value the authorization server
        // published in its OWN discovery document, validated against the issuer
        // used to build the discovery URL at fetch time. It is deliberately NOT
        // `config.issuer` (a user-typed discovery seed) and not the effective
        // issuer reported to cache consumers: the attack being defended against
        // is "this response came from a different authorization server than the
        // one whose metadata I fetched".
        let record = AuthorizationRequestRecord::new(
            metadata.issuer.clone(),
            generate_code_verifier()?,
            generate_state()?,
            iss_presence,
        );

        let (listener, redirect_uri) =
            Self::bind_callback_listener(self.config.redirect_port).await?;

        let auth_url =
            self.build_authorization_url(metadata, &resolved_client_id, &redirect_uri, &record)?;

        tracing::info!("OAuth Authentication Required");
        tracing::info!("Opening browser for authentication...");
        tracing::info!("If the browser doesn't open, visit: {}", auth_url.as_str());

        // A launcher that cannot deliver the URL to a human at all aborts here,
        // rather than leaving the flow waiting five minutes for a callback
        // nobody will deliver.
        self.browser_launcher.open(auth_url.as_str())?;

        // Validation happens INSIDE the listener, before any response byte is
        // written. An `Err` here is unreachable-from-redemption by construction:
        // the token exchange below is only reachable on the `Ok` branch.
        let authorization_code =
            Self::await_validated_authorization_code(listener, record.clone()).await?;

        tracing::info!("Authorization code received");

        // Exchange authorization code for access token
        tracing::debug!("Exchanging authorization code for access token...");

        let token_exchange = TokenExchangeClient::new();
        let token_response = token_exchange
            .exchange_code(
                &metadata.token_endpoint,
                &authorization_code,
                &resolved_client_id,
                None, // No client secret for public clients
                &redirect_uri,
                Some(record.code_verifier()), // PKCE verifier, from the record
            )
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to exchange authorization code for token: {e}"
                ))
            })?;

        tracing::info!("Authentication successful");

        Ok((token_response, identity))
    }

    /// Perform OAuth device code flow (with pre-fetched metadata).
    async fn device_code_flow_with_metadata(
        &self,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<AuthorizationResult> {
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
    ) -> Result<AuthorizationResult> {
        let identity = self.resolve_client_identity_for_flow(metadata).await?;
        let resolved_client_id = identity.client_id.clone();

        // Step 1: Request device code. `offline_access` is deliberately NOT
        // composed in here: the device grant never builds an authorization URL,
        // and SEP-2207's request stage is that URL.
        let scope = self.config.scopes.join(" ");

        let response = self
            .client
            .post(device_auth_endpoint)
            .form(&[
                ("client_id", resolved_client_id.as_str()),
                ("scope", &scope),
            ])
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to request device code: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body =
                collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await?;
            return Err(Error::internal(format!(
                "Device authorization failed ({status}): {}",
                String::from_utf8_lossy(&body)
            )));
        }

        let body = collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await?;
        let device_auth: DeviceAuthResponse = serde_json::from_slice(&body).map_err(|e| {
            Error::internal(format!(
                "Failed to parse device authorization response ({:?} error at line {}, \
                 column {}). The parser's own message is not reproduced here because a data \
                 error echoes the offending input",
                e.classify(),
                e.line(),
                e.column()
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
                    ("client_id", resolved_client_id.as_str()),
                    ("device_code", &device_auth.device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| Error::internal(format!("Failed to poll for token: {e}")))?;

            let status = response.status();
            // Polled in a loop, so an unbounded read here is an unbounded read
            // per poll for as long as the device code lives.
            let raw =
                collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await?;

            if status.is_success() {
                let token_response: TokenResponse = serde_json::from_slice(&raw).map_err(|e| {
                    Error::internal(format!(
                        "Failed to parse token response ({:?} error at line {}, column {}). \
                             The parser's own message is not reproduced here because a data \
                             error echoes the offending input, and a token response body carries \
                             credentials",
                        e.classify(),
                        e.line(),
                        e.column()
                    ))
                })?;

                tracing::info!("Authentication successful");

                // `scopes` stays `config.scopes`: the device grant never builds
                // an authorization URL, so SEP-2207's `offline_access` was never
                // requested on this path and recording it would be a lie. RFC
                // 8628 §3.5 does not require a `refresh_token` either, so this
                // record may legitimately carry none.
                let result = AuthorizationResult {
                    access_token: token_response.access_token,
                    refresh_token: token_response.refresh_token,
                    expires_at: token_response
                        .expires_in
                        .map(|ttl| unix_now_secs().saturating_add(ttl)),
                    scopes: self.config.scopes.clone(),
                    issuer: self.effective_issuer(metadata),
                    client_id: resolved_client_id,
                };
                self.persist_credentials(
                    &metadata.issuer,
                    &result,
                    identity.registered_application_type.as_deref(),
                )
                .await?;

                return Ok(result);
            }

            // Check error response
            if let Ok(error) = serde_json::from_slice::<serde_json::Value>(&raw) {
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

    /// Refresh an existing access token (RFC 6749 §6).
    ///
    /// `stored_client_id` and `granted_scopes` come from the credential record
    /// the caller has ALREADY loaded under this helper's
    /// `(issuer, account, server)` key. They are parameters rather than a second
    /// `store.load` for a correctness reason and not a performance one: a
    /// refresh token and the `client_id` it was issued to are ONE pairing, and
    /// re-reading the store here could pair this refresh token with a
    /// `client_id` another process wrote in between.
    ///
    /// # Errors
    ///
    /// When no `client_id` is available from either place, when the request
    /// fails, when the authorization server refuses, or when the response body
    /// exceeds [`DEFAULT_AUTH_RESPONSE_BYTES`].
    async fn refresh_token(
        &self,
        refresh_token: &str,
        stored_client_id: Option<&str>,
        granted_scopes: &[String],
    ) -> Result<TokenResponse> {
        let metadata = self.get_metadata().await?;
        let token_endpoint = &metadata.token_endpoint;

        // D-14 defect 2. Under dynamic registration the `client_id` is ISSUED,
        // so it lives in the credential record and never in `OAuthConfig` —
        // reading config alone made a DCR-registered client unable to refresh
        // even ONCE, sending it back through a full browser login on every
        // expiry. The stored id wins because it is the one this refresh token
        // was issued to; the configured one is the fallback for a pre-registered
        // client. Only when BOTH are absent is it an error, and the message
        // names both places.
        let client_id = stored_client_id
            .filter(|id| !id.is_empty())
            .or(self.config.client_id.as_deref())
            .ok_or_else(|| {
                Error::internal(
                    "cannot refresh: no client_id in the stored credential record for this \
                     (issuer, account, server), and none in OAuthConfig::client_id. Both places \
                     were checked. Run an interactive authorization to register (or re-register) \
                     this client."
                        .to_string(),
                )
            })?;

        // D-14 defect 3, and stage 4 of SEP-2207's `offline_access` lifecycle:
        //
        //   1. `offline_access` is DECLARED in the DCR client metadata when the
        //      authorization server advertises it (116-10).
        //   2. It is REQUESTED in the authorization request when advertised
        //      (116-10).
        //   3. What the server actually GRANTED is recorded from the token
        //      response's `scope` or, when that field is absent, taken as the
        //      requested scope per RFC 6749 §5.1 (116-10, persisted by 116-11).
        //   4. A refresh sends EXACTLY that recorded set, or omits `scope`
        //      entirely when it is empty.
        //
        // Stage 4 never introduces a scope, never consults `scopes_supported`
        // and never falls back to `config.scopes`. `config.scopes` is what was
        // ASKED for; RFC 6749 §6 says a refresh request's scope "MUST NOT
        // include any scope not originally granted by the resource owner", so
        // re-widening here is a specification violation that a conforming
        // authorization server answers with `invalid_scope` — breaking refresh
        // entirely, which is the opposite of what this fix is for. Reaching for
        // `config.scopes` looks obviously right at this call site and is
        // obviously wrong two protocol stages away.
        //
        // Built as a `Vec` so an empty grant leaves the key OFF the wire rather
        // than sending `scope=`, which a conforming server may read as a
        // request for no scopes at all. This is the shape the sibling
        // `TokenExchangeClient::refresh_token` already uses.
        let scope = granted_scopes.join(" ");
        let mut form: Vec<(&str, &str)> = vec![
            ("client_id", client_id),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];
        if !scope.is_empty() {
            form.push(("scope", scope.as_str()));
        }

        let response = self
            .client
            .post(token_endpoint)
            .form(&form)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to refresh token: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            // A hostile authorization server controls its ERROR bodies too, and
            // an error path is where it has the most freedom. The over-cap
            // refusal propagates verbatim: it names the cap and the observed
            // size and reproduces no byte of what it dropped.
            let body =
                collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await?;
            return Err(Error::internal(format!(
                "Token refresh failed ({status}): {}",
                String::from_utf8_lossy(&body)
            )));
        }

        let body = collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES).await?;
        serde_json::from_slice::<TokenResponse>(&body).map_err(|e| {
            Error::internal(format!(
                "Failed to parse refresh response ({:?} error at line {}, column {}). The \
                 parser's own message is not reproduced here because a data error echoes the \
                 offending input, and a token response body carries credentials",
                e.classify(),
                e.line(),
                e.column()
            ))
        })
    }

    /// Create HTTP middleware chain with OAuth bearer token.
    ///
    /// Obtains an access token (from cache, refresh, or interactive flow)
    /// and wraps it in a middleware chain suitable for HTTP transports.
    pub async fn create_middleware_chain(&self) -> Result<Arc<HttpMiddlewareChain>> {
        let access_token = self.get_access_token().await?;

        // NEVER a prefix of the token itself. See `token_fingerprint`.
        tracing::debug!(
            "Creating OAuth middleware with token {}",
            token_fingerprint(&access_token)
        );

        let bearer_token = BearerToken::new(access_token);
        let oauth_middleware = OAuthClientMiddleware::new(bearer_token);

        let mut chain = HttpMiddlewareChain::new();
        chain.add(Arc::new(oauth_middleware));

        tracing::info!("OAuth middleware added to chain");

        Ok(Arc::new(chain))
    }
}

/// The path of the LEGACY, issuer-less flat token cache
/// (`~/.pmcp/oauth-tokens.json`).
///
/// Uses the user's home directory, falling back to the current directory when
/// the home directory cannot be determined.
///
/// # This file is no longer read (D-17 / SEP-2352)
///
/// It held ONE token and recorded NO issuer, so which authorization server
/// issued it cannot be determined without guessing — and guessing is what
/// SEP-2352 forbids. `OAuthHelper` never opens it, never migrates it and never
/// deletes it; it warns once that it is being discarded and leaves it for the
/// user to remove. One re-login is required.
///
/// The value is still useful as a LOCATION: setting
/// [`OAuthConfig::cache_file`] to it opts a caller into the issuer-keyed store,
/// which lives beside it as `oauth-cache.json` in the same directory. Setting
/// the field to `None` continues to mean "do not cache".
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
///     client_id: Some("my-client".to_string()),
///     client_name: None,
///     dcr_enabled: false,
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

/// [`token_fingerprint`] is the only thing in this module allowed to put a
/// token-derived string into a log line, so the rules it has to satisfy are
/// pinned here rather than left to review.
#[cfg(test)]
mod token_fingerprint_tests {
    use super::*;

    /// A token used across the rows below, long enough that a "first N
    /// characters" regression would visibly leak.
    const TOKEN: &str = "ya29.A0ARrdaM-THIS-IS-A-LIVE-LOOKING-ACCESS-TOKEN-abcdef0123456789";

    /// **The row that matters: the fingerprint reproduces NO part of the
    /// token.**
    ///
    /// A presence assertion ("it contains `sha256:`") is not a detector for a
    /// leak channel — 116-10's finding — so absence is asserted directly, over
    /// every prefix long enough to be identifying, in BOTH directions.
    fn assert_no_token_material(fingerprint: &str, token: &str) {
        assert!(
            !fingerprint.contains(token),
            "the whole token appeared in {fingerprint}"
        );
        for len in 4..=token.len() {
            let prefix = &token[..len];
            assert!(
                !fingerprint.contains(prefix),
                "a {len}-character prefix of the token appeared in {fingerprint}"
            );
        }
    }

    #[test]
    fn a_fingerprint_reproduces_no_part_of_the_token() {
        assert_no_token_material(&token_fingerprint(TOKEN), TOKEN);
    }

    /// The 20-character prefix the previous implementation logged is exactly
    /// what this row forbids, so a revert would fail here rather than in review.
    #[test]
    fn the_previous_twenty_character_prefix_is_absent() {
        let fingerprint = token_fingerprint(TOKEN);
        assert!(
            !fingerprint.contains(&TOKEN[..20]),
            "the old plaintext prefix is back: {fingerprint}"
        );
    }

    /// Stable across calls, so one token can be correlated across log lines —
    /// which is the entire reason for logging anything at all here.
    #[test]
    fn a_fingerprint_is_stable_for_one_token() {
        assert_eq!(token_fingerprint(TOKEN), token_fingerprint(TOKEN));
    }

    /// Distinct for distinct tokens, including two that share a long prefix:
    /// an implementation that hashed only the first few bytes would collide
    /// here and be useless for correlation.
    #[test]
    fn two_tokens_sharing_a_long_prefix_fingerprint_differently() {
        let sibling = format!("{TOKEN}-second");
        assert_ne!(token_fingerprint(TOKEN), token_fingerprint(&sibling));
    }

    /// Shape: the `sha256:` marker plus exactly
    /// [`FINGERPRINT_HEX_CHARS`] lowercase hex digits, so a reader cannot
    /// mistake it for a credential.
    #[test]
    fn a_fingerprint_is_the_marker_plus_twelve_hex_digits() {
        let fingerprint = token_fingerprint(TOKEN);
        let hex = fingerprint
            .strip_prefix("sha256:")
            .expect("the marker prefix");
        assert_eq!(hex.len(), FINGERPRINT_HEX_CHARS);
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "expected lowercase hex, got {hex}"
        );
    }

    /// An empty token is not a panic. It cannot occur through the live flow,
    /// but the helper is a logging primitive and a logging primitive that can
    /// panic is worse than the leak it replaced.
    #[test]
    fn an_empty_token_is_fingerprinted_without_panicking() {
        assert_eq!(
            token_fingerprint("").len(),
            "sha256:".len() + FINGERPRINT_HEX_CHARS
        );
    }
}

/// The credential-store WIRING rules that are decidable without a network.
///
/// The end-to-end evidence lives in `tests/oauth_store_wiring.rs`; these pin the
/// two rules that would otherwise only be visible as a downstream symptom.
#[cfg(test)]
mod credential_store_wiring_tests {
    use super::*;

    /// [`CREDENTIAL_STORE_FILE_NAME`] and `default_credential_path` must name
    /// the same file, or a caller who sets `cache_file` and a caller who does
    /// not would build two different stores inside one directory and a login
    /// through one would be invisible to the other.
    #[test]
    fn the_credential_store_file_name_matches_default_credential_path() {
        let default = crate::shared::credential_file::default_credential_path()
            .expect("a resolvable default credential path");
        assert_eq!(
            default.file_name().and_then(std::ffi::OsStr::to_str),
            Some(CREDENTIAL_STORE_FILE_NAME),
            "the store file name drifted from default_credential_path"
        );
    }

    /// The legacy flat cache and the issuer-keyed store are DIFFERENT files, so
    /// resolving the store never opens or overwrites the legacy document.
    #[test]
    fn the_legacy_flat_cache_and_the_credential_store_are_different_files() {
        let legacy = default_cache_path();
        assert_eq!(
            legacy.file_name().and_then(std::ffi::OsStr::to_str),
            Some("oauth-tokens.json")
        );
        assert_ne!(
            legacy.file_name().and_then(std::ffi::OsStr::to_str),
            Some(CREDENTIAL_STORE_FILE_NAME)
        );
        assert_eq!(
            legacy
                .with_file_name(CREDENTIAL_STORE_FILE_NAME)
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some(CREDENTIAL_STORE_FILE_NAME),
            "the store lives beside the legacy file, not on top of it"
        );
    }

    /// With no `cache_file` and no injected store there is no persistence at
    /// all, which is what `--no-cache` has always meant. Resolution is also
    /// I/O-free in that case: there is nothing to resolve.
    #[test]
    fn no_cache_file_and_no_injected_store_resolves_to_no_store() {
        let helper = OAuthHelper::new(OAuthConfig {
            mcp_server_url: Some("https://mcp.example".to_string()),
            cache_file: None,
            ..OAuthConfig::default()
        })
        .expect("helper");
        assert!(helper.credential_store().is_none());
    }

    /// A configured `cache_file` opts into a store BESIDE it, never onto it.
    #[test]
    fn a_configured_cache_file_resolves_a_store_beside_it() {
        let helper = OAuthHelper::new(OAuthConfig {
            mcp_server_url: Some("https://mcp.example".to_string()),
            cache_file: Some(PathBuf::from("/nonexistent-116-11/oauth-tokens.json")),
            ..OAuthConfig::default()
        })
        .expect("helper");
        let store = helper.credential_store().expect("a resolved store");
        assert!(
            format!("{store:?}").contains("oauth-cache.json"),
            "expected a store beside the legacy file, got {store:?}"
        );
        assert!(
            !std::path::Path::new("/nonexistent-116-11").exists(),
            "resolving a store must not create anything"
        );
    }

    /// The key carries all THREE components, with the server normalized.
    #[test]
    fn the_credential_key_carries_issuer_account_and_normalized_server() {
        let helper = OAuthHelper::new(OAuthConfig {
            mcp_server_url: Some("https://MCP.Example:443/api/".to_string()),
            ..OAuthConfig::default()
        })
        .expect("helper")
        .with_account_scope("cognito-sub-123");

        let server_key = helper.server_key().expect("a normalizable server URL");
        assert_eq!(server_key, "https://mcp.example");

        let key = helper.credential_key("https://as.example", &server_key);
        assert_eq!(key.issuer(), "https://as.example");
        assert_eq!(key.account(), "cognito-sub-123");
        assert_eq!(key.server(), "https://mcp.example");
    }

    /// The default account scope is the empty string — the single-user CLI
    /// case — and the issuer is the fallback server identity.
    #[test]
    fn the_default_account_scope_is_empty_and_the_issuer_is_the_server_fallback() {
        let helper = OAuthHelper::new(OAuthConfig {
            mcp_server_url: None,
            issuer: Some("https://as.example/tenant".to_string()),
            ..OAuthConfig::default()
        })
        .expect("helper");
        let server_key = helper.server_key().expect("a normalizable issuer");
        assert_eq!(server_key, "https://as.example");
        assert_eq!(
            helper
                .credential_key("https://as.example", &server_key)
                .account(),
            ""
        );

        let unaddressable = OAuthHelper::new(OAuthConfig::default()).expect("helper");
        assert!(unaddressable.server_key().is_err());
    }
}

#[cfg(test)]
mod oauth_config_tests {
    use super::*;

    #[test]
    fn oauth_config_default_has_dcr_enabled_and_none_client_id() {
        let c = OAuthConfig::default();
        assert!(
            c.client_id.is_none(),
            "default client_id must be None for DCR auto-fire"
        );
        assert!(c.dcr_enabled, "default dcr_enabled must be true");
        assert!(c.client_name.is_none(), "default client_name is None");
    }

    #[test]
    fn oauth_config_struct_literal_with_some_client_id_compiles() {
        let _c = OAuthConfig {
            issuer: None,
            mcp_server_url: Some("https://x.example".into()),
            client_id: Some("my-client".into()),
            client_name: None,
            dcr_enabled: false,
            scopes: vec![],
            cache_file: None,
            redirect_port: 8080,
        };
    }

    #[test]
    fn dcr_types_are_reexported() {
        // Compile-only: verifies pub use lands `DcrRequest` / `DcrResponse`
        // at `pmcp::client::oauth::*`.
        let _r: super::DcrRequest = super::DcrRequest {
            redirect_uris: vec!["http://localhost:8080/callback".into()],
            client_name: Some("test".into()),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".into()),
            grant_types: vec!["authorization_code".into()],
            response_types: vec![],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };
        let _rsp = super::DcrResponse {
            client_id: "x".into(),
            client_secret: None,
            client_secret_expires_at: None,
            registration_access_token: None,
            registration_client_uri: None,
            token_endpoint_auth_method: None,
            extra: Default::default(),
        };
    }
}

#[cfg(test)]
mod dcr_tests {
    use super::*;
    use crate::server::auth::oauth2::OidcDiscoveryMetadata;

    /// Construct an OidcDiscoveryMetadata with only the fields we care about
    /// for DCR tests. OidcDiscoveryMetadata does NOT implement Default, so we
    /// provide all required fields explicitly.
    fn metadata(reg: Option<&str>) -> OidcDiscoveryMetadata {
        OidcDiscoveryMetadata {
            issuer: "https://issuer.example".into(),
            authorization_endpoint: "https://issuer.example/auth".into(),
            token_endpoint: "https://issuer.example/token".into(),
            jwks_uri: None,
            userinfo_endpoint: None,
            registration_endpoint: reg.map(String::from),
            revocation_endpoint: None,
            introspection_endpoint: None,
            device_authorization_endpoint: None,
            response_types_supported: vec![],
            grant_types_supported: vec![],
            scopes_supported: vec![],
            token_endpoint_auth_methods_supported: vec![],
            code_challenge_methods_supported: vec![],
        }
    }

    #[tokio::test]
    async fn dcr_skipped_when_client_id_provided() {
        let cfg = OAuthConfig {
            client_id: Some("preset".into()),
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let resolved = helper
            .resolve_client_identity_for_flow(&metadata(Some("https://x/register")))
            .await
            .unwrap();
        assert_eq!(resolved.client_id, "preset");
    }

    #[tokio::test]
    async fn dcr_skipped_when_dcr_disabled_with_client_id() {
        let cfg = OAuthConfig {
            client_id: Some("preset".into()),
            dcr_enabled: false,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let resolved = helper
            .resolve_client_identity_for_flow(&metadata(None))
            .await
            .unwrap();
        assert_eq!(resolved.client_id, "preset");
    }

    #[tokio::test]
    async fn dcr_needed_but_unsupported_errors_with_actionable_message() {
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .resolve_client_identity_for_flow(&metadata(None))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("server does not support DCR"),
            "expected actionable DCR-missing message, got: {msg}"
        );
    }

    #[tokio::test]
    async fn dcr_needed_but_disabled_errors_when_client_id_none() {
        let cfg = OAuthConfig {
            client_id: None,
            dcr_enabled: false,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .resolve_client_identity_for_flow(&metadata(Some("https://x/register")))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("dcr_enabled is false"),
            "expected dcr_enabled=false error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn dcr_rejects_http_non_localhost_endpoint() {
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://attacker.example/register", &metadata(None))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("must be https"), "got: {msg}");
    }

    #[test]
    fn dcr_request_body_matches_rfc7591_public_pkce_shape() {
        let req = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec!["http://localhost:8080/callback".into()],
            client_name: Some("pmcp-sdk".into()),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".into()),
            grant_types: vec!["authorization_code".into()],
            response_types: vec![],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["client_name"], "pmcp-sdk");
        assert_eq!(
            v["redirect_uris"],
            serde_json::json!(["http://localhost:8080/callback"])
        );
        assert_eq!(v["grant_types"], serde_json::json!(["authorization_code"]));
        assert_eq!(v["token_endpoint_auth_method"], "none");
    }

    #[test]
    fn dcr_request_body_contains_response_types_code() {
        // Serde-level guard: `DcrRequest` has
        // `#[serde(skip_serializing_if = "Vec::is_empty")]` on `response_types`,
        // so an accidental empty-Vec default would silently drop the field.
        let req = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec!["http://localhost:8080/callback".into()],
            client_name: Some("pmcp-sdk".into()),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".into()),
            grant_types: vec!["authorization_code".into()],
            response_types: vec!["code".into()],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(
            s.contains(r#""response_types":["code"]"#),
            "RFC 7591 §3.1 response_types missing from wire body: {s}"
        );
    }

    #[tokio::test]
    async fn dcr_advertises_127_0_0_1_redirect_not_localhost() {
        // Regression guard: advertising `http://localhost:<port>` causes browsers
        // to resolve to `::1` (IPv6) and miss the IPv4-only callback listener.
        // The mock only matches when the wire body pins `127.0.0.1`; a regression
        // back to `localhost` makes this mock return 501 and the DCR call errors.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/register")
            .match_body(mockito::Matcher::PartialJsonString(
                serde_json::json!({
                    "redirect_uris": ["http://127.0.0.1:8080/callback"]
                })
                .to_string(),
            ))
            .with_status(201)
            .with_body(r#"{"client_id":"ok"}"#)
            .create_async()
            .await;
        let helper = OAuthHelper::new(OAuthConfig {
            dcr_enabled: true,
            redirect_port: 8080,
            ..OAuthConfig::default()
        })
        .unwrap();
        let result = helper
            .do_dynamic_client_registration(&format!("{}/register", server.url()), &metadata(None))
            .await;
        assert!(
            result.is_ok(),
            "DCR body did not pin 127.0.0.1 redirect_uri"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn dcr_accepts_ipv6_loopback_registration_endpoint() {
        // The guard must accept `[::1]` alongside `localhost` and `127.0.0.1`.
        // It rejects BEFORE the HTTP call, so a connection failure on port 9
        // is the expected non-scheme-guard error here.
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://[::1]:9/register", &metadata(None))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        // Must NOT be the scheme-guard error — the guard passed, we only
        // failed on the downstream HTTP call (port 9 is unreachable).
        assert!(
            !msg.contains("must be https"),
            "scheme guard should accept http://[::1] but rejected: {msg}"
        );
    }

    #[tokio::test]
    async fn dcr_accepts_http_localhost_registration_endpoint() {
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://localhost:9/register", &metadata(None))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            !msg.contains("must be https"),
            "scheme guard should accept http://localhost but rejected: {msg}"
        );
    }

    #[tokio::test]
    async fn dcr_accepts_http_ipv4_loopback_registration_endpoint() {
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://127.0.0.1:9/register", &metadata(None))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            !msg.contains("must be https"),
            "scheme guard should accept http://127.0.0.1 but rejected: {msg}"
        );
    }

    #[tokio::test]
    async fn authorize_with_details_fails_cleanly_without_server() {
        // Unit-test scope: verify the method signature compiles and returns
        // an error when no real server is reachable (not a behavior test —
        // full behavior is in the mockito integration test, Task 1.3).
        let cfg = OAuthConfig {
            mcp_server_url: Some("http://localhost:1/nonexistent".into()),
            client_id: Some("x".into()),
            dcr_enabled: false,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper.authorize_with_details().await.unwrap_err();
        // Any error path is acceptable here — the test ensures no panic.
        let _ = format!("{err}");
    }

    #[test]
    fn authorization_result_struct_has_expected_fields() {
        // Compile-time check: every required field is present and public.
        let _r = AuthorizationResult {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_at: Some(1),
            scopes: vec!["openid".into()],
            issuer: Some("https://i.example".into()),
            client_id: "c".into(),
        };
    }

    #[test]
    fn build_auth_result_converts_expires_in_to_expires_at() {
        let token = crate::client::auth::TokenResponse {
            access_token: "a".into(),
            token_type: "Bearer".into(),
            expires_in: Some(3600),
            refresh_token: Some("r".into()),
            scope: Some("openid profile".into()),
        };
        let now = unix_now_secs();
        let r = OAuthHelper::build_auth_result(
            token,
            "c1".into(),
            Some("https://i.example".into()),
            &["openid".into()],
        );
        assert_eq!(r.client_id, "c1");
        assert_eq!(r.refresh_token.as_deref(), Some("r"));
        assert_eq!(r.issuer.as_deref(), Some("https://i.example"));
        assert_eq!(r.scopes, vec!["openid".to_string(), "profile".into()]);
        let expires_at = r.expires_at.expect("expires_at populated");
        assert!(
            expires_at >= now + 3599 && expires_at <= now + 3601,
            "expires_at ({}) should be approximately now+3600 ({})",
            expires_at,
            now + 3600
        );
    }

    #[test]
    fn build_auth_result_falls_back_to_requested_scopes_when_no_grant() {
        let token = crate::client::auth::TokenResponse {
            access_token: "a".into(),
            token_type: "Bearer".into(),
            expires_in: None,
            refresh_token: None,
            scope: None,
        };
        let requested = vec!["openid".to_string(), "email".to_string()];
        let r = OAuthHelper::build_auth_result(token, "c".into(), None, &requested);
        assert_eq!(r.scopes, requested);
        assert!(r.expires_at.is_none());
        assert!(r.refresh_token.is_none());
    }
}

/// SEP-837 and SEP-2207 composition rules, pinned at the level they are
/// DECIDED at rather than at the level they are observed at.
///
/// Each of these three helpers is a private pure function precisely so its rule
/// can be asserted without a network, a log subscriber or a new public field.
/// The wire-level consequences are asserted separately in
/// `tests/oauth_dcr_integration.rs`; a rule that only had wire coverage would be
/// the shape 116-09 measured and named — a suite that is green over a defect it
/// structurally cannot see.
#[cfg(test)]
mod sep837_sep2207_composition_tests {
    use super::*;

    fn dcr_request_registering(redirect_uris: &[&str]) -> DcrRequest {
        DcrRequest {
            redirect_uris: redirect_uris.iter().map(|u| (*u).to_string()).collect(),
            client_name: Some("pmcp-sdk".to_string()),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".to_string()),
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            response_types: vec!["code".to_string()],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        }
    }

    fn owned(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_string()).collect()
    }

    // -----------------------------------------------------------------------
    // apply_application_type — derivation, and the override that outranks it
    // -----------------------------------------------------------------------

    #[test]
    fn a_loopback_redirect_derives_native() {
        let mut request = dcr_request_registering(&["http://127.0.0.1:8080/callback"]);
        let sent = apply_application_type(&mut request).expect("loopback derives");
        assert_eq!(sent, "native");
        assert_eq!(request.application_type(), Some("native"));
    }

    #[test]
    fn an_https_non_loopback_redirect_derives_web() {
        // pmcp's own flow only ever registers a loopback URI, so this row is
        // unreachable end to end and is exercised directly — a platform
        // `oauth-proxy` registering an https redirect is the real caller.
        let mut request = dcr_request_registering(&["https://proxy.example.com/callback"]);
        let sent = apply_application_type(&mut request).expect("https non-loopback derives");
        assert_eq!(sent, "web");
        assert_eq!(request.application_type(), Some("web"));
    }

    #[test]
    fn an_explicit_application_type_is_never_clobbered_by_the_derivation() {
        // D-09's documented override path. The redirect URI here would derive
        // `native`, so a clobbering implementation produces a DIFFERENT value
        // and this test is a real detector rather than a tautology.
        let mut request = dcr_request_registering(&["http://127.0.0.1:8080/callback"]);
        request.set_application_type("web");
        let sent = apply_application_type(&mut request).expect("an override is not re-derived");
        assert_eq!(sent, "web");
        assert_eq!(request.application_type(), Some("web"));
    }

    #[test]
    fn a_mixed_redirect_vector_is_an_error_and_never_a_pick() {
        let mut request = dcr_request_registering(&[
            "http://127.0.0.1:8080/callback",
            "https://proxy.example.com/callback",
        ]);
        let err = apply_application_type(&mut request)
            .expect_err("D-10: a mixed vector is an ERROR, never a silent choice");
        let message = err.to_string();
        assert!(
            message.contains("127.0.0.1"),
            "names the native URI: {message}"
        );
        assert!(
            message.contains("proxy.example.com"),
            "names the web URI: {message}"
        );
        assert_eq!(
            request.application_type(),
            None,
            "a refused derivation must not leave a half-written value on the request"
        );
    }

    // -----------------------------------------------------------------------
    // compose_scopes_with_offline_access — SEP-2207's advertise-conditioned add
    // -----------------------------------------------------------------------

    #[test]
    fn offline_access_is_added_only_when_the_server_advertises_it() {
        let configured = owned(&["openid", "profile"]);
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &owned(&["openid", "offline_access"])),
            owned(&["openid", "profile", "offline_access"]),
            "advertised: appended last, configured order preserved"
        );
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &owned(&["openid", "profile"])),
            owned(&["openid", "profile"]),
            "NOT advertised: absent, because SEP-2207 conditions the request on support"
        );
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &[]),
            owned(&["openid", "profile"]),
            "an empty scopes_supported advertises nothing"
        );
    }

    #[test]
    fn an_already_configured_offline_access_is_not_duplicated() {
        let configured = owned(&["openid", "offline_access"]);
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &owned(&["offline_access"])),
            owned(&["openid", "offline_access"]),
            "a duplicated scope token is legal but is what a strict server rejects"
        );
    }

    #[test]
    fn the_configured_scopes_are_never_mutated_and_never_accumulate() {
        let configured = owned(&["openid"]);
        let advertised = owned(&["offline_access"]);

        let first = compose_scopes_with_offline_access(&configured, &advertised);
        let second = compose_scopes_with_offline_access(&configured, &advertised);

        assert_eq!(
            configured,
            owned(&["openid"]),
            "`OAuthConfig::scopes` is a public field; a caller reusing one config \
             across two flows must not watch it grow"
        );
        assert_eq!(
            first, second,
            "two flows compose the same value, not a longer one"
        );
        assert_eq!(first, owned(&["openid", "offline_access"]));
    }

    #[test]
    fn duplicate_configured_scopes_collapse_to_one_entry() {
        let configured = owned(&["openid", "openid", "profile"]);
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &[]),
            owned(&["openid", "profile"])
        );
    }
}

/// D-11's divergence RULE, pinned where it is DECIDED.
///
/// The module name carries `application_type_divergence` so a
/// `nextest -E 'test(application_type_divergence)'` selector reaches every row
/// here — 116-01 measured that a selector which matches nothing reports success
/// rather than failing, so the name is load-bearing rather than descriptive.
///
/// There is deliberately no log-capture assertion anywhere in this module.
/// `tracing-subscriber` is an OPTIONAL dependency behind the `logging` feature
/// in this repo and not a dev-dependency, so a test built on capturing the
/// `tracing::warn!` would be unrunnable in the default configuration — and
/// would be pinning the REPORTING rather than the rule. The four rows below
/// pin the rule; `tests/oauth_dcr_integration.rs` pins the consequence that
/// actually matters end to end, which is that registration SUCCEEDS anyway.
#[cfg(test)]
mod application_type_divergence_tests {
    use super::*;
    use crate::server::auth::provider::DcrResponse;
    use serde_json::json;

    /// Parse a `DcrResponse` from a body shaped exactly as an authorization
    /// server would send it, so the `extra` carrier is exercised rather than
    /// bypassed.
    fn response_echoing(application_type: serde_json::Value) -> DcrResponse {
        serde_json::from_value(json!({
            "client_id": "issued-id",
            "application_type": application_type,
        }))
        .expect("a DcrResponse parses from a client_id plus an extra key")
    }

    #[test]
    fn an_equal_echo_is_not_a_divergence() {
        let response = response_echoing(json!("native"));
        assert_eq!(response.application_type(), Some("native"));
        assert_eq!(
            application_type_divergence("native", response.application_type()),
            None,
            "the server agreed; there is nothing to warn about"
        );
    }

    #[test]
    fn a_different_echo_is_a_divergence_naming_both_values() {
        let response = response_echoing(json!("web"));
        assert_eq!(
            application_type_divergence("native", response.application_type()),
            Some(("native".to_string(), "web".to_string())),
            "the tuple is (sent, registered) in that order — a warning that named \
             them the other way round would send a developer to change the wrong knob"
        );
    }

    #[test]
    fn an_absent_echo_is_not_a_divergence() {
        // RFC 7591 § 3.2.1 does not require the server to echo accepted
        // metadata, so silence is "no answer" and never "a different answer".
        let response: DcrResponse =
            serde_json::from_value(json!({ "client_id": "issued-id" })).expect("parses");
        assert_eq!(response.application_type(), None);
        assert_eq!(
            application_type_divergence("native", response.application_type()),
            None
        );
    }

    #[test]
    fn a_non_string_echo_reaches_application_type_divergence_as_an_absence() {
        // The accessor projects `Value::as_str` only (116-03), so a hostile
        // non-string value is `None` rather than a coerced `"42"` — and this
        // row proves the two halves compose: a non-string echo must not be
        // reported as a divergence against the string `"native"`.
        for hostile in [json!(42), json!(null), json!(true), json!(["native"])] {
            let response = response_echoing(hostile.clone());
            assert_eq!(
                response.application_type(),
                None,
                "a non-string echo must project to None: {hostile}"
            );
            assert_eq!(
                application_type_divergence("native", response.application_type()),
                None,
                "and must therefore not be reported as a divergence: {hostile}"
            );
        }
    }

    #[test]
    fn an_oversized_error_field_is_truncated_without_reproducing_what_it_dropped() {
        let long = "Z".repeat(MAX_DCR_ERROR_FIELD_CHARS + 500);
        let body =
            json!({ "error": "invalid_redirect_uri", "error_description": long }).to_string();
        let fields = dcr_rejection_fields(body.as_bytes());

        assert_eq!(fields.error.as_deref(), Some("invalid_redirect_uri"));
        let description = fields.error_description.expect("a description");
        assert!(
            description.contains("500 of 700 characters withheld"),
            "the notice must say how much was dropped: {description}"
        );
        assert!(
            description.chars().count() < MAX_DCR_ERROR_FIELD_CHARS + 100,
            "the bounded field must be far shorter than the input"
        );
    }

    #[test]
    fn a_non_string_or_unparseable_rejection_body_yields_no_fields() {
        let coerced = json!({ "error": 42, "error_description": ["nope"] }).to_string();
        let fields = dcr_rejection_fields(coerced.as_bytes());
        assert_eq!(fields.error, None, "a non-string error is never coerced");
        assert_eq!(fields.error_description, None);

        let html = dcr_rejection_fields(b"<html><body>502 Bad Gateway</body></html>");
        assert_eq!(html.error, None);
        assert_eq!(html.error_description, None);
    }
}

#[cfg(test)]
mod dcr_proptest {
    use super::*;
    use proptest::prelude::*;

    fn arb_dcr_request() -> impl Strategy<Value = crate::server::auth::provider::DcrRequest> {
        (
            prop::collection::vec("[a-z][a-z0-9-]{2,30}", 1..3),
            prop::option::of("[a-zA-Z][a-zA-Z0-9 _-]{1,40}"),
            prop::option::of(
                prop::string::string_regex("(none|client_secret_basic|client_secret_post)")
                    .unwrap(),
            ),
        )
            .prop_map(|(uris, name, auth_method)| {
                let redirect_uris = uris
                    .into_iter()
                    .map(|u| format!("http://localhost:8080/{u}"))
                    .collect();
                crate::server::auth::provider::DcrRequest {
                    redirect_uris,
                    client_name: name,
                    client_uri: None,
                    logo_uri: None,
                    contacts: vec![],
                    token_endpoint_auth_method: auth_method,
                    grant_types: vec!["authorization_code".into()],
                    response_types: vec!["code".into()],
                    scope: None,
                    software_id: None,
                    software_version: None,
                    extra: Default::default(),
                }
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn dcr_request_serde_roundtrip(req in arb_dcr_request()) {
            let v = serde_json::to_value(&req).unwrap();
            let back: crate::server::auth::provider::DcrRequest =
                serde_json::from_value(v).unwrap();
            prop_assert_eq!(req.redirect_uris, back.redirect_uris);
            prop_assert_eq!(req.client_name, back.client_name);
            prop_assert_eq!(req.token_endpoint_auth_method, back.token_endpoint_auth_method);
        }

        #[test]
        fn oauth_config_builder_allows_all_combinations(
            has_id in any::<bool>(),
            has_name in any::<bool>(),
            dcr in any::<bool>(),
        ) {
            let cfg = OAuthConfig {
                client_id: has_id.then(|| "id".into()),
                client_name: has_name.then(|| "name".into()),
                dcr_enabled: dcr,
                mcp_server_url: Some("https://x.example".into()),
                ..OAuthConfig::default()
            };
            OAuthHelper::new(cfg).unwrap();
        }
    }
}

// In-tree proptest smoke check for DCR response parsing. The authoritative
// robustness gate is the cargo-fuzz target at `fuzz/fuzz_targets/dcr_response_parser.rs`
// (CLAUDE.md ALWAYS / FUZZ Testing). This module exists for fast per-PR
// regression coverage that runs as part of `cargo test`.
#[cfg(test)]
mod dcr_parser_fuzz {
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]
        #[test]
        fn parser_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
            // Must return Result, never panic. Error paths are acceptable.
            let _ = serde_json::from_slice::<
                crate::server::auth::provider::DcrResponse
            >(&bytes);
        }

        #[test]
        fn parser_accepts_minimal_valid_response(
            id in "[a-zA-Z0-9-]{8,40}",
            has_secret in any::<bool>(),
        ) {
            let mut v = serde_json::json!({"client_id": id});
            if has_secret {
                v["client_secret"] = serde_json::json!("s3cret");
            }
            let parsed: crate::server::auth::provider::DcrResponse =
                serde_json::from_value(v).unwrap();
            prop_assert_eq!(parsed.client_id, id);
            prop_assert_eq!(parsed.client_secret.is_some(), has_secret);
        }
    }
}
