// Net-new code for Phase 90 OAPI-01 (HttpConnector trait + HttpClient) +
// OAPI-03 (HttpAuthProvider + AuthConfig six modes). SHAPE lifted from the SQL
// connector analog (`crate::sql`); BODY lifted from the pmcp-run OpenAPI
// reference (`mcp-openapi-server-core`): the reference HTTP client + the shared
// `mcp-server-common` auth providers. The lift replaces the `mcp_server_common`
// path-dependency with toolkit-owned types.

//! HTTP backend primitives for config-driven OpenAPI MCP servers.
//!
//! This module is the backend seam the single-call synthesizer (Plan 03), the
//! code-mode executor (Plan 04), and the binary dispatch (Plan 06) build on. It
//! mirrors [`crate::sql`] in shape:
//!
//! - [`HttpConnector`] — the `#[async_trait] Send + Sync + 'static` trait that
//!   executes a REST [`Operation`] and returns JSON (analog of `SqlConnector`).
//! - [`HttpConnectorError`] — the `#[non_exhaustive]` error enum whose `Display`
//!   reaches MCP clients and therefore MUST NOT echo credentials or URLs (analog
//!   of `ConnectorError`, mirrors its Connection Security doc-comment).
//! - [`Operation`] / [`Parameter`] / [`ParameterLocation`] — the request model
//!   the trait signature needs. AUTHORITATIVE in [`schema`] (the `openapiv3`
//!   parser is their producer) and re-exported here so the trait signature and
//!   every later plan reference one stable type path (Plan 03 / OAPI-02).
//! - [`join_url`] — the ONE shared `base_url` + `path` concatenation helper. Both
//!   [`client::HttpClient`] (this plan) and Plan 04's `HttpCodeExecutor` call it
//!   instead of re-inlining the trim logic — it preserves an API-Gateway stage
//!   prefix (`/v1`) where `Url::join` would silently drop it (Pitfall 2).
//!
//! The whole module is gated behind the opt-in `http` feature so the curated /
//! no-`http` toolkit build stays light (RESEARCH Pitfall 4).

// Why: HTTP method names ("GET", "POST") and product nouns ("OpenAPI") are
// proper nouns / acronyms clippy::doc_markdown otherwise flags for back-ticks.
#![allow(clippy::doc_markdown)]

use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;

/// Authentication providers for OUTGOING HTTP requests (OAPI-03 / D-05).
pub mod auth;
/// reqwest-backed [`HttpConnector`] implementation (OAPI-01).
pub mod client;
/// OpenAPI schema parsing seam — forward stub filled by Plan 03 (OAPI-02).
pub mod schema;

#[doc(inline)]
pub use auth::{
    create_auth_provider, create_passthrough_auth_provider, AuthConfig, HttpAuthProvider,
};
#[doc(inline)]
pub use client::{HttpClient, HttpConfig};

// Operation / Parameter / ParameterLocation are AUTHORITATIVE in `schema` (the
// parser is their producer). They are re-exported here so the
// [`HttpConnector::execute`] trait signature and every Plan (01/03/04/05) keep
// referencing ONE stable type path — the type never moves home again (Codex
// MEDIUM: keep `Operation` in one place from day one).
#[doc(inline)]
pub use schema::{OpenApiSchema, Operation, Parameter, ParameterLocation};

