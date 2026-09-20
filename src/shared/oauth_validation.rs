//! Target-agnostic validation of OAuth 2.0 authorization SERVER DISCOVERY and
//! of the authorization RESPONSE (RFC 9207 `iss`, plus the CSRF `state`
//! comparison).
//!
//! This module provides the pure decision logic a client must run on the query
//! string it receives at its redirect URI, before it exchanges anything. It
//! performs no I/O: no socket, no browser, no environment read, no clock. It is
//! a function from `(query string, per-request record)` to `Result<code>`.
//!
//! It also holds the pure half of authorization server metadata discovery: the
//! hardened issuer parse
//! ([`validate_issuer_url`](crate::shared::oauth_validation::validate_issuer_url)),
//! SEP-2351's ORDERED candidate list
//! ([`discovery_url_candidates`](crate::shared::oauth_validation::discovery_url_candidates)),
//! RFC 8414 §3.3's anchor comparison
//! ([`issuer_matches_metadata`](crate::shared::oauth_validation::issuer_matches_metadata))
//! and the failure matrix every probe loop shares
//! ([`classify_discovery_failure`](crate::shared::oauth_validation::classify_discovery_failure)).
//! The PROBING — and therefore the network — belongs to the callers; that split
//! is what makes a MUST-ordered probe sequence testable offline. The same
//! module derives OpenID Connect's `application_type` from a client's
//! `redirect_uris`
//! ([`derive_application_type`](crate::shared::oauth_validation::derive_application_type)),
//! which is the value SEP-837 makes an MCP client MUST send at Dynamic Client
//! Registration.
//!
//! # Why it is ungated
//!
//! The native CLI flow in [`crate::client::oauth`] is
//! `#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]`, and the
//! `oauth` feature pulls `webbrowser`, `dirs` and `rand` — none of which exist
//! on a Cloudflare Workers or AWS Lambda redirect handler, and none of which
//! build for `wasm32-unknown-unknown`. A platform handler that only ever sees a
//! query string must still be able to validate it, so this module is
//! **ungated**, exactly like [`crate::shared::pkce`] (contrast the
//! `#[cfg(not(target_arch = "wasm32"))]` peer/stdio entries in
//! [`crate::shared`]). Its only imports are the crate's own error type and
//! [`url`], which is a non-optional dependency and is already the callback
//! parser. **Do not add a `cfg` to this module**, and do not reach for anything
//! else: a second implementation of this table is how a platform handler and a
//! CLI come to disagree about what "valid" means.
//!
//! # The normative table this implements
//!
//! From the MCP specification's *Authorization Response Validation* section,
//! which restates RFC 9207 §2.4:
//!
//! Intra-doc links below are FULLY QUALIFIED on purpose: this module carries an
//! outer `///` rationale on its `pub mod` declaration in [`crate::shared`] as
//! well as this inner `//!` block, and rustdoc resolves the merged result in the
//! DECLARING module's scope — so a bare `IssPresence` here does not resolve and
//! `make doc-check` (which runs with `-D warnings`) fails on it.
//!
//! | `authorization_response_iss_parameter_supported` | `iss` in response | Client action |
//! |---|---|---|
//! | `true` ([`Required`](crate::shared::oauth_validation::IssPresence::Required)) | present | compare, simple string comparison |
//! | `true` | absent | **reject** |
//! | `false`/absent ([`Optional`](crate::shared::oauth_validation::IssPresence::Optional)) | present | compare, simple string comparison |
//! | `false`/absent | absent | proceed |
//!
//! Note rows 1 and 3 are the SAME action. An `iss` that is present is *always*
//! compared; the only thing the advertised flag changes is whether ABSENCE is
//! fatal. That is why
//! [`IssPresence`](crate::shared::oauth_validation::IssPresence) has no
//! "disabled" variant.
//!
//! The specification also forbids normalizing before comparison: "clients MUST
//! NOT apply scheme or host case folding, default-port elision, trailing-slash,
//! or percent-encoding normalization (RFC 3986 Sections 6.2.2-6.2.3) before
//! comparison". Comparison here is `==` on the decoded strings and nothing
//! else.
//!
//! # Examples
//!
//! ```
//! use pmcp::shared::oauth_validation::{
//!     validate_authorization_response, AuthorizationRequestRecord, IssPresence,
//! };
//!
//! let record = AuthorizationRequestRecord::new(
//!     "https://as.example",
//!     "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
//!     "opaque-csrf-state",
//!     IssPresence::Required,
//! );
//!
//! let code = validate_authorization_response(
//!     "code=abc123&state=opaque-csrf-state&iss=https%3A%2F%2Fas.example",
//!     &record,
//! )?;
//! assert_eq!(code, "abc123");
//!
//! // A different issuer is refused, and the refusal is programmatically typed.
//! let err = validate_authorization_response(
//!     "code=abc123&state=opaque-csrf-state&iss=https%3A%2F%2Fevil.example",
//!     &record,
//! )
//! .unwrap_err();
//! assert!(err.is_iss_mismatch());
//! assert_eq!(err.iss_actual(), Some("https://evil.example"));
//! # Ok::<(), pmcp::Error>(())
//! ```

use crate::error::{Error, ErrorCode, Result};
use url::{form_urlencoded, Url};

/// The largest authorization-callback query this module will parse, in bytes.
///
/// The callback query is peer-controlled and unbounded on the wire — the
/// loopback listener reads it straight off a socket, and a platform handler
/// gets whatever the user-agent sent. 8 KiB is roughly an order of magnitude
/// above the largest legitimate response (a `code` and a `state` are each
/// well under 100 bytes, and `error_description` is prose), so a query over it
/// is refused BEFORE parsing rather than allocated and then measured.
pub const MAX_CALLBACK_QUERY_BYTES: usize = 8192;

/// The parameters whose repetition is refused, in the order they are reported.
///
/// Documented as a constant so the rustdoc, the refusal and the tests cannot
/// drift apart.
const SECURITY_PARAMETERS: &[&str] = &["state", "iss", "code", "error", "error_description"];

/// Whether the authorization server's metadata promised an `iss` parameter.
///
/// This is the ONLY configurable part of the decision table. An `iss` that is
/// present is always compared against the recorded issuer whatever this says —
/// that floor is unconditional — so there is deliberately no `Disabled`
/// variant. The choice is only whether ABSENCE of `iss` is fatal.
///
/// `#[non_exhaustive]` because a future specification revision may add a third
/// state (for example, a "required by policy regardless of metadata" tier), and
/// adding an enum variant to an exhaustive public enum is a MAJOR semver break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IssPresence {
    /// The authorization server advertised
    /// `authorization_response_iss_parameter_supported: true`, so a response
    /// without an `iss` is rejected (table row 2).
    Required,
    /// The authorization server said nothing, or said `false`. A response
    /// without an `iss` proceeds (table row 4); a response WITH one is still
    /// compared (table row 3).
    Optional,
}

