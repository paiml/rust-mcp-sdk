//! reqwest-backed [`HttpConnector`] implementation (OAPI-01).
//!
//! Lifts the pmcp-run reference `HttpClient::execute_with_options` body into a
//! toolkit-owned [`HttpClient`] that implements [`HttpConnector`]. The concrete
//! shape mirrors `crate::sql::sqlite::SqliteConnector` (a concrete connector impl
//! + constructor). Construction is LAZY — `new` parses the base URL but contacts
//! no backend (CF-2). URL building uses the shared [`crate::http::join_url`]
//! helper so an API-Gateway stage prefix (`/v1`) survives (Pitfall 2 — explicit
//! path concatenation, never the RFC-3986 url-crate path merge). Error messages
//! never echo the URL or a credential (Pitfall 5).

use super::auth::HttpAuthProvider;
use super::{join_url, HttpConnector, HttpConnectorError, Operation, Parameter, ParameterLocation};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// HTTP client configuration (OWNED here in `http`, mirroring [`super::AuthConfig`]
/// ownership so Plan 02 re-exports it rather than redefining).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConfig {
    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// Number of retry attempts on 5xx / connect / timeout.
    #[serde(default = "default_retries")]
    pub retries: u32,
    /// Base backoff in milliseconds (exponential per attempt).
    #[serde(default = "default_retry_backoff")]
    pub retry_backoff_ms: u64,
    /// `User-Agent` header for all requests.
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    /// Extra headers applied to every request.
    #[serde(default)]
    pub default_headers: HashMap<String, String>,
}

fn default_timeout() -> u64 {
    30
}
fn default_retries() -> u32 {
    3
}
fn default_retry_backoff() -> u64 {
    1000
}
fn default_user_agent() -> String {
    format!("pmcp-server-toolkit/{}", env!("CARGO_PKG_VERSION"))
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_timeout(),
            retries: default_retries(),
            retry_backoff_ms: default_retry_backoff(),
            user_agent: default_user_agent(),
            default_headers: HashMap::new(),
        }
    }
}

/// reqwest-backed [`HttpConnector`].
pub struct HttpClient {
    client: reqwest::Client,
    base_url: url::Url,
    auth: Arc<dyn HttpAuthProvider>,
    http_config: HttpConfig,
    /// The E1 outbound-request policy, when one was registered (Phase 128).
    ///
    /// `None` on every client built by [`HttpClient::new`], which is what keeps
    /// that constructor's signature unchanged and what keeps a server that
    /// registers no policy allocating nothing extra on the request path.
    policy: Option<Arc<dyn crate::policy::RequestPolicy>>,
}

