//! Error types for the MCP SDK.
//!
//! This module provides a comprehensive error type that covers all possible
//! failure modes in the MCP protocol.

pub mod recovery;

use std::fmt;
use thiserror::Error;

/// Result type alias for MCP operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for MCP operations.
#[derive(Error, Debug)]
pub enum Error {
    /// JSON-RPC protocol errors
    #[error("Protocol error: {code} - {message}")]
    Protocol {
        /// Error code as defined in JSON-RPC spec
        code: ErrorCode,
        /// Human-readable error message
        message: String,
        /// Optional additional error data
        data: Option<serde_json::Value>,
    },

    /// Transport-level errors
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    /// Authentication errors
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Timeout errors
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// Capability errors
    #[error("Capability not supported: {0}")]
    UnsupportedCapability(String),

    /// Internal errors
    #[error("Internal error: {0}")]
    Internal(String),

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Cancelled operation
    #[error("Operation cancelled")]
    Cancelled,

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimited,

    /// Circuit breaker is open
    #[error("Circuit breaker is open")]
    CircuitBreakerOpen,

    /// A tool executed but rejected the request at the application level —
    /// e.g. Code Mode policy/validation (`SELECT` missing a `LIMIT`, `DELETE`
    /// not allowed), schema-mismatched input, or any tool that wants the model
    /// to correct its input and retry.
    ///
    /// Unlike the protocol-level variants above (which a `tools/call` handler
    /// surfaces as a JSON-RPC error and therefore read to the model as a
    /// *server fault*), the server's tool dispatch maps `ToolRejected` to a
    /// successful [`CallToolResult`](crate::types::CallToolResult) with
    /// `isError: true`: `message` becomes the text content and `details`
    /// (when present) becomes `structuredContent`. That is the MCP-idiomatic
    /// way to tell the model "your input was not accepted — here is
    /// specifically what to change" so it can self-correct on the next call,
    /// rather than the call appearing to have crashed the server.
    ///
    /// Reach for this from a [`ToolHandler`](crate::server::ToolHandler) when
    /// the failure is the *caller's* to fix. Keep using [`Error::Internal`]
    /// (or [`Error::protocol`]) for genuine faults the caller cannot correct.
    #[error("{message}")]
    ToolRejected {
        /// Human/model-readable summary of why the input was rejected and what
        /// to change (e.g. "SELECT statements must declare a LIMIT").
        message: String,
        /// Optional machine-readable detail (e.g. a Code Mode `violations`
        /// array of `{rule, message, suggestion}`) carried verbatim into the
        /// tool result's `structuredContent` for programmatic clients.
        details: Option<serde_json::Value>,
    },

    /// Other errors
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// ===========================================================================
// Client-local MRTR errors (Phase 113, CLNT-02 / D-06 / D-09).
// ===========================================================================

/// The stable programmatic identity of [`Error::mrtr_round_limit_exceeded`].
///
/// Carried in the error's `data.pmcpError`. It is the discriminator
/// [`Error::is_mrtr_round_limit_exceeded`] matches on, so it is part of the
/// crate's compatibility surface: **do not change this string**.
pub const MRTR_ROUND_LIMIT_MARKER: &str = "MrtrRoundLimitExceeded";

/// The stable programmatic identity of [`Error::input_required_unfulfilled`].
///
/// Carried in the error's `data.pmcpError`. See [`MRTR_ROUND_LIMIT_MARKER`].
pub const MRTR_INPUT_REQUIRED_MARKER: &str = "InputRequiredUnfulfilled";

/// The stable programmatic identity of [`Error::retired_on_v2`].
///
/// Carried in the error's `data.pmcpError`. It is the discriminator
/// [`Error::is_retired_on_v2`] matches on, so it is part of the crate's
/// compatibility surface: **do not change this string**.
pub const RETIRED_ON_V2_MARKER: &str = "RetiredOnV2";

// ===========================================================================
// OAuth authorization-response errors (Phase 116, AUTH-01 / D-03 as corrected
// by RESEARCH A2).
// ===========================================================================

/// The stable programmatic identity of [`Error::iss_mismatch`].
///
/// Carried in the error's `data.pmcpError`. It is the discriminator
/// [`Error::is_iss_mismatch`] matches on, so it is part of the crate's
/// compatibility surface: **do not change this string**.
///
/// One marker covers BOTH failing rows of the spec's `iss` decision table —
/// an `iss` that is present and wrong, and an `iss` that is absent where the
/// authorization server advertised support for it. The two are distinguished
/// by [`Error::iss_actual`] returning `Some(..)` versus `None`, so a caller
/// that only wants "reject this response" needs a single predicate.
pub const ISS_MISMATCH_MARKER: &str = "IssMismatch";

/// The stable programmatic identity of [`Error::state_mismatch`].
///
/// Carried in the error's `data.pmcpError`. See [`ISS_MISMATCH_MARKER`]:
/// **do not change this string**.
pub const STATE_MISMATCH_MARKER: &str = "StateMismatch";

/// The stable programmatic identity of [`Error::reauth_required`].
///
/// Carried in the error's `data.pmcpError`. See [`ISS_MISMATCH_MARKER`]:
/// **do not change this string**.
pub const REAUTH_REQUIRED_MARKER: &str = "ReauthRequired";

/// The `data` member carrying the issuer the client RECORDED before redirecting.
const ISS_EXPECTED_KEY: &str = "expectedIssuer";

/// The `data` member carrying the issuer the authorization response ARRIVED
/// with; JSON `null` when the response carried no `iss` at all.
const ISS_ACTUAL_KEY: &str = "actualIssuer";

/// The `data` member carrying the issuer a re-authorization applies to.
const REAUTH_ISSUER_KEY: &str = "issuer";

/// The `data` member both MRTR markers ride under.
const PMCP_ERROR_KEY: &str = "pmcpError";

/// The `data` member carrying the retired method's name.
const RETIRED_METHOD_KEY: &str = "method";

/// The `data` member carrying the replacement API's name.
const RETIRED_REPLACEMENT_KEY: &str = "replacement";

/// The `data` member carrying the exceeded round limit.
const MRTR_LIMIT_KEY: &str = "limit";

/// The `data` member carrying the verbatim unfulfilled `input_required` result.
const MRTR_RESULT_KEY: &str = "result";

/// JSON-RPC error code for custom errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCode(pub i32);

impl ErrorCode {
    // The 11 associated consts below DELEGATE to the centralized version-gated
    // table in `crate::types::protocol::error_codes` — the single source of
    // truth for every protocol error code (VERS-06). Names and numeric values
    // are unchanged (API-identical, so cargo-semver-checks classifies minor);
    // only the literal is now sourced from the table so this dominant ~210-site
    // surface no longer duplicates the numbers.