/// The per-request record the specification requires a client to keep.
///
/// > "Before redirecting the user-agent, the client MUST record the `issuer`
/// > value from the selected authorization server's validated metadata document
/// > ... and associate it with the same per-request record used to store the
/// > PKCE code verifier (and the `state` value, if used)."
///
/// **One record, not three locals.** The specification's "same per-request
/// record" is a structural requirement, not a stylistic one: when the issuer,
/// the verifier and the state live in three separate variables, nothing stops a
/// flow from validating one request's `state` against another request's record,
/// and nothing makes it a compile error to forget one of them entirely. Binding
/// them into a single value is what makes the validation checkable.
///
/// The fields are PRIVATE and the type is constructed through
/// [`AuthorizationRequestRecord::new`]. That is a semver choice rather than a
/// style one: an all-public-field struct that is not `#[non_exhaustive]` is
/// exhaustively constructible downstream, so adding a field to it is a MAJOR
/// break (`constructible_struct_adds_field`) — which is precisely the trap
/// `OAuthConfig`, `DcrRequest` and `OidcDiscoveryMetadata` already sit in.
/// (Deliberately NOT linked: those three live behind feature gates this ungated
/// module must not assume are enabled.) With private fields, a future field is
/// a minor change.
///
/// The validation is only as good as the recorded issuer: it "provides no
/// protection if the expected issuer was obtained from an unvalidated source".
#[derive(Clone)]
pub struct AuthorizationRequestRecord {
    expected_issuer: String,
    code_verifier: String,
    state: String,
    iss_presence: IssPresence,
}

impl AuthorizationRequestRecord {
    /// Bind one authorization request's issuer, PKCE verifier, `state` and
    /// `iss` policy into the single record the specification requires.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::shared::oauth_validation::{AuthorizationRequestRecord, IssPresence};
    ///
    /// let record = AuthorizationRequestRecord::new(
    ///     "https://as.example",
    ///     "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
    ///     "opaque-csrf-state",
    ///     IssPresence::Optional,
    /// );
    /// assert_eq!(record.expected_issuer(), "https://as.example");
    /// assert_eq!(record.iss_presence(), IssPresence::Optional);
    /// ```
    #[must_use]
    pub fn new(
        expected_issuer: impl Into<String>,
        code_verifier: impl Into<String>,
        state: impl Into<String>,
        iss_presence: IssPresence,
    ) -> Self {
        Self {
            expected_issuer: expected_issuer.into(),
            code_verifier: code_verifier.into(),
            state: state.into(),
            iss_presence,
        }
    }

    /// The issuer recorded from the authorization server's validated metadata.
    #[must_use]
    pub fn expected_issuer(&self) -> &str {
        &self.expected_issuer
    }

    /// The PKCE code verifier this request was built with (RFC 7636 §4.1).
    ///
    /// Not consulted by [`validate_authorization_response`] — it is carried
    /// here because the specification says it must live in the same record, and
    /// because the token exchange that follows validation needs it.
    #[must_use]
    pub fn code_verifier(&self) -> &str {
        &self.code_verifier
    }

    /// The CSRF `state` value this request was sent with.
    #[must_use]
    pub fn state(&self) -> &str {
        &self.state
    }

    /// Whether an absent `iss` is fatal for this request.
    #[must_use]
    pub fn iss_presence(&self) -> IssPresence {
        self.iss_presence
    }
}

/// Redacts the two secrets so a `{:?}` of a record — in a log line, a panic
/// message or a caller's own error type — cannot leak the CSRF `state` or the
/// PKCE verifier. Every field is still named, so the shape stays legible.
impl std::fmt::Debug for AuthorizationRequestRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizationRequestRecord")
            .field("expected_issuer", &self.expected_issuer)
            .field("code_verifier", &"<redacted>")
            .field("state", &"<redacted>")
            .field("iss_presence", &self.iss_presence)
            .finish()
    }
}