impl HttpClient {
    /// Construct a client. LAZY: parses `base_url` but contacts no backend (CF-2).
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] when `base_url` is unparseable or
    /// the reqwest client cannot be built. The error message does NOT echo the URL.
    pub fn new(
        client: reqwest::Client,
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
    ) -> Result<Self, HttpConnectorError> {
        Self::with_config(client, base_url, auth, HttpConfig::default())
    }

    /// Construct a client with an explicit [`HttpConfig`]. LAZY (CF-2).
    ///
    /// # Errors
    ///
    /// As [`HttpClient::new`].
    pub fn with_config(
        client: reqwest::Client,
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
        http_config: HttpConfig,
    ) -> Result<Self, HttpConnectorError> {
        let base_url = url::Url::parse(&base_url)
            .map_err(|_| HttpConnectorError::Backend("invalid base URL".to_string()))?;
        Ok(Self {
            client,
            base_url,
            auth,
            http_config,
            policy: None,
        })
    }

    /// Attach the E1 [`crate::policy::RequestPolicy`] consulted before every
    /// outbound request (Phase 128).
    ///
    /// Cheap clone-with-builder, the same shape as
    /// `HttpCodeExecutor::with_inbound_token`. Neither [`HttpClient::new`] nor
    /// [`HttpClient::with_config`] changed signature.
    #[must_use]
    pub fn with_request_policy(mut self, policy: Arc<dyn crate::policy::RequestPolicy>) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Build a client from an [`HttpConfig`], constructing the reqwest client with
    /// the configured timeout, user-agent, and default headers. LAZY (CF-2).
    ///
    /// # Errors
    ///
    /// As [`HttpClient::new`].
    pub fn from_config(
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
        http_config: HttpConfig,
    ) -> Result<Self, HttpConnectorError> {
        let mut headers = HeaderMap::new();
        if let Ok(ua) = HeaderValue::from_str(&http_config.user_agent) {
            headers.insert(reqwest::header::USER_AGENT, ua);
        }
        for (key, value) in &http_config.default_headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::try_from(key.as_str()),
                HeaderValue::try_from(value.as_str()),
            ) {
                headers.insert(name, val);
            }
        }
        // T-128-39a (Phase 128 security audit, W3 / review WR-02). reqwest's DEFAULT
        // is to follow up to 10 redirects, which would carry a governed request to an
        // endpoint `RequestPolicy` already refused — the exact bypass
        // `pmcp-openapi-server`'s `dispatch.rs` hardens against. This constructor is
        // `pub`, so a downstream consumer reaching for the one that honours
        // `[backend.http]` would otherwise get the un-hardened client. Keep these two
        // in the same shape; a redirect must be an explicit, policy-checked new request.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(http_config.timeout_seconds))
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .build()
            .map_err(|_| HttpConnectorError::Backend("failed to build HTTP client".to_string()))?;
        Self::with_config(client, base_url, auth, http_config)
    }

    /// Substitute path parameters into the operation path template.
    ///
    /// # Phase 128 D4 — the curated surface's path-injection floor
    ///
    /// Two passes, in this order, and the order is load-bearing:
    ///
    /// 1. every path parameter's value is rendered and then checked by
    ///    `pmcp::server::schema_validation::validate_path_placeholder` against that
    ///    parameter's declared rules (`Parameter::placeholder_rules`) — with
    ///    NOTHING substituted yet, so a refusal on the second of two placeholders
    ///    cannot leave a half-substituted path in existence anywhere;
    /// 2. only once every value has passed are the replacements applied, and the
    ///    COMPOSED result is then checked by
    ///    `pmcp::server::schema_validation::validate_resolved_path` before the
    ///    caller can dispatch it.
    ///
    /// Step 2 is not redundant with step 1. A composition belongs to no single
    /// value, so only the composed check can see a segment that a template literal
    /// and a passing value jointly push over the cap, traversal written into the
    /// template itself, or a residual `{`/`}` left by a placeholder with no
    /// supplied argument.
    ///
    /// ## The one narrowing: an operator-written `?` is permitted
    ///
    /// `validate_resolved_path` refuses a query separator anywhere, and core keeps
    /// that strict rule for its other callers. This function splits the composed
    /// path at the FIRST `?` and applies the full, unmodified rule set to each
    /// side, which exempts exactly that one separator and nothing else. It is safe
    /// rather than a hole because step 1 already refuses `?` inside a substituted
    /// value, in literal AND percent-encoded form with a decode-once pass — so a
    /// `?` surviving into the composed string can only have come from the
    /// `[[tools]]` `path` an operator authored, not from caller data. Refusing `..`
    /// from that template catches a traversal bug; refusing `?` from it rejects
    /// legitimate configuration. Same rule, different work. A SECOND `?`, a
    /// dangling `?` with an empty query, a `#` fragment marker and anything else
    /// stay refused, because the query portion faces the same rule.
    ///
    /// ## What counts as a placeholder here
    ///
    /// Substitution is textual, so a spec-derived operation's mid-segment
    /// placeholder (`.../range(address='{address}')`) is substituted. A curated
    /// `[[tools]]` `path`, by contrast, only ever reaches this function in the
    /// whole-segment shape: a segment containing anything other than exactly one
    /// `{name}` spanning the whole segment is refused at CONFIG time by
    /// `crate::config::ServerConfig::validate`, because such a segment produces
    /// either a parameter name no declaration can match or literal braces on the
    /// wire.
    ///
    /// ## Without the `input-validation` feature
    ///
    /// Both checks are `#[cfg(feature = "input-validation")]`-gated and this
    /// function behaves exactly as it did before Phase 128. `input-validation` is
    /// in the toolkit's `default` feature set, so an unenforced build is an
    /// explicit opt-out rather than an accident.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`]:
    ///
    /// - via [`render_scalar`] when a path parameter value is a non-scalar
    ///   (`Object`/`Array`) — such a value would otherwise be JSON-stringified
    ///   into the URL (WR-03);
    /// - when `validate_path_placeholder` refuses a rendered value (the character
    ///   floor, the always-on length cap, or the declared `pattern`/`max_length`
    ///   narrowing on top of them);
    /// - when a declared path parameter has no supplied argument, naming that
    ///   parameter and nothing else;
    /// - when `validate_resolved_path` refuses the composed path on either side of
    ///   an operator-written `?`.
    ///
    /// Every one of these messages names the declared parameter or the rule and
    /// carries no byte of the rejected value and no fragment of the resolved path
    /// (Pitfall 5, as [`render_scalar`] states it).
    fn substitute_path(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<String, HttpConnectorError> {
        // Pass 1 — render and CHECK every contribution. Nothing is applied yet.
        let mut rendered: Vec<(String, String)> = Vec::new();
        for param in operation.path_parameters() {
            let Some(value) = args.get(&param.name) else {
                refuse_missing_path_argument(&param.name)?;
                continue;
            };
            let value_str = render_scalar(&param.name, value)?;
            check_placeholder_value(param, &value_str)?;
            rendered.push((format!("{{{}}}", param.name), value_str));
        }
        // Pass 2 — apply, then check the COMPOSED result.
        let mut path = operation.path.clone();
        for (placeholder, value_str) in &rendered {
            path = path.replace(placeholder, value_str);
        }
        check_composed_path(&path)?;
        Ok(path)
    }

    /// Render one query value: a scalar passes through; an array-of-scalars is
    /// comma-joined (OpenAPI `form`/`explode:false` style); an object or an array
    /// with any non-scalar member is rejected (each member is checked through
    /// [`render_scalar`]).
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] naming `param_name` when `value`
    /// (or any array member) is a non-scalar.
    fn render_query_value(
        param_name: &str,
        value: &serde_json::Value,
    ) -> Result<String, HttpConnectorError> {
        if let serde_json::Value::Array(arr) = value {
            // Comma-separate array members (OpenAPI `form`/`simple` style). A
            // nested non-scalar member is rejected by render_scalar.
            let mut csv = String::new();
            for (i, member) in arr.iter().enumerate() {
                if i > 0 {
                    csv.push(',');
                }
                csv.push_str(&render_scalar(param_name, member)?);
            }
            Ok(csv)
        } else {
            render_scalar(param_name, value)
        }
    }

    /// Build the query map from query-located params present in `args`.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] naming the offending parameter when
    /// a query value is an object, or an array containing a non-scalar member
    /// (WR-03). A scalar or an array-of-scalars behaves exactly as before.
    fn build_query(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<HashMap<String, String>, HttpConnectorError> {
        let mut query = HashMap::new();
        for param in operation.query_parameters() {
            if let Some(value) = args.get(&param.name) {
                query.insert(
                    param.name.clone(),
                    Self::render_query_value(&param.name, value)?,
                );
            }
        }
        Ok(query)
    }

    /// Build the header map from header-located params present in `args`.
    fn build_headers(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<HeaderMap, HttpConnectorError> {
        let mut headers = HeaderMap::new();
        for param in operation.header_parameters() {
            if let Some(value) = args.get(&param.name) {
                let name = HeaderName::try_from(param.name.as_str()).map_err(|_| {
                    HttpConnectorError::InvalidHeader("invalid header name".to_string())
                })?;
                // Reject a non-scalar header value (naming the param) before it
                // can be JSON-stringified into the header (WR-03).
                let rendered = render_scalar(&param.name, value)?;
                let val = HeaderValue::try_from(rendered).map_err(|_| {
                    HttpConnectorError::InvalidHeader("invalid header value".to_string())
                })?;
                headers.insert(name, val);
            }
        }
        Ok(headers)
    }

    /// Collect the request body: every arg that is not routed somewhere else.
    ///
    /// An arg belongs in the payload when it is either a declared
    /// [`ParameterLocation::Body`] parameter — which is what `build_operation`
    /// assigns to a `POST`/`PUT`/`PATCH` tool's non-path parameters — or an
    /// UNDECLARED key, which only reaches here when the tool's schema was built
    /// with `[server.validation] additional_properties = true`. A `Path`, `Query`
    /// or `Header`-located parameter is withheld, because it already travels in the
    /// URL or the headers.
    ///
    /// # Phase 128 CR-02
    ///
    /// The `Body`-located half is the fix. Before it, `build_operation` marked every
    /// non-path declared parameter `Query`, so the `declared` exclusion below
    /// withheld ALL of them and the only route to a payload was an undeclared key —
    /// which `additionalProperties: false`, enforced for the first time in this
    /// release, refuses. A curated mutating tool could accept a payload and send
    /// none of it.
    ///
    /// # The reserved `body` key (WR-10)
    ///
    /// `"body"` is a reserved argument name: when present it becomes the ENTIRE
    /// payload verbatim rather than one field of it. It is no longer also appended
    /// to the query string, because on a body-bearing method a declared parameter
    /// named `body` is now `Body`-located and `build_query` only reads
    /// `Query`-located ones.
    fn build_body(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<serde_json::Value> {
        if !operation.has_request_body {
            return None;
        }
        if let Some(body) = args.get("body") {
            return Some(body.clone());
        }
        let routed_elsewhere: std::collections::HashSet<&str> = operation
            .parameters
            .iter()
            .filter(|p| p.location != ParameterLocation::Body)
            .map(|p| p.name.as_str())
            .collect();
        let body: serde_json::Map<String, serde_json::Value> = args
            .iter()
            .filter(|(k, _)| !routed_elsewhere.contains(k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if body.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(body))
        }
    }

    fn convert_method(method: &str) -> Result<reqwest::Method, HttpConnectorError> {
        match method.to_uppercase().as_str() {
            "GET" => Ok(reqwest::Method::GET),
            "POST" => Ok(reqwest::Method::POST),
            "PUT" => Ok(reqwest::Method::PUT),
            "PATCH" => Ok(reqwest::Method::PATCH),
            "DELETE" => Ok(reqwest::Method::DELETE),
            "HEAD" => Ok(reqwest::Method::HEAD),
            "OPTIONS" => Ok(reqwest::Method::OPTIONS),
            _ => Err(HttpConnectorError::Backend(
                "unknown HTTP method".to_string(),
            )),
        }
    }

    /// Send the request, retrying on 5xx / connect / timeout with exponential backoff.
    async fn send_with_retries(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, HttpConnectorError> {
        let max_retries = self.http_config.retries;
        let mut last_status: Option<u16> = None;
        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = self.http_config.retry_backoff_ms * (1u64 << (attempt - 1));
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            let Some(attempt_request) = request.try_clone() else {
                return Err(HttpConnectorError::Request(
                    "request body is not retryable".to_string(),
                ));
            };
            match attempt_request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_server_error() && attempt < max_retries {
                        last_status = Some(status.as_u16());
                        continue;
                    }
                    return Ok(response);
                },
                Err(e) => {
                    let retryable = e.is_connect() || e.is_timeout();
                    if retryable && attempt < max_retries {
                        continue;
                    }
                    // Redacted: never forward the reqwest error Display (echoes URL).
                    return Err(HttpConnectorError::Request(
                        "transport error contacting backend".to_string(),
                    ));
                },
            }
        }
        Err(HttpConnectorError::Status {
            status: last_status.unwrap_or(0),
        })
    }
}

