//! Backend dispatch: `[backend]` → `(Arc<dyn HttpConnector>, HttpCodeExecutor)`.
//!
//! This is the OpenAPI analog of the SQL binary's `[database] type` →
//! `Arc<dyn SqlConnector>` seam, with one structural difference: the OpenAPI
//! binary serves BOTH a single-call connector surface (`HttpConnector`, Plan 03)
//! AND a Code-Mode / script-tool execution surface (`HttpCodeExecutor`, Plan 04)
//! over the SAME backend. So [`dispatch`] returns the PAIR — a shared
//! `reqwest::Client` + auth provider are built ONCE and threaded into both.
//!
//! # Lazy startup (CF-2)
//!
//! Construction is offline-safe: the `reqwest::Client` is built without
//! contacting the backend, the auth provider is constructed statically, and the
//! `HttpClient` only parses the `base_url`. No spec read, no backend request,
//! and no network call is made at dispatch time — the backend is contacted only
//! on the first tool invocation.
//!
//! # `oauth_passthrough` runtime forwarding (Plan 90-10 / OAPI-03 / OAPI-05)
//!
//! For an `oauth_passthrough` backend, dispatch installs an
//! `OAuthPassthroughAuth` provider via
//! [`create_passthrough_auth_provider`](pmcp_server_toolkit::http::auth::create_passthrough_auth_provider)
//! holding NO construction-time token — the per-request inbound MCP token is
//! threaded in via `apply`'s `inbound_token` from the toolkit handler seam
//! (`request_executor_from_extra`). This is what makes the captured token
//! actually reach `target_header` at runtime; the previous `create_auth_provider`
//! installed a `MissingTokenAuth`/`NoAuth` provider that never forwarded the
//! token. Non-passthrough configs are unaffected:
//! `create_passthrough_auth_provider` delegates to `create_auth_provider` for
//! every other `AuthConfig` variant.
//!
//! # Credential safety (V7 / Pitfall 5 / T-90-06-01)
//!
//! [`DispatchError`]'s `Display` NEVER echoes the backend `base_url`, connection
//! URLs, or any credential substring from the config — it names the backend /
//! field only. The wrapped [`HttpConnectorError`] is already credential-redacted
//! at its source (the toolkit's auth/client constructors strip secrets before
//! constructing it).

use std::sync::Arc;

use pmcp_server_toolkit::code_mode::HttpCodeExecutor;
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::http::auth::create_passthrough_auth_provider;
use pmcp_server_toolkit::http::{HttpClient, HttpConnector, HttpConnectorError};