/// The five security parameters, extracted at most once each.
#[derive(Default)]
struct CallbackParameters {
    state: Option<String>,
    iss: Option<String>,
    code: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Validate an authorization response and return its authorization `code`.
///
/// `raw_query` is the RAW query component with no leading `?` — the form a
/// platform redirect handler receives directly, so a Workers or Lambda function
/// can call this without reconstructing a URL. It is decoded with
/// `application/x-www-form-urlencoded` rules, as RFC 9207 §2.4 REQUIRES before
/// any comparison; never hand-roll that percent-decode.
///
/// # Evaluation order, which is load-bearing
///
/// 1. **`state`** — must be present and equal the recorded value, else
///    [`Error::state_mismatch`].
/// 2. **`iss`** — the four-row table of the module doc, compared with `==` and
///    NO normalization of any kind, else [`Error::iss_mismatch`].
/// 3. **`error`** — only now may an authorization-server-supplied `error` be
///    surfaced. The specification's prohibition is explicit: on an `iss`
///    mismatch the client "MUST NOT act on or display `error`,
///    `error_description`, or `error_uri`". Surfacing it earlier would let an
///    unauthenticated party choose text the client displays.
/// 4. **`code`** — present means success; absent with no `error` is a protocol
///    error naming the missing parameter.
///
/// # Two fail-closed input guards, applied before step 1
///
/// - **Size.** A `raw_query` longer than [`MAX_CALLBACK_QUERY_BYTES`] is
///   refused without being parsed. The refusal names the limit and the observed
///   length and reproduces none of the query.
/// - **Duplicates.** Any of `state`, `iss`, `code`, `error` or
///   `error_description` appearing more than once is refused. A "first wins"
///   rule on a security parameter is a request-smuggling primitive: a proxy or
///   server that takes the LAST occurrence and a client that takes the FIRST
///   disagree about what was validated. Unknown and vendor parameters may
///   repeat freely — they are ignored either way.
///
/// # Errors
///
/// Returns [`Error::state_mismatch`] for a missing or non-matching `state`,
/// [`Error::iss_mismatch`] for a failing `iss` row, and a protocol error for an
/// oversize query, a duplicated security parameter, an authorization-server
/// error response that survived steps 1-2, or a response carrying neither
/// `code` nor `error`.
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::{
///     validate_authorization_response, AuthorizationRequestRecord, IssPresence,
/// };
///
/// let record = AuthorizationRequestRecord::new(
///     "https://as.example",
///     "verifier",
///     "st4te",
///     IssPresence::Optional,
/// );
///
/// // Row 4: nothing advertised, nothing sent — proceed.
/// assert_eq!(
///     validate_authorization_response("code=abc&state=st4te", &record)?,
///     "abc",
/// );
///
/// // A forged state is refused, and the refusal names neither value.
/// let err = validate_authorization_response("code=abc&state=wrong", &record).unwrap_err();
/// assert!(err.is_state_mismatch());
/// assert!(!err.to_string().contains("st4te"));
/// # Ok::<(), pmcp::Error>(())
/// ```
pub fn validate_authorization_response(
    raw_query: &str,
    record: &AuthorizationRequestRecord,
) -> Result<String> {
    ensure_query_within_bounds(raw_query)?;
    let params = parse_callback_parameters(raw_query)?;

    validate_state(params.state.as_deref(), record.state())?;
    validate_iss(params.iss.as_deref(), record)?;

    if let Some(error) = params.error.as_deref() {
        return Err(authorization_server_error(
            error,
            params.error_description.as_deref(),
        ));
    }

    params.code.ok_or_else(missing_authorization_code)
}

/// Refuse an oversize query before a single byte of it is parsed.
fn ensure_query_within_bounds(raw_query: &str) -> Result<()> {
    let observed = raw_query.len();
    if observed > MAX_CALLBACK_QUERY_BYTES {
        return Err(Error::protocol(
            ErrorCode::INVALID_REQUEST,
            format!(
                "authorization callback query is {observed} bytes, over the \
                 MAX_CALLBACK_QUERY_BYTES limit of {MAX_CALLBACK_QUERY_BYTES}; refused without \
                 parsing, and none of it is reproduced here"
            ),
        ));
    }
    Ok(())
}

/// Decode the query once, taking each security parameter at most once.
///
/// The single pass is both the extraction and the duplicate check: a second
/// occurrence of an already-filled slot is the refusal, so there is no window
/// in which a "first wins" value has been adopted.
fn parse_callback_parameters(raw_query: &str) -> Result<CallbackParameters> {
    let mut params = CallbackParameters::default();
    for (key, value) in form_urlencoded::parse(raw_query.as_bytes()) {
        let slot = match key.as_ref() {
            "state" => &mut params.state,
            "iss" => &mut params.iss,
            "code" => &mut params.code,
            "error" => &mut params.error,
            "error_description" => &mut params.error_description,
            // Unknown and vendor parameters are ignored, and may repeat.
            _ => continue,
        };
        if slot.is_some() {
            return Err(duplicate_security_parameter(key.as_ref()));
        }
        *slot = Some(value.into_owned());
    }
    Ok(params)
}

/// The refusal for a repeated security parameter.
///
/// The key is safe to name: this is only ever reached for one of
/// [`SECURITY_PARAMETERS`], so no attacker-chosen text reaches the message.
fn duplicate_security_parameter(key: &str) -> Error {
    Error::protocol(
        ErrorCode::INVALID_REQUEST,
        format!(
            "authorization callback query carries `{key}` more than once. A repeated security \
             parameter is a smuggling primitive — a proxy that takes the last occurrence and a \
             client that takes the first disagree about what was validated — so the response is \
             refused rather than resolved by a first-wins rule. The refused set is \
             {SECURITY_PARAMETERS:?}"
        ),
    )
}

/// Step 1: the CSRF `state` comparison (the specification's D-12 obligation).
///
/// Absence is a mismatch, not a skip: a response that simply omits `state` must
/// not be treated as one that matched.
fn validate_state(received: Option<&str>, expected: &str) -> Result<()> {
    match received {
        Some(received) if received == expected => Ok(()),
        _ => Err(Error::state_mismatch()),
    }
}

/// Step 2: the four-row `iss` table, with no normalization whatsoever.
fn validate_iss(received: Option<&str>, record: &AuthorizationRequestRecord) -> Result<()> {
    let expected = record.expected_issuer();
    match (record.iss_presence(), received) {
        // Rows 1 and 3 — an `iss` that is PRESENT is always compared, whatever
        // the authorization server advertised. Simple string comparison per
        // RFC 3986 §6.2.1: no case folding, no default-port elision, no
        // trailing-slash and no percent-encoding normalization.
        (_, Some(received)) if received == expected => Ok(()),
        (_, Some(received)) => Err(Error::iss_mismatch(expected, Some(received))),
        // Row 2 — advertised and absent is a rejection.
        (IssPresence::Required, None) => Err(Error::iss_mismatch(expected, None)),
        // Row 4 — nothing advertised and nothing sent, so proceed.
        (IssPresence::Optional, None) => Ok(()),
    }
}

/// Step 3: surface an authorization server's own error, once it has earned the
/// right to be displayed by surviving steps 1 and 2.
fn authorization_server_error(error: &str, description: Option<&str>) -> Error {
    let message = match description {
        Some(description) => {
            format!("authorization server returned error `{error}`: {description}")
        },
        None => format!("authorization server returned error `{error}`"),
    };
    Error::protocol(ErrorCode::INVALID_REQUEST, message)
}

/// Step 4: neither outcome parameter was present.
fn missing_authorization_code() -> Error {
    Error::protocol(
        ErrorCode::INVALID_REQUEST,
        "authorization response carries neither `code` nor `error`; there is nothing to exchange \
         and nothing to report",
    )
}

/// Parse the operator-supplied `iss` validation setting, or reject it.
///
/// Accepts `"strict"` ([`IssPresence::Required`]) and `"lenient"`
/// ([`IssPresence::Optional`]), compared case-insensitively after trimming.
/// Everything else — including `"true"`, `"1"` and `"yes"`, which a reasonable
/// operator might well type — returns `None`.
///
/// # Why this is separate from [`iss_presence_from`]
///
/// With a single resolver taking `Option<&str>`, an UNRECOGNIZED value would be
/// indistinguishable from an unset variable, so an operator who wrote
/// `PMCP_OAUTH_ISS_VALIDATION=true` believing they had enabled strictness would
/// get silence and lenient behaviour. Keeping the parse separate lets the call
/// site see `Some(raw)` with a `None` parse and warn — naming the variable and
/// its two accepted values — before falling through to the next precedence
/// tier. **Do not merge these two functions.**
///
/// The environment variable's NAME is read at the call site, never here: this
/// module performs no environment access at all.
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::{parse_iss_env_value, IssPresence};
///
/// assert_eq!(parse_iss_env_value(" STRICT "), Some(IssPresence::Required));
/// assert_eq!(parse_iss_env_value("lenient"), Some(IssPresence::Optional));
/// // A plausible-but-wrong value is rejected LOUDLY rather than failing open.
/// assert_eq!(parse_iss_env_value("true"), None);
/// ```
#[must_use]
pub fn parse_iss_env_value(value: &str) -> Option<IssPresence> {
    match value.trim().to_ascii_lowercase().as_str() {
        "strict" => Some(IssPresence::Required),
        "lenient" => Some(IssPresence::Optional),
        _ => None,
    }
}

/// Resolve the effective [`IssPresence`] from the three precedence tiers.
///
/// Precedence is **environment override > builder setting > discovery flag**.
/// `env_override` is ALREADY PARSED — see [`parse_iss_env_value`] for why the
/// parse is deliberately not folded in here. `discovery_flag` is the
/// authorization server metadata's
/// `authorization_response_iss_parameter_supported`: `Some(true)` means
/// [`IssPresence::Required`]; `Some(false)` and `None` both mean
/// [`IssPresence::Optional`], because the specification treats "false" and
/// "absent" identically.
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::{iss_presence_from, IssPresence};
///
/// // The environment wins over both lower tiers.
/// assert_eq!(
///     iss_presence_from(Some(IssPresence::Required), Some(IssPresence::Optional), Some(false)),
///     IssPresence::Required,
/// );
/// // With no override, an advertising authorization server makes it required.
/// assert_eq!(
///     iss_presence_from(None, None, Some(true)),
///     IssPresence::Required,
/// );
/// // Silence all the way down is the lenient floor.
/// assert_eq!(iss_presence_from(None, None, None), IssPresence::Optional);
/// ```
#[must_use]
pub fn iss_presence_from(
    env_override: Option<IssPresence>,
    builder: Option<IssPresence>,
    discovery_flag: Option<bool>,
) -> IssPresence {
    if let Some(presence) = env_override {
        return presence;
    }
    if let Some(presence) = builder {
        return presence;
    }
    if discovery_flag == Some(true) {
        IssPresence::Required
    } else {
        IssPresence::Optional
    }
}

/// RFC 8414 §3.1's default well-known URI suffix, which MCP adopts explicitly
/// and which pmcp does not try today.
const WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER: &str = "oauth-authorization-server";

/// `OpenID` Connect Discovery 1.0 §4.1's well-known URI suffix.
const WELL_KNOWN_OPENID_CONFIGURATION: &str = "openid-configuration";

/// Parse an authorization server issuer identifier, enforcing RFC 8414 §2.
///
/// Parsing as an absolute [`Url`] is **not** sufficient, which is why this
/// function exists rather than a bare `Url::parse` at each call site. Every
/// rule below is enforced, and every rejection names the rule it enforced:
///
/// | Rule | Reason |
/// |---|---|
/// | scheme MUST be `https` | an issuer identified over cleartext can be swapped in transit |
/// | except `http` on a loopback host | pmcp's own development and test flows use loopback, and RFC 8252 §7.3 blesses it |
/// | userinfo MUST be absent | `https://honest.example@evil.example` reads as the honest host to a human and resolves to the attacker's; no legitimate issuer has userinfo |
/// | fragment MUST be absent | RFC 8414 §2 |
/// | query MUST be absent | RFC 8414 §2, and a query would survive into the built candidate URL |
/// | host MUST be present and non-empty | there is nothing to connect to otherwise |
///
/// The loopback exception accepts the three spellings a real flow produces: an
/// IPv4 loopback literal (canonically `127.0.0.1`), the IPv6 literal `::1` in
/// its bracketed authority form `[::1]`, and the name `localhost`. A listener
/// that binds IPv4 while a browser resolves `localhost` to `::1` is precisely
/// why all three must be accepted.
///
/// The parsed [`Url`] is returned so callers do not re-parse. Every other
/// function in this family calls this one first.
///
/// # Errors
///
/// Returns [`Error::Validation`] naming the specific rule violated — never a
/// generic "invalid URL". The refusal deliberately does **not** reproduce the
/// offending issuer string: an issuer can carry a userinfo password, and an
/// error message ends up in logs. The scheme and host are named instead, since
/// neither is a credential.
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::validate_issuer_url;
///
/// let parsed = validate_issuer_url("https://auth.example.com/tenant1")?;
/// assert_eq!(parsed.host_str(), Some("auth.example.com"));
///
/// // The loopback development exception is the ONLY permitted `http`.
/// assert!(validate_issuer_url("http://127.0.0.1:8080").is_ok());
/// assert!(validate_issuer_url("http://auth.example.com").is_err());
///
/// // Userinfo is authority confusion, and is refused outright.
/// assert!(validate_issuer_url("https://honest.example@evil.example").is_err());
/// # Ok::<(), pmcp::Error>(())
/// ```
pub fn validate_issuer_url(issuer: &str) -> Result<Url> {
    let parsed = Url::parse(issuer).map_err(unparseable_issuer)?;

    if !issuer_scheme_permitted(&parsed) {
        return Err(forbidden_issuer_scheme(&parsed));
    }
    // Defense in depth. With today's `url` crate this branch is unreachable
    // from an accepted scheme: `https://` fails to parse with `EmptyHost`, and
    // `https:///path` is NOT host-less — WHATWG's "special authority ignore
    // slashes" state consumes the third slash, so it parses as `https://path/`
    // (measured; pinned by a test). The check stays because an issuer with no
    // authority must fail closed rather than build a candidate against nothing.
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err(Error::validation(
            "an authorization server issuer MUST have a non-empty host; the offending value is \
             not reproduced here because an issuer string can carry userinfo credentials",
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(issuer_carries_userinfo(&parsed));
    }
    if parsed.fragment().is_some() {
        return Err(issuer_carries_component(&parsed, "fragment"));
    }
    if parsed.query().is_some() {
        return Err(issuer_carries_component(&parsed, "query"));
    }

    Ok(parsed)
}

/// `https` always, `http` only on a loopback host.
fn issuer_scheme_permitted(parsed: &Url) -> bool {
    match parsed.scheme() {
        "https" => true,
        "http" => is_loopback_host(parsed),
        _ => false,
    }
}

/// Whether the authority is one of the loopback spellings RFC 8252 §7.3 blesses.
fn is_loopback_host(parsed: &Url) -> bool {
    match parsed.host() {
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

/// The refusal for a string that is not an absolute URL at all.
fn unparseable_issuer(source: url::ParseError) -> Error {
    Error::validation(format!(
        "an authorization server issuer MUST be an absolute URL with a non-empty host \
         ({source}); the offending value is not reproduced here because an issuer string can \
         carry userinfo credentials"
    ))
}

/// The refusal for a scheme outside the `https` rule and its loopback exception.
fn forbidden_issuer_scheme(parsed: &Url) -> Error {
    Error::validation(format!(
        "an authorization server issuer uses scheme `{}` (host `{}`), but the scheme MUST be \
         `https`. The single exception is `http` on a loopback host (`127.0.0.1`, `::1`, \
         `localhost`), per RFC 8252 section 7.3",
        parsed.scheme(),
        parsed.host_str().unwrap_or("<none>"),
    ))
}

/// The refusal for a userinfo component, which never names the value.
fn issuer_carries_userinfo(parsed: &Url) -> Error {
    Error::validation(format!(
        "an authorization server issuer for host `{}` carries a userinfo component. RFC 8414 \
         issuers have no userinfo, and `https://honest.example@evil.example` reads as the honest \
         host to a human while resolving to the attacker's. The value is not reproduced here \
         because userinfo can carry a password",
        parsed.host_str().unwrap_or("<none>"),
    ))
}

/// The refusal for a fragment or a query, both forbidden by RFC 8414 §2.
fn issuer_carries_component(parsed: &Url, component: &str) -> Error {
    Error::validation(format!(
        "an authorization server issuer for host `{}` carries a {component}. RFC 8414 section 2 \
         forbids a query and a fragment on the issuer identifier, and either would survive into \
         the discovery URL built from it",
        parsed.host_str().unwrap_or("<none>"),
    ))
}

/// Derive SEP-2351's ORDERED list of authorization server metadata endpoints.
///
/// # This is an ordered probe sequence, not a replacement
///
/// RFC 8414 §3.1 specifies **insertion** of the well-known segment for the
/// `oauth-authorization-server` suffix; `OpenID` Connect Discovery 1.0 §4.1
/// specifies **appending** for `openid-configuration`; RFC 8414 §5 reconciles
/// the two with a fallback order, and the MCP specification makes that order a
/// client MUST. The caller probes the candidates in the order returned and uses
/// the first that yields a valid document.
///
/// The appended form — candidate 3 for a path-bearing issuer, candidate 2 for a
/// path-less one — is today's only pmcp behaviour and it MUST remain in the
/// list. Measured 2026-08-02 against Microsoft Entra ID, whose URL appears in
/// this SDK's own doctests: the appended form returns **200**, and both
/// inserted forms return **404**. An implementation that "fixed" discovery by
/// replacing append with insert would break every authorization server of that
/// shape.
///
/// For an issuer WITH a path component, e.g. `https://auth.example.com/tenant1`:
///
/// 1. `https://auth.example.com/.well-known/oauth-authorization-server/tenant1`
/// 2. `https://auth.example.com/.well-known/openid-configuration/tenant1`
/// 3. `https://auth.example.com/tenant1/.well-known/openid-configuration`
///
/// For an issuer WITHOUT one, e.g. `https://auth.example.com`, candidates 2 and
/// 3 coincide, so the list is exactly two:
///
/// 1. `https://auth.example.com/.well-known/oauth-authorization-server`
/// 2. `https://auth.example.com/.well-known/openid-configuration`
///
/// # Why the probing is NOT here
///
/// This function derives candidates only. The network belongs to the callers,
/// and that split is what makes a MUST-ordered sequence testable offline and in
/// a `wasm32` build. The failure-handling half of the probe loop is
/// [`classify_discovery_failure`].
///
/// # Errors
///
/// Returns whatever
/// [`validate_issuer_url`]
/// rejects. A hostile issuer never reaches candidate construction, so no
/// partially-formed URL is ever produced.
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::discovery_url_candidates;
///
/// let candidates = discovery_url_candidates("https://auth.example.com/tenant1")?;
/// let rendered: Vec<&str> = candidates.iter().map(|url| url.as_str()).collect();
/// assert_eq!(
///     rendered,
///     vec![
///         "https://auth.example.com/.well-known/oauth-authorization-server/tenant1",
///         "https://auth.example.com/.well-known/openid-configuration/tenant1",
///         "https://auth.example.com/tenant1/.well-known/openid-configuration",
///     ],
/// );
///
/// // A path-less issuer has exactly two candidates, and a trailing slash is
/// // a formatting difference rather than a path component.
/// assert_eq!(
///     discovery_url_candidates("https://auth.example.com/")?,
///     discovery_url_candidates("https://auth.example.com")?,
/// );
/// # Ok::<(), pmcp::Error>(())
/// ```
pub fn discovery_url_candidates(issuer: &str) -> Result<Vec<Url>> {
    let base = validate_issuer_url(issuer)?;
    // A trailing slash is a formatting difference, not a path component:
    // `https://as.example/` and `https://as.example` must derive the same list,
    // with no doubled slash anywhere in it.
    let path = base.path().trim_end_matches('/').to_owned();

    let mut candidates = vec![
        well_known_inserted(&base, WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER, &path),
        well_known_inserted(&base, WELL_KNOWN_OPENID_CONFIGURATION, &path),
    ];
    if !path.is_empty() {
        // For a path-less issuer the appended form IS candidate 2, so pushing
        // it again would duplicate an entry and waste a round trip.
        candidates.push(well_known_appended(&base, &path));
    }
    Ok(candidates)
}

/// RFC 8414 §3.1's insertion form: the well-known segment goes between the host
/// and the issuer's path.
fn well_known_inserted(base: &Url, suffix: &str, trimmed_path: &str) -> Url {
    let mut candidate = base.clone();
    candidate.set_path(&format!("/.well-known/{suffix}{trimmed_path}"));
    candidate
}

/// `OpenID` Connect Discovery 1.0 §4.1's appended form, and pmcp's only form
/// before SEP-2351.
fn well_known_appended(base: &Url, trimmed_path: &str) -> Url {
    let mut candidate = base.clone();
    candidate.set_path(&format!(
        "{trimmed_path}/.well-known/{WELL_KNOWN_OPENID_CONFIGURATION}"
    ));
    candidate
}

/// Compare a discovery document's `issuer` against the issuer used to build the
/// URL it came from — RFC 8414 §3.3 / `OpenID` Connect Discovery §4.3.
///
/// The comparison is a simple string comparison with **no normalization**: the
/// same rule, from the same family of specifications, as the RFC 9207 `iss`
/// comparison in
/// [`validate_authorization_response`].
///
/// # Why this function exists at all
///
/// The specification's own worked example: a document fetched from
/// `https://attacker.example/.well-known/oauth-authorization-server` that
/// contains `"issuer": "https://honest.example"` MUST be rejected.
///
/// Without this check, the `iss` validation is anchored on a value the
/// authorization server chose for itself, so an attacker who can influence
/// discovery serves a document naming any issuer they like and the RFC 9207
/// comparison then trivially succeeds against it. The authorization
/// specification says so directly: the `iss` validation "provides no protection
/// if the expected issuer was obtained from an unvalidated source".
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::issuer_matches_metadata;
///
/// assert!(issuer_matches_metadata("https://as.example", "https://as.example"));
///
/// // The specification's worked attack.
/// assert!(!issuer_matches_metadata("https://attacker.example", "https://honest.example"));
///
/// // No normalization of any kind — a trailing slash is a different issuer.
/// assert!(!issuer_matches_metadata("https://as.example", "https://as.example/"));
/// ```
#[must_use]
pub fn issuer_matches_metadata(issuer_used_to_build_url: &str, document_issuer: &str) -> bool {
    issuer_used_to_build_url == document_issuer
}

/// Whether two URLs share an origin: scheme, host and EFFECTIVE port.
///
/// The effective port is what makes `https://as.example` and
/// `https://as.example:443` the same origin. The path is deliberately not part
/// of the comparison, because an origin is not a location.
///
/// Callers use this to judge a discovery HTTP redirect: a redirect that stays
/// within the issuer's origin is ordinary server routing, while one that leaves
/// it hands document authorship to a different host and must be refused.
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::same_origin;
/// use url::Url;
///
/// let a = Url::parse("https://as.example/.well-known/openid-configuration")?;
/// let b = Url::parse("https://as.example:443/elsewhere")?;
/// assert!(same_origin(&a, &b));
///
/// let elsewhere = Url::parse("https://cdn.example/.well-known/openid-configuration")?;
/// assert!(!same_origin(&a, &elsewhere));
/// # Ok::<(), url::ParseError>(())
/// ```
#[must_use]
pub fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme()
        && a.host_str() == b.host_str()
        && a.port_or_known_default() == b.port_or_known_default()
}

/// Why one discovery candidate did not yield a usable metadata document.
///
/// The distinction the enum draws is not "which error" but "what the failure
/// says about the endpoint": whether nothing usable ARRIVED, or whether bytes
/// arrived and cannot be trusted. That is the distinction
/// [`classify_discovery_failure`]
/// turns into an outcome.
///
/// `#[non_exhaustive]` because a future revision may name a failure class this
/// one does not, and adding a variant to an exhaustive public enum is a MAJOR
/// semver break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiscoveryFailure {
    /// The endpoint returned HTTP 404 — the ordinary "this authorization server
    /// does not serve this well-known form" answer the probe exists to handle.
    NotFound,
    /// The endpoint returned some other HTTP status. Carries the raw status so
    /// the 5xx/other split is made in one place rather than at each call site.
    HttpStatus(u16),
    /// The request never completed: connection refused, DNS failure, TLS
    /// failure, timeout.
    Transport,
    /// A response arrived but its body is not the JSON document this endpoint
    /// is defined to serve.
    InvalidJson,
    /// The document's `issuer` differs from the issuer used to build the URL —
    /// RFC 8414 §3.3. See
    /// [`issuer_matches_metadata`].
    IssuerMismatch,
    /// The response body exceeded the caller's read cap. A discovery document
    /// is a few kilobytes; anything larger is either hostile or broken.
    BodyOverCap,
    /// The document parsed, but a security-relevant member is malformed — for
    /// example a non-`https` `token_endpoint`, or an `issuer` that is not a
    /// string.
    MalformedSecurityMetadata,
}

/// What a caller does about a [`DiscoveryFailure`].
///
/// # How the three compose in a probe loop
///
/// `Retry` means "re-attempt **this** candidate within the existing
/// `max_retries` budget, and once that budget is exhausted treat it as
/// `Fallback`". `Fallback` means "move to the next candidate; if there is none,
/// discovery has failed". `Terminal` means "abort discovery outright — do not
/// retry, do not try another candidate, and do not use any document".
///
/// Both discovery call sites implement that same loop, which is why the rule is
/// written here rather than in either of them.
///
/// `#[non_exhaustive]` for the same semver reason as
/// [`DiscoveryFailure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiscoveryOutcome {
    /// Try the next candidate in the ordered list.
    Fallback,
    /// Re-attempt this candidate within the existing retry budget, then fall
    /// back.
    Retry,
    /// Abort discovery. Never fall through to another candidate.
    Terminal,
}

/// The discovery outcome matrix, as one pure function.
///
/// | Failure | Outcome | Why |
/// |---|---|---|
/// | `NotFound` (404) | `Fallback` | the ordinary "not this form" answer; the whole reason there is an ordered list |
/// | `HttpStatus(4xx)` other than 404 | `Fallback` | the endpoint answered and refused; another form may still serve |
/// | `HttpStatus(5xx)` | `Retry` | the endpoint is the right one and is temporarily unwell |
/// | `Transport` | `Retry` | nothing arrived; a retry is the only way to distinguish transient from permanent |
/// | `InvalidJson` | `Fallback` | this endpoint does not serve the document; another form may |
/// | `IssuerMismatch` | `Terminal` | a document ARRIVED and lied about its issuer |
/// | `BodyOverCap` | `Terminal` | a peer sent more than a metadata document can legitimately be |
/// | `MalformedSecurityMetadata` | `Terminal` | a document ARRIVED with a broken security member |
///
/// # Why three of the rows are terminal
///
/// Falling through to a later candidate on "any" failure turns an issuer
/// mismatch, malformed metadata or an oversized body into a silent DOWNGRADE:
/// an attacker who can make candidate 1 fail in a security-relevant way gets
/// the client to accept candidate 3 instead. The three terminal rows are the
/// whole point of the matrix — they are never a fallback trigger, and no
/// peer-chosen status code can make an availability failure terminal either.
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::{
///     classify_discovery_failure, DiscoveryFailure, DiscoveryOutcome,
/// };
///
/// // A 404 is what the ordered probe is FOR.
/// assert_eq!(
///     classify_discovery_failure(DiscoveryFailure::NotFound),
///     DiscoveryOutcome::Fallback,
/// );
/// // A document that lied about its issuer aborts discovery; it never causes
/// // the client to quietly accept a later candidate instead.
/// assert_eq!(
///     classify_discovery_failure(DiscoveryFailure::IssuerMismatch),
///     DiscoveryOutcome::Terminal,
/// );
/// ```
#[must_use]
pub fn classify_discovery_failure(failure: DiscoveryFailure) -> DiscoveryOutcome {
    match failure {
        DiscoveryFailure::NotFound | DiscoveryFailure::InvalidJson => DiscoveryOutcome::Fallback,
        DiscoveryFailure::HttpStatus(status) if (500..=599).contains(&status) => {
            DiscoveryOutcome::Retry
        },
        DiscoveryFailure::HttpStatus(_) => DiscoveryOutcome::Fallback,
        DiscoveryFailure::Transport => DiscoveryOutcome::Retry,
        DiscoveryFailure::IssuerMismatch
        | DiscoveryFailure::BodyOverCap
        | DiscoveryFailure::MalformedSecurityMetadata => DiscoveryOutcome::Terminal,
    }
}

/// `OpenID` Connect's `application_type` client metadata value.
///
/// SEP-837 makes an MCP client MUST specify this at Dynamic Client
/// Registration: "Omitting it defaults to `web` under OIDC, which can conflict
/// with native-style redirect URIs; non-OIDC servers safely ignore the
/// parameter."
///
/// `#[non_exhaustive]` because `OpenID` Connect permits an authorization server
/// to define further values, and adding a variant to an exhaustive public enum
/// is a MAJOR semver break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApplicationType {
    /// Desktop applications, mobile apps, CLI tools, and locally-hosted web
    /// applications reached over loopback. Registered as `"native"`.
    Native,
    /// Remote, browser-based applications served from a non-local host.
    /// Registered as `"web"`.
    Web,
}

impl ApplicationType {
    /// The exact wire value, from `OpenID` Connect Dynamic Client Registration
    /// §2.
    ///
    /// These two strings are **compatibility surface**, not an implementation
    /// detail. An authorization server that receives an unrecognised value
    /// treats it as absent and silently falls back to the OIDC `web` default,
    /// so a change from `"native"` to `"Native"` would break registration for
    /// every native client while failing no round trip and no serialization
    /// test. They are pinned by hand in `tests/oauth_application_type.rs`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::shared::oauth_validation::ApplicationType;
    ///
    /// assert_eq!(ApplicationType::Native.as_str(), "native");
    /// assert_eq!(ApplicationType::Web.as_str(), "web");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Web => "web",
        }
    }
}