/// Render a JSON scalar for use in a path / query / header position, REJECTING
/// non-scalar values (WR-03 / GAP 4).
///
/// # The decided rule (uniform)
///
/// The `http::schema::Parameter` model carries NO OpenAPI `style` / `explode` /
/// `type` hint, so there is no per-parameter serialization directive to honor;
/// the rule must therefore be uniform across every path / query / header
/// position:
///
/// - A scalar (`String`, `Number`, `Bool`, `Null`) renders to a bare string
///   (`Null` → `"null"`, matching the `code_mode::HttpCodeExecutor::scalar_str`
///   counterpart so the two HTTP surfaces stay consistent).
/// - A query parameter that is an **array of scalars** is comma-joined by the
///   caller ([`build_query`]); each member is rendered through this function so a
///   nested non-scalar member is rejected.
/// - An `Object`, an array containing any non-scalar member, or ANY non-scalar
///   in path / header position is **rejected** with a typed error that names the
///   parameter — it is NEVER JSON-stringified into the URL/header (which would
///   leak literal `{`/`[`/`"` that then percent-encode into a silently-wrong
///   request).
///
/// # Errors
///
/// Returns [`HttpConnectorError::Backend`] naming `param_name` when `value` is a
/// non-scalar (`Object` or `Array`). Per the module's redaction discipline
/// (Pitfall 5) the message names the PARAMETER ONLY — never the value.
fn render_scalar(
    param_name: &str,
    value: &serde_json::Value,
) -> Result<String, HttpConnectorError> {
    match value {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        serde_json::Value::Null => Ok("null".to_string()),
        // Object OR Array: non-scalar in a path/query/header position is rejected
        // rather than silently JSON-stringified. Name the param ONLY (Pitfall 5).
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            Err(HttpConnectorError::Backend(format!(
                "param '{param_name}' must be a scalar (non-scalar values are \
                 not supported in path/query/header position)"
            )))
        },
    }
}

// -----------------------------------------------------------------------------
// Phase 128 D4 — the three checks `substitute_path` calls, each as a `cfg` PAIR.
//
// A written `cfg(not(...))` half rather than `#[cfg]` inside the function body:
// the enforced and unenforced shapes are then both visible at a glance, and
// `substitute_path` reads as one flow in either build. There is NO second copy of
// any rule here — each enforced half calls the ONE core implementation in
// `pmcp::server::schema_validation`, which is what keeps the curated surface and
// the Code Mode surface on a single denylist (Q2).
// -----------------------------------------------------------------------------

/// Map a core placeholder refusal into this connector's error type.
///
/// `PlaceholderRefusal`'s own `Display` is value-free — it names the declared
/// parameter and the declared expectation — so forwarding it verbatim keeps the two
/// HTTP surfaces indistinguishable to a caller: they differ only in error TYPE
/// (`HttpConnectorError::Backend` here, `ExecutionError::RuntimeError` on the Code
/// Mode side), never in wording.
#[cfg(feature = "input-validation")]
fn refusal_to_backend_error(
    refusal: &pmcp::server::schema_validation::PlaceholderRefusal,
) -> HttpConnectorError {
    HttpConnectorError::Backend(format!("{refusal}"))
}

/// Check ONE rendered path-parameter value against the D4 floor, the always-on
/// cap, and this parameter's declared narrowing.
///
/// # Errors
///
/// [`HttpConnectorError::Backend`] carrying the core refusal, which names the
/// parameter and never the value.
#[cfg(feature = "input-validation")]
fn check_placeholder_value(param: &Parameter, value_str: &str) -> Result<(), HttpConnectorError> {
    pmcp::server::schema_validation::validate_path_placeholder(
        &param.name,
        value_str,
        &param.placeholder_rules(),
    )
    .map_err(|refusal| refusal_to_backend_error(&refusal))
}

/// The `input-validation`-off half: the pre-Phase-128 behaviour, which applied no
/// character check to a path-parameter value at all.
#[cfg(not(feature = "input-validation"))]
fn check_placeholder_value(_param: &Parameter, _value_str: &str) -> Result<(), HttpConnectorError> {
    Ok(())
}

/// Check the COMPOSED path, exempting one operator-written `?`.
///
/// The split lives HERE and deliberately not in core: `validate_resolved_path` is a
/// general-purpose composed-path checker with other callers that want the strict
/// `?`-anywhere rule, and relaxing it there would weaken all of them. Both sides of
/// the first `?` face the full, unmodified rule set, so a second `?`, a fragment
/// marker, traversal on either side, an over-cap query and an empty query portion
/// all stay refused for free. See `HttpClient::substitute_path` for why exempting
/// exactly that one byte is safe.
///
/// # Errors
///
/// [`HttpConnectorError::Backend`] carrying the core refusal, which names the rule
/// and never any byte of the path.
#[cfg(feature = "input-validation")]
fn check_composed_path(path: &str) -> Result<(), HttpConnectorError> {
    let checked = match path.split_once('?') {
        // No operator-written separator: the whole string is a path.
        None => pmcp::server::schema_validation::validate_resolved_path(path),
        // Exactly one operator-written `?`. The separator itself is permitted;
        // both sides still face the full, unmodified rule set.
        Some((path_part, query_part)) => {
            pmcp::server::schema_validation::validate_resolved_path(path_part)
                .and_then(|()| pmcp::server::schema_validation::validate_resolved_path(query_part))
        },
    };
    checked.map_err(|refusal| refusal_to_backend_error(&refusal))
}

/// The `input-validation`-off half: no composed check, so a residual `{name}` from
/// a path parameter with no supplied argument reaches the outbound URL exactly as
/// it did before Phase 128.
#[cfg(not(feature = "input-validation"))]
fn check_composed_path(_path: &str) -> Result<(), HttpConnectorError> {
    Ok(())
}

/// Refuse a declared path parameter that has no supplied argument.
///
/// Its own refusal rather than leaning on the composed check's residual-brace rule,
/// for one reason: this is the only route that can name the PARAMETER. The composed
/// refusal is param-agnostic by construction (`param: "path segment"`), because a
/// composition belongs to no single parameter — so it would tell a caller that
/// something in the path is wrong without saying which declaration to supply. This
/// is a presence check, not a second copy of the character denylist.
///
/// D1's `required` keyword does not make this unreachable: it refuses only when the
/// `[[tools.parameters]]` declaration says `required = true`, while
/// `tools.rs::build_operation` marks every template path parameter required
/// INDEPENDENTLY of any declaration — so the two can disagree, and measurably did.
///
/// # Errors
///
/// Always [`HttpConnectorError::Backend`], naming `param_name` and nothing else —
/// never the template and never the partially-substituted path.
#[cfg(feature = "input-validation")]
fn refuse_missing_path_argument(param_name: &str) -> Result<(), HttpConnectorError> {
    Err(HttpConnectorError::Backend(format!(
        "param '{param_name}' is a declared path parameter and must be supplied"
    )))
}

/// The `input-validation`-off half: the parameter is skipped, which is the
/// pre-Phase-128 behaviour (the literal placeholder text stays in the path).
#[cfg(not(feature = "input-validation"))]
fn refuse_missing_path_argument(_param_name: &str) -> Result<(), HttpConnectorError> {
    Ok(())
}

impl HttpClient {
    /// Consult the registered E1 policy, if any, for one already-assembled
    /// outbound request (Phase 128).
    ///
    /// Its own helper so the `execute` body keeps ONE added statement and stays
    /// well under the cognitive-complexity 25 gate. Returns `Ok(())` immediately
    /// when no policy is registered, which is the no-allocation empty case.
    ///
    /// # Errors
    ///
    /// [`HttpConnectorError::PolicyRefused`] carrying the policy's OWN message.
    async fn run_request_policy(
        &self,
        tool: &str,
        method: &str,
        path: &str,
        query: &[(String, String)],
        body: Option<&serde_json::Value>,
    ) -> Result<(), HttpConnectorError> {
        let Some(policy) = self.policy.as_ref() else {
            return Ok(());
        };
        let req = crate::policy::OutboundRequest::new(tool, method, path, query, body);
        policy
            .check(&req)
            .await
            .map_err(|refusal| HttpConnectorError::PolicyRefused(refusal.message().to_string()))
    }