/// Error returned when [`dispatch`] cannot produce the connector/executor pair.
///
/// # Security
///
/// `Display` names the backend / field ONLY. It MUST NOT echo the backend
/// `base_url`, any connection URL, or any credential substring from the config
/// (V7 / Pitfall 5 / T-90-06-01). The wrapped [`HttpConnectorError`] is redacted
/// at the toolkit source.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DispatchError {
    /// The config declares no `[backend]` section, so there is no REST API to
    /// dispatch to. Names no value.
    #[error("[backend] section is required (declare base_url + optional [backend.auth])")]
    MissingBackend,

    /// Constructing the outgoing auth provider from `[backend.auth]` failed.
    /// The wrapped error is credential-redacted at the toolkit source.
    #[error("backend auth provider construction failed: {0}")]
    Auth(#[source] HttpConnectorError),

    /// Constructing the single-call [`HttpConnector`] failed (e.g. an
    /// unparseable `base_url`). The wrapped error does NOT echo the URL.
    #[error("backend connector construction failed: {0}")]
    Connector(#[source] HttpConnectorError),

    /// `[backend].base_url` holds a `${VAR}` / `env:VAR` reference that could
    /// not be resolved from the process environment (the variable is unset, or
    /// set to an empty / whitespace-only value).
    ///
    /// Without this the literal `${...}` would be handed to the connector and
    /// every outgoing request would target a nonsense URL — `validate()` cannot
    /// catch it, because the placeholder string is non-empty and passes the
    /// parse-time emptiness rule (T-120-16).
    ///
    /// # Security (T-120-17)
    ///
    /// `Display` names the FIELD and the environment-variable NAME only. It
    /// does NOT echo a resolved URL or any credential substring — the wrapped
    /// toolkit error carries the variable name and nothing else.
    #[error("backend base_url reference could not be resolved: {0}")]
    UnresolvedBaseUrl(#[source] pmcp_server_toolkit::ToolkitError),
}

/// Select and construct the `(HttpConnector, HttpCodeExecutor)` pair for the
/// configured `[backend]`.
///
/// Reads `cfg.backend` (error [`DispatchError::MissingBackend`] when absent),
/// builds the outgoing auth provider via
/// [`create_passthrough_auth_provider`](pmcp_server_toolkit::http::auth::create_passthrough_auth_provider)
/// (an `oauth_passthrough` backend installs an `OAuthPassthroughAuth` provider
/// holding NO construction-time token — its per-request token is threaded by the
/// toolkit handler seam, Plan 90-10; every other config delegates to
/// `create_auth_provider`), builds a shared `reqwest::Client` (lazy — no
/// network, CF-2), and constructs
/// both an [`HttpClient`] (single-call connector, Plan 03) and an
/// [`HttpCodeExecutor`] (Code-Mode / script-tool execution surface, Plan 04)
/// over the SAME client + base_url + auth. Returns the pair.
///
/// Construction is offline-safe: no spec read, no backend request, and no
/// network call is made here — the backend is contacted only on the first tool
/// invocation (CF-2).
///
/// # Errors
///
/// - [`DispatchError::MissingBackend`] when `[backend]` is absent.
/// - [`DispatchError::Auth`] when the auth provider cannot be built.
/// - [`DispatchError::UnresolvedBaseUrl`] when `base_url` holds a `${VAR}` /
///   `env:VAR` reference whose environment variable is unset or empty.
/// - [`DispatchError::Connector`] when the single-call connector cannot be built
///   (e.g. an unparseable `base_url`).
pub async fn dispatch(
    cfg: &ServerConfig,
) -> Result<(Arc<dyn HttpConnector>, HttpCodeExecutor), DispatchError> {
    let backend = cfg.backend.as_ref().ok_or(DispatchError::MissingBackend)?;

    // Auth construction (Plan 90-10 / H1): for an `oauth_passthrough` backend
    // this installs an `OAuthPassthroughAuth` provider holding NO token here; the
    // per-request inbound MCP token is threaded in by the toolkit handler seam
    // (`request_executor_from_extra` → `HttpCodeExecutor::with_inbound_token` →
    // `apply`'s `inbound_token`) so it actually reaches `target_header`. Every
    // non-passthrough config delegates to `create_auth_provider` (unchanged).
    let auth =
        create_passthrough_auth_provider(&backend.auth, None).map_err(DispatchError::Auth)?;

    // Lazy (CF-2): the reqwest client is built without contacting the backend.
    // Shared by BOTH the single-call connector and the Code-Mode executor so a
    // single connection pool serves the whole binary.
    let client = reqwest::Client::new();

    // Endpoint resolution (Phase 120 / PKG-03): `base_url` may be a `${VAR}` /
    // `env:VAR` reference the target environment fills, so resolve it ONCE here
    // and thread the RESOLVED value into both constructors. Reading
    // `backend.base_url` directly would send the literal `${...}` to the wire.
    // Still lazy — this is an environment lookup, not a network call (CF-2).
    let base_url = backend
        .resolved_base_url()
        .map_err(DispatchError::UnresolvedBaseUrl)?;

    // Single-call connector (Plan 03). LAZY: parses base_url, no network.
    let connector = HttpClient::new(client.clone(), base_url.clone(), auth.clone())
        .map_err(DispatchError::Connector)?;
    let connector: Arc<dyn HttpConnector> = Arc::new(connector);

    // Code-Mode / script-tool execution surface (Plan 04). The SAME client +
    // base_url + auth — D-02 (one engine feeds tools + code-mode).
    let http_exec = HttpCodeExecutor::new(client, base_url, auth);

    Ok((connector, http_exec))
}

#[cfg(test)]
mod tests {
    use super::{dispatch, DispatchError};
    use pmcp_server_toolkit::config::ServerConfig;

    /// RAII guard: set (or remove) an env var, restore the prior value on drop
    /// — including when an assertion panics mid-test. A trailing `remove_var`
    /// line is skipped on panic and leaks the variable into every later test
    /// in the binary; the toolkit's `tests/support::EnvVarGuard` exists for the
    /// same reason (this crate's unit tests cannot reach that module, so the
    /// minimal twin lives here). Each test uses a UNIQUE variable name, which
    /// is what stands in for a lock.
    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(old) => std::env::set_var(self.key, old),
                None => std::env::remove_var(self.key),
            }
        }
    }

    /// A london-tube-shaped config with a `[backend]` block (base_url + no auth).
    fn cfg_with_backend() -> ServerConfig {
        let toml = r#"
[server]
name = "tube"
version = "0.1.0"

[backend]
base_url = "https://api.tfl.gov.uk"

[[tools]]
name = "get_line_status"
description = "Status for a tube line"
path = "/Line/{id}/Status"
method = "GET"

[[tools.parameters]]
name = "id"
type = "string"
required = true
"#;
        ServerConfig::from_toml_strict_validated(toml).expect("parse")
    }

    #[tokio::test]
    async fn dispatch_builds_pair_offline_without_network() {
        // CF-2: dispatch must build the connector+executor pair with NO network
        // call (no spec read, no backend request). The wiremock-free, fast
        // resolution here is the offline proof — a real backend contact would
        // hang/fail.
        let cfg = cfg_with_backend();
        let result = dispatch(&cfg).await;
        assert!(
            result.is_ok(),
            "dispatch must build the pair offline (CF-2): {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn dispatch_missing_backend_is_an_error() {
        let toml = r#"
[server]
name = "t"
version = "0.1.0"
"#;
        let cfg = ServerConfig::from_toml_strict_validated(toml).expect("parse");
        let err = dispatch(&cfg)
            .await
            .err()
            .expect("missing backend must error");
        assert!(
            matches!(err, DispatchError::MissingBackend),
            "absent [backend] yields MissingBackend, got {err:?}"
        );
    }

    /// PKG-03 / T-120-16: a `${VAR}` endpoint whose variable is UNSET must be a
    /// dispatch ERROR, never a server that boots pointed at the literal
    /// `${...}`. `validate()` cannot catch this — the placeholder is non-empty.
    #[tokio::test]
    async fn dispatch_unresolved_base_url_reference_is_an_error() {
        // A uniquely-named variable so this test cannot collide with a sibling.
        const VAR: &str = "PMCP_OPENAPI_DISPATCH_UNSET_BASE_URL_TEST";
        let _guard = EnvVarGuard::unset(VAR);

        let toml = format!(
            r#"
[server]
name = "tube"
version = "0.1.0"

[backend]
base_url = "${{{VAR}}}"
"#
        );
        let cfg = ServerConfig::from_toml_strict_validated(&toml)
            .expect("a ${{VAR}} base_url parses and VALIDATES — it is non-empty");
        let err = dispatch(&cfg)
            .await
            .err()
            .expect("an unset ${VAR} base_url must fail dispatch");
        assert!(
            matches!(err, DispatchError::UnresolvedBaseUrl(_)),
            "unset endpoint reference yields UnresolvedBaseUrl, got {err:?}"
        );
        // The rendered error names the variable so an operator can act on it.
        assert!(
            err.to_string().contains(VAR),
            "the error must name the environment variable: {err}"
        );
    }

    /// PKG-03: a RESOLVED `${VAR}` endpoint dispatches normally — the resolved
    /// value, not the placeholder, reaches the connector.
    #[tokio::test]
    async fn dispatch_resolves_a_set_base_url_reference() {
        const VAR: &str = "PMCP_OPENAPI_DISPATCH_SET_BASE_URL_TEST";
        // Guard, not a bare set_var: restoration must survive a failing assert,
        // or the variable leaks into every later test in this binary.
        let _guard = EnvVarGuard::set(VAR, "http://127.0.0.1:9999");

        let toml = format!(
            r#"
[server]
name = "tube"
version = "0.1.0"

[backend]
base_url = "${{{VAR}}}"
"#
        );
        let cfg = ServerConfig::from_toml_strict_validated(&toml).expect("parse");
        let result = dispatch(&cfg).await;
        assert!(
            result.is_ok(),
            "a resolvable ${{VAR}} endpoint dispatches: {:?}",
            result.err()
        );
        // The guard restores VAR on drop — panic-safe, unlike a trailing remove_var.
    }

    #[test]
    fn dispatch_error_display_redacts_backend_and_secrets() {
        // Pitfall 5 / T-90-06-01: no DispatchError Display may echo the backend
        // base_url, a connection URL, or any credential substring. We assert the
        // backend base_url itself is absent (Codex LOW).
        let base_url = "https://api.tfl.gov.uk";
        let secret = "super-secret-token";
        let errors = [
            DispatchError::MissingBackend,
            DispatchError::Auth(pmcp_server_toolkit::http::HttpConnectorError::Backend(
                "invalid base URL".to_string(),
            )),
            DispatchError::Connector(pmcp_server_toolkit::http::HttpConnectorError::Backend(
                "invalid base URL".to_string(),
            )),
            // T-120-17: the new endpoint-resolution variant is held to the SAME
            // rule — it may name the env var and the field, never the URL.
            DispatchError::UnresolvedBaseUrl(
                pmcp_server_toolkit::ToolkitError::UnresolvedBaseUrlRef {
                    var: "TFL_BASE_URL".to_string(),
                },
            ),
        ];
        for err in &errors {
            let rendered = format!("{err}");
            assert!(
                !rendered.contains(base_url),
                "DispatchError Display leaked the backend base_url: {rendered}"
            );
            assert!(
                !rendered.contains(secret),
                "DispatchError Display leaked a credential: {rendered}"
            );
        }
    }
}