    /// Parse error (-32700)
    pub const PARSE_ERROR: Self = Self(crate::types::protocol::error_codes::PARSE_ERROR);
    /// Invalid request (-32600)
    pub const INVALID_REQUEST: Self = Self(crate::types::protocol::error_codes::INVALID_REQUEST);
    /// Method not found (-32601)
    pub const METHOD_NOT_FOUND: Self = Self(crate::types::protocol::error_codes::METHOD_NOT_FOUND);
    /// Invalid params (-32602)
    pub const INVALID_PARAMS: Self = Self(crate::types::protocol::error_codes::INVALID_PARAMS);
    /// Internal error (-32603)
    pub const INTERNAL_ERROR: Self = Self(crate::types::protocol::error_codes::INTERNAL_ERROR);
    /// Request timeout (-32001)
    pub const REQUEST_TIMEOUT: Self = Self(crate::types::protocol::error_codes::REQUEST_TIMEOUT);
    /// Unsupported capability (-32002).
    ///
    /// Delegates to `error_codes::UNSUPPORTED_CAPABILITY` — the capability
    /// semantic of `-32002`, DISTINCT from the frozen `V1_TASK_PENDING` code
    /// that shares the same number.
    pub const UNSUPPORTED_CAPABILITY: Self =
        Self(crate::types::protocol::error_codes::UNSUPPORTED_CAPABILITY);
    /// Authentication required (-32003)
    pub const AUTHENTICATION_REQUIRED: Self =
        Self(crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED);
    /// Permission denied (-32004)
    pub const PERMISSION_DENIED: Self =
        Self(crate::types::protocol::error_codes::PERMISSION_DENIED);
    /// Rate limit exceeded (-32005)
    pub const RATE_LIMITED: Self = Self(crate::types::protocol::error_codes::RATE_LIMITED);
    /// Circuit breaker open (-32006)
    pub const CIRCUIT_BREAKER_OPEN: Self =
        Self(crate::types::protocol::error_codes::CIRCUIT_BREAKER_OPEN);

    /// Create a custom error code.
    pub const fn other(code: i32) -> Self {
        Self(code)
    }

    /// Convert error code to i32 value.
    pub fn as_i32(&self) -> i32 {
        self.0
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Implement Hash for `ErrorCode` to use in `HashMap`
impl std::hash::Hash for ErrorCode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Transport-specific errors.
#[derive(Error, Debug)]
pub enum TransportError {
    /// IO error
    #[error("IO error: {0}")]
    Io(String),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Request error
    #[error("Request error: {0}")]
    Request(String),

    /// Send error
    #[error("Send error: {0}")]
    Send(String),

    /// WebSocket error (when feature enabled)
    #[cfg(feature = "websocket")]
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    /// HTTP error (when feature enabled)
    #[cfg(feature = "http")]
    #[error("HTTP error: {0}")]
    Http(String),
}

impl From<std::io::Error> for TransportError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Transport(TransportError::Io(err.to_string()))
    }
}

impl Error {
    /// Create a new internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Create a new protocol error.
    pub fn protocol(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Protocol {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Get the error code for this error.
    pub fn error_code(&self) -> Option<ErrorCode> {
        match self {
            Self::Protocol { code, .. } => Some(*code),
            Self::Timeout(_) => Some(ErrorCode::REQUEST_TIMEOUT),
            Self::Authentication(_) => Some(ErrorCode::AUTHENTICATION_REQUIRED),
            Self::RateLimited => Some(ErrorCode::RATE_LIMITED),
            Self::CircuitBreakerOpen => Some(ErrorCode::CIRCUIT_BREAKER_OPEN),
            _ => None,
        }
    }

    /// Create a validation error.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    /// Create a parse error.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::PARSE_ERROR,
            message: message.into(),
            data: None,
        }
    }

    /// Create an authentication error.
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication(message.into())
    }