    /// The shared `execute` body, carrying the MCP tool name (Phase 128 E1).
    ///
    /// [`HttpConnector::execute`] passes `""` (no tool to name) and
    /// [`HttpConnector::execute_for_tool`] passes the synthesized tool's own
    /// name, so there is ONE request path rather than two that can drift.
    async fn execute_inner(
        &self,
        tool: &str,
        operation: &Operation,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError> {
        let empty = serde_json::Map::new();
        let args_map = args.as_object().unwrap_or(&empty);

        // Build URL via the shared join_url helper (explicit concat, never the
        // url-crate RFC-3986 path merge) — preserves a stage prefix like /v1
        // (Pitfall 2 / T-90-01-05).
        let substituted = Self::substitute_path(operation, args_map)?;
        let joined = join_url(self.base_url.as_str(), &substituted);
        let mut url = url::Url::parse(&joined)
            .map_err(|_| HttpConnectorError::Backend("constructed URL is invalid".to_string()))?;

        let mut query = Self::build_query(operation, args_map)?;
        let mut headers = Self::build_headers(operation, args_map)?;
        let request_body = Self::build_body(operation, args_map);

        // Phase 128 E1 / D-12 — the outbound-policy hook, and its position is
        // load-bearing rather than incidental.
        //
        // It sits AFTER `join_url` because the policy must see the URL as it will
        // be sent (resolved placeholders, stage prefix applied) rather than the
        // `[[tools]]` template. It sits BEFORE `self.auth.apply` because that call
        // is the first moment a credential exists in `headers` / `query`, and
        // D-12's guarantee is that third-party policy code cannot observe one. A
        // refusal therefore returns before auth AND before the send: nothing is
        // authenticated and nothing leaves.
        //
        // `query` is snapshotted SORTED so a policy sees a deterministic order
        // (the source is a HashMap). It carries no auth pair for the reason above:
        // an API-key-in-query credential is contributed by the call below.
        let mut policy_query: Vec<(String, String)> =
            query.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        policy_query.sort();
        self.run_request_policy(
            tool,
            &operation.method.to_uppercase(),
            &joined,
            &policy_query,
            request_body.as_ref(),
        )
        .await?;
        drop(policy_query);

        // Single-call tools have no per-request passthrough token (Plan 04/06 carry
        // it through HttpCodeExecutor); pass None here.
        self.auth.apply(&mut headers, &mut query, None).await?;

        // Why: reqwest 0.13 gates `RequestBuilder::query` behind a `query` feature
        // (verified in reqwest-0.13.2 request.rs:`#[cfg(feature = "query")]`). The
        // toolkit deliberately does NOT enable that feature (Pitfall 4 / lean
        // build), so query params are appended to the URL via `url`'s built-in,
        // percent-encoding query-pair serializer instead.
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in &query {
                pairs.append_pair(key, value);
            }
            drop(pairs);
        }

        let method = Self::convert_method(&operation.method)?;
        let mut request = self.client.request(method, url);
        request = request.headers(headers);
        if let Some(body) = request_body {
            request = request.json(&body);
        }

        let response = self.send_with_retries(request).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(HttpConnectorError::Status {
                status: status.as_u16(),
            });
        }
        let body = response
            .text()
            .await
            .map_err(|_| HttpConnectorError::Request("failed to read response body".to_string()))?;
        if body.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&body).map_err(|_| {
            HttpConnectorError::Backend("response body was not valid JSON".to_string())
        })
    }

    /// A clone of this client carrying `policy`.
    ///
    /// Backs the [`HttpConnector::governed`] override — the route a policy takes
    /// to a connector that has ALREADY been erased to `Arc<dyn HttpConnector>` by
    /// the time the hooks value is in scope, which is exactly the situation
    /// `pmcp-openapi-server`'s `build_server` is in.
    fn cloned_with_policy(&self, policy: Arc<dyn crate::policy::RequestPolicy>) -> Self {
        Self {
            client: self.client.clone(),
            base_url: self.base_url.clone(),
            auth: Arc::clone(&self.auth),
            http_config: self.http_config.clone(),
            policy: Some(policy),
        }
    }
}