/// Derive [`ApplicationType`] from the `redirect_uris` a client is about to
/// register, requiring UNANIMITY.
///
/// | Redirect URI shape | Classification |
/// |---|---|
/// | loopback host (`127.0.0.1`, `::1` including the bracketed `[::1]` form, `localhost`) | `Native` |
/// | a scheme that is neither `http` nor `https` (a private-use scheme) | `Native` |
/// | `https` with a non-loopback host | `Web` |
/// | `http` with a non-loopback host | **error** |
///
/// # This is a heuristic for the common case, not the specification's rule
///
/// SEP-837 classifies by application NATURE — "desktop applications, mobile
/// apps, CLI tools, and locally-hosted web applications accessed via
/// `localhost`" are native; "remote browser-based applications served from a
/// non-local host" are web. This function instead derives from
/// `redirect_uris`, because that is the value a caller has in hand at the
/// registration site.
///
/// The two agree for every case pmcp produces: pmcp's own DCR call hardcodes
/// `http://127.0.0.1:{port}/callback` (RFC 8252 §7.3) and therefore derives
/// `native`, while a platform `oauth-proxy` with an https redirect derives
/// `web`. Where they disagree, `DcrRequest::set_application_type` remains the
/// authoritative override — it deliberately performs no validation precisely so
/// that it can override this derivation.
///
/// # Why a mixed vector is an error rather than a pick
///
/// Picking one classification for a vector that contains both would register a
/// client whose declared type contradicts some of its own redirect URIs. An
/// authorization server enforcing OIDC's redirect-URI constraints per type will
/// then either reject the registration (best case) or accept a redirect URI it
/// should have refused — which is an open-redirect primitive. The refusal names
/// both offending URIs and both classifications, because the operator's next
/// action is to decide which one the client actually is.
///
/// # Errors
///
/// Returns [`Error::Validation`] for an empty `redirect_uris`, an unparseable
/// redirect URI, a cleartext `http` redirect to a non-loopback host, and a
/// mixed classification. Never a silent default: every one of those is a case
/// where guessing decides where an authorization code is delivered.
///
/// # Examples
///
/// ```
/// use pmcp::shared::oauth_validation::{derive_application_type, ApplicationType};
///
/// // What pmcp's own Dynamic Client Registration call registers.
/// let native = vec!["http://127.0.0.1:8080/callback".to_string()];
/// assert_eq!(derive_application_type(&native)?, ApplicationType::Native);
/// assert_eq!(derive_application_type(&native)?.as_str(), "native");
///
/// // A remote, browser-based deployment.
/// let web = vec!["https://app.example.com/callback".to_string()];
/// assert_eq!(derive_application_type(&web)?, ApplicationType::Web);
///
/// // A mixed vector is refused, and the refusal names both offenders.
/// let mixed = vec![native[0].clone(), web[0].clone()];
/// let err = derive_application_type(&mixed).unwrap_err();
/// assert!(err.to_string().contains("app.example.com"));
/// # Ok::<(), pmcp::Error>(())
/// ```
pub fn derive_application_type(redirect_uris: &[String]) -> Result<ApplicationType> {
    let mut native: Option<&str> = None;
    let mut web: Option<&str> = None;

    for uri in redirect_uris {
        match classify_redirect_uri(uri)? {
            ApplicationType::Native => native = native.or(Some(uri.as_str())),
            ApplicationType::Web => web = web.or(Some(uri.as_str())),
        }
    }

    match (native, web) {
        (Some(native_uri), Some(web_uri)) => Err(mixed_application_types(native_uri, web_uri)),
        (Some(_), None) => Ok(ApplicationType::Native),
        (None, Some(_)) => Ok(ApplicationType::Web),
        (None, None) => Err(empty_redirect_uris()),
    }
}