    /// Create a timeout error.
    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout(duration_ms)
    }

    /// Create a not found error.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    /// Create a tool-level rejection.
    ///
    /// The server's `tools/call` dispatch maps this to a
    /// [`CallToolResult`](crate::types::CallToolResult) with `isError: true`
    /// (NOT a JSON-RPC protocol error), so the model sees the reason and can
    /// retry with corrected input. `details` is carried into the result's
    /// `structuredContent`. See [`Error::ToolRejected`].
    pub fn tool_rejected(message: impl Into<String>, details: Option<serde_json::Value>) -> Self {
        Self::ToolRejected {
            message: message.into(),
            details,
        }
    }

    /// Create an unsupported capability error.
    pub fn unsupported_capability(capability: impl Into<String>) -> Self {
        Self::UnsupportedCapability(capability.into())
    }

    /// Create from JSON-RPC error.
    pub fn from_jsonrpc_error(error: crate::types::jsonrpc::JSONRPCError) -> Self {
        Self::Protocol {
            code: ErrorCode(error.code),
            message: error.message,
            data: error.data,
        }
    }

    /// Create a protocol error with just a message.
    pub fn protocol_msg(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INTERNAL_ERROR,
            message: message.into(),
            data: None,
        }
    }

    /// Check if this error matches a specific error code.
    pub fn is_error_code(&self, code: ErrorCode) -> bool {
        matches!(self.error_code(), Some(c) if c == code)
    }

    /// Create a capability error.
    pub fn capability(message: impl Into<String>) -> Self {
        Self::UnsupportedCapability(message.into())
    }

    /// Create an invalid state error.
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::InvalidState(message.into())
    }

    /// Create a cancelled error.
    pub fn cancelled() -> Self {
        Self::Cancelled
    }

    /// Create an invalid params error.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INVALID_PARAMS,
            message: message.into(),
            data: None,
        }
    }

    /// Create a method not found error.
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::METHOD_NOT_FOUND,
            message: format!("Method not found: {}", method.into()),
            data: None,
        }
    }

    // =======================================================================
    // Client-local MRTR errors (Phase 113, CLNT-02).
    // =======================================================================
    //
    // # Why these are NOT `Error` enum variants
    //
    // `pmcp::Error` is deliberately NOT `#[non_exhaustive]`, so adding a
    // variant is a MAJOR semver break (`cargo semver-checks` lint
    // `enum_variant_added`), and the v2.5 milestone is scoped as additive
    // (2.x minor). Both errors therefore ride the EXISTING
    // [`Error::Protocol`] variant, discriminated by a stable marker string in
    // `data.pmcpError` and read back through the named predicates below.
    //
    // A future contributor "fixing" these into enum variants would break every
    // downstream `match` on `Error`. Don't.
    //
    // # Why they do not squat a spec-reserved error code
    //
    // Both are CLIENT-LOCAL: they are produced by the client's own MRTR loop
    // and are never placed on the wire. They carry
    // [`ErrorCode::INTERNAL_ERROR`]'s number because a local give-up is not a
    // server-authored protocol condition — reserving a new `-32xxx` for
    // something that never travels would pollute the protocol's namespace.

    /// The MRTR gather→resend loop gave up after `limit` rounds (D-09).
    ///
    /// Returned by [`Client::call_tool`](crate::Client::call_tool) and the
    /// `*_mrtr` methods when a server keeps answering `input_required`. The
    /// bound protects BOTH client shapes: it stops a buggy or hostile server
    /// from re-prompting a human indefinitely AND from spinning an autonomous
    /// agent indefinitely. No handler is invoked for the round that trips it.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// let err = Error::mrtr_round_limit_exceeded(8);
    /// assert!(err.is_mrtr_round_limit_exceeded());
    /// assert_eq!(err.mrtr_round_limit(), Some(8));
    /// assert!(!Error::internal("nope").is_mrtr_round_limit_exceeded());
    /// ```
    #[must_use]
    pub fn mrtr_round_limit_exceeded(limit: usize) -> Self {
        Self::Protocol {
            // The field is `ErrorCode`, not a bare `i32` — the value comes
            // from the centralized VERS-06 table and is WRAPPED here.
            code: ErrorCode::INTERNAL_ERROR,
            message: format!(
                "MRTR round limit exceeded: gave up after {limit} rounds without a complete result"
            ),
            data: Some(serde_json::json!({
                PMCP_ERROR_KEY: MRTR_ROUND_LIMIT_MARKER,
                MRTR_LIMIT_KEY: limit,
            })),
        }
    }

    /// Whether this is the [`Error::mrtr_round_limit_exceeded`] give-up.
    #[must_use]
    pub fn is_mrtr_round_limit_exceeded(&self) -> bool {
        self.pmcp_error_marker() == Some(MRTR_ROUND_LIMIT_MARKER)
    }

    /// The round limit that was exceeded, for an
    /// [`Error::mrtr_round_limit_exceeded`]; `None` for any other error.
    #[must_use]
    pub fn mrtr_round_limit(&self) -> Option<usize> {
        if !self.is_mrtr_round_limit_exceeded() {
            return None;
        }
        let limit = self.protocol_data()?.get(MRTR_LIMIT_KEY)?.as_u64()?;
        usize::try_from(limit).ok()
    }

    /// The server asked for input the client could not supply, so the
    /// `input_required` result is handed back to the caller (D-06).
    ///
    /// This exists because the concrete result structs cannot carry an
    /// `input_required` result:
    /// [`CallToolResult::content`](crate::types::CallToolResult) has
    /// `#[serde(default)]`, so such a result deserializes into a silently EMPTY
    /// success, and `ReadResourceResult.contents` has no default, so the same
    /// result fails to deserialize at all. Neither is "returns the result to the
    /// caller".
    ///
    /// The full result — including its verbatim `raw` object — is recoverable
    /// through [`Error::input_required_result`]. Prefer the additive
    /// [`Client::call_tool_mrtr`](crate::Client::call_tool_mrtr) family when you
    /// want this outcome as a value rather than an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// let result: pmcp::types::mrtr::InputRequiredResult = serde_json::from_value(
    ///     serde_json::json!({ "resultType": "input_required", "requestState": "opaque" }),
    /// )
    /// .unwrap();
    ///
    /// let err = Error::input_required_unfulfilled(result);
    /// assert!(err.is_input_required_unfulfilled());
    /// assert_eq!(
    ///     err.input_required_result().unwrap().request_state.as_deref(),
    ///     Some("opaque"),
    /// );
    /// ```
    #[must_use]
    pub fn input_required_unfulfilled(result: crate::types::mrtr::InputRequiredResult) -> Self {
        // `InputRequiredResult.raw` is `#[serde(skip_serializing)]`, and it is
        // a SUPERSET of the modeled fields (the verbatim result object the
        // server sent). Carry it whenever it is an object so nothing is lost;
        // fall back to the modeled projection for a hand-built value.
        let payload = if result.raw.is_object() {
            result.raw
        } else {
            serde_json::to_value(&result).unwrap_or(serde_json::Value::Null)
        };
        Self::Protocol {
            code: ErrorCode::INTERNAL_ERROR,
            message: "the server requires more input, and no registered handler could supply it — \
                 see Error::input_required_result() or the *_mrtr client methods"
                .to_string(),
            data: Some(serde_json::json!({
                PMCP_ERROR_KEY: MRTR_INPUT_REQUIRED_MARKER,
                MRTR_RESULT_KEY: payload,
            })),
        }
    }

    /// Whether this carries an unfulfilled `input_required` result.
    #[must_use]
    pub fn is_input_required_unfulfilled(&self) -> bool {
        self.pmcp_error_marker() == Some(MRTR_INPUT_REQUIRED_MARKER)
    }

    /// The unfulfilled `input_required` result, for an
    /// [`Error::input_required_unfulfilled`]; `None` for any other error.
    ///
    /// Returned by value (not by reference) because the payload is stored
    /// serialized inside the error's `data`, which is what keeps this an
    /// additive change to the existing [`Error::Protocol`] variant.
    #[must_use]
    pub fn input_required_result(&self) -> Option<crate::types::mrtr::InputRequiredResult> {
        if !self.is_input_required_unfulfilled() {
            return None;
        }
        let payload = self.protocol_data()?.get(MRTR_RESULT_KEY)?;
        serde_json::from_value(payload.clone()).ok()
    }

    /// A method the 2026-07-28 schema RETIRED was called on a v2 connection
    /// (Phase 113, HTTP-04; Phase 114, TASK-03).
    ///
    /// `resources/subscribe` and `resources/unsubscribe` are gone from the v2
    /// schema, and a pmcp v2 server answers both with `404` + `-32601`. Rather
    /// than perform that pointless round trip and hand back an opaque
    /// method-not-found, the client fails fast LOCALLY with this error, whose
    /// message names the replacement API.
    ///
    /// # The message names ONLY the caller-supplied replacement
    ///
    /// It used to append a hard-coded `(Client::subscriptions_listen)`, from
    /// when the subscriptions pair were the only two retired methods. Phase 114
    /// added a SECOND family — `tasks/list` and `tasks/result` are absent from
    /// the v2 tasks extension — for which that parenthetical is simply FALSE, and
    /// a refusal message is the one signal a caller has for choosing its next
    /// move. The pmcp API pointer belongs in the caller's `replacement` string,
    /// not baked into a shared builder.
    ///
    /// Like the two MRTR client-local errors above, this rides the existing
    /// [`Error::Protocol`] variant discriminated by a marker in
    /// `data.pmcpError`, because [`enum@Error`] is not `#[non_exhaustive]` and a new
    /// variant would be a MAJOR semver break. Don't "fix" it into one.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// let err = Error::retired_on_v2("resources/subscribe", "subscriptions/listen");
    /// assert!(err.is_retired_on_v2());
    /// assert_eq!(err.retired_method(), Some("resources/subscribe"));
    /// assert!(err.to_string().contains("subscriptions/listen"));
    /// assert!(!Error::internal("nope").is_retired_on_v2());
    /// ```
    #[must_use]
    pub fn retired_on_v2(method: &str, replacement: &str) -> Self {
        Self::Protocol {
            // The field is `ErrorCode`, not a bare `i32` — the value comes from
            // the centralized VERS-06 table and is WRAPPED here. `-32601` is the
            // code the SERVER would have answered with; producing it locally
            // keeps a caller that already branches on method-not-found working.
            code: ErrorCode::METHOD_NOT_FOUND,
            message: format!(
                "{method} was removed in MCP 2026-07-28 and this connection speaks that version; \
                 use {replacement} instead"
            ),
            data: Some(serde_json::json!({
                PMCP_ERROR_KEY: RETIRED_ON_V2_MARKER,
                RETIRED_METHOD_KEY: method,
                RETIRED_REPLACEMENT_KEY: replacement,
            })),
        }
    }

    /// Whether this is the [`Error::retired_on_v2`] local fail-fast.
    #[must_use]
    pub fn is_retired_on_v2(&self) -> bool {
        self.pmcp_error_marker() == Some(RETIRED_ON_V2_MARKER)
    }

    /// The retired method name, for an [`Error::retired_on_v2`]; `None` for any
    /// other error.
    #[must_use]
    pub fn retired_method(&self) -> Option<&str> {
        self.retired_field(RETIRED_METHOD_KEY)
    }

    /// The replacement API a caller should use instead of the retired method.
    #[must_use]
    pub fn retired_replacement(&self) -> Option<&str> {
        self.retired_field(RETIRED_REPLACEMENT_KEY)
    }

    /// One string field of an [`Error::retired_on_v2`] marker payload.
    ///
    /// Borrows rather than allocating: every caller of the two public accessors
    /// only ever compares the value.
    fn retired_field(&self, key: &str) -> Option<&str> {
        self.marker_field(RETIRED_ON_V2_MARKER, key)
    }

    /// One string field of a marker payload, guarded by the marker it belongs to.
    ///
    /// Every marker family in this module reads its payload the same way: check
    /// that THIS error carries THAT marker, then project one `data` key out as a
    /// string. Writing that out per family made the guard-before-read invariant a
    /// convention rather than a single enforced function, so a fourth family
    /// would have arrived as a fourth copy.
    ///
    /// Borrows rather than allocating — every caller only compares the value.
    fn marker_field(&self, marker: &str, key: &str) -> Option<&str> {
        if self.pmcp_error_marker() != Some(marker) {
            return None;
        }
        self.protocol_data()?.get(key)?.as_str()
    }

    // =======================================================================
    // OAuth authorization-response errors (Phase 116, AUTH-01).
    // =======================================================================
    //
    // # Why these ride `Error::Protocol` and not `Error::Authentication`
    //
    // The obvious carrier for an OAuth failure looks like
    // [`Error::Authentication`], but it is a bare-`String` tuple variant with
    // no `data` member, and the whole marker machinery below —
    // `protocol_data()` and `pmcp_error_marker()` — matches only
    // `Self::Protocol`. An `iss_mismatch` returning `Authentication` would make
    // `is_iss_mismatch()` return `false` for its own constructor's output.
    // A test pins this so a future refactor cannot silently move the carrier.
    //
    // As with the MRTR and retired-RPC errors above, they are NOT new enum
    // variants: [`Error`] is deliberately not `#[non_exhaustive]`, so a new
    // variant is a MAJOR semver break and this milestone is additive.
    //
    // # Why `ErrorCode::INVALID_REQUEST`
    //
    // All three are produced LOCALLY by the client while inspecting an
    // authorization response that is malformed or hostile. `-32600` describes
    // the thing that was rejected — a request/response the client refuses to
    // act on — rather than a transport fault or an internal failure, which is
    // the same reasoning by which `retired_on_v2` carries `-32601`.

    /// The authorization response's `iss` failed RFC 9207 §2.4 validation.
    ///
    /// `actual` is `Some(iss)` when the response carried an `iss` that is not
    /// byte-identical to the recorded issuer, and `None` for the spec's row-2
    /// case: the authorization server advertises
    /// `authorization_response_iss_parameter_supported` and sent no `iss` at
    /// all. Both rows share [`ISS_MISMATCH_MARKER`], so one predicate covers
    /// "reject this response"; [`Error::iss_actual`] separates them when a
    /// caller cares which happened.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// let err = Error::iss_mismatch("https://as.example", Some("https://evil.example"));
    /// assert!(err.is_iss_mismatch());
    /// assert_eq!(err.iss_expected(), Some("https://as.example"));
    /// assert_eq!(err.iss_actual(), Some("https://evil.example"));
    ///
    /// // Row 2: the parameter was advertised but not sent.
    /// assert_eq!(Error::iss_mismatch("https://as.example", None).iss_actual(), None);
    ///
    /// assert!(!Error::internal("nope").is_iss_mismatch());
    /// ```
    #[must_use]
    pub fn iss_mismatch(expected: &str, actual: Option<&str>) -> Self {
        let message = match actual {
            Some(actual) => format!(
                "authorization response `iss` mismatch: recorded issuer {expected}, response \
                 carried {actual} — rejecting per RFC 9207 §2.4"
            ),
            None => format!(
                "authorization response is missing the `iss` parameter, but {expected} advertises \
                 authorization_response_iss_parameter_supported — rejecting per RFC 9207 §2.4"
            ),
        };
        Self::Protocol {
            // The field is `ErrorCode`, not a bare `i32` — the value comes from
            // the centralized VERS-06 table and is WRAPPED here.
            code: ErrorCode::INVALID_REQUEST,
            message,
            data: Some(serde_json::json!({
                PMCP_ERROR_KEY: ISS_MISMATCH_MARKER,
                ISS_EXPECTED_KEY: expected,
                // `None` serializes to JSON `null`, which `as_str()` reads back
                // as `None` — so the row-2 case needs no second marker.
                ISS_ACTUAL_KEY: actual,
            })),
        }
    }

    /// Whether this is the [`Error::iss_mismatch`] rejection.
    #[must_use]
    pub fn is_iss_mismatch(&self) -> bool {
        self.pmcp_error_marker() == Some(ISS_MISMATCH_MARKER)
    }

    /// The issuer the client RECORDED before redirecting, for an
    /// [`Error::iss_mismatch`]; `None` for any other error.
    #[must_use]
    pub fn iss_expected(&self) -> Option<&str> {
        self.iss_field(ISS_EXPECTED_KEY)
    }

    /// The issuer the authorization response actually carried, for an
    /// [`Error::iss_mismatch`].
    ///
    /// `None` means either "this is not an `iss` mismatch" or "the response
    /// carried no `iss`" — check [`Error::is_iss_mismatch`] first to tell them
    /// apart.
    #[must_use]
    pub fn iss_actual(&self) -> Option<&str> {
        self.iss_field(ISS_ACTUAL_KEY)
    }

    /// One string field of an [`Error::iss_mismatch`] marker payload.
    ///
    /// Borrows rather than allocating, mirroring [`Error::retired_field`].
    fn iss_field(&self, key: &str) -> Option<&str> {
        self.marker_field(ISS_MISMATCH_MARKER, key)
    }

    /// The authorization response's `state` did not match the value recorded
    /// for this authorization request (CSRF / code-injection defence).
    ///
    /// # Why this takes no arguments
    ///
    /// The expected `state` is a CSRF secret and the received one is
    /// attacker-controlled. Naming either in the refusal turns the refusal into
    /// a disclosure channel — the same rule the refused-body path states at
    /// `src/shared/sse_optimized.rs`. A caller that needs to correlate the
    /// failure already holds the per-request record.
    ///
    /// Absence of `state` from the response is this error too, not a skip.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// let err = Error::state_mismatch();
    /// assert!(err.is_state_mismatch());
    /// assert!(!err.to_string().contains("Ml3n4L0PxQ"));
    /// assert!(!Error::internal("nope").is_state_mismatch());
    /// ```
    #[must_use]
    pub fn state_mismatch() -> Self {
        Self::Protocol {
            code: ErrorCode::INVALID_REQUEST,
            message: "authorization response `state` did not match the value recorded for this \
                      authorization request; neither value is reproduced here because the \
                      expected one is a CSRF secret and the received one is attacker-controlled"
                .to_string(),
            data: Some(serde_json::json!({
                PMCP_ERROR_KEY: STATE_MISMATCH_MARKER,
            })),
        }
    }

    /// Whether this is the [`Error::state_mismatch`] rejection.
    #[must_use]
    pub fn is_state_mismatch(&self) -> bool {
        self.pmcp_error_marker() == Some(STATE_MISMATCH_MARKER)
    }

    /// Stored credentials for `issuer` can no longer be used, and the caller
    /// must run a fresh authorization against it (SEP-2352).
    ///
    /// `reason` is operator-facing prose — for example, a refresh grant the
    /// authorization server rejected, or credentials bound to a DIFFERENT
    /// issuer than the one protected-resource metadata now names.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// let err = Error::reauth_required("https://as.example", "refresh failed");
    /// assert!(err.is_reauth_required());
    /// assert_eq!(err.reauth_issuer(), Some("https://as.example"));
    /// assert!(!Error::internal("nope").is_reauth_required());
    /// ```
    #[must_use]
    pub fn reauth_required(issuer: &str, reason: &str) -> Self {
        Self::Protocol {
            code: ErrorCode::INVALID_REQUEST,
            message: format!("re-authorization with {issuer} is required: {reason}"),
            data: Some(serde_json::json!({
                PMCP_ERROR_KEY: REAUTH_REQUIRED_MARKER,
                REAUTH_ISSUER_KEY: issuer,
            })),
        }
    }

    /// Whether this is the [`Error::reauth_required`] signal.
    #[must_use]
    pub fn is_reauth_required(&self) -> bool {
        self.pmcp_error_marker() == Some(REAUTH_REQUIRED_MARKER)
    }

    /// The issuer a [`Error::reauth_required`] applies to; `None` for any other
    /// error.
    #[must_use]
    pub fn reauth_issuer(&self) -> Option<&str> {
        self.marker_field(REAUTH_REQUIRED_MARKER, REAUTH_ISSUER_KEY)
    }

    /// The `data` object of an [`Error::Protocol`], if it has one.
    fn protocol_data(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        match self {
            Self::Protocol { data, .. } => data.as_ref()?.as_object(),
            _ => None,
        }
    }

    /// The `data.pmcpError` marker string, if this error carries one.
    fn pmcp_error_marker(&self) -> Option<&str> {
        self.protocol_data()?.get(PMCP_ERROR_KEY)?.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = Error::internal("test error");
        assert!(matches!(err, Error::Internal(_)));

        let err = Error::protocol(ErrorCode::INVALID_REQUEST, "bad request");
        assert!(matches!(err, Error::Protocol { .. }));
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(ErrorCode::PARSE_ERROR.as_i32(), -32700);
        assert_eq!(ErrorCode::RATE_LIMITED.as_i32(), -32005);
        assert_eq!(ErrorCode::CIRCUIT_BREAKER_OPEN.as_i32(), -32006);
    }

    // =======================================================================
    // Client-local MRTR errors (Phase 113, CLNT-02).
    // =======================================================================

    mod mrtr {
        use super::*;
        use crate::types::mrtr::InputRequiredResult;
        use serde_json::json;

        fn input_required(raw: serde_json::Value) -> InputRequiredResult {
            serde_json::from_value(raw).expect("the fixture is a valid input_required result")
        }

        #[test]
        fn round_limit_error_is_distinguishable() {
            let err = Error::mrtr_round_limit_exceeded(8);
            assert!(err.is_mrtr_round_limit_exceeded());
            assert!(!err.is_input_required_unfulfilled());
        }

        #[test]
        fn an_unrelated_error_is_not_the_round_limit() {
            assert!(!Error::internal("x").is_mrtr_round_limit_exceeded());
            assert!(!Error::internal("x").is_input_required_unfulfilled());
            // A `Protocol` error with no `data` must not match either.
            assert!(!Error::protocol(ErrorCode::INTERNAL_ERROR, "x").is_mrtr_round_limit_exceeded());
        }

        #[test]
        fn round_limit_error_carries_the_limit() {
            assert_eq!(
                Error::mrtr_round_limit_exceeded(8).mrtr_round_limit(),
                Some(8)
            );
            assert_eq!(Error::internal("x").mrtr_round_limit(), None);
        }

        #[test]
        fn round_limit_display_names_the_bound() {
            let rendered = Error::mrtr_round_limit_exceeded(8).to_string();
            assert!(
                rendered.contains('8'),
                "the limit must be visible: {rendered}"
            );
            assert!(
                rendered.contains("round limit"),
                "the reason must be visible: {rendered}"
            );
        }

        #[test]
        fn input_required_error_is_distinguishable() {
            let err = Error::input_required_unfulfilled(input_required(
                json!({ "resultType": "input_required", "requestState": "opaque" }),
            ));
            assert!(err.is_input_required_unfulfilled());
            assert!(!err.is_mrtr_round_limit_exceeded());
        }

        /// The plan's acceptance criterion: the accessor ROUND-TRIPS the
        /// `requestState` (and the `inputRequests` map) the server sent, so a
        /// caller loses nothing by receiving this as an error.
        #[test]
        fn input_required_error_round_trips_the_result() {
            let raw = json!({
                "resultType": "input_required",
                "requestState": "opaque-token",
                "inputRequests": {
                    "user_name": {
                        "method": "elicitation/create",
                        "params": { "message": "who?", "requestedSchema": {} }
                    }
                },
                "_meta": { "vendor/key": 1 },
                "somethingUnmodelled": true
            });
            let err = Error::input_required_unfulfilled(input_required(raw.clone()));
            let recovered = err.input_required_result().expect("the payload survives");

            assert_eq!(recovered.request_state.as_deref(), Some("opaque-token"));
            assert_eq!(recovered.result_type, "input_required");
            let requests = recovered.input_requests.expect("inputRequests survive");
            assert_eq!(requests.len(), 1);
            assert!(requests.contains_key("user_name"));
            assert_eq!(
                recovered.raw, raw,
                "the VERBATIM result object must survive, unmodelled keys included"
            );
        }

        #[test]
        fn input_required_result_is_none_for_other_errors() {
            assert!(Error::internal("x").input_required_result().is_none());
            assert!(Error::mrtr_round_limit_exceeded(8)
                .input_required_result()
                .is_none());
        }

        /// Both constructors build a `Protocol` error with a properly WRAPPED
        /// `ErrorCode` (the field is `ErrorCode`, not a bare `i32`).
        #[test]
        fn both_errors_carry_a_wrapped_error_code() {
            for err in [
                Error::mrtr_round_limit_exceeded(3),
                Error::input_required_unfulfilled(input_required(
                    json!({ "resultType": "input_required" }),
                )),
            ] {
                assert!(matches!(err, Error::Protocol { .. }));
                assert_eq!(err.error_code(), Some(ErrorCode::INTERNAL_ERROR));
            }
        }

        /// The markers are the programmatic identity — a rename is a silent
        /// break for anyone matching on them.
        #[test]
        fn markers_are_stable_strings() {
            assert_eq!(MRTR_ROUND_LIMIT_MARKER, "MrtrRoundLimitExceeded");
            assert_eq!(MRTR_INPUT_REQUIRED_MARKER, "InputRequiredUnfulfilled");
            assert_eq!(RETIRED_ON_V2_MARKER, "RetiredOnV2");
        }
    }

    /// The v2 retired-RPC local fail-fast (Phase 113, HTTP-04).
    mod retired_on_v2 {
        use super::*;

        #[test]
        fn it_is_identifiable_and_carries_both_names() {
            let err = Error::retired_on_v2("resources/subscribe", "subscriptions/listen");
            assert!(err.is_retired_on_v2());
            assert_eq!(err.retired_method(), Some("resources/subscribe"));
            assert_eq!(err.retired_replacement(), Some("subscriptions/listen"));
        }

        /// The message must be ACTIONABLE: it names the replacement API, which
        /// is the whole reason this beats the server's opaque `-32601`.
        #[test]
        fn the_message_names_the_replacement() {
            let err = Error::retired_on_v2("resources/unsubscribe", "subscriptions/listen");
            let message = err.to_string();
            assert!(message.contains("subscriptions/listen"), "{message}");
            assert!(message.contains("resources/unsubscribe"), "{message}");
        }

        /// It rides `Error::Protocol` with a WRAPPED code — no new enum
        /// variant, because `Error` is not `#[non_exhaustive]`.
        #[test]
        fn it_rides_the_protocol_variant_with_method_not_found() {
            let err = Error::retired_on_v2("resources/subscribe", "subscriptions/listen");
            assert!(matches!(err, Error::Protocol { .. }));
            assert_eq!(err.error_code(), Some(ErrorCode::METHOD_NOT_FOUND));
        }

        /// The predicates must not fire on unrelated errors, including the two
        /// sibling MRTR markers.
        #[test]
        fn other_errors_are_not_mistaken_for_it() {
            for err in [
                Error::internal("nope"),
                Error::protocol(ErrorCode::METHOD_NOT_FOUND, "Method not found: whatever"),
                Error::mrtr_round_limit_exceeded(3),
            ] {
                assert!(!err.is_retired_on_v2(), "{err}");
                assert!(err.retired_method().is_none());
                assert!(err.retired_replacement().is_none());
            }
        }
    }

    /// The three OAuth authorization-response discriminators (Phase 116,
    /// AUTH-01 / D-03 as corrected by RESEARCH A2).
    mod auth_markers {
        use super::*;

        /// Row 1/3 of the spec's `iss` table: an `iss` was present and did not
        /// match. Both issuers are recoverable programmatically, so a caller
        /// never has to parse the message (T-116-01).
        #[test]
        fn iss_mismatch_is_identifiable_and_carries_both_issuers() {
            let err = Error::iss_mismatch("https://as.example", Some("https://evil.example"));
            assert!(err.is_iss_mismatch());
            assert_eq!(err.iss_expected(), Some("https://as.example"));
            assert_eq!(err.iss_actual(), Some("https://evil.example"));
        }

        /// Row 2 of the spec's `iss` table: the AS advertises
        /// `authorization_response_iss_parameter_supported` and sent no `iss`.
        /// One marker covers both rows; the ABSENCE is expressed as
        /// `iss_actual() == None`, not as a second marker.
        #[test]
        fn iss_mismatch_with_an_absent_iss_reports_no_actual_issuer() {
            let err = Error::iss_mismatch("https://as.example", None);
            assert!(err.is_iss_mismatch());
            assert_eq!(err.iss_expected(), Some("https://as.example"));
            assert_eq!(err.iss_actual(), None);
        }

        /// The two rows must be distinguishable in human-readable prose even
        /// though they share one marker — the message is the only signal an
        /// operator reading a log has.
        #[test]
        fn iss_mismatch_messages_distinguish_a_wrong_iss_from_an_absent_one() {
            let wrong =
                Error::iss_mismatch("https://as.example", Some("https://evil.example")).to_string();
            let absent = Error::iss_mismatch("https://as.example", None).to_string();
            assert_ne!(wrong, absent, "the two rows must not read identically");
            assert!(wrong.contains("https://evil.example"), "{wrong}");
            assert!(
                absent.contains("missing") || absent.contains("absent"),
                "the row-2 message must say the parameter was not sent: {absent}"
            );
        }

        /// T-116-03: the expected `state` is a CSRF secret and the received one
        /// is attacker-controlled. Echoing EITHER turns a refusal into a
        /// disclosure channel, so the constructor takes no arguments at all.
        #[test]
        fn state_mismatch_discloses_neither_the_expected_nor_the_received_value() {
            let err = Error::state_mismatch();
            assert!(err.is_state_mismatch());
            let rendered = err.to_string();
            for secret in [
                "Ml3n4L0PxQ-expected-csrf-secret",
                "attacker-supplied-state-value",
            ] {
                assert!(
                    !rendered.contains(secret),
                    "the refusal must not reproduce a state value: {rendered}"
                );
            }
            assert!(
                rendered.contains("state"),
                "the reason must still be legible: {rendered}"
            );
        }

        /// The re-authorization signal names the issuer it applies to, so a
        /// multi-issuer client knows WHICH credential to refresh.
        #[test]
        fn reauth_required_names_its_issuer_and_its_reason() {
            let err = Error::reauth_required("https://as.example", "refresh failed");
            assert!(err.is_reauth_required());
            assert_eq!(err.reauth_issuer(), Some("https://as.example"));
            let rendered = err.to_string();
            assert!(rendered.contains("refresh failed"), "{rendered}");
            assert!(rendered.contains("https://as.example"), "{rendered}");
        }

        /// All three ride `Error::Protocol` — no new enum variant, because
        /// `Error` is not `#[non_exhaustive]` and a variant would be MAJOR.
        #[test]
        fn iss_mismatch_state_mismatch_and_reauth_required_ride_the_protocol_variant() {
            for err in [
                Error::iss_mismatch("https://as.example", Some("https://evil.example")),
                Error::iss_mismatch("https://as.example", None),
                Error::state_mismatch(),
                Error::reauth_required("https://as.example", "refresh failed"),
            ] {
                assert!(matches!(err, Error::Protocol { .. }), "{err}");
                assert_eq!(err.error_code(), Some(ErrorCode::INVALID_REQUEST));
            }
        }

        /// The three predicates are mutually exclusive and do not fire on
        /// unrelated errors, including the sibling markers on the same variant.
        #[test]
        fn unrelated_errors_are_neither_iss_mismatch_state_mismatch_nor_reauth_required() {
            for err in [
                Error::internal("nope"),
                Error::protocol(ErrorCode::INVALID_REQUEST, "bad request"),
                Error::retired_on_v2("resources/subscribe", "subscriptions/listen"),
                Error::mrtr_round_limit_exceeded(3),
            ] {
                assert!(!err.is_iss_mismatch(), "{err}");
                assert!(!err.is_state_mismatch(), "{err}");
                assert!(!err.is_reauth_required(), "{err}");
                assert!(err.iss_expected().is_none());
                assert!(err.iss_actual().is_none());
                assert!(err.reauth_issuer().is_none());
            }

            // Cross-predicate: each marker answers only for itself.
            let iss = Error::iss_mismatch("https://as.example", None);
            assert!(!iss.is_state_mismatch());
            assert!(!iss.is_reauth_required());
            assert!(iss.reauth_issuer().is_none());

            let state = Error::state_mismatch();
            assert!(!state.is_iss_mismatch());
            assert!(!state.is_reauth_required());

            let reauth = Error::reauth_required("https://as.example", "x");
            assert!(!reauth.is_iss_mismatch());
            assert!(!reauth.is_state_mismatch());
            assert!(reauth.iss_expected().is_none());
        }

        /// Pins RESEARCH finding A2. `Error::Authentication` is a bare-`String`
        /// tuple variant with NO `data` member, and `protocol_data()` matches
        /// only `Self::Protocol` — so a marker moved back onto `Authentication`
        /// would make its own predicate return `false` (T-116-05).
        #[test]
        fn the_authentication_variant_cannot_carry_an_iss_mismatch_marker() {
            let err = Error::Authentication(format!(
                "{{\"pmcpError\":\"{ISS_MISMATCH_MARKER}\"}} looks like a marker but is a String"
            ));
            assert!(!err.is_iss_mismatch());
            assert!(!err.is_state_mismatch());
            assert!(!err.is_reauth_required());
            assert!(err.iss_expected().is_none());
        }

        /// The marker strings are the programmatic identity — a rename is a
        /// silent break for anyone matching on them.
        #[test]
        fn iss_mismatch_state_mismatch_and_reauth_required_markers_are_stable_strings() {
            assert_eq!(ISS_MISMATCH_MARKER, "IssMismatch");
            assert_eq!(STATE_MISMATCH_MARKER, "StateMismatch");
            assert_eq!(REAUTH_REQUIRED_MARKER, "ReauthRequired");
        }
    }
}