#[async_trait]
impl HttpConnector for HttpClient {
    async fn execute(
        &self,
        operation: &Operation,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError> {
        // No tool to name: a caller driving the connector directly rather than
        // through a synthesized handler.
        self.execute_inner("", operation, args).await
    }

    async fn execute_for_tool(
        &self,
        tool: &str,
        operation: &Operation,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError> {
        self.execute_inner(tool, operation, args).await
    }

    fn has_request_policy(&self) -> bool {
        self.policy.is_some()
    }

    fn governed(
        &self,
        policy: Arc<dyn crate::policy::RequestPolicy>,
    ) -> Option<Arc<dyn HttpConnector>> {
        Some(Arc::new(self.cloned_with_policy(policy)))
    }

    fn base_url(&self) -> &str {
        self.base_url.as_str()
    }
}

// -----------------------------------------------------------------------------
// Phase 128 D4 test support + the two sibling test modules.
//
// `mod placeholder_floor` and `mod query_separator` are SIBLINGS of `mod tests`
// at the `client` module level, not children of it. Both are still selected by
// this plan's `--lib http::client` verify filter (a module prefix), and the names
// mirror `pmcp-code-mode`'s `executor::query_separator` so the two surfaces'
// boundary suites read alike. Being children of `client` is what gives them
// access to the private `HttpClient::substitute_path`.
// -----------------------------------------------------------------------------

/// Fixtures shared by the two Phase 128 D4 test modules.
#[cfg(all(test, feature = "input-validation"))]
mod d4_support {
    use super::{HttpClient, HttpConnectorError, Operation};
    use crate::http::{Parameter, ParameterLocation};

    /// A `GET` operation on `path` carrying `parameters`.
    pub fn op(path: &str, parameters: Vec<Parameter>) -> Operation {
        Operation {
            method: "GET".to_string(),
            path: path.to_string(),
            parameters,
            has_request_body: false,
            base_url: None,
        }
    }

    /// A required path parameter with no declared narrowing (floor + cap only).
    pub fn path_param(name: &str) -> Parameter {
        Parameter::new(name, ParameterLocation::Path, true)
    }

    /// Substitute `pairs` into `path`, treating every named key as a path
    /// parameter with no declared narrowing.
    pub fn substitute(
        path: &str,
        pairs: &[(&str, serde_json::Value)],
    ) -> Result<String, HttpConnectorError> {
        let parameters = pairs.iter().map(|(k, _)| path_param(k)).collect();
        let mut args = serde_json::Map::new();
        for (k, v) in pairs {
            args.insert((*k).to_string(), v.clone());
        }
        HttpClient::substitute_path(&op(path, parameters), &args)
    }

    /// Substitute a single string `value` for `{name}` in `path`.
    pub fn substitute_one(
        path: &str,
        name: &str,
        value: &str,
    ) -> Result<String, HttpConnectorError> {
        substitute(
            path,
            &[(name, serde_json::Value::String(value.to_string()))],
        )
    }
}

/// The curated surface's D4 floor: every rendered placeholder value faces
/// `validate_path_placeholder` before ANY substitution is applied, and the
/// composed result faces `validate_resolved_path` before dispatch.
#[cfg(all(test, feature = "input-validation"))]
mod placeholder_floor {
    use super::d4_support::{op, path_param, substitute, substitute_one};
    use super::{HttpClient, HttpConnectorError};
    use crate::http::{Parameter, ParameterLocation};
    use pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH;

    /// Assert a refusal names the parameter and carries no byte of the value and
    /// no fragment of the resolved path.
    fn assert_value_free(err: &HttpConnectorError, param: &str, value: &str, path_fragment: &str) {
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
        let rendered = err.to_string();
        assert!(
            rendered.contains(param),
            "the refusal must name the declared parameter: {rendered}"
        );
        assert!(
            !rendered.contains(value),
            "the refusal must carry no byte of the value: {rendered}"
        );
        assert!(
            !rendered.contains(path_fragment),
            "the refusal must never contain the resolved path: {rendered}"
        );
    }

    /// CR-01 row: a query separator inside a placeholder value.
    #[test]
    fn placeholder_floor_refuses_a_query_separator_in_a_value() {
        let value = "current?string=x";
        let err = substitute_one("/content/{version}/CUI", "version", value).unwrap_err();
        assert_value_free(&err, "version", value, "/content/");
    }

    /// CR-01 row: parent-directory traversal inside a placeholder value.
    #[test]
    fn placeholder_floor_refuses_traversal_in_a_value() {
        let value = "current/../../search/current";
        let err = substitute_one("/content/{version}/CUI", "version", value).unwrap_err();
        assert_value_free(&err, "version", value, "/content/");
    }

    /// Percent-encoded traversal in UPPER-case hex — the decode-once pass is what
    /// has to catch it, not an enumerated denylist of spellings.
    #[test]
    fn placeholder_floor_refuses_upper_case_encoded_traversal() {
        let err = substitute_one("/content/{version}/CUI", "version", "a%2E%2Eb").unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }

    /// Adjacency edge: a value that is EXACTLY a denied character. The check is a
    /// character rule, not a substring-position heuristic.
    #[test]
    fn placeholder_floor_refuses_a_value_that_is_exactly_a_denied_character() {
        let err = substitute_one("/content/{version}/CUI", "version", "?").unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }

    /// Encoding edge: a literal NUL byte, and separately its percent-encoded form.
    #[test]
    fn placeholder_floor_refuses_a_nul_byte_in_both_forms() {
        assert!(substitute_one("/x/{v}", "v", "a\u{0}b").is_err());
        assert!(substitute_one("/x/{v}", "v", "a%00b").is_err());
    }

    /// An empty value is refused — it would otherwise compose an empty segment.
    #[test]
    fn placeholder_floor_refuses_an_empty_value() {
        assert!(substitute_one("/x/{v}", "v", "").is_err());
    }

    /// The always-on cap holds at exactly `PLACEHOLDER_MAX_LENGTH`.
    #[test]
    fn placeholder_floor_accepts_the_cap_and_refuses_one_more() {
        let at_cap = "a".repeat(PLACEHOLDER_MAX_LENGTH);
        assert_eq!(
            substitute_one("/x/{v}", "v", &at_cap).expect("at the cap"),
            format!("/x/{at_cap}")
        );
        let over_cap = "a".repeat(PLACEHOLDER_MAX_LENGTH + 1);
        assert!(substitute_one("/x/{v}", "v", &over_cap).is_err());
    }

    /// A conforming value matching its DECLARED pattern is accepted and the path
    /// is fully substituted.
    #[test]
    fn placeholder_floor_accepts_a_value_matching_its_declared_pattern() {
        let parameters = vec![
            Parameter::new("cui", ParameterLocation::Path, true).with_rules(
                Some("^C[0-9]+$".to_string()),
                Some(32),
                false,
            ),
        ];
        let mut args = serde_json::Map::new();
        args.insert("cui".to_string(), serde_json::json!("C0018787"));
        let resolved = HttpClient::substitute_path(&op("/CUI/{cui}/content", parameters), &args)
            .expect("a conforming value must be accepted");
        assert_eq!(resolved, "/CUI/C0018787/content");
    }

    /// D-10: the DECLARED pattern narrows on top of the floor.
    #[test]
    fn placeholder_floor_refuses_a_value_failing_its_declared_pattern() {
        let parameters = vec![
            Parameter::new("cui", ParameterLocation::Path, true).with_rules(
                Some("^C[0-9]+$".to_string()),
                None,
                false,
            ),
        ];
        let mut args = serde_json::Map::new();
        args.insert("cui".to_string(), serde_json::json!("notacui"));
        let err = HttpClient::substitute_path(&op("/CUI/{cui}", parameters), &args).unwrap_err();
        assert!(err.to_string().contains("cui"), "{err}");
        assert!(!err.to_string().contains("notacui"), "{err}");
    }

    /// Empty edge: a template with NO placeholders is returned unchanged and gains
    /// zero new refusals. The composed check still runs, and passes.
    #[test]
    fn placeholder_floor_leaves_a_placeholder_free_template_untouched() {
        let resolved = HttpClient::substitute_path(
            &op("/Line/Mode/tube/Status", vec![]),
            &serde_json::Map::new(),
        )
        .expect("a placeholder-free template must be unaffected");
        assert_eq!(resolved, "/Line/Mode/tube/Status");
    }

    /// Phase 128 CR-01 — a root operation is callable on the curated surface.
    ///
    /// `substitute_path` calls `check_composed_path` UNCONDITIONALLY, so before the
    /// core fix a spec declaring `paths: { "/": { get: … } }` — a health or index
    /// endpoint — failed every `tools/call` with `param 'path segment' must not be
    /// empty`. This is the caller-side row for the core exemption; the trailing-slash
    /// refusal it must not re-open is asserted immediately below.
    #[test]
    fn placeholder_floor_accepts_the_root_path_and_still_refuses_a_trailing_slash() {
        let resolved = HttpClient::substitute_path(&op("/", vec![]), &serde_json::Map::new())
            .expect("a `GET /` operation must be callable — the root is the shortest legal path");
        assert_eq!(resolved, "/");

        // An empty tail placeholder composes to a trailing `/`, which stays refused.
        let err = substitute_one("/search/{v}", "v", "")
            .expect_err("an empty tail placeholder must stay refused");
        assert!(
            matches!(err, HttpConnectorError::Backend(_)),
            "the refusal is a Backend error naming the position: {err}"
        );
        assert!(
            HttpClient::substitute_path(&op("/search/", vec![]), &serde_json::Map::new()).is_err(),
            "a literal trailing slash in the template stays refused by decision"
        );
    }

    /// A refusal on the SECOND of two placeholders aborts with no
    /// partially-substituted path in existence — the first value is rendered and
    /// checked but nothing is applied until every value has passed.
    #[test]
    fn placeholder_floor_refuses_the_second_of_two_placeholders_without_substituting() {
        let err = substitute(
            "/a/{first}/b/{second}",
            &[
                ("first", serde_json::json!("ok")),
                ("second", serde_json::json!("../escape")),
            ],
        )
        .unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains("second"), "{rendered}");
        assert!(
            !rendered.contains("/a/ok/b/"),
            "no partially-substituted path may appear anywhere: {rendered}"
        );
    }

    /// A path parameter ABSENT from `args` is refused, naming the parameter only —
    /// rather than leaving the literal `{name}` in the outbound URL.
    #[test]
    fn placeholder_floor_refuses_an_absent_path_argument() {
        let err = HttpClient::substitute_path(
            &op("/users/{id}/profile", vec![path_param("id")]),
            &serde_json::Map::new(),
        )
        .unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains("id"), "{rendered}");
        assert!(
            !rendered.contains('{') && !rendered.contains('}'),
            "the refusal must not echo the template: {rendered}"
        );
        assert!(
            !rendered.contains("/users/"),
            "the refusal must not echo the path: {rendered}"
        );
    }

    /// The COMPOSED check is the only mechanism that can see this: a template
    /// literal prefix plus a value that each pass on their own compose a segment
    /// over the cap. Spec-derived operations carry mid-segment placeholders, so
    /// this shape is reachable without any curated config.
    #[test]
    fn placeholder_floor_refuses_a_composed_segment_over_the_cap() {
        let prefix = "p".repeat(100);
        let value = "v".repeat(200);
        let err = substitute_one(&format!("/x/{prefix}{{id}}"), "id", &value).unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }

    /// The composed check also refuses a residual `{`/`}` arriving from a template
    /// the curated config parser would not recognize — the spec-derived route that
    /// config validation cannot reach.
    #[test]
    fn placeholder_floor_refuses_a_residual_brace_from_an_unrecognized_template() {
        let err = HttpClient::substitute_path(&op("/x/{a}/y/{b}", vec![path_param("a")]), &{
            let mut args = serde_json::Map::new();
            args.insert("a".to_string(), serde_json::json!("ok"));
            args
        })
        .unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }

    /// A template literal carrying traversal is refused by the composed check
    /// alone: no per-value check ever sees a literal, so this row isolates the
    /// composed mechanism.
    #[test]
    fn placeholder_floor_refuses_traversal_written_into_the_template_literal() {
        let err = HttpClient::substitute_path(&op("/a/../b", vec![]), &serde_json::Map::new())
            .unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }
}

/// The `?` narrowing inherited from plan 05, mirrored on the CURATED surface and
/// pinned in BOTH directions.
///
/// A `[[tools]]` `path` is operator-authored configuration, exactly as a Code Mode
/// script's literal path text is operator-authored script text — so the same
/// asymmetry applies: refusing `..` from it catches a traversal bug, while refusing
/// `?` from it rejects legitimate authoring. `substitute_path` therefore splits the
/// composed path at the FIRST `?` and applies the full, unmodified rule set to each
/// side. Nine of the twelve rows below assert what did NOT change, because a
/// narrowing pinned only by accept-rows is indistinguishable from a deleted check.
#[cfg(all(test, feature = "input-validation"))]
mod query_separator {
    use super::d4_support::substitute_one;
    use super::{HttpClient, Operation};
    use pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH;

    /// Substitute nothing — a placeholder-free template, checked as composed.
    fn literal(path: &str) -> Result<String, super::HttpConnectorError> {
        HttpClient::substitute_path(
            &Operation {
                method: "GET".to_string(),
                path: path.to_string(),
                parameters: vec![],
                has_request_body: false,
                base_url: None,
            },
            &serde_json::Map::new(),
        )
    }

    // ---- ACCEPTED: the separator an operator wrote into the config ----

    /// The row the narrowing exists for: a `?` in a curated `[[tools]]` path.
    #[test]
    fn query_separator_accepts_an_author_written_query_string() {
        assert_eq!(
            literal("/Line/Mode/tube/Status?detail=true").expect("author query accepted"),
            "/Line/Mode/tube/Status?detail=true"
        );
    }

    /// The change-request's own curated shape: an author query alongside a floored
    /// placeholder. Both mechanisms coexist on one path.
    #[test]
    fn query_separator_accepts_a_literal_query_alongside_a_floored_placeholder() {
        assert_eq!(
            substitute_one("/content/{version}/CUI?string=x", "version", "current")
                .expect("author query plus conforming placeholder accepted"),
            "/content/current/CUI?string=x"
        );
    }

    /// The Graph-style `$select` projection shape in-tree consumers author.
    #[test]
    fn query_separator_accepts_a_graph_style_dollar_projection() {
        let resolved = literal(
            "/drives/D/items/I/workbook/worksheets/C/range(address='A2:D7')?$select=values",
        )
        .expect("a Graph $select projection must be accepted");
        assert!(resolved.ends_with("?$select=values"), "{resolved}");
    }

    // ---- STILL REFUSED: everything the split does not relax ----

    #[test]
    fn query_separator_still_refuses_traversal_in_the_path_portion() {
        assert!(
            literal("/a/../b?x=1").is_err(),
            "appending a query must not launder a traversal"
        );
    }

    #[test]
    fn query_separator_still_refuses_traversal_in_the_query_portion() {
        let err = literal("/search?next=../../etc/passwd").unwrap_err();
        assert!(!err.to_string().contains("passwd"), "{err}");
    }

    #[test]
    fn query_separator_still_refuses_a_control_byte_in_the_query_portion() {
        assert!(literal("/search?x=a%00b").is_err());
    }

    #[test]
    fn query_separator_still_refuses_an_over_cap_query_portion() {
        let long = "z".repeat(PLACEHOLDER_MAX_LENGTH + 1);
        assert!(literal(&format!("/search?q={long}")).is_err());
    }

    #[test]
    fn query_separator_still_refuses_a_second_question_mark() {
        assert!(
            literal("/search?a=1?b=2").is_err(),
            "only the FIRST `?` is split off; one exemption, not a licence"
        );
    }

    #[test]
    fn query_separator_still_refuses_an_empty_query_portion() {
        assert!(
            literal("/search?").is_err(),
            "a dangling `?` is the same class as a trailing `/`"
        );
    }

    #[test]
    fn query_separator_still_refuses_a_fragment_marker() {
        assert!(literal("/search#frag").is_err());
    }