/// Classify one redirect URI, in the rule order documented on
/// [`derive_application_type`].
fn classify_redirect_uri(uri: &str) -> Result<ApplicationType> {
    let parsed = Url::parse(uri).map_err(|source| unparseable_redirect_uri(uri, source))?;
    if is_loopback_host(&parsed) {
        return Ok(ApplicationType::Native);
    }
    match parsed.scheme() {
        "https" => Ok(ApplicationType::Web),
        "http" => Err(cleartext_remote_redirect_uri(uri)),
        // A private-use / custom scheme is the other RFC 8252 native shape.
        _ => Ok(ApplicationType::Native),
    }
}

/// The refusal for `redirect_uris: []`.
fn empty_redirect_uris() -> Error {
    Error::validation(
        "cannot derive `application_type`: the `redirect_uris` vector is empty, so there is \
         nothing to classify. Supply at least one redirect URI, or set the value explicitly with \
         `DcrRequest::set_application_type`",
    )
}

/// The refusal for a redirect URI that is not a URI.
fn unparseable_redirect_uri(uri: &str, source: url::ParseError) -> Error {
    Error::validation(format!(
        "cannot derive `application_type`: redirect URI `{uri}` is not a parseable absolute URI \
         ({source}). Classifying an unparseable redirect would be guessing where the \
         authorization code is delivered"
    ))
}