/// Concatenate a base URL and a request path with exactly one separating slash,
/// PRESERVING any non-root path already on the base.
///
/// This is the ONE shared URL-join helper for the `http` module (de-dup: both
/// [`client::HttpClient`] and Plan 04's `HttpCodeExecutor` call it). It is
/// deliberately NOT `Url::join`, which follows RFC 3986 and treats an absolute
/// request path (e.g. `/users`) as REPLACING the base path — that silently drops
/// an API-Gateway stage prefix like `/v1` (Pitfall 2 / T-90-01-05).
///
/// # Examples
///
/// ```
/// # // join_url is pub(crate); the behaviour is asserted in the module tests.
/// // join_url("https://x/v1", "/users") == "https://x/v1/users"
/// ```
#[must_use]
pub(crate) fn join_url(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Errors an [`HttpConnector`] implementation may surface.
///
/// The enum is `#[non_exhaustive]` so later plans can add failure modes
/// additively without a semver break (mirrors [`crate::sql::ConnectorError`]).
///
/// # Security
///
/// The inner `String` of every variant reaches MCP clients via `Display`.
/// Implementors MUST NOT include the request URL, an `Authorization` header
/// value, a bearer token, or an `app_key` in any inner `String` — those are
/// credentials or capability-bearing locators. Construct error messages from
/// non-secret context only (status code, a static reason). This mirrors the
/// `ConnectorError::Connection` discipline in `sql/mod.rs` (T-90-01-01).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HttpConnectorError {
    /// The outgoing request failed at the transport layer (connect / timeout /
    /// body read). The reqwest error is deliberately NOT forwarded verbatim —
    /// its `Display` can echo the URL — so this carries a redacted reason only.
    #[error("http request failed: {0}")]
    Request(String),

    /// The backend returned a non-2xx HTTP status.
    #[error("http backend returned status {status}")]
    Status {
        /// The HTTP status code (e.g. `401`, `503`).
        status: u16,
    },

    /// Authentication could not be applied to the outgoing request (e.g. a
    /// required passthrough token was absent). The reason MUST NOT echo the
    /// token or header value.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// A header name or value could not be constructed from the configured /
    /// supplied value. The reason MUST NOT echo a credential header's value.
    #[error("invalid header: {0}")]
    InvalidHeader(String),

    /// A backend / configuration problem not covered by the variants above
    /// (e.g. an unparseable base URL, an unknown HTTP method).
    #[error("http backend error: {0}")]
    Backend(String),
}