    /// THE row that proves the narrowing is not a hole: the template carries an
    /// author-written `?` (legal) AND a placeholder value carries an injected one
    /// (still refused by the per-value floor, which is the mechanism the narrowing
    /// relies on for its safety argument).
    #[test]
    fn query_separator_still_refuses_an_injected_separator_from_a_value() {
        let payload = "2026AA?string=x";
        let err = substitute_one("/search/{v}?detail=true", "v", payload).unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains('v'), "{rendered}");
        assert!(
            !rendered.contains("2026AA") && !rendered.contains('?'),
            "the refusal must carry no byte of the value: {rendered}"
        );
    }

    /// The second value route: an injected TRAVERSAL alongside an author query.
    #[test]
    fn query_separator_still_refuses_an_injected_traversal_from_a_value() {
        assert!(substitute_one("/search/{v}?detail=true", "v", "../../etc/passwd").is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::auth::NoAuth;
    use crate::http::{Parameter, ParameterLocation};

    fn get_user_op() -> Operation {
        Operation {
            method: "GET".to_string(),
            path: "/users/{id}".to_string(),
            parameters: vec![
                Parameter::new("id", ParameterLocation::Path, true),
                Parameter::new("verbose", ParameterLocation::Query, false),
            ],
            has_request_body: false,
            base_url: None,
        }
    }

    #[test]
    fn test_build_url_with_path_prefix() {
        // Regression: an API-Gateway stage prefix /v1 survives via join_url.
        let client = HttpClient::new(
            reqwest::Client::new(),
            "https://xxx.execute-api.eu-west-1.amazonaws.com/v1/".to_string(),
            Arc::new(NoAuth),
        )
        .unwrap();
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!("42"));
        let substituted = HttpClient::substitute_path(&op, &args).unwrap();
        let joined = join_url(client.base_url(), &substituted);
        assert_eq!(
            joined,
            "https://xxx.execute-api.eu-west-1.amazonaws.com/v1/users/42"
        );
    }

    #[test]
    fn test_substitute_path_replaces_placeholder() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!(7));
        assert_eq!(HttpClient::substitute_path(&op, &args).unwrap(), "/users/7");
    }

    #[test]
    fn test_build_query_skips_path_params() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!("42"));
        args.insert("verbose".to_string(), serde_json::json!(true));
        let query = HttpClient::build_query(&op, &args).unwrap();
        assert_eq!(query.get("verbose"), Some(&"true".to_string()));
        assert!(!query.contains_key("id"));
    }

    // -- WR-03 / GAP 4: fallible scalar renderer (reject non-scalar params) -----

    /// An array-of-scalars query param comma-joins (unchanged OpenAPI
    /// `form`/`explode:false` behavior).
    #[test]
    fn render_query_value_comma_joins_scalar_array() {
        let rendered =
            HttpClient::render_query_value("tags", &serde_json::json!(["a", 2, true])).unwrap();
        assert_eq!(rendered, "a,2,true");
    }

    /// A scalar query param renders bare (unchanged).
    #[test]
    fn render_query_value_scalar_passthrough() {
        assert_eq!(
            HttpClient::render_query_value("q", &serde_json::json!("hi")).unwrap(),
            "hi"
        );
        assert_eq!(
            HttpClient::render_query_value("n", &serde_json::json!(7)).unwrap(),
            "7"
        );
    }

    /// `render_scalar` renders Null as the bare string `"null"` (matches the
    /// code_mode `scalar_str` counterpart).
    #[test]
    fn render_scalar_null_is_bare_null() {
        assert_eq!(
            render_scalar("x", &serde_json::Value::Null).unwrap(),
            "null"
        );
    }

    /// An OBJECT path param is rejected, naming the param; the error never echoes
    /// the value and never produces a JSON-stringified `{`/`[`/`"`.
    #[test]
    fn substitute_path_rejects_object_param() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!({"nested": "x"}));
        let err = HttpClient::substitute_path(&op, &args).unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        let rendered = err.to_string();
        assert!(
            rendered.contains("id"),
            "error must name the param: {rendered}"
        );
        for forbidden in ['{', '[', '"'] {
            assert!(
                !rendered.contains(forbidden),
                "must not echo JSON: {rendered}"
            );
        }
        // Pitfall 5: never echo the value.
        assert!(
            !rendered.contains("nested"),
            "must not echo the value: {rendered}"
        );
    }

    /// An OBJECT query param is rejected, naming the param.
    #[test]
    fn build_query_rejects_object_param() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("verbose".to_string(), serde_json::json!({"k": "v"}));
        let err = HttpClient::build_query(&op, &args).unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        assert!(err.to_string().contains("verbose"));
    }

    /// An array CONTAINING a non-scalar member is rejected (the scalar comma-join
    /// is preserved only for scalar-only arrays).
    #[test]
    fn render_query_value_rejects_array_with_object_member() {
        let err = HttpClient::render_query_value("tags", &serde_json::json!(["ok", {"bad": 1}]))
            .unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        assert!(err.to_string().contains("tags"));
    }

    /// A non-scalar HEADER param is rejected, naming the param.
    #[test]
    fn build_headers_rejects_non_scalar_param() {
        let op = Operation {
            method: "GET".to_string(),
            path: "/x".to_string(),
            parameters: vec![Parameter::new("x-trace", ParameterLocation::Header, false)],
            has_request_body: false,
            base_url: None,
        };
        // An ARRAY in header position is non-scalar (arrays comma-join ONLY in
        // query position) and is rejected.
        let mut args = serde_json::Map::new();
        args.insert("x-trace".to_string(), serde_json::json!(["a", "b"]));
        let err = HttpClient::build_headers(&op, &args).unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        assert!(err.to_string().contains("x-trace"));
        // An OBJECT in header position is likewise rejected.
        let mut args2 = serde_json::Map::new();
        args2.insert("x-trace".to_string(), serde_json::json!({"k": "v"}));
        let err2 = HttpClient::build_headers(&op, &args2).unwrap_err();
        assert!(matches!(err2, HttpConnectorError::Backend(_)));
        assert!(err2.to_string().contains("x-trace"));
        // A scalar header value still succeeds.
        let mut args3 = serde_json::Map::new();
        args3.insert("x-trace".to_string(), serde_json::json!("abc"));
        let headers = HttpClient::build_headers(&op, &args3).unwrap();
        assert_eq!(headers.get("x-trace").unwrap(), "abc");
    }

    #[test]
    fn test_new_is_lazy_and_rejects_bad_url() {
        // Lazy: a bad URL fails synchronously without any network (CF-2).
        let err = HttpClient::new(
            reqwest::Client::new(),
            "not a url".to_string(),
            Arc::new(NoAuth),
        )
        .err()
        .expect("bad URL should error");
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        let rendered = err.to_string();
        assert!(!rendered.contains("not a url"), "must not echo the bad URL");
    }

    #[tokio::test]
    async fn http_connector_get_returns_json() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": 42, "name": "Ada"})),
            )
            .mount(&server)
            .await;

        let client =
            HttpClient::new(reqwest::Client::new(), server.uri(), Arc::new(NoAuth)).unwrap();
        let op = get_user_op();
        let args = serde_json::json!({"id": "42"});
        let result = client.execute(&op, &args).await.unwrap();
        assert_eq!(result["id"], 42);
        assert_eq!(result["name"], "Ada");
    }

    #[tokio::test]
    async fn http_connector_post_sends_body_and_auth() {
        use wiremock::matchers::{body_json, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/items"))
            .and(header("authorization", "Bearer tok"))
            .and(body_json(serde_json::json!({"name": "widget"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let auth = crate::http::auth::create_auth_provider(&crate::http::AuthConfig::Bearer {
            token: "tok".to_string(),
            required: true,
        })
        .unwrap();
        let client = HttpClient::new(reqwest::Client::new(), server.uri(), auth).unwrap();
        let op = Operation {
            method: "POST".to_string(),
            path: "/items".to_string(),
            parameters: vec![],
            has_request_body: true,
            base_url: None,
        };
        let args = serde_json::json!({"name": "widget"});
        let result = client.execute(&op, &args).await.unwrap();
        assert_eq!(result["ok"], true);
    }

    /// Phase 128 CR-02 — a DECLARED `Body`-located parameter reaches the JSON
    /// payload, and does NOT also reach the query string.
    ///
    /// The row above proves only the UNDECLARED route (`parameters: vec![]`), which
    /// is the route `additionalProperties: false` closed. This one declares the
    /// parameters, which is the shape `build_operation` actually synthesizes.
    #[tokio::test]
    async fn http_connector_post_sends_declared_body_parameters_as_the_payload() {
        use wiremock::matchers::{body_json, method, path, query_param_is_missing};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/items"))
            .and(body_json(
                serde_json::json!({"name": "widget", "note": "free text"}),
            ))
            // The value must travel ONCE. Before CR-02 a declared parameter was
            // `Query`-located, so it was appended here instead.
            .and(query_param_is_missing("name"))
            .and(query_param_is_missing("note"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let client =
            HttpClient::new(reqwest::Client::new(), server.uri(), Arc::new(NoAuth)).unwrap();
        let op = Operation {
            method: "POST".to_string(),
            path: "/items".to_string(),
            parameters: vec![
                Parameter::new("name", ParameterLocation::Body, true),
                Parameter::new("note", ParameterLocation::Body, false),
            ],
            has_request_body: true,
            base_url: None,
        };
        let args = serde_json::json!({"name": "widget", "note": "free text"});
        let result = client.execute(&op, &args).await.unwrap();
        assert_eq!(result["ok"], true);
    }

    /// Phase 128 CR-02 — a `Query`-located parameter on a body-bearing method is
    /// still a query parameter and is still withheld from the payload, so the fix
    /// is a ROUTING change rather than "everything goes in the body now".
    #[test]
    fn build_body_withholds_a_query_located_parameter_on_a_post() {
        let op = Operation {
            method: "POST".to_string(),
            path: "/items".to_string(),
            parameters: vec![
                Parameter::new("dry_run", ParameterLocation::Query, false),
                Parameter::new("name", ParameterLocation::Body, true),
            ],
            has_request_body: true,
            base_url: None,
        };
        let args = serde_json::json!({"dry_run": "true", "name": "widget"})
            .as_object()
            .expect("object")
            .clone();
        let body = HttpClient::build_body(&op, &args).expect("a body is built");
        assert_eq!(body, serde_json::json!({"name": "widget"}));
        let query = HttpClient::build_query(&op, &args).expect("a query is built");
        assert_eq!(query.get("dry_run").map(String::as_str), Some("true"));
        assert!(
            !query.contains_key("name"),
            "a Body-located parameter must not reach the query string: {query:?}"
        );
    }

    #[tokio::test]
    async fn http_connector_maps_non_2xx_to_status_without_url() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client =
            HttpClient::new(reqwest::Client::new(), server.uri(), Arc::new(NoAuth)).unwrap();
        let op = get_user_op();
        let args = serde_json::json!({"id": "42"});
        let err = client.execute(&op, &args).await.unwrap_err();
        assert!(matches!(err, HttpConnectorError::Status { status: 404 }));
        let rendered = err.to_string();
        assert!(rendered.contains("404"));
        assert!(
            !rendered.contains("http://"),
            "status error must not echo the URL"
        );
    }
}

// -----------------------------------------------------------------------------
// Phase 128 E1 — the curated surface's outbound-policy seam.
//
// A SIBLING of `mod tests` at the `client` module level, like `placeholder_floor`
// and `query_separator` above, so the `--lib http::client` filter selects it while
// it keeps access to the private `HttpClient` internals.
// -----------------------------------------------------------------------------

/// The E1 hook on the curated single-call surface: it must run before auth and
/// before the send.
#[cfg(test)]
mod request_policy_seam {
    use super::{HttpClient, HttpConfig, HttpConnectorError};
    use crate::http::auth::HttpAuthProvider;
    use crate::http::{HttpConnector, Operation, Parameter, ParameterLocation};
    use crate::policy::{OutboundRequest, PolicyRefusal, RequestPolicy};
    use async_trait::async_trait;
    use reqwest::header::{HeaderMap, HeaderValue};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// An auth provider that RECORDS whether it was invoked, so a refusal test can
    /// prove the policy ran BEFORE auth rather than merely before the send.
    struct RecordingAuth {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl HttpAuthProvider for RecordingAuth {
        async fn apply(
            &self,
            headers: &mut HeaderMap,
            _query: &mut HashMap<String, String>,
            _inbound_token: Option<&str>,
        ) -> Result<(), HttpConnectorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            headers.insert("authorization", HeaderValue::from_static("Bearer tok"));
            Ok(())
        }
    }

    /// One recorded `OutboundRequest`: `(tool, path, query, body)`.
    type Seen = Arc<Mutex<Vec<(String, String, Vec<(String, String)>, Option<String>)>>>;

    /// Records every request it is shown, then allows or refuses.
    struct Recorder {
        seen: Seen,
        refuse: Option<&'static str>,
    }

    #[async_trait]
    impl RequestPolicy for Recorder {
        async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
            self.seen.lock().expect("lock").push((
                req.tool.to_string(),
                req.path.to_string(),
                req.query.to_vec(),
                req.body.map(ToString::to_string),
            ));
            match self.refuse {
                Some(msg) => Err(PolicyRefusal::new(msg)),
                None => Ok(()),
            }
        }
    }

    fn op() -> Operation {
        Operation {
            method: "GET".to_string(),
            path: "/users/{id}".to_string(),
            parameters: vec![
                Parameter::new("id", ParameterLocation::Path, true),
                Parameter::new("q", ParameterLocation::Query, false),
            ],
            has_request_body: false,
            base_url: None,
        }
    }

    /// A client over `base_url` carrying `policy`, a recording auth provider, and
    /// ZERO retries (so a refusal test cannot be confused by a retry loop).
    fn client(
        base_url: String,
        policy: Option<Arc<dyn RequestPolicy>>,
    ) -> (HttpClient, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let auth = Arc::new(RecordingAuth {
            calls: Arc::clone(&calls),
        });
        let cfg = HttpConfig {
            retries: 0,
            ..HttpConfig::default()
        };
        let c = HttpClient::with_config(reqwest::Client::new(), base_url, auth, cfg)
            .expect("client builds");
        let c = match policy {
            Some(p) => c.with_request_policy(p),
            None => c,
        };
        (c, calls)
    }

    fn recorder(refuse: Option<&'static str>) -> (Arc<Recorder>, Seen) {
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        (
            Arc::new(Recorder {
                seen: Arc::clone(&seen),
                refuse,
            }),
            seen,
        )
    }

    #[tokio::test]
    async fn a_refusing_policy_stops_the_request_before_auth_and_before_the_send() {
        use wiremock::MockServer;
        // No mock is mounted: any request that escapes the policy 404s, so a
        // false GREEN here cannot masquerade as success.
        let server = MockServer::start().await;
        let (policy, _seen) = recorder(Some("refused by test policy"));
        let (client, auth_calls) = client(server.uri(), Some(policy));

        let err = client
            .execute(&op(), &serde_json::json!({ "id": "42" }))
            .await
            .expect_err("the policy refuses");

        assert_eq!(
            err.to_string(),
            "outbound request refused by policy: refused by test policy",
            "the refusal must carry the policy's own message"
        );
        assert_eq!(
            auth_calls.load(Ordering::SeqCst),
            0,
            "the auth provider must NOT have been invoked — the hook is before auth"
        );
        let requests = server
            .received_requests()
            .await
            .expect("wiremock records requests");
        assert!(requests.is_empty(), "a refusal must send nothing");
    }

    #[tokio::test]
    async fn the_same_refused_call_twice_is_identical_and_sends_nothing() {
        use wiremock::MockServer;
        let server = MockServer::start().await;
        let (policy, _seen) = recorder(Some("refused by test policy"));
        let (client, _auth) = client(server.uri(), Some(policy));

        let first = client
            .execute(&op(), &serde_json::json!({ "id": "42" }))
            .await
            .expect_err("refuses");
        let second = client
            .execute(&op(), &serde_json::json!({ "id": "42" }))
            .await
            .expect_err("refuses again");
        assert_eq!(first.to_string(), second.to_string());
        assert!(server
            .received_requests()
            .await
            .expect("recorded")
            .is_empty());
    }

    #[tokio::test]
    async fn an_allowing_policy_lets_the_request_through_and_auth_is_applied() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let (policy, _seen) = recorder(None);
        let (client, auth_calls) = client(server.uri(), Some(policy));
        let out = client
            .execute(&op(), &serde_json::json!({ "id": "42" }))
            .await
            .expect("allowed");
        assert_eq!(out["ok"], true);
        assert_eq!(auth_calls.load(Ordering::SeqCst), 1);
        assert_eq!(server.received_requests().await.expect("recorded").len(), 1);
    }

    #[tokio::test]
    async fn the_policy_sees_the_resolved_path_and_the_query_pairs() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let (policy, seen) = recorder(None);
        let (client, _auth) = client(server.uri(), Some(policy));
        client
            .execute(&op(), &serde_json::json!({ "id": "42", "q": "hay" }))
            .await
            .expect("allowed");

        let seen = seen.lock().expect("lock");
        assert_eq!(seen.len(), 1, "exactly one invocation per logical request");
        let (_tool, observed_path, query, body) = &seen[0];
        assert!(
            observed_path.ends_with("/users/42"),
            "the policy must see the SUBSTITUTED path, got {observed_path}"
        );
        assert!(
            !observed_path.contains('{'),
            "the policy must never see the template"
        );
        assert_eq!(query.as_slice(), &[("q".to_string(), "hay".to_string())]);
        assert!(body.is_none(), "a GET carries no body");
    }

    #[tokio::test]
    async fn no_policy_behaves_exactly_as_before() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let (client, auth_calls) = client(server.uri(), None);
        assert!(!client.has_request_policy());
        let out = client
            .execute(&op(), &serde_json::json!({ "id": "42" }))
            .await
            .expect("succeeds");
        assert_eq!(out["ok"], true);
        assert_eq!(auth_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_policy_is_told_which_tool_the_call_came_from() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let (policy, seen) = recorder(None);
        let (client, _auth) = client(server.uri(), Some(policy));
        client
            .execute_for_tool("get_user", &op(), &serde_json::json!({ "id": "42" }))
            .await
            .expect("allowed");

        let seen = seen.lock().expect("lock");
        assert_eq!(seen[0].0, "get_user");
    }

    #[test]
    fn a_governed_connector_reports_its_policy_through_the_dyn_trait() {
        let (policy, _seen) = recorder(None);
        let (client, _auth) = client("https://example.test".to_string(), None);
        let bare: Arc<dyn HttpConnector> = Arc::new(client);
        assert!(
            !bare.has_request_policy(),
            "a bare connector carries no policy"
        );
        let governed = bare
            .governed(policy)
            .expect("HttpClient supports policy attachment");
        assert!(
            governed.has_request_policy(),
            "a registered policy must be observable on the dyn connector, or it could \
             look registered while never running"
        );
    }
}