/// The refusal for cleartext `http` to a host that is not loopback.
fn cleartext_remote_redirect_uri(uri: &str) -> Error {
    Error::validation(format!(
        "cannot derive `application_type`: redirect URI `{uri}` uses cleartext `http` with a \
         non-loopback host. That is neither a permitted native loopback redirect (RFC 8252 \
         section 7.3) nor a valid web redirect, and it would deliver the authorization code to \
         anyone on the network path"
    ))
}

/// The refusal for a `redirect_uris` vector that classifies both ways.
fn mixed_application_types(native_uri: &str, web_uri: &str) -> Error {
    Error::validation(format!(
        "cannot derive `application_type`: `redirect_uris` mixes classifications. `{native_uri}` \
         classifies as `native` (a loopback or private-use redirect) while `{web_uri}` classifies \
         as `web` (a remote https redirect). This is refused rather than resolved by picking one, \
         because the wrong pick registers a client whose declared type contradicts its own \
         redirect URIs. Register one client per application type, or set the value explicitly \
         with `DcrRequest::set_application_type`"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(presence: IssPresence) -> AuthorizationRequestRecord {
        AuthorizationRequestRecord::new("https://as.example", "verifier", "st4te", presence)
    }

    /// The record is the specification's "same per-request record": all four
    /// values are readable back, and none of them was silently reordered.
    #[test]
    fn the_record_round_trips_every_value_it_was_built_with() {
        let record = record(IssPresence::Required);
        assert_eq!(record.expected_issuer(), "https://as.example");
        assert_eq!(record.code_verifier(), "verifier");
        assert_eq!(record.state(), "st4te");
        assert_eq!(record.iss_presence(), IssPresence::Required);
    }

    /// A `{:?}` of the record must not leak the CSRF state or the PKCE
    /// verifier — a record can end up inside a caller's own error or log line.
    ///
    /// The two sentinels are deliberately chosen NOT to be substrings of the
    /// field names `code_verifier` and `state`; the first draft used the
    /// literal `"verifier"` and failed against `code_verifier:` in the output,
    /// which would have been a false positive for redaction had the assertion
    /// pointed the other way.
    #[test]
    fn the_record_debug_redacts_both_secrets() {
        let record = AuthorizationRequestRecord::new(
            "https://as.example",
            "pkce-c0d3-s3cr3t",
            "csrf-t0k3n",
            IssPresence::Optional,
        );
        let rendered = format!("{record:?}");
        assert!(!rendered.contains("pkce-c0d3-s3cr3t"), "{rendered}");
        assert!(!rendered.contains("csrf-t0k3n"), "{rendered}");
        // The shape stays legible: every field is still named.
        for field in [
            "expected_issuer",
            "code_verifier",
            "state",
            "iss_presence",
            "https://as.example",
        ] {
            assert!(rendered.contains(field), "{rendered}");
        }
    }

    /// The private single-pass decoder performs the RFC 9207 §2.4
    /// `application/x-www-form-urlencoded` decode, so `%3A` becomes `:` BEFORE
    /// any comparison happens.
    #[test]
    fn parameters_are_form_urlencoded_decoded_before_comparison() {
        let params = parse_callback_parameters("iss=https%3A%2F%2Fas.example&code=a+b")
            .expect("a well-formed query parses");
        assert_eq!(params.iss.as_deref(), Some("https://as.example"));
        // `+` is a space in the form-urlencoded production, not a literal plus.
        assert_eq!(params.code.as_deref(), Some("a b"));
    }

    /// Every one of the five security parameters is refused on repetition, and
    /// the refusal does not depend on whether the two values agree.
    #[test]
    fn every_security_parameter_is_refused_when_repeated() {
        for key in SECURITY_PARAMETERS {
            for query in [format!("{key}=a&{key}=b"), format!("{key}=a&{key}=a")] {
                let err = parse_callback_parameters(&query)
                    .err()
                    .unwrap_or_else(|| panic!("a repeated `{key}` must be refused: {query}"));
                assert!(err.to_string().contains(*key), "{err}");
            }
        }
    }

    /// The control: an unknown parameter may repeat, because ignoring it twice
    /// is the same as ignoring it once.
    #[test]
    fn an_unknown_parameter_may_repeat() {
        let params = parse_callback_parameters("vendor_key=a&vendor_key=b&code=c&state=st4te")
            .expect("a repeated unknown parameter is not an error");
        assert_eq!(params.code.as_deref(), Some("c"));
        assert_eq!(params.state.as_deref(), Some("st4te"));
    }

    /// The size guard fires before parsing and reproduces none of the query.
    #[test]
    fn an_oversize_query_is_refused_without_being_echoed() {
        let marker = "PLANTED-MARKER";
        let padding = "x".repeat(MAX_CALLBACK_QUERY_BYTES);
        let query = format!("code={marker}{padding}&state=st4te");
        let err = ensure_query_within_bounds(&query).expect_err("over the limit");
        let rendered = err.to_string();
        assert!(
            rendered.contains(&MAX_CALLBACK_QUERY_BYTES.to_string()),
            "{rendered}"
        );
        assert!(rendered.contains(&query.len().to_string()), "{rendered}");
        assert!(!rendered.contains(marker), "{rendered}");
    }

    /// Exactly at the limit is accepted — the guard is `>`, not `>=`, so a
    /// legitimate maximal query is not refused by an off-by-one.
    #[test]
    fn a_query_exactly_at_the_limit_is_accepted() {
        let at_limit = "x".repeat(MAX_CALLBACK_QUERY_BYTES);
        assert!(ensure_query_within_bounds(&at_limit).is_ok());
        let over_limit = "x".repeat(MAX_CALLBACK_QUERY_BYTES + 1);
        assert!(ensure_query_within_bounds(&over_limit).is_err());
    }

    /// A response with neither `code` nor `error` names the parameter it wanted.
    #[test]
    fn a_response_with_no_outcome_parameter_says_so() {
        let err = validate_authorization_response("state=st4te", &record(IssPresence::Optional))
            .expect_err("nothing to exchange");
        assert!(err.to_string().contains("code"), "{err}");
    }

    /// The loopback predicate is shared by the issuer parse and (in the same
    /// module) by the `application_type` derivation, so it is pinned directly
    /// rather than only through its two callers.
    #[test]
    fn the_loopback_predicate_covers_all_three_spellings_and_nothing_else() {
        for loopback in [
            "http://127.0.0.1:8080",
            "http://localhost",
            "http://LOCALHOST",
            "http://[::1]:9000",
        ] {
            let parsed = Url::parse(loopback).expect("a well-formed URL");
            assert!(is_loopback_host(&parsed), "{loopback}");
        }
        for remote in [
            "https://auth.example.com",
            "https://127.0.0.1.example.com",
            "https://localhost.example.com",
        ] {
            let parsed = Url::parse(remote).expect("a well-formed URL");
            assert!(!is_loopback_host(&parsed), "{remote}");
        }
    }

    /// The two path arithmetics, exercised directly so a change to either is
    /// visible without reading the ordered list that composes them.
    #[test]
    fn the_two_well_known_forms_place_the_segment_on_opposite_sides() {
        let base = Url::parse("https://as.example/tenant1").expect("a well-formed issuer");
        assert_eq!(
            well_known_inserted(&base, WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER, "/tenant1").as_str(),
            "https://as.example/.well-known/oauth-authorization-server/tenant1"
        );
        assert_eq!(
            well_known_appended(&base, "/tenant1").as_str(),
            "https://as.example/tenant1/.well-known/openid-configuration"
        );
        // A path-less issuer collapses both `openid-configuration` forms onto
        // the same URL — which is why the returned list is two, not three.
        let root = Url::parse("https://as.example").expect("a well-formed issuer");
        assert_eq!(
            well_known_inserted(&root, WELL_KNOWN_OPENID_CONFIGURATION, "").as_str(),
            well_known_appended(&root, "").as_str()
        );
    }

    /// `https` on a loopback host is the one row the integration suite cannot
    /// reach through a natural pmcp flow, and it is the row where the two
    /// classification rules could disagree: the loopback rule must win over the
    /// `https`-means-web rule, because a locally-hosted application serving TLS
    /// over loopback is still a native application.
    #[test]
    fn loopback_wins_over_the_https_rule() {
        for uri in [
            "https://localhost:8443/callback",
            "https://127.0.0.1:8443/callback",
            "https://[::1]:8443/callback",
        ] {
            assert_eq!(
                classify_redirect_uri(uri).expect("a loopback redirect classifies"),
                ApplicationType::Native,
                "{uri}"
            );
        }
    }

    /// A percent-encoded path segment must survive candidate construction
    /// unchanged: re-encoding it would silently change the issuer's identity,
    /// and decoding it would apply exactly the RFC 3986 §6.2.2.2 normalization
    /// the anchor comparison forbids.
    #[test]
    fn a_percent_encoded_path_is_neither_decoded_nor_double_encoded() {
        let candidates = discovery_url_candidates("https://as.example/%74enant")
            .expect("a well-formed issuer with an encoded path");
        for candidate in &candidates {
            assert!(candidate.as_str().contains("%74enant"), "{candidate}");
        }
    }
}