/// Backend-agnostic HTTP connector trait (OAPI-01).
///
/// The analog of [`crate::sql::SqlConnector`] for REST backends: an
/// implementation executes an [`Operation`] against a configured base URL and
/// returns the response body as JSON. [`base_url`](HttpConnector::base_url) is
/// the analog of `SqlConnector::dialect()` — a cheap accessor used by the
/// synthesizer / prompt assembly.
///
/// # Example
///
/// A minimal connector. The example defines a LOCAL dummy struct so the doctest
/// does not depend on any downstream crate (mirrors the `SqlConnector` doctest).
///
/// ```no_run
/// use pmcp_server_toolkit::http::{HttpConnector, HttpConnectorError, Operation};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct Dummy;
///
/// #[async_trait]
/// impl HttpConnector for Dummy {
///     fn base_url(&self) -> &str { "https://api.example.com/v1" }
///     async fn execute(&self, _operation: &Operation, _args: &Value)
///         -> Result<Value, HttpConnectorError> {
///         Ok(Value::Null)
///     }
/// }
/// ```
#[async_trait]
pub trait HttpConnector: Send + Sync + 'static {
    /// Execute `operation` with the caller-supplied `args` (a JSON object whose
    /// keys map to path / query / header / body parameters) and return the
    /// response body as a [`serde_json::Value`].
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError`] when the request fails at the transport
    /// layer ([`HttpConnectorError::Request`]), the backend returns a non-2xx
    /// status ([`HttpConnectorError::Status`]), authentication cannot be applied
    /// ([`HttpConnectorError::Auth`]), or a header is invalid
    /// ([`HttpConnectorError::InvalidHeader`]). Per the type-level Security note,
    /// no error message echoes a URL or credential.
    async fn execute(
        &self,
        operation: &Operation,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError>;

    /// The configured base URL (analog of `SqlConnector::dialect()`).
    fn base_url(&self) -> &str;

    /// Execute `operation` on behalf of the MCP tool named `tool` (Phase 128 E1).
    ///
    /// The tool name is the one thing an `Operation` cannot carry — it describes an
    /// endpoint, not a tool — and an E1 [`crate::policy::RequestPolicy`] that keys
    /// on the tool needs it. The synthesized single-call handler calls THIS method;
    /// the default body delegates to [`execute`](HttpConnector::execute), so an
    /// out-of-repo implementation compiles unchanged and simply reports no tool.
    ///
    /// # Errors
    ///
    /// As [`execute`](HttpConnector::execute).
    async fn execute_for_tool(
        &self,
        tool: &str,
        operation: &Operation,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError> {
        let _ = tool;
        self.execute(operation, args).await
    }

    /// Whether this connector consults an E1 policy before sending (Phase 128).
    ///
    /// Default `false`. Overridden by [`crate::http::HttpClient`]. It exists so a
    /// registered-but-unreached policy cannot look registered: the wiring that
    /// attaches one lives in a different crate
    /// (`pmcp-openapi-server`'s `build_server`) and the connector is an
    /// `Arc<dyn HttpConnector>` by then, so the only way to PROVE the attachment
    /// took is to ask through the trait.
    fn has_request_policy(&self) -> bool {
        false
    }

    /// A clone of this connector that consults `policy` before every outbound
    /// request (Phase 128 E1), or `None` when the implementation cannot host one.
    ///
    /// `None` is the default, and a caller that holds a policy MUST treat `None`
    /// as a hard problem rather than a silent no-op — a policy the operator
    /// registered and the connector never consults is precisely the
    /// present-but-inert defect this phase exists to close.
    fn governed(
        &self,
        policy: Arc<dyn crate::policy::RequestPolicy>,
    ) -> Option<Arc<dyn HttpConnector>> {
        let _ = policy;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_url_preserves_prefix() {
        // The API-Gateway stage prefix `/v1` survives; exactly one slash joins.
        assert_eq!(join_url("https://x/v1", "/users"), "https://x/v1/users");
        // Trailing slash on base + leading slash on path collapse to one.
        assert_eq!(join_url("https://x/v1/", "/users"), "https://x/v1/users");
        // No leading slash on path still joins with exactly one slash.
        assert_eq!(join_url("https://x/v1", "users"), "https://x/v1/users");
        // Root base.
        assert_eq!(join_url("https://x", "/users"), "https://x/users");
    }

    /// T-90-01-01: the rendered `Display` of every error variant MUST NOT echo a
    /// URL, an `Authorization`/`Bearer` value, or an `app_key`. Mirrors the SQL
    /// redaction test `test_connection_display_does_not_echo_password`.
    #[test]
    fn test_http_error_display_does_not_echo_secret() {
        let variants = [
            HttpConnectorError::Request("connect timed out".to_string()),
            HttpConnectorError::Status { status: 401 },
            HttpConnectorError::Auth("required token absent".to_string()),
            HttpConnectorError::InvalidHeader("name contains illegal byte".to_string()),
            HttpConnectorError::Backend("unknown method".to_string()),
        ];
        for err in &variants {
            let rendered = format!("{err}");
            for forbidden in ["Bearer", "Authorization", "app_key", "https://", "http://"] {
                assert!(
                    !rendered.contains(forbidden),
                    "HttpConnectorError Display must not echo {forbidden:?}; got {rendered:?}"
                );
            }
        }
    }

    /// display_no_secret: the named `verify` automated check — Status{401}
    /// renders the code but never a credential token.
    #[test]
    fn display_no_secret_status_shows_code() {
        let rendered = HttpConnectorError::Status { status: 401 }.to_string();
        assert!(
            rendered.contains("401"),
            "status code must be visible: {rendered:?}"
        );
        for forbidden in ["Bearer", "Authorization", "app_key", "https://"] {
            assert!(!rendered.contains(forbidden), "must not echo {forbidden:?}");
        }
    }

    #[test]
    fn connector_trait_object_is_send_sync_static() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<Box<dyn HttpConnector>>();
    }
}
